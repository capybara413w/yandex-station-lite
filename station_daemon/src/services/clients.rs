use crate::config::merge_extra_payload;
use crate::services::deepseek::DeepSeekResponse;
use reqwest::multipart::{Form, Part};
use reqwest::Client;
use serde_json::Value;
use std::future::Future;
use std::pin::Pin;
use tracing::{error, info, warn};

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

// ============================================================================
// 1. LLM Client Architecture
// ============================================================================

/// Trait for querying an LLM model with system instructions and user prompt.
pub trait LlmClient: Send + Sync {
    fn query<'a>(
        &'a self,
        prompt: &'a str,
        system_prompt: &'a str,
    ) -> BoxFuture<'a, Result<DeepSeekResponse, String>>;
}

/// OpenRouter LLM Client (compatible with OpenAI chat completions)
pub struct OpenRouterLlmClient {
    pub client: Client,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub extra_payload: Option<Value>,
}

impl OpenRouterLlmClient {
    pub fn new(
        client: Client,
        api_key: String,
        model: String,
        temperature: f32,
        max_tokens: u32,
        extra_payload: Option<Value>,
    ) -> Self {
        Self {
            client,
            api_key,
            model,
            temperature,
            max_tokens,
            extra_payload,
        }
    }
}

impl LlmClient for OpenRouterLlmClient {
    fn query<'a>(
        &'a self,
        prompt: &'a str,
        system_prompt: &'a str,
    ) -> BoxFuture<'a, Result<DeepSeekResponse, String>> {
        Box::pin(async move {
            let key = self.api_key.trim();
            if key.is_empty() {
                return Err("OpenRouter API key is empty".to_string());
            }

            let model_name = if self.model.trim().is_empty() {
                "deepseek/deepseek-chat".to_string()
            } else {
                self.model.trim().to_string()
            };

            let effective_system_prompt = if system_prompt.to_lowercase().contains("json") {
                system_prompt.to_string()
            } else {
                let base = if system_prompt.trim().is_empty() {
                    "Ты — умный голосовой ассистент смарт-колонки с поддержкой управления музыкой и светодиодной подсветкой.".to_string()
                } else {
                    system_prompt.to_string()
                };
                format!(
                    "{}\n\nВАЖНО: Отвечай строго валидным JSON объектом (JSON format): {{\"text\": \"...\", \"color\": [R, G, B], \"action\": \"play\" | \"stop\" | \"resume\" | \"none\", \"song\": \"...\"}}",
                    base
                )
            };

            let mut body = serde_json::json!({
                "model": model_name,
                "messages": [
                    { "role": "system", "content": &effective_system_prompt },
                    { "role": "user", "content": prompt }
                ],
                "temperature": self.temperature,
                "max_tokens": self.max_tokens
            });

            let model_lower = model_name.to_lowercase();
            if !model_lower.contains("reasoner") && !model_lower.contains("r1") {
                body["response_format"] = serde_json::json!({ "type": "json_object" });
            }

            // Merge user-supplied extra JSON parameters
            merge_extra_payload(&mut body, self.extra_payload.as_ref());

            info!(
                "Sending query to OpenRouter (model: {}) with payload keys: {:?}",
                model_name,
                body.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>())
            );

            let res = self
                .client
                .post("https://openrouter.ai/api/v1/chat/completions")
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", key))
                .header("HTTP-Referer", "https://yandex-station-lite.local")
                .header("X-Title", "Yandex Station Lite Control Center")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("OpenRouter request error: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_txt = res.text().await.unwrap_or_default();

                if status.as_u16() == 400 && (err_txt.contains("response_format") || err_txt.contains("json") || err_txt.contains("JSON")) {
                    warn!("OpenRouter rejected response_format, retrying without response_format...");
                    if let Some(obj) = body.as_object_mut() {
                        obj.remove("response_format");
                    }
                    if let Ok(retry_res) = self
                        .client
                        .post("https://openrouter.ai/api/v1/chat/completions")
                        .header("Content-Type", "application/json")
                        .header("Authorization", format!("Bearer {}", key))
                        .header("HTTP-Referer", "https://yandex-station-lite.local")
                        .header("X-Title", "Yandex Station Lite Control Center")
                        .json(&body)
                        .send()
                        .await
                    {
                        if retry_res.status().is_success() {
                            if let Ok(val) = retry_res.json::<Value>().await {
                                let content = val["choices"][0]["message"]["content"].as_str().unwrap_or("{}");
                                return Ok(parse_clean_json_response(content, prompt));
                            }
                        }
                    }
                }

                return Err(format!("OpenRouter returned HTTP {}: {}", status, err_txt));
            }

            let val: Value = res
                .json()
                .await
                .map_err(|e| format!("Invalid JSON from OpenRouter: {}", e))?;

            let content = val["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("{}");

            Ok(parse_clean_json_response(content, prompt))
        })
    }
}

