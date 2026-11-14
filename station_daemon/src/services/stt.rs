use crate::config::{AppConfig, ProxyConfig, SttConfig};
use crate::hardware::LedController;
use crate::services::clients::{
    BoxFuture, CloudflareSttClient, GeminiSttClient, LocalSttClient, OpenRouterSttClient, SttClient,
};
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

pub struct SttService {
    record_device: String,
    sample_rate: u32,
    config: Arc<RwLock<AppConfig>>,
    client: Client,
}

impl SttService {
    pub fn new(record_device: String, sample_rate: u32, config: Arc<RwLock<AppConfig>>, proxy: &ProxyConfig) -> Self {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");

        if proxy.enabled {
            let local_proxy = "http://127.0.0.1:8889";
            if let Ok(p) = reqwest::Proxy::all(local_proxy) {
                info!("STT Service using local proxy bridge: {}", local_proxy);
                builder = builder.proxy(p);
            }
        }

        let client = builder.build().unwrap_or_else(|_| Client::new());

        Self {
            record_device,
            sample_rate,
            config,
            client,
        }
    }

    /// Captures audio from the microphone using real-time Voice Activity Detection (VAD).
    /// Dynamically tracks speech onset, keeps recording while the user is talking,
    /// and automatically stops when the user finishes speaking (after ~0.85-1.0s of silence following speech), up to max_secs.
    pub async fn record_pcm_with_vad(&self, max_secs: u32) -> Result<Vec<u8>, String> {
        let rec_dev = self.record_device.clone();
        let rate_str = self.sample_rate.to_string();
        let arecord_bin = if Path::new("/opt/bin/arecord").exists() {
            "/opt/bin/arecord"
        } else if Path::new("/usr/bin/arecord").exists() {
            "/usr/bin/arecord"
        } else {
            "arecord"
        };

        tokio::task::spawn_blocking(move || {
            use std::io::Read;
            use std::collections::VecDeque;

            let mut child = Command::new(arecord_bin)
                .args([
                    "-D", &rec_dev,
                    "-f", "S16_LE",
                    "-r", &rate_str,
                    "-c", "1",
                    "-t", "raw",
                    "-q",
                ])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| format!("Failed to spawn arecord: {}", e))?;

            let mut stdout = child.stdout.take()
                .ok_or_else(|| "Failed to capture arecord stdout".to_string())?;

            // 50ms chunk at 16000Hz, 16-bit mono = 800 samples = 1600 bytes
            const CHUNK_SIZE: usize = 1600;
            let mut chunk = vec![0u8; CHUNK_SIZE];
            let mut pre_roll: VecDeque<Vec<u8>> = VecDeque::with_capacity(8); // ~350ms pre-roll
            let mut recorded: Vec<u8> = Vec::with_capacity(16000 * 2 * max_secs as usize);

            let mut speech_started = false;
            let mut speech_chunks: usize = 0;
            let mut silence_chunks: usize = 0;
            let mut total_chunks: usize = 0;
            let mut noise_floor_sum: f64 = 0.0;
            let mut noise_samples_count: usize = 0;
            let max_total_chunks = (max_secs * 20) as usize; // 20 chunks per second

            // Silence duration after speech to consider sentence finished: ~900ms = 18 chunks
            let max_silence_chunks = 18;
            // Minimum speech duration before silence cutoff can terminate: ~2.2 seconds (44 chunks)
            let min_speech_chunks = 44;
            // Timeout if user never starts speaking: 6.0s = 120 chunks
            let max_idle_chunks = 120;

            loop {
                if stdout.read_exact(&mut chunk).is_err() {
                    break;
                }
                total_chunks += 1;

                // Compute RMS energy
                let mut sum_sq: f64 = 0.0;
                let sample_count = CHUNK_SIZE / 2;
                for i in 0..sample_count {
                    let s = i16::from_le_bytes([chunk[i * 2], chunk[i * 2 + 1]]) as f64;
                    sum_sq += s * s;
                }
                let rms = (sum_sq / sample_count as f64).sqrt();

                if total_chunks <= 6 {
                    noise_floor_sum += rms;
                    noise_samples_count += 1;
                }

                let noise_floor = if noise_samples_count > 0 {
                    noise_floor_sum / noise_samples_count as f64
                } else {
                    150.0
                };

                let speech_threshold = (noise_floor * 1.8).clamp(380.0, 1500.0);
                let silence_threshold = speech_threshold * 0.65;

                if !speech_started {
                    if pre_roll.len() >= 7 {
                        pre_roll.pop_front();
                    }
                    pre_roll.push_back(chunk.clone());

                    if rms > speech_threshold {
                        speech_started = true;
                        for pr in pre_roll.drain(..) {
                            recorded.extend_from_slice(&pr);
                        }
                    } else if total_chunks >= max_idle_chunks {
                        let _ = child.kill();
                        let _ = child.wait();
                        return Err("Таймаут: речь не обнаружена".to_string());
                    }
                } else {
                    recorded.extend_from_slice(&chunk);
                    speech_chunks += 1;

                    if rms < silence_threshold {
                        silence_chunks += 1;
                        if silence_chunks >= max_silence_chunks && speech_chunks >= min_speech_chunks {
                            // User finished speaking and minimum duration satisfied!
                            break;
                        }
                    } else {
                        silence_chunks = 0;
                    }

                    if total_chunks >= max_total_chunks {
                        break;
                    }
                }
            }

            let _ = child.kill();
            let _ = child.wait();

            if recorded.is_empty() {
                return Err("Записано 0 байт".to_string());
            }

            Ok(recorded)
        })
        .await
        .map_err(|e| format!("VAD recording thread error: {}", e))?
    }

    /// Records audio from Station microphone using Voice Activity Detection (VAD),
    /// converts to WAV container and transcribes via selected provider.
    /// Manages /tmp/listen.lock and LED indications (Blue recording, Amber processing, Clear on finish).
    pub async fn record_and_transcribe(&self, led: &LedController) -> Result<String, String> {
        let lock_path = "/tmp/listen.lock";
        if Path::new(lock_path).exists() {
            warn!("Recording already in progress (/tmp/listen.lock exists)");
            return Err("Recording busy".to_string());
        }

        let _ = fs::write(lock_path, "");

        // 1. Kill any existing music players so microphone doesn't pick up speaker sound
        let _ = Command::new("killall").args(["-9", "mpg123", "play.sh"]).output();
        let _ = fs::remove_file("/tmp/music_playing");

        // 2. Kill quasar if running (release ALSA mic capture)
        let _ = Command::new("killall")
            .args(["-9", "quasar_launcher2", "rootkeeper.sh", "maind"])
            .output();

        // 3. Start pulsing/breathing animation: "Listening" (0, 160, 255)
        let pulse_stop = led.start_breathing(0, 160, 255);
        info!(
            "Microphone recording started with VAD (device: {}, mono 16kHz raw PCM)...",
            self.record_device
        );

        // 4. Capture speech with Voice Activity Detection (up to 15 seconds max)
        let pcm_res = self.record_pcm_with_vad(15).await;
        // Stop breathing animation immediately once recording concludes
        pulse_stop.store(false, std::sync::atomic::Ordering::Relaxed);

        let audio_bytes = match pcm_res {
            Ok(b) if !b.is_empty() => b,
            Ok(_) => {
                let _ = fs::remove_file(lock_path);
                led.clear();
                return Err("Captured audio is empty".to_string());
            }
            Err(e) => {
                let _ = fs::remove_file(lock_path);
                led.clear();
                return Err(e);
            }
        };

        // 5. Indicate Amber: "Processing" (220, 150, 0)
        led.set_rgb(220, 150, 0);

        let wav_bytes = pcm_to_wav(&audio_bytes, self.sample_rate, 1, 16);

        let res = self.transcribe_wav(&wav_bytes).await;
        let _ = fs::remove_file(lock_path);
        led.clear();

        match res {
            Ok(transcript) => {
                info!("Recognized speech: '{}'", transcript);
                Ok(transcript)
            }
            Err(e) => {
                warn!("STT error: {}", e);
                Err(e)
            }
        }
    }

    /// Transcribes WAV audio bytes using the configured STT provider.
    pub async fn transcribe_wav(&self, wav_bytes: &[u8]) -> Result<String, String> {
        let (stt_cfg, gemini_api_key) = {
            let cfg = self.config.read().await;
            (cfg.stt.clone(), cfg.deepseek.gemini_api_key.clone())
        };

        let provider = stt_cfg.provider.to_lowercase();

        // Check if Cloudflare credentials are unset/default placeholder
        let cf_dummy = stt_cfg.account_id.is_empty()
            || stt_cfg.account_id.contains("YOUR_CLOUDFLARE")
            || stt_cfg.api_key.contains("YOUR_CLOUDFLARE");
        let has_gemini = !stt_cfg.gemini_api_key.is_empty() || !gemini_api_key.is_empty();

        // 1. Google Gemini Multimodal STT (Explicit or seamless fallback when CF is not configured)
        if provider == "gemini" || ((provider == "cloudflare" || provider == "workers_ai") && cf_dummy && has_gemini) {
            let key = if !stt_cfg.gemini_api_key.is_empty() {
                stt_cfg.gemini_api_key.clone()
            } else {
                gemini_api_key.clone()
            };
            let model = if !stt_cfg.gemini_model.is_empty() {
                stt_cfg.gemini_model.clone()
            } else {
                "gemini-flash-lite-latest".to_string()
            };
            if provider != "gemini" {
                info!("Cloudflare STT account_id not set; seamlessly routing STT to Google Gemini...");
            }
            let client = GeminiSttClient::new(
                self.client.clone(),
                key,
                model,
                stt_cfg.extra_payload.clone(),
            );
            return client.transcribe(wav_bytes).await;
        }

        if provider == "openrouter" {
            let key = if !stt_cfg.openrouter_api_key.is_empty() {
                stt_cfg.openrouter_api_key.clone()
            } else {
                stt_cfg.api_key.clone()
            };
            let client = OpenRouterSttClient::new(
                self.client.clone(),
                key,
                stt_cfg.openrouter_model.clone(),
                stt_cfg.language.clone(),
                stt_cfg.extra_payload.clone(),
            );
            return client.transcribe(wav_bytes).await;
        }

        if provider == "cloudflare" || provider == "workers_ai" {
            let client = CloudflareSttClient::new(
                self.client.clone(),
                stt_cfg.account_id.clone(),
                stt_cfg.api_key.clone(),
                stt_cfg.model.clone(),
                stt_cfg.extra_payload.clone(),
            );
            return client.transcribe(wav_bytes).await;
        }

        if provider == "custom" || provider == "local" {
            let client = LocalSttClient::new(
                self.client.clone(),
                stt_cfg.custom_url.clone(),
                stt_cfg.api_key.clone(),
                stt_cfg.extra_payload.clone(),
            );
            return client.transcribe(wav_bytes).await;
        }

        let trans_res = match provider.as_str() {
            "openai" => {
                let key = if !stt_cfg.openai_api_key.is_empty() {
                    &stt_cfg.openai_api_key
                } else {
                    &stt_cfg.api_key
                };
                if key.is_empty() {
                    warn!("OpenAI STT API key not configured");
                    return Err("OpenAI STT API key not configured".to_string());
                }
                let model = if !stt_cfg.openai_model.is_empty() {
                    &stt_cfg.openai_model
                } else {
                    "whisper-1"
                };
                info!("Sending audio ({} bytes) to OpenAI Whisper (model: {})...", wav_bytes.len(), model);
                let part = Part::bytes(wav_bytes.to_vec())
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| format!("Multipart error: {}", e))?;
                let mut form = Form::new()
                    .part("file", part)
                    .text("model", model.to_string());
                if !stt_cfg.language.is_empty() {
                    form = form.text("language", stt_cfg.language.clone());
                }
                self.client
                    .post("https://api.openai.com/v1/audio/transcriptions")
                    .header("Authorization", format!("Bearer {}", key))
                    .multipart(form)
                    .send()
                    .await
            }
            "groq" => {
                let key = if !stt_cfg.groq_api_key.is_empty() {
                    &stt_cfg.groq_api_key
                } else {
                    &stt_cfg.api_key
                };
                if key.is_empty() {
                    warn!("Groq STT API key not configured");
                    return Err("Groq STT API key not configured".to_string());
                }
                let model = if !stt_cfg.groq_model.is_empty() {
                    &stt_cfg.groq_model
                } else {
                    "whisper-large-v3-turbo"
                };
                info!("Sending audio ({} bytes) to Groq Whisper (model: {})...", wav_bytes.len(), model);
                let part = Part::bytes(wav_bytes.to_vec())
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| format!("Multipart error: {}", e))?;
                let mut form = Form::new()
                    .part("file", part)
                    .text("model", model.to_string());
                if !stt_cfg.language.is_empty() {
                    form = form.text("language", stt_cfg.language.clone());
                }
                self.client
                    .post("https://api.groq.com/openai/v1/audio/transcriptions")
                    .header("Authorization", format!("Bearer {}", key))
                    .multipart(form)
                    .send()
                    .await
            }
            "deepgram" => {
                let key = if !stt_cfg.deepgram_api_key.is_empty() {
                    &stt_cfg.deepgram_api_key
                } else {
                    &stt_cfg.api_key
                };
                if key.is_empty() {
                    warn!("Deepgram STT API key not configured");
                    return Err("Deepgram STT API key not configured".to_string());
                }
                let model = if !stt_cfg.deepgram_model.is_empty() {
                    &stt_cfg.deepgram_model
                } else {
                    "nova-2"
                };
                let lang = if !stt_cfg.language.is_empty() {
                    &stt_cfg.language
                } else {
                    "ru"
                };
                let url = format!(
                    "https://api.deepgram.com/v1/listen?model={}&language={}&smart_format=true",
                    model, lang
                );
                info!("Sending audio ({} bytes) to Deepgram (model: {}, lang: {})...", wav_bytes.len(), model, lang);
                self.client
                    .post(&url)
                    .header("Authorization", format!("Token {}", key))
                    .header("Content-Type", "audio/wav")
                    .body(wav_bytes.to_vec())
                    .send()
                    .await
            }
            _ => {
                let fallback = CloudflareSttClient::new(
                    self.client.clone(),
                    stt_cfg.account_id.clone(),
                    stt_cfg.api_key.clone(),
                    stt_cfg.model.clone(),
                    stt_cfg.extra_payload.clone(),
                );
                return fallback.transcribe(wav_bytes).await;
            }
        };

        match trans_res {
            Ok(resp) => {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    info!("STT [{}] Raw Response: {}", provider, body_text);

                    if let Some(transcript) = parse_stt_result(&provider, &body_text) {
                        Ok(transcript)
                    } else {
                        warn!("STT [{}]: speech not recognized or silence in response", provider);
                        Err(format!("STT [{}]: speech not recognized (silence)", provider))
                    }
                } else {
                    error!("STT [{}] API error HTTP {}: {}", provider, status, body_text);
                    Err(format!("STT [{}] returned HTTP {}: {}", provider, status, body_text))
                }
            }
            Err(e) => {
                error!("STT [{}] network error: {}", provider, e);
                Err(format!("STT [{}] network error: {}", provider, e))
            }
        }
    }
}

