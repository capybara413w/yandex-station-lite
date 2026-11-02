use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_web_dir")]
    pub web_static_dir: String,
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}
fn default_port() -> u16 {
    5000
}
fn default_web_dir() -> String {
    "/data/station_web".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepSeekConfig {
    #[serde(default = "default_llm_provider")]
    pub provider: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_model")]
    pub model: String,
    #[serde(default = "default_temperature")]
    pub temperature: f32,
    #[serde(default = "default_max_tokens")]
    pub max_tokens: u32,
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,
    #[serde(default)]
    pub openrouter_api_key: String,
    #[serde(default = "default_openrouter_model")]
    pub openrouter_model: String,
    #[serde(default)]
    pub groq_api_key: String,
    #[serde(default = "default_groq_model")]
    pub groq_model: String,
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default = "default_openai_model")]
    pub openai_model: String,
    #[serde(default)]
    pub mistral_api_key: String,
    #[serde(default = "default_mistral_model")]
    pub mistral_model: String,
    #[serde(default)]
    pub custom_base_url: String,
    #[serde(default)]
    pub custom_model: String,
    #[serde(default)]
    pub anthropic_api_key: String,
    #[serde(default = "default_anthropic_model")]
    pub anthropic_model: String,
    #[serde(default)]
    pub gemini_api_key: String,
    #[serde(default = "default_gemini_model")]
    pub gemini_model: String,
    #[serde(default)]
    pub perplexity_api_key: String,
    #[serde(default = "default_perplexity_model")]
    pub perplexity_model: String,
    #[serde(default)]
    pub together_api_key: String,
    #[serde(default = "default_together_model")]
    pub together_model: String,
    #[serde(default)]
    pub cloudflare_account_id: String,
    #[serde(default)]
    pub cloudflare_api_token: String,
    #[serde(default = "default_cloudflare_llm_model")]
    pub cloudflare_model: String,
    #[serde(default)]
    pub extra_payload: Option<serde_json::Value>,
}

fn default_cloudflare_llm_model() -> String {
    "@cf/meta/llama-3.1-8b-instruct".to_string()
}

impl Default for DeepSeekConfig {
    fn default() -> Self {
        Self {
            provider: default_llm_provider(),
            api_key: String::new(),
            model: default_model(),
            temperature: default_temperature(),
            max_tokens: default_max_tokens(),
            system_prompt: default_system_prompt(),
            openrouter_api_key: String::new(),
            openrouter_model: default_openrouter_model(),
            groq_api_key: String::new(),
            groq_model: default_groq_model(),
            openai_api_key: String::new(),
            openai_model: default_openai_model(),
            mistral_api_key: String::new(),
            mistral_model: default_mistral_model(),
            custom_base_url: String::new(),
            custom_model: String::new(),
            anthropic_api_key: String::new(),
            anthropic_model: default_anthropic_model(),
            gemini_api_key: String::new(),
            gemini_model: default_gemini_model(),
            perplexity_api_key: String::new(),
            perplexity_model: default_perplexity_model(),
            together_api_key: String::new(),
            together_model: default_together_model(),
            cloudflare_account_id: String::new(),
            cloudflare_api_token: String::new(),
            cloudflare_model: default_cloudflare_llm_model(),
            extra_payload: None,
        }
    }
}

fn default_anthropic_model() -> String {
    "claude-3-5-haiku-latest".to_string()
}
fn default_gemini_model() -> String {
    "gemini-2.0-flash".to_string()
}
fn default_perplexity_model() -> String {
    "sonar".to_string()
}
fn default_together_model() -> String {
    "meta-llama/Llama-3.3-70B-Instruct-Turbo".to_string()
}