/// Cloudflare Workers AI LLM Client
pub struct CloudflareLlmClient {
    pub client: Client,
    pub account_id: String,
    pub api_token: String,
    pub model: String,
    pub extra_payload: Option<Value>,
}

impl CloudflareLlmClient {
    pub fn new(
        client: Client,
        account_id: String,
        api_token: String,
        model: String,
        extra_payload: Option<Value>,
    ) -> Self {
        Self {
            client,
            account_id,
            api_token,
            model,
            extra_payload,
        }
    }
}

impl LlmClient for CloudflareLlmClient {
    fn query<'a>(
        &'a self,
        prompt: &'a str,
        system_prompt: &'a str,
    ) -> BoxFuture<'a, Result<DeepSeekResponse, String>> {
        Box::pin(async move {
            let acc_id = self.account_id.trim();
            let token = self.api_token.trim();
            if acc_id.is_empty() || token.is_empty() {
                return Err("Cloudflare Account ID or API token is empty".to_string());
            }

            let model = if self.model.trim().is_empty() {
                "@cf/meta/llama-3.1-8b-instruct"
            } else {
                self.model.trim()
            };

            let url = format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{}",
                acc_id, model
            );

            let mut body = serde_json::json!({
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": prompt }
                ]
            });

            // Merge custom extra_payload (e.g. max_tokens, temperature, stream: false)
            merge_extra_payload(&mut body, self.extra_payload.as_ref());

            info!(
                "Sending query to Cloudflare Workers AI LLM (model: {})",
                model
            );

            let res = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Cloudflare LLM request error: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_txt = res.text().await.unwrap_or_default();
                return Err(format!("Cloudflare LLM returned HTTP {}: {}", status, err_txt));
            }

            let val: Value = res
                .json()
                .await
                .map_err(|e| format!("Invalid JSON from Cloudflare LLM: {}", e))?;

            let content = if let Some(resp_str) = val.pointer("/result/response").and_then(|v| v.as_str()) {
                resp_str
            } else if let Some(resp_str) = val.get("response").and_then(|v| v.as_str()) {
                resp_str
            } else {
                "{}"
            };

            Ok(parse_clean_json_response(content, prompt))
        })
    }
}

/// Local or Custom LLM Client (e.g. Ollama, LM Studio, vLLM, local Python server)
pub struct LocalLlmClient {
    pub client: Client,
    pub endpoint_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_tokens: u32,
    pub extra_payload: Option<Value>,
}

impl LocalLlmClient {
    pub fn new(
        client: Client,
        endpoint_url: String,
        api_key: String,
        model: String,
        temperature: f32,
        max_tokens: u32,
        extra_payload: Option<Value>,
    ) -> Self {
        Self {
            client,
            endpoint_url,
            api_key,
            model,
            temperature,
            max_tokens,
            extra_payload,
        }
    }
}