impl SttClient for SttService {
    fn transcribe<'a>(
        &'a self,
        wav_bytes: &'a [u8],
    ) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move { self.transcribe_wav(wav_bytes).await })
    }
}

/// Prepends a standard 44-byte RIFF/WAV header to raw PCM samples.
/// If the data already begins with "RIFF", it is returned unchanged.
pub fn pcm_to_wav(pcm_data: &[u8], sample_rate: u32, channels: u16, bits_per_sample: u16) -> Vec<u8> {
    if pcm_data.starts_with(b"RIFF") {
        return pcm_data.to_vec();
    }
    let data_len = pcm_data.len() as u32;
    let byte_rate = sample_rate * (channels as u32) * ((bits_per_sample as u32) / 8);
    let block_align = channels * (bits_per_sample / 8);
    let total_len = 36u32.saturating_add(data_len);

    let mut wav = Vec::with_capacity(44 + pcm_data.len());
    // RIFF header
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&total_len.to_le_bytes());
    wav.extend_from_slice(b"WAVE");

    // fmt chunk
    wav.extend_from_slice(b"fmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size (16 for PCM)
    wav.extend_from_slice(&1u16.to_le_bytes());  // AudioFormat (1 = PCM)
    wav.extend_from_slice(&channels.to_le_bytes());
    wav.extend_from_slice(&sample_rate.to_le_bytes());
    wav.extend_from_slice(&byte_rate.to_le_bytes());
    wav.extend_from_slice(&block_align.to_le_bytes());
    wav.extend_from_slice(&bits_per_sample.to_le_bytes());

    // data chunk
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    wav.extend_from_slice(pcm_data);

    wav
}

