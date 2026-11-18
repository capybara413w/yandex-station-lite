use crate::config::{merge_extra_payload, AppConfig, ProxyConfig};
use crate::services::chat::ChatMessageItem;
use crate::services::clients::{
    BoxFuture, CloudflareLlmClient, LlmClient, LocalLlmClient, OpenRouterLlmClient,
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HaCall {
    pub domain: String,
    pub service: String,
    #[serde(default)]
    pub service_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepSeekResponse {
    pub text: String,
    pub color: [u8; 3],
    pub action: String,
    pub song: String,
    #[serde(default)]
    pub ha_calls: Vec<HaCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timer_call: Option<crate::services::timer_alarm::TimerCall>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub alarm_call: Option<crate::services::timer_alarm::AlarmCall>,
}

pub struct DeepSeekService {
    config: Arc<RwLock<AppConfig>>,
    client: Client,
}

impl DeepSeekService {
    pub fn new(config: Arc<RwLock<AppConfig>>, proxy: &ProxyConfig) -> Self {
        let mut builder = Client::builder().timeout(std::time::Duration::from_secs(45));

        if proxy.enabled {
            let local_proxy = "http://127.0.0.1:8889";
            match reqwest::Proxy::all(local_proxy) {
                Ok(p) => {
                    info!("Using local proxy bridge for DeepSeek: {}", local_proxy);
                    builder = builder.proxy(p);
                }
                Err(e) => {
                    warn!("Failed to configure proxy for DeepSeek: {}", e);
                }
            }
        }

        let client = builder.build().unwrap_or_else(|_| Client::new());
        Self { config, client }
    }

    /// Queries active LLM provider (OpenRouter, Cloudflare, Groq, OpenAI, Mistral, Anthropic, Gemini, DeepSeek, or Custom) chat completions API.
    pub async fn query(&self, prompt: &str) -> Result<DeepSeekResponse, String> {
        self.query_with_history(prompt, &[]).await
    }

    /// Queries active LLM provider including full multi-turn conversation context.
    pub async fn query_with_history(
        &self,
        prompt: &str,
        history: &[ChatMessageItem],
    ) -> Result<DeepSeekResponse, String> {
        self.query_with_context(prompt, history, "").await
    }

    /// Queries active LLM provider with conversation history and dynamic Smart Home context.
    pub async fn query_with_context(
        &self,
        prompt: &str,
        history: &[ChatMessageItem],
        smart_home_context: &str,
    ) -> Result<DeepSeekResponse, String> {
        let (
            provider,
            api_key,
            model,
            system_prompt,
            temperature,
            max_tokens,
            custom_url,
            cf_account_id,
            cf_api_token,
            cf_model,
            extra_payload,
        ) = {
            let cfg = self.config.read().await;
            let prov = cfg.deepseek.provider.trim().to_lowercase();
            let p = if prov.is_empty() {
                if !cfg.deepseek.openrouter_api_key.is_empty() {
                    "openrouter".to_string()
                } else if !cfg.deepseek.cloudflare_api_token.is_empty() {
                    "cloudflare".to_string()
                } else if !cfg.deepseek.api_key.is_empty() {
                    "deepseek".to_string()
                } else {
                    "openrouter".to_string()
                }
            } else {
                prov
            };

            let (k, m) = match p.as_str() {
                "openrouter" => (
                    if !cfg.deepseek.openrouter_api_key.is_empty() {
                        cfg.deepseek.openrouter_api_key.clone()
                    } else {
                        cfg.deepseek.api_key.clone()
                    },
                    if !cfg.deepseek.openrouter_model.is_empty() {
                        cfg.deepseek.openrouter_model.clone()
                    } else {
                        "deepseek/deepseek-chat".to_string()
                    },
                ),
                "cloudflare" => (
                    if !cfg.deepseek.cloudflare_api_token.is_empty() {
                        cfg.deepseek.cloudflare_api_token.clone()
                    } else {
                        cfg.deepseek.api_key.clone()
                    },
                    if !cfg.deepseek.cloudflare_model.is_empty() {
                        cfg.deepseek.cloudflare_model.clone()
                    } else {
                        "@cf/meta/llama-3.1-8b-instruct".to_string()
                    },
                ),
                "groq" => (
                    if !cfg.deepseek.groq_api_key.is_empty() {
                        cfg.deepseek.groq_api_key.clone()
                    } else {
                        cfg.deepseek.api_key.clone()
                    },
                    if !cfg.deepseek.groq_model.is_empty() {
                        cfg.deepseek.groq_model.clone()
                    } else {
                        "llama-3.3-70b-versatile".to_string()
                    },
                ),
                "openai" => (
                    if !cfg.deepseek.openai_api_key.is_empty() {
                        cfg.deepseek.openai_api_key.clone()
                    } else {
                        cfg.deepseek.api_key.clone()
                    },
                    if !cfg.deepseek.openai_model.is_empty() {
                        cfg.deepseek.openai_model.clone()
                    } else {
                        "gpt-4o-mini".to_string()
                    },
                ),
                "mistral" => (
                    if !cfg.deepseek.mistral_api_key.is_empty() {
                        cfg.deepseek.mistral_api_key.clone()
                    } else {
                        cfg.deepseek.api_key.clone()
                    },
                    if !cfg.deepseek.mistral_model.is_empty() {
                        cfg.deepseek.mistral_model.clone()
                    } else {
                        "mistral-small-latest".to_string()
                    },
                ),
                "anthropic" => (
                    cfg.deepseek.anthropic_api_key.clone(),
                    if !cfg.deepseek.anthropic_model.is_empty() {
                        cfg.deepseek.anthropic_model.clone()
                    } else {
                        "claude-3-5-haiku-latest".to_string()
                    },
                ),
                "gemini" => (
                    if !cfg.deepseek.gemini_api_key.is_empty() {
                        cfg.deepseek.gemini_api_key.clone()
                    } else {
                        cfg.deepseek.api_key.clone()
                    },
                    if !cfg.deepseek.gemini_model.is_empty() {
                        cfg.deepseek.gemini_model.clone()
                    } else {
                        "gemini-3.8-flash".to_string()
                    },
                ),
                "perplexity" => (
                    cfg.deepseek.perplexity_api_key.clone(),
                    if !cfg.deepseek.perplexity_model.is_empty() {
                        cfg.deepseek.perplexity_model.clone()
                    } else {
                        "sonar".to_string()
                    },
                ),
                "together" => (
                    cfg.deepseek.together_api_key.clone(),
                    if !cfg.deepseek.together_model.is_empty() {
                        cfg.deepseek.together_model.clone()
                    } else {
                        "meta-llama/Llama-3.3-70B-Instruct-Turbo".to_string()
                    },
                ),
                "custom" | "local" => (
                    cfg.deepseek.api_key.clone(),
                    if !cfg.deepseek.custom_model.is_empty() {
                        cfg.deepseek.custom_model.clone()
                    } else {
                        cfg.deepseek.model.clone()
                    },
                ),
                _ => (cfg.deepseek.api_key.clone(), cfg.deepseek.model.clone()),
            };

            (
                p,
                k.trim().to_string(),
                m.trim().to_string(),
                cfg.deepseek.system_prompt.clone(),
                cfg.deepseek.temperature,
                cfg.deepseek.max_tokens,
                cfg.deepseek.custom_base_url.clone(),
                cfg.deepseek.cloudflare_account_id.clone(),
                cfg.deepseek.cloudflare_api_token.clone(),
                cfg.deepseek.cloudflare_model.clone(),
                cfg.deepseek.extra_payload.clone(),
            )
        };

        let effective_system_prompt = {
            let base = if system_prompt.trim().is_empty() {
                "Ты — умный голосовой ассистент смарт-колонки с поддержкой музыки, светодиодной подсветки и Умного Дома (Home Assistant).".to_string()
            } else {
                system_prompt.clone()
            };
            let mut prompt_full = format!(
                "{}\n\nВАЖНО: Все голосовые команды и действия выполняются СТРОГО через тебя! Отвечай СТРОГО валидным JSON объектом (без markdown-оберток ```json):\n{{\n  \"text\": \"краткий естественный ответ для озвучки (1-2 предложения)\",\n  \"color\": [R, G, B],\n  \"action\": \"play\" | \"stop\" | \"resume\" | \"none\",\n  \"song\": \"название трека (только если action=play)\",\n  \"ha_calls\": [\n    {{\"domain\": \"light|switch|...\", \"service\": \"turn_on|turn_off|toggle\", \"service_data\": {{\"entity_id\": \"...\"}}}}\n  ],\n  \"timer_call\": null | {{\"action\": \"set\" | \"cancel\" | \"cancel_all\", \"seconds\": 300, \"label\": \"таймер\"}},\n  \"alarm_call\": null | {{\"action\": \"set\" | \"cancel\", \"time\": \"08:00\", \"label\": \"будильник\"}}\n}}",
                base
            );
            if !smart_home_context.is_empty() {
                prompt_full.push_str("\n\n");
                prompt_full.push_str(smart_home_context);
            }
            prompt_full
        };

        // 1. Direct dispatch to dedicated LlmClient implementations
        if provider == "openrouter" {
            let client = OpenRouterLlmClient::new(
                self.client.clone(),
                api_key,
                model,
                temperature,
                max_tokens,
                extra_payload,
            );
            return client.query_with_history(prompt, &effective_system_prompt, history).await;
        }

        if provider == "cloudflare" {
            let token = if !cf_api_token.is_empty() {
                cf_api_token
            } else {
                api_key
            };
            let model_name = if !cf_model.is_empty() {
                cf_model
            } else {
                model
            };
            let client = CloudflareLlmClient::new(
                self.client.clone(),
                cf_account_id,
                token,
                model_name,
                extra_payload,
            );
            return client.query_with_history(prompt, &effective_system_prompt, history).await;
        }

        if provider == "custom" || provider == "local" {
            let client = LocalLlmClient::new(
                self.client.clone(),
                custom_url,
                api_key,
                model,
                temperature,
                max_tokens,
                extra_payload,
            );
            return client.query_with_history(prompt, &effective_system_prompt, history).await;
        }

        if api_key.is_empty() {
            info!("No API key for provider '{}', using local fallback", provider);
            return Ok(self.fallback_response(prompt));
        }

        let endpoint_url = match provider.as_str() {
            "groq" => "https://api.groq.com/openai/v1/chat/completions".to_string(),
            "openai" => "https://api.openai.com/v1/chat/completions".to_string(),
            "mistral" => "https://api.mistral.ai/v1/chat/completions".to_string(),
            "gemini" => "https://generativelanguage.googleapis.com/v1beta/openai/chat/completions".to_string(),
            "perplexity" => "https://api.perplexity.ai/chat/completions".to_string(),
            "together" => "https://api.together.xyz/v1/chat/completions".to_string(),
            "anthropic" => "https://api.anthropic.com/v1/messages".to_string(),
            _ => "https://api.deepseek.com/chat/completions".to_string(),
        };

        let start_idx = if history.len() > 14 { history.len() - 14 } else { 0 };

        let mut body = if provider == "anthropic" {
            let mut messages = Vec::new();
            for item in &history[start_idx..] {
                if item.role == "user" {
                    messages.push(serde_json::json!({ "role": "user", "content": item.text }));
                } else if item.role == "assistant" {
                    let content = serde_json::json!({
                        "text": item.text,
                        "color": item.color.unwrap_or([0, 200, 255]),
                        "action": item.action.as_deref().unwrap_or("none"),
                        "song": item.song.as_deref().unwrap_or("")
                    }).to_string();
                    messages.push(serde_json::json!({ "role": "assistant", "content": content }));
                }
            }
            messages.push(serde_json::json!({ "role": "user", "content": prompt }));

            serde_json::json!({
                "model": model,
                "system": &effective_system_prompt,
                "messages": messages,
                "temperature": temperature,
                "max_tokens": max_tokens
            })
        } else {
            let mut messages = Vec::new();
            messages.push(serde_json::json!({
                "role": "system",
                "content": &effective_system_prompt
            }));

            for item in &history[start_idx..] {
                if item.role == "user" {
                    messages.push(serde_json::json!({ "role": "user", "content": item.text }));
                } else if item.role == "assistant" {
                    let content = serde_json::json!({
                        "text": item.text,
                        "color": item.color.unwrap_or([0, 200, 255]),
                        "action": item.action.as_deref().unwrap_or("none"),
                        "song": item.song.as_deref().unwrap_or("")
                    }).to_string();
                    messages.push(serde_json::json!({ "role": "assistant", "content": content }));
                }
            }
            messages.push(serde_json::json!({ "role": "user", "content": prompt }));

            let mut b = serde_json::json!({
                "model": model,
                "messages": messages,
                "temperature": temperature,
                "max_tokens": max_tokens
            });
            let model_lower = model.to_lowercase();
            if !model_lower.contains("reasoner") && !model_lower.contains("r1") {
                b["response_format"] = serde_json::json!({ "type": "json_object" });
            }
            b
        };

        // Merge extra_payload into body
        merge_extra_payload(&mut body, extra_payload.as_ref());

        if provider == "gemini" {
            if let Some(obj) = body.as_object_mut() {
                obj.remove("repetition_penalty");
                obj.remove("presence_penalty");
                obj.remove("frequency_penalty");
            }
        }

        let mut req = self
            .client
            .post(&endpoint_url)
            .header("Content-Type", "application/json");

        if provider == "anthropic" {
            req = req
                .header("x-api-key", &api_key)
                .header("anthropic-version", "2023-06-01");
        } else if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", api_key));
        }

        info!("Calling LLM [{}] via {} (model: {})...", provider, endpoint_url, model);
        let res = req
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("HTTP request error to {}: {}", provider, e))?;

        if !res.status().is_success() {
            let status = res.status();
            let err_txt = res.text().await.unwrap_or_default();
            error!("LLM [{}] error HTTP {}: {}", provider, status, err_txt);

            // Automatic fallback: if provider rejected response_format or demanded 'json' in prompt, retry once without response_format
            if status.as_u16() == 400 && (err_txt.contains("response_format") || err_txt.contains("json") || err_txt.contains("JSON")) {
                warn!("LLM [{}] rejected response_format, retrying without response_format...", provider);
                if let Some(obj) = body.as_object_mut() {
                    obj.remove("response_format");
                }
                let mut retry_req = self.client.post(&endpoint_url).header("Content-Type", "application/json");
                if provider == "anthropic" {
                    retry_req = retry_req.header("x-api-key", &api_key).header("anthropic-version", "2023-06-01");
                } else if !api_key.is_empty() {
                    retry_req = retry_req.header("Authorization", format!("Bearer {}", api_key));
                }

                if let Ok(retry_res) = retry_req.json(&body).send().await {
                    if retry_res.status().is_success() {
                        if let Ok(val) = retry_res.json::<serde_json::Value>().await {
                            let content = if provider == "anthropic" {
                                val["content"][0]["text"].as_str().unwrap_or("{}")
                            } else {
                                val["choices"][0]["message"]["content"].as_str().unwrap_or("{}")
                            };
                            info!("LLM [{}] raw content (retry without response_format): {}", provider, content);
                            return Ok(crate::services::clients::parse_clean_json_response(content, prompt));
                        }
                    }
                }
            }

            return Err(format!("{} returned {}: {}", provider, status, err_txt));
        }

        let val: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("Invalid JSON response: {}", e))?;

        let content = if provider == "anthropic" {
            val["content"][0]["text"].as_str().unwrap_or("{}")
        } else {
            val["choices"][0]["message"]["content"]
                .as_str()
                .unwrap_or("{}")
        };
        info!("LLM [{}] raw content: {}", provider, content);
        Ok(crate::services::clients::parse_clean_json_response(content, prompt))
    }

    fn parse_json_or_fallback(&self, content: &str, prompt: &str) -> DeepSeekResponse {
        let cleaned = content
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        if let Ok(resp) = serde_json::from_str::<DeepSeekResponse>(cleaned) {
            return resp;
        }

        // If direct parse fails, try extracting JSON substring
        if let (Some(start), Some(end)) = (cleaned.find('{'), cleaned.rfind('}')) {
            if start <= end {
                if let Ok(resp) = serde_json::from_str::<DeepSeekResponse>(&cleaned[start..=end]) {
                    return resp;
                }
            }
        }

        self.fallback_response(prompt)
    }

    fn fallback_response(&self, prompt: &str) -> DeepSeekResponse {
        let p_lower = prompt.to_lowercase();
        let (text, color, action, song) = if p_lower.contains("синий") || p_lower.contains("blue") {
            ("Включаю синюю подсветку.".to_string(), [0, 80, 255], "none".to_string(), String::new())
        } else if p_lower.contains("красный") || p_lower.contains("red") {
            ("Включаю красный цвет.".to_string(), [255, 0, 0], "none".to_string(), String::new())
        } else if p_lower.contains("зеленый") || p_lower.contains("зелёный") || p_lower.contains("green") {
            ("Включаю зеленый цвет.".to_string(), [0, 255, 0], "none".to_string(), String::new())
        } else if p_lower.contains("стоп") || p_lower.contains("выключи") || p_lower.contains("останови") {
            ("Останавливаю воспроизведение музыки.".to_string(), [0, 150, 255], "stop".to_string(), String::new())
        } else if p_lower.contains("продолжи") || p_lower.contains("возобнови") {
            ("Продолжаю воспроизведение.".to_string(), [0, 255, 100], "resume".to_string(), String::new())
        } else if p_lower.contains("включи") || p_lower.contains("сыграй") || p_lower.contains("поставь") {
            let song = prompt
                .replace("включи", "")
                .replace("сыграй", "")
                .replace("поставь", "")
                .replace("музыку", "")
                .trim()
                .to_string();
            let query = if song.is_empty() {
                "Lofi chill beats".to_string()
            } else {
                song
            };
            (format!("Включаю {}.", query), [180, 0, 255], "play".to_string(), query)
        } else {
            (format!("Яндекс Станция Лайт слушает: {}.", prompt), [0, 150, 255], "none".to_string(), String::new())
        };

        DeepSeekResponse {
            text,
            color,
            action,
            song,
            ha_calls: Vec::new(),
            timer_call: None,
            alarm_call: None,
        }
    }
}

impl LlmClient for DeepSeekService {
    fn query<'a>(
        &'a self,
        prompt: &'a str,
        _system_prompt: &'a str,
    ) -> BoxFuture<'a, Result<DeepSeekResponse, String>> {
        Box::pin(async move { self.query(prompt).await })
    }

    fn query_with_history<'a>(
        &'a self,
        prompt: &'a str,
        _system_prompt: &'a str,
        history: &'a [ChatMessageItem],
    ) -> BoxFuture<'a, Result<DeepSeekResponse, String>> {
        Box::pin(async move { self.query_with_history(prompt, history).await })
    }
}