impl LlmClient for LocalLlmClient {
    fn query<'a>(
        &'a self,
        prompt: &'a str,
        system_prompt: &'a str,
    ) -> BoxFuture<'a, Result<DeepSeekResponse, String>> {
        Box::pin(async move {
            let base = self.endpoint_url.trim().trim_end_matches('/');
            let url = if base.is_empty() {
                "http://localhost:11434/v1/chat/completions".to_string()
            } else if base.ends_with("/chat/completions") {
                base.to_string()
            } else {
                format!("{}/chat/completions", base)
            };

            let model_name = if self.model.trim().is_empty() {
                "llama3".to_string()
            } else {
                self.model.trim().to_string()
            };

            let mut body = serde_json::json!({
                "model": model_name,
                "messages": [
                    { "role": "system", "content": system_prompt },
                    { "role": "user", "content": prompt }
                ],
                "temperature": self.temperature,
                "max_tokens": self.max_tokens
            });

            merge_extra_payload(&mut body, self.extra_payload.as_ref());

            info!("Sending query to Local LLM endpoint: {}", url);

            let mut req = self
                .client
                .post(&url)
                .header("Content-Type", "application/json");

            if !self.api_key.trim().is_empty() {
                req = req.header("Authorization", format!("Bearer {}", self.api_key.trim()));
            }

            let res = req
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Local LLM request failed: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_txt = res.text().await.unwrap_or_default();
                return Err(format!("Local LLM returned HTTP {}: {}", status, err_txt));
            }

            let val: Value = res
                .json()
                .await
                .map_err(|e| format!("Invalid JSON from Local LLM: {}", e))?;

            let content = val["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("{}");

            Ok(parse_clean_json_response(content, prompt))
        })
    }
}

// ============================================================================
// 2. STT Client Architecture
// ============================================================================

/// Trait for speech recognition of captured WAV audio bytes.
pub trait SttClient: Send + Sync {
    fn transcribe<'a>(
        &'a self,
        wav_bytes: &'a [u8],
    ) -> BoxFuture<'a, Result<String, String>>;
}

/// OpenRouter Audio Transcriptions STT Client
pub struct OpenRouterSttClient {
    pub client: Client,
    pub api_key: String,
    pub model: String,
    pub language: String,
    pub extra_payload: Option<Value>,
}

impl OpenRouterSttClient {
    pub fn new(
        client: Client,
        api_key: String,
        model: String,
        language: String,
        extra_payload: Option<Value>,
    ) -> Self {
        Self {
            client,
            api_key,
            model,
            language,
            extra_payload,
        }
    }
}

impl SttClient for OpenRouterSttClient {
    fn transcribe<'a>(
        &'a self,
        wav_bytes: &'a [u8],
    ) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let key = self.api_key.trim();
            if key.is_empty() {
                return Err("OpenRouter STT API key not configured".to_string());
            }

            let model = if !self.model.trim().is_empty() {
                self.model.trim()
            } else {
                "openai/whisper"
            };

            let part = Part::bytes(wav_bytes.to_vec())
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| format!("Multipart error: {}", e))?;

            let mut form = Form::new()
                .part("file", part)
                .text("model", model.to_string());

            if !self.language.trim().is_empty() {
                form = form.text("language", self.language.trim().to_string());
            }

            // Merge extra_payload fields into multipart form
            if let Some(extra_map) = self.extra_payload.as_ref().and_then(|v| v.as_object()) {
                for (k, v) in extra_map {
                    let val_str = match v {
                        Value::String(s) => s.clone(),
                        _ => v.to_string(),
                    };
                    form = form.text(k.clone(), val_str);
                }
            }

            info!("Calling OpenRouter STT (model: {})...", model);

            let res = self
                .client
                .post("https://openrouter.ai/api/v1/audio/transcriptions")
                .header("Authorization", format!("Bearer {}", key))
                .header("HTTP-Referer", "https://yandex-station-lite.local")
                .header("X-Title", "Yandex Station Lite Control Center")
                .multipart(form)
                .send()
                .await
                .map_err(|e| format!("OpenRouter STT request error: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("OpenRouter STT HTTP {}: {}", status, err_text));
            }

            let body_text = res.text().await.unwrap_or_default();
            if let Some(txt) = parse_transcript_json(&body_text) {
                Ok(txt)
            } else {
                Err("OpenRouter STT: text not found in response".to_string())
            }
        })
    }
}

