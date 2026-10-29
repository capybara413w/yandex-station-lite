use crate::config::{ProxyConfig, SttConfig};
use crate::hardware::LedController;
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use std::fs;
use std::path::Path;
use std::process::Command;
use tracing::{error, info, warn};

pub struct SttService {
    record_device: String,
    sample_rate: u32,
    config: SttConfig,
    client: Client,
}

impl SttService {
    pub fn new(record_device: String, sample_rate: u32, config: SttConfig, proxy: &ProxyConfig) -> Self {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .connect_timeout(std::time::Duration::from_secs(10))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");

        if proxy.enabled && !proxy.socks5_url.is_empty() {
            if let Ok(p) = reqwest::Proxy::all(&proxy.socks5_url) {
                info!("STT Service using SOCKS5 proxy: {}", proxy.socks5_url);
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

    /// Records audio from Station microphone (raw Linear PCM, 16-bit, 16000Hz, mono),
    /// converts to WAV container and transcribes via Cloudflare Workers AI (@cf/openai/whisper).
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

        // 3. Indicate Blue: "Listening" (0, 100, 255)
        led.set_rgb(0, 100, 255);
        info!(
            "Microphone recording started (4 seconds, device: {}, mono 16kHz raw PCM)...",
            self.record_device
        );

        let mic_raw = "/tmp/mic.raw";
        let _ = fs::remove_file(mic_raw);

        let rec_dev = self.record_device.clone();
        let rate_str = self.sample_rate.to_string();

        let arecord_bin = if Path::new("/opt/bin/arecord").exists() {
            "/opt/bin/arecord"
        } else if Path::new("/usr/bin/arecord").exists() {
            "/usr/bin/arecord"
        } else {
            "arecord"
        };

        // 4. Capture 4 seconds of raw Linear PCM (16-bit, mono) via arecord (-t raw)
        let rec_result = tokio::task::spawn_blocking(move || {
            Command::new(arecord_bin)
                .args([
                    "-D", &rec_dev,
                    "-f", "S16_LE",
                    "-r", &rate_str,
                    "-c", "1",
                    "-t", "raw",
                    "-d", "4",
                    mic_raw,
                ])
                .output()
        })
        .await
        .map_err(|e| format!("Task join error: {}", e))?;

        if let Err(e) = rec_result {
            let _ = fs::remove_file(lock_path);
            led.clear();
            error!("arecord execution failed: {}", e);
            return Err(format!("arecord execution failed: {}", e));
        }

        // 5. Indicate Amber: "Processing" (220, 150, 0)
        led.set_rgb(220, 150, 0);

        let audio_bytes = match fs::read(mic_raw) {
            Ok(b) if !b.is_empty() => b,
            _ => {
                let _ = fs::remove_file(lock_path);
                led.clear();
                error!("Captured raw audio file /tmp/mic.raw is empty or missing");
                return Err("Failed to read captured audio file /tmp/mic.raw".to_string());
            }
        };

        let provider = self.config.provider.to_lowercase();
        let wav_bytes = pcm_to_wav(&audio_bytes, self.sample_rate, 1, 16);

        let trans_res = match provider.as_str() {
            "openrouter" => {
                let key = if !self.config.openrouter_api_key.is_empty() {
                    &self.config.openrouter_api_key
                } else {
                    &self.config.api_key
                };
                if key.is_empty() {
                    let _ = fs::remove_file(lock_path);
                    led.clear();
                    warn!("OpenRouter STT API key not configured");
                    return Err("OpenRouter STT API key not configured".to_string());
                }
                let model = if !self.config.openrouter_model.is_empty() {
                    &self.config.openrouter_model
                } else {
                    "openai/whisper"
                };
                info!("Sending audio ({} bytes) to OpenRouter Whisper (model: {})...", wav_bytes.len(), model);
                let part = Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| format!("Multipart error: {}", e))?;
                let mut form = Form::new()
                    .part("file", part)
                    .text("model", model.to_string());
                if !self.config.language.is_empty() {
                    form = form.text("language", self.config.language.clone());
                }
                self.client
                    .post("https://openrouter.ai/api/v1/audio/transcriptions")
                    .header("Authorization", format!("Bearer {}", key))
                    .header("HTTP-Referer", "https://station.local")
                    .header("X-Title", "Yandex Station Lite Daemon")
                    .multipart(form)
                    .send()
                    .await
            }
            "openai" => {
                let key = if !self.config.openai_api_key.is_empty() {
                    &self.config.openai_api_key
                } else {
                    &self.config.api_key
                };
                if key.is_empty() {
                    let _ = fs::remove_file(lock_path);
                    led.clear();
                    warn!("OpenAI STT API key not configured");
                    return Err("OpenAI STT API key not configured".to_string());
                }
                let model = if !self.config.openai_model.is_empty() {
                    &self.config.openai_model
                } else {
                    "whisper-1"
                };
                info!("Sending audio ({} bytes) to OpenAI Whisper (model: {})...", wav_bytes.len(), model);
                let part = Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| format!("Multipart error: {}", e))?;
                let mut form = Form::new()
                    .part("file", part)
                    .text("model", model.to_string());
                if !self.config.language.is_empty() {
                    form = form.text("language", self.config.language.clone());
                }
                self.client
                    .post("https://api.openai.com/v1/audio/transcriptions")
                    .header("Authorization", format!("Bearer {}", key))
                    .multipart(form)
                    .send()
                    .await
            }
            "groq" => {
                let key = if !self.config.groq_api_key.is_empty() {
                    &self.config.groq_api_key
                } else {
                    &self.config.api_key
                };
                if key.is_empty() {
                    let _ = fs::remove_file(lock_path);
                    led.clear();
                    warn!("Groq STT API key not configured");
                    return Err("Groq STT API key not configured".to_string());
                }
                let model = if !self.config.groq_model.is_empty() {
                    &self.config.groq_model
                } else {
                    "whisper-large-v3-turbo"
                };
                info!("Sending audio ({} bytes) to Groq Whisper (model: {})...", wav_bytes.len(), model);
                let part = Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| format!("Multipart error: {}", e))?;
                let mut form = Form::new()
                    .part("file", part)
                    .text("model", model.to_string());
                if !self.config.language.is_empty() {
                    form = form.text("language", self.config.language.clone());
                }
                self.client
                    .post("https://api.groq.com/openai/v1/audio/transcriptions")
                    .header("Authorization", format!("Bearer {}", key))
                    .multipart(form)
                    .send()
                    .await
            }
            "deepgram" => {
                let key = if !self.config.deepgram_api_key.is_empty() {
                    &self.config.deepgram_api_key
                } else {
                    &self.config.api_key
                };
                if key.is_empty() {
                    let _ = fs::remove_file(lock_path);
                    led.clear();
                    warn!("Deepgram STT API key not configured");
                    return Err("Deepgram STT API key not configured".to_string());
                }
                let model = if !self.config.deepgram_model.is_empty() {
                    &self.config.deepgram_model
                } else {
                    "nova-2"
                };
                let lang = if !self.config.language.is_empty() {
                    &self.config.language
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
                    .body(wav_bytes)
                    .send()
                    .await
            }
            "custom" => {
                let url = self.config.custom_url.trim();
                if url.is_empty() {
                    let _ = fs::remove_file(lock_path);
                    led.clear();
                    return Err("Custom STT endpoint URL not configured".to_string());
                }
                info!("Sending audio ({} bytes) to Custom STT ({})...", wav_bytes.len(), url);
                let part = Part::bytes(wav_bytes)
                    .file_name("audio.wav")
                    .mime_str("audio/wav")
                    .map_err(|e| format!("Multipart error: {}", e))?;
                let form = Form::new()
                    .part("file", part)
                    .text("model", "whisper".to_string());
                let mut req = self.client.post(url).multipart(form);
                if !self.config.api_key.is_empty() {
                    req = req.header("Authorization", format!("Bearer {}", self.config.api_key));
                }
                req.send().await
            }
            _ => {
                // Cloudflare Workers AI default
                let api_key = self.config.api_key.trim();
                let account_id = self.config.account_id.trim();
                if api_key.is_empty() {
                    let _ = fs::remove_file(lock_path);
                    led.clear();
                    warn!("Cloudflare API token (api_key) is not configured in config.json");
                    return Err("Cloudflare API key not configured".to_string());
                }
                if account_id.is_empty() {
                    let _ = fs::remove_file(lock_path);
                    led.clear();
                    warn!("Cloudflare account_id is not configured in config.json");
                    return Err("Cloudflare account_id not configured".to_string());
                }
                let model = if self.config.model.trim().is_empty() {
                    "@cf/openai/whisper"
                } else {
                    self.config.model.trim()
                };
                let api_url = format!(
                    "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{}",
                    account_id, model
                );
                info!(
                    "Audio captured ({} raw PCM bytes -> {} WAV bytes), sending to Cloudflare Workers AI (model: {})...",
                    audio_bytes.len(),
                    wav_bytes.len(),
                    model
                );
                self.client
                    .post(&api_url)
                    .header("Authorization", format!("Bearer {}", api_key))
                    .header("Content-Type", "application/octet-stream")
                    .body(wav_bytes)
                    .send()
                    .await
            }
        };

        let _ = fs::remove_file(lock_path);

        match trans_res {
            Ok(resp) => {
                let status = resp.status();
                let body_text = resp.text().await.unwrap_or_default();
                if status.is_success() {
                    info!("STT [{}] Raw Response: {}", provider, body_text);

                    if let Some(transcript) = parse_stt_result(&provider, &body_text) {
                        info!("Recognized speech via {}: '{}'", provider, transcript);
                        led.clear();
                        Ok(transcript)
                    } else {
                        led.clear();
                        warn!("STT [{}]: speech not recognized or silence in response", provider);
                        Err(format!("STT [{}]: speech not recognized (silence)", provider))
                    }
                } else {
                    led.clear();
                    error!("STT [{}] API error HTTP {}: {}", provider, status, body_text);
                    Err(format!("STT [{}] returned HTTP {}: {}", provider, status, body_text))
                }
            }
            Err(e) => {
                led.clear();
                error!("STT [{}] network error: {}", provider, e);
                Err(format!("STT [{}] network error: {}", provider, e))
            }
        }
    }
}

/// Prepends a standard 44-byte RIFF/WAV header to raw PCM samples.
/// If the data already begins with "RIFF", it is returned unchanged.
fn pcm_to_wav(pcm_data: &[u8], sample_rate: u32, channels: u16, bits_per_sample: u16) -> Vec<u8> {
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