fn default_llm_provider() -> String {
    "openrouter".to_string()
}
fn default_openrouter_model() -> String {
    "deepseek/deepseek-chat".to_string()
}
fn default_groq_model() -> String {
    "llama-3.3-70b-versatile".to_string()
}
fn default_openai_model() -> String {
    "gpt-4o-mini".to_string()
}
fn default_mistral_model() -> String {
    "mistral-small-latest".to_string()
}
fn default_model() -> String {
    "deepseek-chat".to_string()
}
fn default_temperature() -> f32 {
    0.7
}
fn default_max_tokens() -> u32 {
    250
}
fn default_system_prompt() -> String {
    "Ты — умный голосовой ассистент смарт-колонки с поддержкой управления музыкой и светодиодной подсветкой. Отвечай кратко в 1-2 предложения строго валидным JSON: {\"text\": \"...\", \"color\": [R, G, B], \"action\": \"play\" | \"stop\" | \"resume\" | \"none\", \"song\": \"...\"}.".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SttConfig {
    #[serde(default = "default_stt_provider")]
    pub provider: String,
    #[serde(default)]
    pub api_key: String,
    #[serde(default)]
    pub account_id: String,
    #[serde(default = "default_stt_model")]
    pub model: String,
    #[serde(default = "default_stt_language")]
    pub language: String,
    #[serde(default)]
    pub openrouter_api_key: String,
    #[serde(default = "default_openrouter_stt_model")]
    pub openrouter_model: String,
    #[serde(default)]
    pub openai_api_key: String,
    #[serde(default = "default_openai_stt_model")]
    pub openai_model: String,
    #[serde(default)]
    pub groq_api_key: String,
    #[serde(default = "default_groq_stt_model")]
    pub groq_model: String,
    #[serde(default)]
    pub deepgram_api_key: String,
    #[serde(default = "default_deepgram_stt_model")]
    pub deepgram_model: String,
    #[serde(default)]
    pub custom_url: String,
    #[serde(default)]
    pub extra_payload: Option<serde_json::Value>,
}

impl Default for SttConfig {
    fn default() -> Self {
        Self {
            provider: default_stt_provider(),
            api_key: String::new(),
            account_id: String::new(),
            model: default_stt_model(),
            language: default_stt_language(),
            openrouter_api_key: String::new(),
            openrouter_model: default_openrouter_stt_model(),
            openai_api_key: String::new(),
            openai_model: default_openai_stt_model(),
            groq_api_key: String::new(),
            groq_model: default_groq_stt_model(),
            deepgram_api_key: String::new(),
            deepgram_model: default_deepgram_stt_model(),
            custom_url: String::new(),
            extra_payload: None,
        }
    }
}

fn default_stt_provider() -> String {
    "cloudflare".to_string()
}
fn default_stt_model() -> String {
    "@cf/openai/whisper".to_string()
}
fn default_stt_language() -> String {
    "ru".to_string()
}
fn default_openrouter_stt_model() -> String {
    "openai/whisper".to_string()
}
fn default_openai_stt_model() -> String {
    "whisper-1".to_string()
}
fn default_groq_stt_model() -> String {
    "whisper-large-v3-turbo".to_string()
}
fn default_deepgram_stt_model() -> String {
    "nova-2".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_proxy_url")]
    pub socks5_url: String,
}

fn default_proxy_url() -> String {
    "socks5://127.0.0.1:1080".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    #[serde(default = "default_volume")]
    pub initial_volume: u8,
    #[serde(default = "default_mixer")]
    pub mixer_control_name: String,
    #[serde(default = "default_card")]
    pub alsa_card: String,
    #[serde(default = "default_rec_device")]
    pub record_device: String,
    #[serde(default = "default_sample_rate")]
    pub sample_rate: u32,
}

fn default_volume() -> u8 {
    6
}
fn default_mixer() -> String {
    "TAS5805M Digital".to_string()
}
fn default_card() -> String {
    "hw:0,0".to_string()
}
fn default_rec_device() -> String {
    "hw:0,1".to_string()
}
fn default_sample_rate() -> u32 {
    16000
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevenLabsConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_eleven_model")]
    pub model_id: String,
    #[serde(default = "default_eleven_voice")]
    pub voice_id: String,
}

fn default_eleven_model() -> String {
    "eleven_multilingual_v2".to_string()
}
fn default_eleven_voice() -> String {
    "21m00Tcm4TlvDq8ikWAM".to_string()
}

impl Default for ElevenLabsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model_id: default_eleven_model(),
            voice_id: default_eleven_voice(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudflareTtsConfig {
    #[serde(default)]
    pub account_id: String,
    #[serde(default)]
    pub api_token: String,
    #[serde(default = "default_cf_tts_model")]
    pub model: String,
    #[serde(default = "default_cf_tts_voice")]
    pub voice_id: String,
    #[serde(default = "default_cf_tts_lang")]
    pub lang: String,
    #[serde(default)]
    pub extra_payload: Option<serde_json::Value>,
}

fn default_cf_tts_model() -> String {
    "@cf/myshell/melotts-v1".to_string()
}
fn default_cf_tts_voice() -> String {
    "21m00Tcm4TlvDq8ikWAM".to_string()
}
fn default_cf_tts_lang() -> String {
    "ru".to_string()
}

impl Default for CloudflareTtsConfig {
    fn default() -> Self {
        Self {
            account_id: String::new(),
            api_token: String::new(),
            model: default_cf_tts_model(),
            voice_id: default_cf_tts_voice(),
            lang: default_cf_tts_lang(),
            extra_payload: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenAiTtsConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_openai_tts_model")]
    pub model: String,
    #[serde(default = "default_openai_tts_voice")]
    pub voice: String,
}

fn default_openai_tts_model() -> String {
    "tts-1".to_string()
}
fn default_openai_tts_voice() -> String {
    "alloy".to_string()
}

impl Default for OpenAiTtsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_openai_tts_model(),
            voice: default_openai_tts_voice(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepgramTtsConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_deepgram_tts_model")]
    pub model: String,
}

fn default_deepgram_tts_model() -> String {
    "aura-asteria-en".to_string()
}

impl Default for DeepgramTtsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_deepgram_tts_model(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CartesiaTtsConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_cartesia_model")]
    pub model_id: String,
    #[serde(default = "default_cartesia_voice")]
    pub voice_id: String,
}

fn default_cartesia_model() -> String {
    "sonic-multilingual".to_string()
}
fn default_cartesia_voice() -> String {
    "a0e998e3-182d-4f0d-9fb9-e82429f3303a".to_string()
}

impl Default for CartesiaTtsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model_id: default_cartesia_model(),
            voice_id: default_cartesia_voice(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomTtsConfig {
    #[serde(default)]
    pub url: String,
}

impl Default for CustomTtsConfig {
    fn default() -> Self {
        Self { url: String::new() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenRouterTtsConfig {
    #[serde(default)]
    pub api_key: String,
    #[serde(default = "default_openrouter_tts_model")]
    pub model: String,
    #[serde(default = "default_openrouter_tts_voice")]
    pub voice: String,
    #[serde(default)]
    pub extra_payload: Option<serde_json::Value>,
}

fn default_openrouter_tts_model() -> String {
    "openai/tts-1".to_string()
}
fn default_openrouter_tts_voice() -> String {
    "alloy".to_string()
}

impl Default for OpenRouterTtsConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            model: default_openrouter_tts_model(),
            voice: default_openrouter_tts_voice(),
            extra_payload: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtsConfig {
    #[serde(default = "default_tts_provider")]
    pub provider: String,
    #[serde(default)]
    pub openrouter: OpenRouterTtsConfig,
    #[serde(default)]
    pub elevenlabs: ElevenLabsConfig,
    #[serde(default)]
    pub cloudflare: CloudflareTtsConfig,
    #[serde(default)]
    pub openai: OpenAiTtsConfig,
    #[serde(default)]
    pub deepgram: DeepgramTtsConfig,
    #[serde(default)]
    pub cartesia: CartesiaTtsConfig,
    #[serde(default)]
    pub custom: CustomTtsConfig,
    #[serde(default)]
    pub extra_payload: Option<serde_json::Value>,
}

fn default_tts_provider() -> String {
    "google".to_string()
}

impl Default for TtsConfig {
    fn default() -> Self {
        Self {
            provider: default_tts_provider(),
            openrouter: OpenRouterTtsConfig::default(),
            elevenlabs: ElevenLabsConfig::default(),
            cloudflare: CloudflareTtsConfig::default(),
            openai: OpenAiTtsConfig::default(),
            deepgram: DeepgramTtsConfig::default(),
            cartesia: CartesiaTtsConfig::default(),
            custom: CustomTtsConfig::default(),
            extra_payload: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HardwareConfig {
    #[serde(default = "default_input_device")]
    pub input_event_device: String,
    #[serde(default = "default_led_rings")]
    pub led_rings_count: usize,
    #[serde(default = "default_sysfs_base")]
    pub led_sysfs_base: String,
}

fn default_input_device() -> String {
    "/dev/input/event1".to_string()
}
fn default_led_rings() -> usize {
    3
}
fn default_sysfs_base() -> String {
    "/sys/class/leds".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub server: ServerConfig,
    pub deepseek: DeepSeekConfig,
    pub stt: SttConfig,
    #[serde(default)]
    pub tts: TtsConfig,
    pub proxy: ProxyConfig,
    pub audio: AudioConfig,
    pub hardware: HardwareConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind_address: default_bind_address(),
                port: default_port(),
                web_static_dir: default_web_dir(),
            },
            deepseek: DeepSeekConfig::default(),
            stt: SttConfig::default(),
            tts: TtsConfig::default(),
            proxy: ProxyConfig {
                enabled: false,
                socks5_url: default_proxy_url(),
            },
            audio: AudioConfig {
                initial_volume: default_volume(),
                mixer_control_name: default_mixer(),
                alsa_card: default_card(),
                record_device: default_rec_device(),
                sample_rate: default_sample_rate(),
            },
            hardware: HardwareConfig {
                input_event_device: default_input_device(),
                led_rings_count: default_led_rings(),
                led_sysfs_base: default_sysfs_base(),
            },
        }
    }
}

impl AppConfig {
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Self {
        let path = path.as_ref();
        if path.exists() {
            match fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<AppConfig>(&content) {
                    Ok(cfg) => {
                        tracing::info!("Loaded config from {:?}", path);
                        return cfg;
                    }
                    Err(e) => {
                        tracing::error!("Error parsing config {:?}: {}, using defaults", path, e);
                    }
                },
                Err(e) => {
                    tracing::error!("Error reading config {:?}: {}, using defaults", path, e);
                }
            }
        } else {
            tracing::warn!("Config file {:?} not found, initializing with defaults", path);
        }
        Self::default()
    }
}

/// Merges user-supplied extra_payload JSON object into base JSON request object.
/// If extra_payload contains keys that exist in base, extra_payload overrides them.
pub fn merge_extra_payload(base: &mut serde_json::Value, extra: Option<&serde_json::Value>) {
    if let Some(extra_val) = extra {
        if let (Some(base_map), Some(extra_map)) = (base.as_object_mut(), extra_val.as_object()) {
            for (k, v) in extra_map {
                base_map.insert(k.clone(), v.clone());
            }
        }
    }
}

// ----------------------------------------------------------------------------
// Assistant Config REST API Models (GET/POST /api/config/assistant)
// ----------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantStageInfo {
    pub provider: String,
    pub model: String,
    pub has_key: bool,
    pub key_masked: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub account_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extra_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantConfigResponse {
    pub llm: AssistantStageInfo,
    pub stt: AssistantStageInfo,
    pub tts: AssistantStageInfo,
    pub system_prompt: String,
    pub temperature: f32,
    pub max_tokens: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateAssistantStagePayload {
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub account_id: Option<String>,
    #[serde(default)]
    pub voice: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub endpoint_url: Option<String>,
    #[serde(default)]
    pub extra_payload: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct UpdateAssistantConfigPayload {
    #[serde(default)]
    pub llm: Option<UpdateAssistantStagePayload>,
    #[serde(default)]
    pub stt: Option<UpdateAssistantStagePayload>,
    #[serde(default)]
    pub tts: Option<UpdateAssistantStagePayload>,
    #[serde(default)]
    pub system_prompt: Option<String>,
    #[serde(default)]
    pub temperature: Option<f32>,
    #[serde(default)]
    pub max_tokens: Option<u32>,
}