/// Cloudflare Workers AI Whisper STT Client
pub struct CloudflareSttClient {
    pub client: Client,
    pub account_id: String,
    pub api_key: String,
    pub model: String,
    pub extra_payload: Option<Value>,
}

impl CloudflareSttClient {
    pub fn new(
        client: Client,
        account_id: String,
        api_key: String,
        model: String,
        extra_payload: Option<Value>,
    ) -> Self {
        Self {
            client,
            account_id,
            api_key,
            model,
            extra_payload,
        }
    }
}

impl SttClient for CloudflareSttClient {
    fn transcribe<'a>(
        &'a self,
        wav_bytes: &'a [u8],
    ) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let key = self.api_key.trim();
            let acc = self.account_id.trim();
            if key.is_empty() || acc.is_empty() {
                return Err("Cloudflare Account ID or API token is empty".to_string());
            }

            let model = if self.model.trim().is_empty() {
                "@cf/openai/whisper"
            } else {
                self.model.trim()
            };

            let url = format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{}",
                acc, model
            );

            info!("Calling Cloudflare Workers AI STT (model: {})...", model);

            let res = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", key))
                .header("Content-Type", "application/octet-stream")
                .body(wav_bytes.to_vec())
                .send()
                .await
                .map_err(|e| format!("Cloudflare STT error: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("Cloudflare STT HTTP {}: {}", status, err_text));
            }

            let body_text = res.text().await.unwrap_or_default();
            if let Some(txt) = parse_transcript_json(&body_text) {
                Ok(txt)
            } else {
                Err("Cloudflare STT: empty transcript or silence".to_string())
            }
        })
    }
}

/// Local/Custom STT Client (e.g. whisper.cpp, faster-whisper, local HTTP server)
pub struct LocalSttClient {
    pub client: Client,
    pub endpoint_url: String,
    pub api_key: String,
    pub extra_payload: Option<Value>,
}

impl LocalSttClient {
    pub fn new(
        client: Client,
        endpoint_url: String,
        api_key: String,
        extra_payload: Option<Value>,
    ) -> Self {
        Self {
            client,
            endpoint_url,
            api_key,
            extra_payload,
        }
    }
}

impl SttClient for LocalSttClient {
    fn transcribe<'a>(
        &'a self,
        wav_bytes: &'a [u8],
    ) -> BoxFuture<'a, Result<String, String>> {
        Box::pin(async move {
            let url = self.endpoint_url.trim();
            if url.is_empty() {
                return Err("Local STT endpoint URL is empty".to_string());
            }

            let part = Part::bytes(wav_bytes.to_vec())
                .file_name("audio.wav")
                .mime_str("audio/wav")
                .map_err(|e| format!("Multipart error: {}", e))?;

            let mut form = Form::new()
                .part("file", part)
                .text("model", "whisper".to_string());

            if let Some(extra_map) = self.extra_payload.as_ref().and_then(|v| v.as_object()) {
                for (k, v) in extra_map {
                    let val_str = match v {
                        Value::String(s) => s.clone(),
                        _ => v.to_string(),
                    };
                    form = form.text(k.clone(), val_str);
                }
            }

            info!("Calling Local STT: {}", url);

            let mut req = self.client.post(url).multipart(form);
            if !self.api_key.trim().is_empty() {
                req = req.header("Authorization", format!("Bearer {}", self.api_key.trim()));
            }

            let res = req
                .send()
                .await
                .map_err(|e| format!("Local STT request error: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("Local STT HTTP {}: {}", status, err_text));
            }

            let body_text = res.text().await.unwrap_or_default();
            if let Some(txt) = parse_transcript_json(&body_text) {
                Ok(txt)
            } else {
                Err("Local STT: text not found in response".to_string())
            }
        })
    }
}