fn parse_stt_result(provider: &str, raw: &str) -> Option<String> {
    match provider {
        "deepgram" => parse_deepgram_stt(raw).or_else(|| parse_openai_whisper(raw)),
        "openrouter" | "openai" | "groq" | "custom" => parse_openai_whisper(raw).or_else(|| parse_cloudflare_stt(raw)),
        _ => parse_cloudflare_stt(raw).or_else(|| parse_openai_whisper(raw)),
    }
}

fn parse_openai_whisper(raw: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(raw).ok()?;
    if let Some(text) = val.get("text").and_then(|t| t.as_str()) {
        let clean = text.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }
    None
}

fn parse_deepgram_stt(raw: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(raw).ok()?;
    if let Some(t) = val
        .pointer("/results/channels/0/alternatives/0/transcript")
        .and_then(|s| s.as_str())
    {
        let clean = t.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }
    parse_openai_whisper(raw)
}

/// Parses Cloudflare Workers AI Whisper response:
/// Typical success: {"result": {"text": "привет", "word_count": 1, ...}, "success": true}
/// Or {"result": "привет"}
/// Or {"text": "привет"}
fn parse_cloudflare_stt(raw: &str) -> Option<String> {
    let val: serde_json::Value = serde_json::from_str(raw).ok()?;

    // 1. Standard Cloudflare Workers AI result object: {"result": {"text": "..."}}
    if let Some(text) = val.get("result").and_then(|r| r.get("text")).and_then(|t| t.as_str()) {
        let clean = text.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }

    // 2. Direct string result: {"result": "text"}
    if let Some(text) = val.get("result").and_then(|r| r.as_str()) {
        let clean = text.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }

    // 3. Fallback for top-level {"text": "..."}
    if let Some(text) = val.get("text").and_then(|t| t.as_str()) {
        let clean = text.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cloudflare_standard() {
        let response = r#"{
            "result": {
                "text": "включи музыку",
                "word_count": 2,
                "words": []
            },
            "success": true,
            "errors": [],
            "messages": []
        }"#;
        assert_eq!(parse_cloudflare_stt(response), Some("включи музыку".to_string()));
    }

    #[test]
    fn test_parse_cloudflare_direct_result() {
        let response = r#"{"result": "привет мир", "success": true}"#;
        assert_eq!(parse_cloudflare_stt(response), Some("привет мир".to_string()));
    }

    #[test]
    fn test_parse_cloudflare_top_level_text() {
        let response = r#"{"text": "тестовая команда"}"#;
        assert_eq!(parse_cloudflare_stt(response), Some("тестовая команда".to_string()));
    }

    #[test]
    fn test_parse_cloudflare_empty_or_errors() {
        let response = r#"{"result": {"text": "   "}, "success": true}"#;
        assert_eq!(parse_cloudflare_stt(response), None);

        let err_resp = r#"{"result": null, "success": false, "errors": [{"message": "Invalid token"}]}"#;
        assert_eq!(parse_cloudflare_stt(err_resp), None);

        assert_eq!(parse_cloudflare_stt(""), None);
    }

    #[test]
    fn test_pcm_to_wav_header() {
        let dummy_pcm = vec![0u8; 3200];
        let wav = pcm_to_wav(&dummy_pcm, 16000, 1, 16);
        assert_eq!(&wav[0..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(&wav[12..16], b"fmt ");
        assert_eq!(&wav[36..40], b"data");
        assert_eq!(wav.len(), 44 + 3200);

        // Does not double-wrap if already WAV
        let duplicate = pcm_to_wav(&wav, 16000, 1, 16);
        assert_eq!(duplicate.len(), wav.len());
    }
}
