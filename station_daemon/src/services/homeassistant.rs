use crate::config::AppConfig;
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HassState {
    pub entity_id: String,
    pub state: String,
    #[serde(default)]
    pub attributes: serde_json::Value,
    #[serde(default)]
    pub last_changed: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HassCallServiceRequest {
    pub domain: String,
    pub service: String,
    #[serde(default)]
    pub service_data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HassProcessRequest {
    pub text: String,
    #[serde(default = "default_lang_ru")]
    pub language: String,
}

fn default_lang_ru() -> String {
    "ru".to_string()
}

pub struct HomeAssistantService {
    config: Arc<RwLock<AppConfig>>,
    client: reqwest::Client,
}

impl HomeAssistantService {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .build()
            .unwrap_or_default();

        Self { config, client }
    }

    /// Returns base URL and token from config
    async fn get_auth(&self) -> Result<(String, String), String> {
        let cfg = self.config.read().await;
        let ha = &cfg.homeassistant;
        if !ha.enabled {
            return Err("Home Assistant integration is disabled in settings".to_string());
        }
        if ha.token.trim().is_empty() {
            return Err("Home Assistant Long-Lived Access Token is empty".to_string());
        }
        let url = ha.url.trim_end_matches('/').to_string();
        Ok((url, ha.token.trim().to_string()))
    }

    fn build_headers(token: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Ok(val) = HeaderValue::from_str(&format!("Bearer {}", token)) {
            headers.insert(AUTHORIZATION, val);
        }
        headers
    }

    /// Check connection & ping HA API
    pub async fn check_status(&self) -> Result<serde_json::Value, String> {
        let (base_url, token) = self.get_auth().await?;
        let url = format!("{}/api/", base_url);

        let resp = self
            .client
            .get(&url)
            .headers(Self::build_headers(&token))
            .send()
            .await
            .map_err(|e| format!("Failed to connect to Home Assistant: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HA returned HTTP status: {}", resp.status()));
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Invalid JSON response from HA: {}", e))?;

        Ok(body)
    }

    /// Fetch all device states/entities from HA
    pub async fn get_entities(&self) -> Result<Vec<HassState>, String> {
        let (base_url, token) = self.get_auth().await?;
        let url = format!("{}/api/states", base_url);

        let resp = self
            .client
            .get(&url)
            .headers(Self::build_headers(&token))
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("HTTP error {}: Failed to get states", resp.status()));
        }

        let states: Vec<HassState> = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse states JSON: {}", e))?;

        Ok(states)
    }

    /// Call a service (e.g. light/turn_on, switch/toggle, climate/set_temperature)
    pub async fn call_service(
        &self,
        domain: &str,
        service: &str,
        service_data: serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        let (base_url, token) = self.get_auth().await?;
        let url = format!("{}/api/services/{}/{}", base_url, domain, service);

        // Normalize entity_id: if passed without domain (e.g. "podsvetka_na_kukhne"), prepend "{domain}."
        let mut final_data = service_data;
        if let Some(obj) = final_data.as_object_mut() {
            if let Some(eid) = obj.get("entity_id").and_then(|v| v.as_str()) {
                if !eid.contains('.') {
                    obj.insert("entity_id".to_string(), serde_json::Value::String(format!("{}.{}", domain, eid)));
                }
            }
        }

        info!("Calling Home Assistant service: {}/{} with data: {:?}", domain, service, final_data);

        let resp = self
            .client
            .post(&url)
            .headers(Self::build_headers(&token))
            .json(&final_data)
            .send()
            .await
            .map_err(|e| format!("Failed to call service: {}", e))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let err_text = resp.text().await.unwrap_or_default();
            return Err(format!("Service call failed (HTTP {}): {}", status, err_text));
        }

        let result: serde_json::Value = resp
            .json()
            .await
            .unwrap_or_else(|_| serde_json::json!({"status": "ok"}));

        Ok(result)
    }

    /// Builds a compact text summary of smart home devices for the LLM system prompt
    pub async fn get_entities_summary(&self) -> String {
        let entities = match self.get_entities().await {
            Ok(e) => e,
            Err(_) => return String::new(),
        };

        let mut lines = Vec::new();
        lines.push("\n[АКТУАЛЬНЫЕ УСТРОЙСТВА И ДАТЧИКИ УМНОГО ДОМА (Home Assistant)]:".to_string());
        for e in entities {
            let domain = e.entity_id.split('.').next().unwrap_or("");
            if !matches!(domain, "light" | "switch" | "sensor" | "binary_sensor" | "input_boolean" | "sun" | "todo" | "climate") {
                continue;
            }
            if e.entity_id.contains("backup") {
                continue;
            }
            let friendly_name = e.attributes.get("friendly_name")
                .and_then(|v| v.as_str())
                .unwrap_or(&e.entity_id);
            let unit = e.attributes.get("unit_of_measurement")
                .and_then(|v| v.as_str())
                .unwrap_or("");

            if !unit.is_empty() {
                lines.push(format!("- {} (\"{}\", состояние: {} {})", e.entity_id, friendly_name, e.state, unit));
            } else {
                lines.push(format!("- {} (\"{}\", состояние: {})", e.entity_id, friendly_name, e.state));
            }
        }

        lines.push("\nИНСТРУКЦИЯ ПО УПРАВЛЕНИЮ ДОМОМ:".to_string());
        lines.push("Если пользователь просит включить/выключить/переключить устройство:".to_string());
        lines.push("ОБЯЗАТЕЛЬНО добавь в ответ заполненный массив \"ha_calls\": [{\"domain\": \"...\", \"service\": \"turn_on\"|\"turn_off\"|\"toggle\", \"service_data\": {\"entity_id\": \"...\"}}]. Не оставляй ha_calls пустым при командах управления!".to_string());
        lines.push("Если пользователь спрашивает статус (например: какая температура, закрыто ли окно, горит ли свет), используй актуальные данные выше и ответь текстом в поле \"text\", оставив \"ha_calls\": [].".to_string());

        lines.join("\n")
    }

    /// Process a natural language voice command via Home Assistant Assist (/api/conversation/process)
    /// Returns Some(speech_text) if Home Assistant processed an action or recognized the intent,
    /// or None if HA didn't match any intent (so we fall back to LLM).
    pub async fn process_conversation(&self, text: &str) -> Result<Option<String>, String> {
        let (base_url, token) = match self.get_auth().await {
            Ok(auth) => auth,
            Err(_) => return Ok(None), // HA not enabled or configured -> skip to LLM
        };

        {
            let cfg = self.config.read().await;
            if !cfg.homeassistant.conversation_api {
                return Ok(None);
            }
        }

        let url = format!("{}/api/conversation/process", base_url);
        let payload = serde_json::json!({
            "text": text,
            "language": "ru"
        });

        let resp = self
            .client
            .post(&url)
            .headers(Self::build_headers(&token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Conversation API request failed: {}", e))?;

        if !resp.status().is_success() {
            warn!("HA conversation API returned HTTP {}", resp.status());
            return Ok(None);
        }

        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse conversation response: {}", e))?;

        // Extract speech text and response type
        // Expected structure:
        // response.response_type == "action_done"
        // response.speech.plain.speech
        let response_type = body
            .pointer("/response/response_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let speech = body
            .pointer("/response/speech/plain/speech")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        info!(
            "HA Assist result for '{}': response_type='{}', speech={:?}",
            text, response_type, speech
        );

        if response_type == "action_done" {
            // HA executed the command!
            return Ok(speech.or_else(|| Some("Готово".to_string())));
        } else if response_type == "query_answer" {
            return Ok(speech);
        } else if response_type == "error" {
            // Intent not recognized, e.g. "Sorry, I couldn't understand that"
            return Ok(None);
        }

        Ok(None)
    }

    /// Push media_player state to Home Assistant (registers/updates media_player.station_lite entity)
    pub async fn sync_media_player_state(
        &self,
        state: &str,
        volume_level: f32,
        title: &str,
        artist: &str,
        artwork: Option<&str>,
        duration_secs: u64,
        position_secs: u64,
    ) -> Result<(), String> {
        let (base_url, token, entity_id) = {
            let cfg = self.config.read().await;
            let ha = &cfg.homeassistant;
            if !ha.enabled || !ha.media_player_sync {
                return Ok(());
            }
            if ha.token.trim().is_empty() {
                return Ok(());
            }
            (
                ha.url.trim_end_matches('/').to_string(),
                ha.token.trim().to_string(),
                ha.media_player_entity.clone(),
            )
        };

        let url = format!("{}/api/states/{}", base_url, entity_id);

        let mut attributes = serde_json::json!({
            "friendly_name": "Яндекс Станция Лайт",
            "supported_features": 64063,
            "volume_level": volume_level.clamp(0.0, 1.0),
            "is_volume_muted": false,
            "media_title": title,
            "media_artist": artist,
            "media_duration": duration_secs,
            "media_position": position_secs,
            "device_class": "speaker",
            "source": "Station Daemon",
            "app_name": "Yandex Station Daemon",
        });

        if let Some(art) = artwork {
            if !art.is_empty() {
                attributes["entity_picture"] = serde_json::Value::String(art.to_string());
            }
        }

        let payload = serde_json::json!({
            "state": state,
            "attributes": attributes,
        });

        let resp = self
            .client
            .post(&url)
            .headers(Self::build_headers(&token))
            .json(&payload)
            .send()
            .await
            .map_err(|e| format!("Failed to push state to HA: {}", e))?;

        if !resp.status().is_success() {
            warn!("Failed to sync media_player state to HA: HTTP {}", resp.status());
        }

        Ok(())
    }
}