// ============================================================================
// 3. TTS Client Architecture
// ============================================================================

/// Trait for speech synthesis of text into playable audio bytes (MP3/WAV).
pub trait TtsClient: Send + Sync {
    fn synthesize<'a>(
        &'a self,
        text: &'a str,
    ) -> BoxFuture<'a, Result<Vec<u8>, String>>;
}

/// OpenRouter Speech Synthesis Client (/v1/audio/speech)
pub struct OpenRouterTtsClient {
    pub client: Client,
    pub api_key: String,
    pub model: String,
    pub voice: String,
    pub extra_payload: Option<Value>,
}

impl OpenRouterTtsClient {
    pub fn new(
        client: Client,
        api_key: String,
        model: String,
        voice: String,
        extra_payload: Option<Value>,
    ) -> Self {
        Self {
            client,
            api_key,
            model,
            voice,
            extra_payload,
        }
    }
}

impl TtsClient for OpenRouterTtsClient {
    fn synthesize<'a>(
        &'a self,
        text: &'a str,
    ) -> BoxFuture<'a, Result<Vec<u8>, String>> {
        Box::pin(async move {
            let key = self.api_key.trim();
            if key.is_empty() {
                return Err("OpenRouter TTS API key is empty".to_string());
            }

            let model = if self.model.trim().is_empty() {
                "openai/tts-1".to_string()
            } else {
                self.model.trim().to_string()
            };

            let voice = if self.voice.trim().is_empty() {
                "alloy".to_string()
            } else {
                self.voice.trim().to_string()
            };

            let mut body = serde_json::json!({
                "model": model,
                "input": text,
                "voice": voice,
                "response_format": "mp3"
            });

            // Also provide voice_id for ElevenLabs or third-party engines routed via OpenRouter
            if !voice.is_empty() && !voice.eq_ignore_ascii_case("alloy") {
                body["voice_id"] = serde_json::Value::String(voice.clone());
            }

            // Merge extra JSON parameters (e.g. speed, voice_id, stability, similarity_boost, etc.)
            merge_extra_payload(&mut body, self.extra_payload.as_ref());

            info!(
                "Calling OpenRouter TTS [model: {}, voice: {}] with payload keys: {:?}",
                model,
                voice,
                body.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>())
            );

            let res = self
                .client
                .post("https://openrouter.ai/api/v1/audio/speech")
                .header("Authorization", format!("Bearer {}", key))
                .header("Content-Type", "application/json")
                .header("HTTP-Referer", "https://yandex-station-lite.local")
                .header("X-Title", "Yandex Station Lite Control Center")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("OpenRouter TTS error: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("OpenRouter TTS HTTP {}: {}", status, err_text));
            }

            let bytes = res
                .bytes()
                .await
                .map_err(|e| format!("OpenRouter TTS byte read error: {}", e))?;

            if bytes.is_empty() {
                return Err("OpenRouter TTS returned 0 bytes".to_string());
            }

            Ok(bytes.to_vec())
        })
    }
}

/// Cloudflare Workers AI TTS Client (MeloTTS, ElevenLabs, MMS, etc.)
pub struct CloudflareTtsClient {
    pub client: Client,
    pub account_id: String,
    pub api_token: String,
    pub model: String,
    pub voice_id: String,
    pub lang: String,
    pub extra_payload: Option<Value>,
}

impl CloudflareTtsClient {
    pub fn new(
        client: Client,
        account_id: String,
        api_token: String,
        model: String,
        voice_id: String,
        lang: String,
        extra_payload: Option<Value>,
    ) -> Self {
        Self {
            client,
            account_id,
            api_token,
            model,
            voice_id,
            lang,
            extra_payload,
        }
    }
}

