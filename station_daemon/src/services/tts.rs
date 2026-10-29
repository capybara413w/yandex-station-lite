use crate::config::{
    AppConfig, CartesiaTtsConfig, CustomTtsConfig, DeepgramTtsConfig, ElevenLabsConfig,
    OpenAiTtsConfig, OpenRouterTtsConfig, ProxyConfig,
};
use crate::hardware::LedController;
use reqwest::Client;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

fn get_mpg123_bin() -> &'static str {
    if std::path::Path::new("/opt/bin/mpg123").exists() {
        "/opt/bin/mpg123"
    } else if std::path::Path::new("/usr/bin/mpg123").exists() {
        "/usr/bin/mpg123"
    } else {
        "mpg123"
    }
}

pub struct TtsService {
    client: Client,
    config: Arc<RwLock<AppConfig>>,
    is_speaking: Arc<AtomicBool>,
    current_mpg_pid: Arc<AtomicU32>,
}

impl TtsService {
    pub fn new(config: Arc<RwLock<AppConfig>>, proxy: &ProxyConfig) -> Self {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(12))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64)");

        if proxy.enabled && !proxy.socks5_url.is_empty() {
            if let Ok(p) = reqwest::Proxy::all(&proxy.socks5_url) {
                info!("TTS Service using SOCKS5 proxy: {}", proxy.socks5_url);
                builder = builder.proxy(p);
            }
        }

        let client = builder.build().unwrap_or_else(|_| Client::new());

        // Ensure clean initial state
        let _ = fs::remove_file("/tmp/led_pulse");
        let _ = fs::remove_file("/tmp/tts.mp3");

        Self {
            client,
            config,
            is_speaking: Arc::new(AtomicBool::new(false)),
            current_mpg_pid: Arc::new(AtomicU32::new(0)),
        }
    }

    /// Checks if TTS speech is actively playing.
    pub fn is_speaking(&self) -> bool {
        self.is_speaking.load(Ordering::SeqCst) || Path::new("/tmp/led_pulse").exists()
    }

    /// Immediately stops any ongoing speech playback and pulsation animation.
    pub fn stop(&self, led: &LedController) {
        if !self.is_speaking() {
            return;
        }

        info!("Interrupting TTS speech playback...");
        self.is_speaking.store(false, Ordering::SeqCst);
        let _ = fs::remove_file("/tmp/led_pulse");

        let pid = self.current_mpg_pid.swap(0, Ordering::SeqCst);
        if pid > 0 {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
        }
        let _ = Command::new("killall").args(["-9", "mpg123"]).output();

        // Restore LED state
        if Path::new("/tmp/music_playing").exists() {
            led.set_music_aura();
        } else {
            led.clear();
        }
    }

    async fn synthesize_elevenlabs(&self, text: &str, el_cfg: &ElevenLabsConfig) -> Result<Vec<u8>, String> {
        let key = el_cfg.api_key.trim();
        if key.is_empty() {
            return Err("ElevenLabs API key is empty".to_string());
        }

        let voice_id = if el_cfg.voice_id.trim().is_empty() {
            "21m00Tcm4TlvDq8ikWAM"
        } else {
            el_cfg.voice_id.trim()
        };

        let model_id = if el_cfg.model_id.trim().is_empty() {
            "eleven_multilingual_v2"
        } else {
            el_cfg.model_id.trim()
        };

        let url = format!("https://api.elevenlabs.io/v1/text-to-speech/{}", voice_id);
        let payload = serde_json::json!({
            "text": text,
            "model_id": model_id,
            "voice_settings": {
                "stability": 0.5,
                "similarity_boost": 0.75
            }
        });

        info!("Calling ElevenLabs TTS [model: {}, voice: {}]...", model_id, voice_id);
        let res = self
            .client
            .post(&url)
            .header("xi-api-key", key)
            .header("Content-Type", "application/json")
            .header("Accept", "audio/mpeg")
            .json(&payload)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("ElevenLabs request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("ElevenLabs HTTP {}: {}", status, err_body));
        }

        let bytes = res.bytes().await.map_err(|e| format!("ElevenLabs read error: {}", e))?;
        if bytes.is_empty() {
            return Err("ElevenLabs returned empty stream".to_string());
        }
        Ok(bytes.to_vec())
    }

    async fn synthesize_cloudflare(
        &self,
        text: &str,
        acc_id: &str,
        token: &str,
        model: &str,
    ) -> Result<Vec<u8>, String> {
        if acc_id.is_empty() || token.is_empty() {
            return Err("Cloudflare Account ID or API Token is empty".to_string());
        }

        let model_name = if model.trim().is_empty() {
            "@cf/myshell/melotts-v1"
        } else {
            model.trim()
        };

        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{}",
            acc_id, model_name
        );
        let payload = if model_name.contains("elevenlabs") {
            serde_json::json!({
                "prompt": text,
                "text": text,
                "model_id": if model_name.contains("flash") {
                    "eleven_flash_v2_5"
                } else if model_name.contains("turbo") {
                    "eleven_turbo_v2_5"
                } else {
                    "eleven_multilingual_v2"
                }
            })
        } else {
            serde_json::json!({
                "prompt": text,
                "text": text,
                "language": "ru"
            })
        };

        info!("Calling Cloudflare TTS [{}] for account {}...", model_name, acc_id);
        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("Cloudflare request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("Cloudflare TTS HTTP {}: {}", status, err_body));
        }

        let bytes = res.bytes().await.map_err(|e| format!("Cloudflare read error: {}", e))?;
        if bytes.is_empty() {
            return Err("Cloudflare returned 0 bytes".to_string());
        }
        Ok(bytes.to_vec())
    }

    async fn synthesize_google(&self, text: &str) -> Result<Vec<u8>, String> {
        let chunks = split_text_into_chunks(text, 160);
        info!("Synthesizing Google TTS for {} chunks: '{}'", chunks.len(), text);

        let mut audio_data = Vec::new();
        for (idx, chunk) in chunks.iter().enumerate() {
            let encoded = urlencoding::encode(chunk);
            let url = format!(
                "https://translate.google.com/translate_tts?ie=UTF-8&tl=ru&client=tw-ob&q={}",
                encoded
            );

            let res = self
                .client
                .get(&url)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
                .timeout(std::time::Duration::from_secs(5))
                .send()
                .await;

            match res {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        match resp.bytes().await {
                            Ok(bytes) if !bytes.is_empty() => {
                                audio_data.extend_from_slice(&bytes);
                            }
                            Ok(_) => {
                                error!("Google TTS chunk {} returned empty byte stream", idx + 1);
                                return Err("Google TTS returned 0 bytes".to_string());
                            }
                            Err(e) => {
                                error!("Failed to read Google TTS chunk {} bytes: {}", idx + 1, e);
                                return Err(format!("TTS byte read error: {}", e));
                            }
                        }
                    } else {
                        let err_text = resp.text().await.unwrap_or_default();
                        error!("Google TTS chunk {} failed HTTP {}: {}", idx + 1, status, err_text);
                        return Err(format!("Google TTS returned HTTP {}", status));
                    }
                }
                Err(e) => {
                    error!("Google TTS chunk {} network timeout/error: {}", idx + 1, e);
                    return Err(format!("Google TTS request error: {}", e));
                }
            }
        }

        if audio_data.is_empty() {
            return Err("Received empty audio stream from Google TTS".to_string());
        }
        Ok(audio_data)
    }

    async fn synthesize_openai(&self, text: &str, openai_cfg: &OpenAiTtsConfig) -> Result<Vec<u8>, String> {
        let key = openai_cfg.api_key.trim();
        if key.is_empty() {
            return Err("OpenAI API key is empty".to_string());
        }

        let model = if openai_cfg.model.trim().is_empty() {
            "tts-1"
        } else {
            openai_cfg.model.trim()
        };
        let voice = if openai_cfg.voice.trim().is_empty() {
            "alloy"
        } else {
            openai_cfg.voice.trim()
        };

        let url = "https://api.openai.com/v1/audio/speech";
        let payload = serde_json::json!({
            "model": model,
            "input": text,
            "voice": voice,
            "response_format": "mp3"
        });

        info!("Calling OpenAI TTS [model: {}, voice: {}]...", model, voice);
        let res = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("OpenAI TTS request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("OpenAI TTS HTTP {}: {}", status, err_body));
        }

        let bytes = res.bytes().await.map_err(|e| format!("OpenAI TTS read error: {}", e))?;
        if bytes.is_empty() {
            return Err("OpenAI TTS returned empty stream".to_string());
        }
        Ok(bytes.to_vec())
    }

    async fn synthesize_deepgram(&self, text: &str, deepgram_cfg: &DeepgramTtsConfig) -> Result<Vec<u8>, String> {
        let key = deepgram_cfg.api_key.trim();
        if key.is_empty() {
            return Err("Deepgram API key is empty".to_string());
        }
        let model = if deepgram_cfg.model.trim().is_empty() {
            "aura-asteria-en"
        } else {
            deepgram_cfg.model.trim()
        };
        let url = format!("https://api.deepgram.com/v1/speak?model={}", model);
        let payload = serde_json::json!({ "text": text });

        info!("Calling Deepgram TTS [model: {}]...", model);
        let res = self
            .client
            .post(&url)
            .header("Authorization", format!("Token {}", key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .timeout(std::time::Duration::from_secs(12))
            .send()
            .await
            .map_err(|e| format!("Deepgram TTS request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("Deepgram TTS HTTP {}: {}", status, err_body));
        }

        let bytes = res.bytes().await.map_err(|e| format!("Deepgram TTS read error: {}", e))?;
        if bytes.is_empty() {
            return Err("Deepgram TTS returned empty stream".to_string());
        }
        Ok(bytes.to_vec())
    }

    async fn synthesize_cartesia(&self, text: &str, cartesia_cfg: &CartesiaTtsConfig) -> Result<Vec<u8>, String> {
        let key = cartesia_cfg.api_key.trim();
        if key.is_empty() {
            return Err("Cartesia API key is empty".to_string());
        }
        let model = if cartesia_cfg.model_id.trim().is_empty() {
            "sonic-multilingual"
        } else {
            cartesia_cfg.model_id.trim()
        };
        let voice = if cartesia_cfg.voice_id.trim().is_empty() {
            "a0e998e3-182d-4f0d-9fb9-e82429f3303a"
        } else {
            cartesia_cfg.voice_id.trim()
        };

        let url = "https://api.cartesia.ai/tts/bytes";
        let payload = serde_json::json!({
            "transcript": text,
            "model_id": model,
            "voice": {
                "mode": "id",
                "id": voice
            },
            "output_format": {
                "container": "mp3",
                "bit_rate": 128000,
                "sample_rate": 24000
            }
        });

        info!("Calling Cartesia TTS [model: {}, voice: {}]...", model, voice);
        let res = self
            .client
            .post(url)
            .header("X-API-Key", key)
            .header("Cartesia-Version", "2024-06-10")
            .header("Content-Type", "application/json")
            .json(&payload)
            .timeout(std::time::Duration::from_secs(12))
            .send()
            .await
            .map_err(|e| format!("Cartesia TTS request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("Cartesia TTS HTTP {}: {}", status, err_body));
        }

        let bytes = res.bytes().await.map_err(|e| format!("Cartesia TTS read error: {}", e))?;
        if bytes.is_empty() {
            return Err("Cartesia TTS returned empty stream".to_string());
        }
        Ok(bytes.to_vec())
    }

    async fn synthesize_custom(&self, text: &str, custom_cfg: &CustomTtsConfig) -> Result<Vec<u8>, String> {
        let url_base = custom_cfg.url.trim();
        if url_base.is_empty() {
            return Err("Custom TTS endpoint URL is empty".to_string());
        }
        let url = if url_base.contains('?') {
            format!("{}&text={}", url_base, urlencoding::encode(text))
        } else {
            format!("{}?text={}", url_base, urlencoding::encode(text))
        };

        info!("Calling Custom TTS [{}]...", url_base);
        let res = self
            .client
            .get(&url)
            .timeout(std::time::Duration::from_secs(12))
            .send()
            .await
            .map_err(|e| format!("Custom TTS request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("Custom TTS HTTP {}: {}", status, err_body));
        }

        let bytes = res.bytes().await.map_err(|e| format!("Custom TTS read error: {}", e))?;
        if bytes.is_empty() {
            return Err("Custom TTS returned empty stream".to_string());
        }
        Ok(bytes.to_vec())
    }

    async fn synthesize_openrouter(
        &self,
        text: &str,
        openrouter_cfg: &OpenRouterTtsConfig,
        deepseek_or_key: &str,
    ) -> Result<Vec<u8>, String> {
        let key = if !openrouter_cfg.api_key.trim().is_empty() {
            openrouter_cfg.api_key.trim()
        } else {
            deepseek_or_key.trim()
        };

        if key.is_empty() {
            return Err("OpenRouter API key is empty".to_string());
        }

        let model = if !openrouter_cfg.model.trim().is_empty() {
            openrouter_cfg.model.trim()
        } else {
            "openai/tts-1"
        };

        let voice = if !openrouter_cfg.voice.trim().is_empty() {
            openrouter_cfg.voice.trim()
        } else {
            "alloy"
        };

        let url = "https://openrouter.ai/api/v1/audio/speech";
        let payload = serde_json::json!({
            "model": model,
            "input": text,
            "voice": voice,
        });

        info!("Calling OpenRouter TTS [model: {}, voice: {}]...", model, voice);
        let res = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", key))
            .header("HTTP-Referer", "https://station.local")
            .header("X-Title", "Yandex Station Lite Daemon")
            .header("Content-Type", "application/json")
            .json(&payload)
            .timeout(std::time::Duration::from_secs(15))
            .send()
            .await
            .map_err(|e| format!("OpenRouter TTS request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_body = res.text().await.unwrap_or_default();
            return Err(format!("OpenRouter TTS HTTP {}: {}", status, err_body));
        }

        let bytes = res.bytes().await.map_err(|e| format!("OpenRouter TTS read error: {}", e))?;
        if bytes.is_empty() {
            return Err("OpenRouter TTS returned empty stream".to_string());
        }
        Ok(bytes.to_vec())
    }

    /// Synthesizes speech via configured TTS provider (Google, ElevenLabs, Cloudflare, OpenAI, OpenRouter, Deepgram, Cartesia, Custom),
    /// animates LED ring, and plays via mpg123.
    pub async fn speak(&self, text: &str, color: [u8; 3], led: &LedController) -> Result<(), String> {
        let clean_text = text.trim();
        if clean_text.is_empty() {
            return Ok(());
        }

        // 1. Stop any previous speech
        self.stop(led);

        // 2. Fetch TTS config
        let (provider, openrouter_cfg, eleven_cfg, cf_cfg, cf_stt_acc, cf_stt_token, openai_cfg, deepgram_cfg, cartesia_cfg, custom_cfg, deepseek_or_key) = {
            let cfg = self.config.read().await;
            (
                cfg.tts.provider.to_lowercase(),
                cfg.tts.openrouter.clone(),
                cfg.tts.elevenlabs.clone(),
                cfg.tts.cloudflare.clone(),
                cfg.stt.account_id.clone(),
                cfg.stt.api_key.clone(),
                cfg.tts.openai.clone(),
                cfg.tts.deepgram.clone(),
                cfg.tts.cartesia.clone(),
                cfg.tts.custom.clone(),
                cfg.deepseek.openrouter_api_key.clone(),
            )
        };

        // 3. Synthesize audio data
        let audio_data = match provider.as_str() {
            "openrouter" => {
                match self.synthesize_openrouter(clean_text, &openrouter_cfg, &deepseek_or_key).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("OpenRouter TTS failed: {}. Falling back to Google TTS.", e);
                        self.synthesize_google(clean_text).await?
                    }
                }
            }
            "elevenlabs" => {
                match self.synthesize_elevenlabs(clean_text, &eleven_cfg).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("ElevenLabs failed: {}. Falling back to Google TTS.", e);
                        self.synthesize_google(clean_text).await?
                    }
                }
            }
            "cloudflare" => {
                let acc = if !cf_cfg.account_id.trim().is_empty() {
                    cf_cfg.account_id.trim()
                } else {
                    cf_stt_acc.trim()
                };
                let tok = if !cf_cfg.api_token.trim().is_empty() {
                    cf_cfg.api_token.trim()
                } else {
                    cf_stt_token.trim()
                };
                match self.synthesize_cloudflare(clean_text, acc, tok, &cf_cfg.model).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("Cloudflare TTS failed: {}. Falling back to Google TTS.", e);
                        self.synthesize_google(clean_text).await?
                    }
                }
            }
            "openai" => {
                match self.synthesize_openai(clean_text, &openai_cfg).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("OpenAI TTS failed: {}. Falling back to Google TTS.", e);
                        self.synthesize_google(clean_text).await?
                    }
                }
            }
            "deepgram" => {
                match self.synthesize_deepgram(clean_text, &deepgram_cfg).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("Deepgram TTS failed: {}. Falling back to Google TTS.", e);
                        self.synthesize_google(clean_text).await?
                    }
                }
            }
            "cartesia" => {
                match self.synthesize_cartesia(clean_text, &cartesia_cfg).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("Cartesia TTS failed: {}. Falling back to Google TTS.", e);
                        self.synthesize_google(clean_text).await?
                    }
                }
            }
            "custom" => {
                match self.synthesize_custom(clean_text, &custom_cfg).await {
                    Ok(data) => data,
                    Err(e) => {
                        warn!("Custom TTS failed: {}. Falling back to Google TTS.", e);
                        self.synthesize_google(clean_text).await?
                    }
                }
            }
            _ => self.synthesize_google(clean_text).await?,
        };

        // 4. Save to /tmp/tts.mp3
        let tts_file = "/tmp/tts.mp3";
        if let Err(e) = fs::write(tts_file, &audio_data) {
            error!("Failed to write /tmp/tts.mp3: {}", e);
            return Err(format!("Failed to write /tmp/tts.mp3: {}", e));
        }

        // 5. Verify that /tmp/tts.mp3 size > 0 bytes before invoking mpg123
        let file_size = fs::metadata(tts_file)
            .map(|m| m.len())
            .unwrap_or(0);
        if file_size == 0 {
            error!("TTS file /tmp/tts.mp3 is 0 bytes, aborting playback");
            return Err("TTS file is empty (0 bytes)".to_string());
        }

        // 6. Read hardware volume scale from /data/volume
        let vol_step = fs::read_to_string("/data/volume")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(6)
            .clamp(1, 10);
        let scale = (vol_step * 1000).clamp(500, 12000);

        // 7. Start Native LED Pulsation Animation (Breathing rhythm)
        self.is_speaking.store(true, Ordering::SeqCst);
        let _ = fs::write("/tmp/led_pulse", "");

        let is_speaking_flag = self.is_speaking.clone();
        let led_base = "/sys/class/leds".to_string();
        let [r, g, b] = color;

        tokio::spawn(async move {
            let factors = [10, 8, 6, 4, 3, 4, 6, 8];
            while is_speaking_flag.load(Ordering::SeqCst) && Path::new("/tmp/led_pulse").exists() {
                for &f in &factors {
                    if !is_speaking_flag.load(Ordering::SeqCst) || !Path::new("/tmp/led_pulse").exists() {
                        break;
                    }
                    let cr = (r as u32 * f / 10) as u8;
                    let cg = (g as u32 * f / 10) as u8;
                    let cb = (b as u32 * f / 10) as u8;

                    for i in 1..=3 {
                        let _ = fs::write(format!("{}/micro:red:ring{}/brightness", led_base, i), cr.to_string());
                        let _ = fs::write(format!("{}/micro:green:ring{}/brightness", led_base, i), cg.to_string());
                        let _ = fs::write(format!("{}/micro:blue:ring{}/brightness", led_base, i), cb.to_string());
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(90)).await;
                }
            }
        });

        // 8. Play synthesized audio via mpg123 on hw:0,0
        let mpg_bin = get_mpg123_bin();

        info!("Playing /tmp/tts.mp3 ({} bytes, scale: {}) on hw:0,0 via {}", file_size, scale, mpg_bin);

        let pid_holder = self.current_mpg_pid.clone();
        let is_speaking_done = self.is_speaking.clone();

        let play_res = tokio::task::spawn_blocking(move || {
            match Command::new(mpg_bin)
                .args(["-q", "-a", "hw:0,0", "-f", &scale.to_string(), tts_file])
                .spawn()
            {
                Ok(mut child) => {
                    pid_holder.store(child.id(), Ordering::SeqCst);
                    let status = child.wait();
                    pid_holder.store(0, Ordering::SeqCst);
                    status
                }
                Err(e) => Err(e),
            }
        })
        .await;

        // 9. Finalize speech and restore LED
        is_speaking_done.store(false, Ordering::SeqCst);
        let _ = fs::remove_file("/tmp/led_pulse");

        // Gentle fade out or restore music aura
        if Path::new("/tmp/music_playing").exists() {
            led.set_music_aura();
        } else {
            // Hold solid color for 300ms then extinguish
            led.set_rgb(r, g, b);
            tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            for f in [6, 3, 0] {
                led.set_rgb((r as u32 * f / 10) as u8, (g as u32 * f / 10) as u8, (b as u32 * f / 10) as u8);
                tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;
            }
            led.clear();
        }

        match play_res {
            Ok(Ok(_)) => Ok(()),
            Ok(Err(e)) => {
                error!("mpg123 playback error: {}", e);
                Err(format!("mpg123 execution error: {}", e))
            }
            Err(e) => {
                error!("Task spawn error for mpg123: {}", e);
                Err(format!("Task spawn error: {}", e))
            }
        }
    }
}

/// Splits text into coherent sentences or phrases not exceeding max_len characters.
fn split_text_into_chunks(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    // Split by punctuation delimiters
    for sentence in text.split_inclusive(&['.', '!', '?', ';', '\n'][..]) {
        let sentence_trimmed = sentence.trim();
        if sentence_trimmed.is_empty() {
            continue;
        }

        if current.len() + sentence_trimmed.len() + 1 <= max_len {
            if !current.is_empty() {
                current.push(' ');
            }
            current.push_str(sentence_trimmed);
        } else {
            if !current.is_empty() {
                chunks.push(current);
                current = String::new();
            }

            // If a single sentence exceeds max_len, split by words
            if sentence_trimmed.len() > max_len {
                for word in sentence_trimmed.split_whitespace() {
                    if current.len() + word.len() + 1 <= max_len {
                        if !current.is_empty() {
                            current.push(' ');
                        }
                        current.push_str(word);
                    } else {
                        if !current.is_empty() {
                            chunks.push(current);
                            current = String::new();
                        }
                        current.push_str(word);
                    }
                }
            } else {
                current.push_str(sentence_trimmed);
            }
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}