impl TtsClient for CloudflareTtsClient {
    fn synthesize<'a>(
        &'a self,
        text: &'a str,
    ) -> BoxFuture<'a, Result<Vec<u8>, String>> {
        Box::pin(async move {
            let acc = self.account_id.trim();
            let token = self.api_token.trim();
            if token.is_empty() {
                return Err("Cloudflare API token is empty".to_string());
            }

            let raw_model = self.model.trim();
            let is_elevenlabs = raw_model.contains("elevenlabs") || raw_model.contains("eleven-");

            // If user supplied an ElevenLabs key (xi-...) or selected ElevenLabs
            if token.starts_with("xi-") {
                let v_id = if !self.voice_id.trim().is_empty() {
                    self.voice_id.trim()
                } else {
                    "21m00Tcm4TlvDq8ikWAM"
                };
                let m_id = if raw_model.contains("flash") {
                    "eleven_flash_v2_5"
                } else if raw_model.contains("turbo") {
                    "eleven_turbo_v2_5"
                } else {
                    "eleven_multilingual_v2"
                };
                let el_url = format!("https://api.elevenlabs.io/v1/text-to-speech/{}", v_id);
                let mut el_body = serde_json::json!({
                    "text": text,
                    "model_id": m_id
                });
                merge_extra_payload(&mut el_body, self.extra_payload.as_ref());

                info!(
                    "ElevenLabs direct routing: voice {}, model {}",
                    v_id, m_id
                );

                let res = self
                    .client
                    .post(&el_url)
                    .header("xi-api-key", token)
                    .header("Content-Type", "application/json")
                    .json(&el_body)
                    .send()
                    .await
                    .map_err(|e| format!("ElevenLabs request error: {}", e))?;

                if !res.status().is_success() {
                    let status = res.status();
                    let err_text = res.text().await.unwrap_or_default();
                    return Err(format!("ElevenLabs HTTP {}: {}", status, err_text));
                }

                let bytes = res
                    .bytes()
                    .await
                    .map_err(|e| format!("ElevenLabs byte read error: {}", e))?;
                return Ok(bytes.to_vec());
            }

            let is_custom_worker_url = raw_model.starts_with("http://") || raw_model.starts_with("https://");

            let model = if raw_model.is_empty()
                || raw_model == "@cf/myshell/melotts-v1"
                || raw_model == "@cf/myshell/melotts"
                || is_elevenlabs
            {
                if is_elevenlabs {
                    info!("Cloudflare Workers AI does not have ElevenLabs endpoint (/ai/run). Automatically routing to native Cloudflare MeloTTS [@cf/myshell-ai/melotts]");
                }
                "@cf/myshell-ai/melotts"
            } else {
                raw_model
            };

            let url = if is_custom_worker_url {
                raw_model.to_string()
            } else {
                if acc.is_empty() {
                    return Err("Cloudflare Account ID is empty".to_string());
                }
                format!(
                    "https://api.cloudflare.com/client/v4/accounts/{}/ai/run/{}",
                    acc, model
                )
            };

            let language = if !self.lang.trim().is_empty() {
                self.lang.trim()
            } else {
                "ru"
            };

            let mut body = if is_custom_worker_url {
                serde_json::json!({
                    "text": text,
                    "prompt": text,
                    "voice": if !self.voice_id.trim().is_empty() { self.voice_id.trim() } else { "ru-RU-SvetlanaNeural" },
                    "lang": language
                })
            } else if model.contains("aura") {
                serde_json::json!({
                    "text": text
                })
            } else {
                // @cf/myshell-ai/melotts expects { "prompt": text, "lang": "ru" }
                serde_json::json!({
                    "prompt": text,
                    "text": text,
                    "lang": language
                })
            };

            // Merge extra JSON parameters (e.g. lang, speed, stability, etc.)
            merge_extra_payload(&mut body, self.extra_payload.as_ref());

            info!(
                "Calling Cloudflare Workers AI TTS [{}] for account {} with payload: {:?}",
                model,
                acc,
                body.as_object().map(|o| o.keys().cloned().collect::<Vec<_>>())
            );

            let res = self
                .client
                .post(&url)
                .header("Authorization", format!("Bearer {}", token))
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Cloudflare TTS error: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("Cloudflare TTS HTTP {}: {}", status, err_text));
            }

            let bytes = res
                .bytes()
                .await
                .map_err(|e| format!("Cloudflare TTS byte read error: {}", e))?;

            if bytes.is_empty() {
                return Err("Cloudflare TTS returned 0 bytes".to_string());
            }

            // Check if response is JSON (MeloTTS returns {"result": {"audio": "<base64>"}} or {"audio": "<base64>"})
            if bytes.starts_with(b"{") {
                if let Ok(val) = serde_json::from_slice::<Value>(&bytes) {
                    let b64_candidate = val.pointer("/result/audio")
                        .or_else(|| val.get("audio"))
                        .or_else(|| val.get("result"))
                        .and_then(|v| v.as_str());

                    if let Some(b64) = b64_candidate {
                        if let Some(decoded) = decode_base64(b64) {
                            info!("Decoded {} base64 audio bytes from Cloudflare MeloTTS", decoded.len());
                            return Ok(decoded);
                        }
                    }

                    if let Some(err_msg) = val.pointer("/errors/0/message").and_then(|v| v.as_str()) {
                        return Err(format!("Cloudflare Workers AI TTS error: {}", err_msg));
                    }
                }
            }

            Ok(bytes.to_vec())
        })
    }
}

/// Local/Custom TTS Client (e.g. Piper, Coqui, MaryTTS, local HTTP server)
pub struct LocalTtsClient {
    pub client: Client,
    pub endpoint_url: String,
    pub extra_payload: Option<Value>,
}

impl LocalTtsClient {
    pub fn new(client: Client, endpoint_url: String, extra_payload: Option<Value>) -> Self {
        Self {
            client,
            endpoint_url,
            extra_payload,
        }
    }
}

impl TtsClient for LocalTtsClient {
    fn synthesize<'a>(
        &'a self,
        text: &'a str,
    ) -> BoxFuture<'a, Result<Vec<u8>, String>> {
        Box::pin(async move {
            let url = self.endpoint_url.trim();
            if url.is_empty() {
                return Err("Local TTS endpoint URL is empty".to_string());
            }

            let mut body = serde_json::json!({
                "text": text,
            });

            merge_extra_payload(&mut body, self.extra_payload.as_ref());

            info!("Calling Local TTS endpoint: {}", url);

            let res = self
                .client
                .post(url)
                .header("Content-Type", "application/json")
                .json(&body)
                .send()
                .await
                .map_err(|e| format!("Local TTS request error: {}", e))?;

            if !res.status().is_success() {
                let status = res.status();
                let err_text = res.text().await.unwrap_or_default();
                return Err(format!("Local TTS HTTP {}: {}", status, err_text));
            }

            let bytes = res
                .bytes()
                .await
                .map_err(|e| format!("Local TTS byte read error: {}", e))?;

            if bytes.is_empty() {
                return Err("Local TTS returned 0 bytes".to_string());
            }

            Ok(bytes.to_vec())
        })
    }
}

/// Free Google Translate TTS Client
pub struct GoogleTtsClient {
    pub client: Client,
}

impl GoogleTtsClient {
    pub fn new(client: Client) -> Self {
        Self { client }
    }
}

impl TtsClient for GoogleTtsClient {
    fn synthesize<'a>(
        &'a self,
        text: &'a str,
    ) -> BoxFuture<'a, Result<Vec<u8>, String>> {
        Box::pin(async move {
            let encoded = urlencoding::encode(text);
            let url = format!(
                "https://translate.google.com/translate_tts?ie=UTF-8&tl=ru&client=tw-ob&q={}",
                encoded
            );

            let res = self
                .client
                .get(&url)
                .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64)")
                .send()
                .await
                .map_err(|e| format!("Google TTS error: {}", e))?;

            if !res.status().is_success() {
                return Err(format!("Google TTS returned HTTP {}", res.status()));
            }

            let bytes = res
                .bytes()
                .await
                .map_err(|e| format!("Google TTS byte read error: {}", e))?;

            if bytes.is_empty() {
                return Err("Google TTS returned 0 bytes".to_string());
            }

            Ok(bytes.to_vec())
        })
    }
}

// ============================================================================
// Helper Parsers
// ============================================================================

pub fn parse_clean_json_response(content: &str, prompt: &str) -> DeepSeekResponse {
    let cleaned = content
        .trim()
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    let json_candidate = if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
        if start <= end { &cleaned[start..=end] } else { cleaned }
    } else {
        cleaned
    };

    if let Ok(v) = serde_json::from_str::<Value>(json_candidate) {
        if let Some(obj) = v.as_object() {
            let text = obj.get("text").and_then(|t| t.as_str()).unwrap_or("").to_string();
            let action = obj.get("action").and_then(|a| a.as_str()).unwrap_or("none").to_string();
            let song = obj.get("song").and_then(|s| s.as_str()).unwrap_or("").to_string();
            let color = if let Some(arr) = obj.get("color").and_then(|c| c.as_array()) {
                if arr.len() == 3 {
                    [
                        arr[0].as_u64().unwrap_or(0) as u8,
                        arr[1].as_u64().unwrap_or(150) as u8,
                        arr[2].as_u64().unwrap_or(255) as u8,
                    ]
                } else {
                    [0, 150, 255]
                }
            } else {
                [0, 150, 255]
            };
            if !text.is_empty() {
                return DeepSeekResponse { text, color, action, song };
            }
        }
    }

    if let Ok(resp) = serde_json::from_str::<DeepSeekResponse>(cleaned) {
        return resp;
    }

    // Default fallback if model didn't return valid JSON
    let p_lower = prompt.to_lowercase();
    let color = if p_lower.contains("синий") || p_lower.contains("blue") {
        [0, 80, 255]
    } else if p_lower.contains("красный") || p_lower.contains("red") {
        [255, 30, 30]
    } else if p_lower.contains("зеленый") || p_lower.contains("green") {
        [0, 255, 60]
    } else {
        [0, 150, 255]
    };

    DeepSeekResponse {
        text: if cleaned.is_empty() || cleaned == "{}" {
            "Команда принята.".to_string()
        } else {
            cleaned.to_string()
        },
        color,
        action: "none".to_string(),
        song: String::new(),
    }
}

pub fn parse_transcript_json(raw: &str) -> Option<String> {
    let val: Value = serde_json::from_str(raw).ok()?;

    // Standard OpenAI/OpenRouter {"text": "..."}
    if let Some(t) = val.get("text").and_then(|v| v.as_str()) {
        let clean = t.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }

    // Cloudflare Workers AI {"result": {"text": "..."}}
    if let Some(t) = val.pointer("/result/text").and_then(|v| v.as_str()) {
        let clean = t.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }

    // Cloudflare direct string {"result": "..."}
    if let Some(t) = val.get("result").and_then(|v| v.as_str()) {
        let clean = t.trim();
        if !clean.is_empty() {
            return Some(clean.to_string());
        }
    }

    None
}

pub fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let clean = input.trim();
    let mut table = [255u8; 256];
    for (i, b) in b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/".iter().enumerate() {
        table[*b as usize] = i as u8;
    }
    let mut out = Vec::with_capacity(clean.len() * 3 / 4);
    let mut buf = 0u32;
    let mut bits = 0;
    for &b in clean.as_bytes() {
        if b == b'=' {
            break;
        }
        let val = table[b as usize];
        if val == 255 {
            continue; // skip whitespace or invalid characters
        }
        buf = (buf << 6) | (val as u32);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    if out.is_empty() {
        None
    } else {
        Some(out)
    }
}
