use crate::config::{
    AppConfig, AssistantConfigResponse, AssistantStageInfo, StreamingConfig, UpdateAssistantConfigPayload,
};
use crate::hardware::{AudioHardware, LedController, NetworkHardware, StationButtonEvent};
use crate::services::{
    list_available_packs, play_sound, synthesize_sound, ChatService, ChatSession,
    ChatSessionSummary, DeepSeekService, HomeAssistantService, PlaybackState, PlayerService,
    SoundCloudService, StreamingService, StreamingStatus, SttService, TimerAlarmService, TtsService,
    VisualizerService, WakeWordService, WeatherService, YtMusicService,
};
use crate::services::homeassistant::{HassCallServiceRequest, HassProcessRequest};
use axum::{
    extract::{DefaultBodyLimit, Path as AxumPath, Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::net::UdpSocket;
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{mpsc, RwLock};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tracing::{error, info, warn};

const EMBEDDED_HTML: &str = include_str!("embedded_ui.html");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub id: u64,
    pub time: String,
    pub level: String,
    pub message: String,
}

pub struct LogBuffer {
    entries: RwLock<VecDeque<LogEntry>>,
    counter: AtomicU64,
}

impl LogBuffer {
    pub fn new() -> Self {
        let mut buffer = VecDeque::with_capacity(100);
        buffer.push_back(LogEntry {
            id: 1,
            time: get_current_time_str(),
            level: "INFO".to_string(),
            message: "Station Daemon Web Panel initialized on speaker".to_string(),
        });
        Self {
            entries: RwLock::new(buffer),
            counter: AtomicU64::new(2),
        }
    }

    pub async fn push(&self, level: &str, message: &str) {
        let mut guard = self.entries.write().await;
        if guard.len() >= 100 {
            guard.pop_front();
        }
        let id = self.counter.fetch_add(1, Ordering::Relaxed);
        guard.push_back(LogEntry {
            id,
            time: get_current_time_str(),
            level: level.to_string(),
            message: message.to_string(),
        });
    }

    pub async fn get_all(&self) -> Vec<LogEntry> {
        self.entries.read().await.iter().cloned().collect()
    }

    pub async fn clear(&self) {
        let mut guard = self.entries.write().await;
        guard.clear();
    }
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<RwLock<AppConfig>>,
    pub audio: Arc<AudioHardware>,
    pub led: Arc<LedController>,
    pub network: Arc<NetworkHardware>,
    pub deepseek: Arc<DeepSeekService>,
    pub player: Arc<PlayerService>,
    pub soundcloud: Arc<SoundCloudService>,
    pub ytmusic: Arc<YtMusicService>,
    pub stt: Arc<SttService>,
    pub tts: Arc<TtsService>,
    pub visualizer: Arc<VisualizerService>,
    pub btn_tx: mpsc::Sender<StationButtonEvent>,
    pub logs: Arc<LogBuffer>,
    pub start_time: Instant,
    pub chat: Arc<ChatService>,
    pub wakeword: Arc<WakeWordService>,
    pub homeassistant: Arc<HomeAssistantService>,
    pub weather: Arc<WeatherService>,
    pub timer_alarm: Arc<TimerAlarmService>,
    pub streaming: Arc<StreamingService>,
}

pub fn create_router(state: AppState) -> Router {
    let static_dir = {
        // Safe read or default
        "/data/station_web".to_string()
    };

    Router::new()
        // Web UI Root
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        // Status & Info
        .route("/api/status", get(get_status_handler))
        .route("/api/player/status", get(get_status_handler))
        // Wake-Word & Voice Activation
        .route(
            "/api/wakeword",
            get(get_wakeword_handler).post(post_wakeword_handler),
        )
        // Volume
        .route("/api/volume", post(set_volume_handler))
        .route("/api/player/volume", post(set_volume_handler))
        // Player actions
        .route("/api/player/action", post(player_action_handler))
        .route("/api/player/play_song", post(play_song_handler))
        .route("/api/player/seek", post(seek_handler))
        .route("/api/player/autoplay", get(get_autoplay_handler).post(set_autoplay_handler))
        .route("/api/player/next", post(player_next_handler))
        .route("/api/player/queue", get(player_queue_handler))
        // Music integration (YouTube Music, SoundCloud, Radio)
        .route("/api/music/search", get(music_search_handler))
        .route("/api/soundcloud/search", get(music_search_handler))
        .route("/api/soundcloud/play", post(soundcloud_play_handler))
        // YouTube Music Personalized & Streaming API
        .route("/api/ytmusic/status", get(ytmusic_status_handler))
        .route("/api/ytmusic/cookie", post(ytmusic_cookie_handler))
        .route("/api/ytmusic/home", get(ytmusic_home_handler))
        .route("/api/ytmusic/liked", get(ytmusic_liked_handler))
        .route("/api/ytmusic/playlists", get(ytmusic_playlists_handler))
        .route("/api/ytmusic/playlist/:id", get(ytmusic_playlist_tracks_handler))
        .route("/api/ytmusic/autoplay/:id", get(ytmusic_autoplay_handler))
        .route("/api/ytmusic/search", get(ytmusic_search_handler))
        .route("/api/ytmusic/play", post(ytmusic_play_handler))
        // Visualizer (3-Band Reactive Lightshow: Low, Mid, High)
        .route("/api/visualizer", get(get_visualizer_handler))
        .route("/api/visualizer/toggle", post(toggle_visualizer_handler))
        .route(
            "/api/visualizer/config",
            get(get_visualizer_config_handler).post(post_visualizer_config_handler),
        )
        // LED Ring controls
        .route("/api/led", post(set_led_handler))
        // TTS Speech
        .route("/api/tts/speak", post(post_speak_handler))
        // STT & Microphone
        .route("/api/mic/listen", post(trigger_mic_handler))
        .route("/api/stt/listen", post(trigger_mic_handler))
        // Virtual Hardware Buttons
        .route("/api/button/press", post(virtual_button_handler))
        // Ask / LLM query endpoints
        .route("/api/ask", post(post_ask_handler))
        // Chat History & Sessions
        .route(
            "/api/chats",
            get(get_chats_handler).delete(delete_all_chats_handler),
        )
        .route("/api/chats/new", post(post_new_chat_handler))
        .route(
            "/api/chats/:id",
            get(get_chat_by_id_handler).delete(delete_chat_by_id_handler),
        )
        // Config management
        .route("/api/config", get(get_config_handler).post(post_config_handler))
        .route(
            "/api/config/assistant",
            get(get_assistant_config_handler).post(post_assistant_config_handler),
        )
        // DNS Management
        .route("/api/dns/status", get(get_dns_status_handler))
        .route("/api/dns/test", post(test_dns_handler))
        // Logs
        .route("/api/logs", get(get_logs_handler))
        .route("/api/logs/system", get(get_logs_handler))
        .route("/api/logs/system/clear", post(clear_logs_handler))
        // Wi-Fi Management
        .route("/api/wifi/status", get(get_wifi_status_handler))
        .route("/api/wifi/scan", get(scan_wifi_handler).post(scan_wifi_handler))
        .route("/api/wifi/connect", post(connect_wifi_handler))
        .route("/api/wifi/restart", post(restart_wifi_handler))
        .route("/api/wifi/saved", get(get_saved_wifi_handler))
        .route("/api/wifi/select", post(select_saved_wifi_handler))
        .route("/api/wifi/remove", post(remove_saved_wifi_handler))
        // Wi-Fi 88x2cs Power Saving (/sys/module/88x2cs/parameters/)
        .route(
            "/api/wifi/powersave",
            get(get_wifi_powersave_handler).post(set_wifi_powersave_handler),
        )
        .route(
            "/api/wifi/powersave/optimize",
            post(optimize_wifi_powersave_handler),
        )
        // Bluetooth Management
        .route("/api/bluetooth/status", get(get_bluetooth_status_handler))
        .route("/api/bluetooth/toggle", post(toggle_bluetooth_handler))
        .route(
            "/api/bluetooth/discoverable",
            post(set_bluetooth_discoverable_handler),
        )
        .route(
            "/api/bluetooth/scan",
            get(scan_bluetooth_handler).post(scan_bluetooth_handler),
        )
        .route("/api/bluetooth/connect", post(connect_bluetooth_handler))
        .route(
            "/api/bluetooth/disconnect",
            post(disconnect_bluetooth_handler),
        )
        .route("/api/bluetooth/pair", post(pair_bluetooth_handler))
        .route("/api/bluetooth/remove", post(remove_bluetooth_handler))
        // Terminal command execution
        .route("/api/terminal/run", post(terminal_run_handler))
        .route("/api/terminal/exec", post(terminal_run_handler))
        .route("/api/command", post(terminal_run_handler))
        // Dual Boot & System Switch
        .route("/api/dualboot/status", get(get_dualboot_status_handler))
        .route("/api/dualboot/choose", post(post_dualboot_choose_handler))
        .route("/api/dualboot/switch_quasar", post(post_switch_quasar_handler))
        .route("/api/dualboot/mode", post(post_dualboot_mode_handler))
        // Home Assistant Integration
        .route("/api/hass/status", get(get_hass_status_handler))
        .route("/api/hass/entities", get(get_hass_entities_handler))
        .route("/api/hass/call", post(post_hass_call_handler))
        .route("/api/hass/process", post(post_hass_process_handler))
        // Weather Service (Open-Meteo)
        .route("/api/weather", get(get_weather_handler))
        .route("/api/weather/config", post(post_weather_config_handler))
        // Timers & Alarms Management
        .route("/api/timers", get(get_timers_handler).post(post_timer_handler))
        .route("/api/timers/cancel_all", post(cancel_all_timers_handler))
        .route("/api/timers/:id", delete(delete_timer_handler))
        .route("/api/alarms", get(get_alarms_handler).post(post_alarm_handler))
        .route("/api/alarms/:id", delete(delete_alarm_handler).put(put_alarm_handler))
        .route("/api/alarm/stop", post(post_alarm_stop_handler))
        // Streaming Sinks (AirPlay, Spotify Connect, Snapcast)
        .route("/api/streaming/status", get(get_streaming_status_handler))
        .route("/api/streaming/config", post(post_streaming_config_handler))
        .route("/api/streaming/restart", post(restart_streaming_handler))
        // Bluetooth A2DP Sink
        .route("/api/bluetooth/sink/enable", post(enable_bt_sink_handler))
        .route("/api/bluetooth/sink/disable", post(disable_bt_sink_handler))
        // Sound Packs & Chimes
        .route("/api/sounds/packs", get(get_sound_packs_handler))
        .route("/api/sounds/pack", post(set_active_sound_pack_handler))
        .route("/api/sounds/preview", get(preview_sound_handler).post(preview_sound_handler))
        // Home Assistant Media Player direct API
        .route("/api/media_player/play_media", post(media_player_play_handler))
        .route("/api/media_player/sync", post(sync_ha_media_player_handler))
        // Static assets fallback if /data/station_web exists
        .nest_service("/static", ServeDir::new(static_dir))
        .nest_service("/assets", ServeDir::new("/data/station_web/assets"))
        .nest_service("/fonts", ServeDir::new("/data/station_web/fonts"))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(
            CorsLayer::new()
                .allow_origin(tower_http::cors::Any)
                .allow_methods([
                    axum::http::Method::GET,
                    axum::http::Method::POST,
                    axum::http::Method::DELETE,
                    axum::http::Method::PUT,
                    axum::http::Method::OPTIONS,
                ])
                .allow_headers([
                    axum::http::header::CONTENT_TYPE,
                    axum::http::header::AUTHORIZATION,
                    axum::http::header::HeaderName::from_static("x-station-request"),
                ]),
        )
        .with_state(state)
}

/// Serves the web control panel:
/// Checks if /data/station_web/index.html is installed on disk; if not, falls back to EMBEDDED_HTML.
async fn index_handler(State(_state): State<AppState>) -> impl IntoResponse {
    if let Ok(content) = std::fs::read_to_string("/data/station_web/index.html") {
        Html(content)
    } else {
        Html(EMBEDDED_HTML.to_string())
    }
}

async fn health_handler() -> impl IntoResponse {
    let local_ip = get_local_ip();
    let (ram_used_mb, ram_total_mb) = get_mem_stats();
    let cpu_temp_c = get_cpu_temp();
    let cpu_load_percent = get_cpu_load_percent();
    Json(serde_json::json!({
        "status": "ok",
        "service": "Yandex Station Lite Autonomous Daemon",
        "arch": "aarch64",
        "target": "Amlogic AXG A113X (238MB RAM)",
        "station_ip": local_ip,
        "ram_used_mb": ram_used_mb,
        "ram_total_mb": ram_total_mb,
        "cpu_temp_c": cpu_temp_c,
        "cpu_load_percent": cpu_load_percent,
    }))
}

#[derive(Serialize)]
struct SystemStatusResponse {
    volume: u8,
    volume_percent: u8,
    state: String,
    track: String,
    artist: String,
    artwork_url: Option<String>,
    position_secs: u64,
    duration_secs: u64,
    is_recording: bool,
    is_speaking: bool,
    visualizer_enabled: bool,
    visualizer_levels: [u8; 3],
    visualizer_config: crate::services::visualizer::VisualizerConfig,
    led_color: [u8; 3],
    station_ip: String,
    uptime_secs: u64,
    ram_used_mb: u64,
    ram_total_mb: u64,
    cpu_temp_c: f32,
    cpu_load_percent: u8,
    autoplay: bool,
    wakeword_enabled: bool,
    wakeword_phrase: String,
}

async fn get_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let st = match state.player.get_state().await {
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
        PlaybackState::Stopped => "stopped",
    };
    let volume = state.audio.get_volume();
    let volume_percent = volume.saturating_mul(10);
    let track = state.player.get_current_track().await;
    let artist = state.player.get_current_artist().await;
    let artwork_url = state.player.get_current_artwork().await;
    let (position_secs, duration_secs) = state.player.get_position_and_duration().await;
    let is_recording = Path::new("/tmp/listen.lock").exists();
    let is_speaking = state.tts.is_speaking();
    let visualizer_enabled = state.visualizer.is_enabled();
    let visualizer_levels = state.visualizer.get_levels().await;
    let visualizer_config = state.visualizer.get_config().await;
    let led_color = state.led.get_rgb();
    let station_ip = get_local_ip();
    let (ram_used_mb, ram_total_mb) = get_mem_stats();
    let cpu_temp_c = get_cpu_temp();
    let cpu_load_percent = get_cpu_load_percent();
    let uptime_secs = state.start_time.elapsed().as_secs();
    let autoplay = state.player.is_autoplay();
    let wakeword_enabled = state.wakeword.is_enabled();
    let wakeword_phrase = state.wakeword.get_phrase().await;

    Json(SystemStatusResponse {
        volume,
        volume_percent,
        state: st.to_string(),
        track,
        artist,
        artwork_url,
        position_secs,
        duration_secs,
        is_recording,
        is_speaking,
        visualizer_enabled,
        visualizer_levels,
        visualizer_config,
        led_color,
        station_ip,
        uptime_secs,
        ram_used_mb,
        ram_total_mb,
        cpu_temp_c,
        cpu_load_percent,
        autoplay,
        wakeword_enabled,
        wakeword_phrase,
    })
}

#[derive(Deserialize)]
struct SetVolumeRequest {
    volume: u8,
}

async fn set_volume_handler(
    State(state): State<AppState>,
    Json(payload): Json<SetVolumeRequest>,
) -> impl IntoResponse {
    // Support either 1-10 step or 0-100 percentage from sliders
    let step = if payload.volume > 10 {
        ((payload.volume as f32 / 10.0).round() as u8).clamp(1, 10)
    } else {
        payload.volume.clamp(1, 10)
    };
    let new_vol = state.audio.set_volume(step);
    state
        .logs
        .push("INFO", &format!("Web: громкость изменена на шаг {}/10", new_vol))
        .await;

    Json(serde_json::json!({
        "success": true,
        "volume": new_vol,
        "volume_percent": new_vol * 10
    }))
}

#[derive(Deserialize)]
struct PlayerActionRequest {
    action: String,
    #[serde(default)]
    song: Option<String>,
    #[serde(default)]
    position: Option<u64>,
    #[serde(default)]
    seconds: Option<u64>,
}

async fn player_action_handler(
    State(state): State<AppState>,
    Json(payload): Json<PlayerActionRequest>,
) -> impl IntoResponse {
    // If speaking, stop speech first
    state.tts.stop(&state.led);

    let action_str = payload.action.to_lowercase();
    match action_str.as_str() {
        "pause" => {
            state.player.pause(&state.led).await;
            state.logs.push("INFO", "Web: плеер приостановлен (пауза)").await;
        }
        "resume" => {
            state.player.resume(&state.led).await;
            state.logs.push("INFO", "Web: плеер возобновлён").await;
        }
        "stop" => {
            state.player.stop(&state.led).await;
            state.logs.push("INFO", "Web: плеер остановлен").await;
        }
        "seek" => {
            let pos = payload.position.or(payload.seconds).unwrap_or(0);
            state.player.seek(pos, &state.led).await;
            state
                .logs
                .push("INFO", &format!("Web: перемотка на {}с", pos))
                .await;
        }
        "restart" | "play" => {
            let track = payload.song.unwrap_or_else(|| "Lofi chill beats".to_string());
            state.player.play_soundcloud_or_query(&state.soundcloud, &track, &state.led).await;
            state.logs.push("INFO", &format!("Web: SoundCloud/плеер запрос '{}'", track)).await;
        }
        "next" | "skip" => {
            state.player.play_next(&state.ytmusic, &state.led).await;
            state.logs.push("INFO", "Web: переход к следующему треку (автоподбор)").await;
        }
        _ => {}
    }

    let st = match state.player.get_state().await {
        PlaybackState::Playing => "playing",
        PlaybackState::Paused => "paused",
        PlaybackState::Stopped => "stopped",
    };
    let (pos, dur) = state.player.get_position_and_duration().await;

    Json(serde_json::json!({
        "success": true,
        "state": st,
        "position_seconds": pos,
        "duration_seconds": dur
    }))
}

#[derive(Deserialize, Default)]
struct AutoplayToggleRequest {
    #[serde(default)]
    autoplay: Option<bool>,
}

async fn get_autoplay_handler(State(state): State<AppState>) -> impl IntoResponse {
    Json(serde_json::json!({
        "success": true,
        "autoplay": state.player.is_autoplay()
    }))
}

async fn set_autoplay_handler(
    State(state): State<AppState>,
    Json(payload): Json<AutoplayToggleRequest>,
) -> impl IntoResponse {
    let new_val = match payload.autoplay {
        Some(v) => v,
        None => !state.player.is_autoplay(),
    };
    state.player.set_autoplay(new_val);

    // Save to persistent config
    {
        let mut cfg = state.config.write().await;
        cfg.audio.autoplay = new_val;
        let save_path = if std::path::Path::new("/data").exists() {
            "/data/config.json"
        } else {
            "config.json"
        };
        if let Ok(json_str) = serde_json::to_string_pretty(&*cfg) {
            let _ = tokio::fs::write(save_path, json_str).await;
        }
    }

    state
        .logs
        .push(
            "INFO",
            &format!(
                "Web: автоподбор треков {}",
                if new_val { "включен" } else { "отключен" }
            ),
        )
        .await;

    Json(serde_json::json!({
        "success": true,
        "autoplay": new_val
    }))
}

#[derive(Deserialize)]
struct UpdateWakewordRequest {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    phrase: Option<String>,
}

async fn get_wakeword_handler(State(state): State<AppState>) -> impl IntoResponse {
    let enabled = state.wakeword.is_enabled();
    let phrase = state.wakeword.get_phrase().await;
    Json(serde_json::json!({
        "success": true,
        "enabled": enabled,
        "phrase": phrase
    }))
}

async fn post_wakeword_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateWakewordRequest>,
) -> impl IntoResponse {
    if let Some(en) = payload.enabled {
        state.wakeword.set_enabled(en);
    }
    if let Some(phrase) = payload.phrase {
        if !phrase.trim().is_empty() {
            state.wakeword.set_phrase(phrase.trim().to_lowercase()).await;
        }
    }

    let enabled = state.wakeword.is_enabled();
    let phrase = state.wakeword.get_phrase().await;

    // Persist to config.json
    {
        let mut cfg = state.config.write().await;
        cfg.audio.wakeword_enabled = enabled;
        cfg.audio.wakeword_phrase = phrase.clone();
        let save_path = if std::path::Path::new("/data").exists() {
            "/data/config.json"
        } else {
            "config.json"
        };
        if let Ok(json_str) = serde_json::to_string_pretty(&*cfg) {
            let _ = tokio::fs::write(save_path, json_str).await;
        }
    }

    state
        .logs
        .push(
            "INFO",
            &format!(
                "Web: Wake-Word «{}» {}",
                phrase,
                if enabled { "включен" } else { "отключен" }
            ),
        )
        .await;

    Json(serde_json::json!({
        "success": true,
        "enabled": enabled,
        "phrase": phrase
    }))
}

async fn player_next_handler(State(state): State<AppState>) -> impl IntoResponse {
    let played = state.player.play_next(&state.ytmusic, &state.led).await;
    Json(serde_json::json!({
        "success": played
    }))
}

async fn player_queue_handler(State(state): State<AppState>) -> impl IntoResponse {
    let queue = state.player.get_queue().await;
    Json(serde_json::json!({
        "success": true,
        "queue": queue
    }))
}

#[derive(Deserialize)]
struct SeekRequest {
    #[serde(default, alias = "position", alias = "seconds")]
    position_seconds: u64,
}

async fn seek_handler(
    State(state): State<AppState>,
    Json(payload): Json<SeekRequest>,
) -> impl IntoResponse {
    state.player.seek(payload.position_seconds, &state.led).await;
    state
        .logs
        .push(
            "INFO",
            &format!("Web: перемотка трека на {}с", payload.position_seconds),
        )
        .await;

    let (pos, dur) = state.player.get_position_and_duration().await;
    Json(serde_json::json!({
        "success": true,
        "position_seconds": pos,
        "duration_seconds": dur
    }))
}

async fn get_visualizer_handler(State(state): State<AppState>) -> impl IntoResponse {
    let enabled = state.visualizer.is_enabled();
    let levels = state.visualizer.get_levels().await;
    let config = state.visualizer.get_config().await;
    Json(serde_json::json!({
        "enabled": enabled,
        "levels": levels,
        "bass": levels[0],
        "mid": levels[1],
        "high": levels[2],
        "config": config
    }))
}

async fn get_visualizer_config_handler(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.visualizer.get_config().await;
    Json(config)
}

#[derive(Deserialize)]
struct UpdateVisualizerConfigRequest {
    #[serde(default)]
    enabled: Option<bool>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    color: Option<[u8; 3]>,
    #[serde(default)]
    bass_cutoff_hz: Option<f32>,
    #[serde(default)]
    high_cutoff_hz: Option<f32>,
    #[serde(default)]
    speed: Option<String>,
    #[serde(default)]
    sensitivity: Option<f32>,
}

async fn post_visualizer_config_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateVisualizerConfigRequest>,
) -> impl IntoResponse {
    let mut current = state.visualizer.get_config().await;
    if let Some(en) = payload.enabled {
        current.enabled = en;
    }
    if let Some(mode) = payload.mode {
        current.mode = mode;
    }
    if let Some(col) = payload.color {
        current.color = col;
    }
    if let Some(b_hz) = payload.bass_cutoff_hz {
        current.bass_cutoff_hz = b_hz;
    }
    if let Some(h_hz) = payload.high_cutoff_hz {
        current.high_cutoff_hz = h_hz;
    }
    if let Some(spd) = payload.speed {
        current.speed = spd;
    }
    if let Some(sens) = payload.sensitivity {
        current.sensitivity = sens;
    }
    state.visualizer.update_config(current.clone()).await;

    state
        .logs
        .push(
            "INFO",
            &format!(
                "Web: конфиг светомузыки (цвет: {:?}, режим: {}, НЧ: {}Гц, ВЧ: {}Гц)",
                current.color, current.mode, current.bass_cutoff_hz, current.high_cutoff_hz
            ),
        )
        .await;

    Json(serde_json::json!({
        "success": true,
        "config": current
    }))
}

#[derive(Deserialize)]
struct ToggleVisualizerRequest {
    #[serde(default)]
    enabled: Option<bool>,
}

async fn toggle_visualizer_handler(
    State(state): State<AppState>,
    Json(payload): Json<ToggleVisualizerRequest>,
) -> impl IntoResponse {
    let new_val = match payload.enabled {
        Some(val) => {
            state.visualizer.set_enabled(val);
            val
        }
        None => {
            let cur = state.visualizer.is_enabled();
            state.visualizer.set_enabled(!cur);
            !cur
        }
    };

    state
        .logs
        .push(
            "INFO",
            &format!(
                "Web: 3-полосная светомузыка (НЧ/СЧ/ВЧ) {}",
                if new_val { "включена" } else { "отключена" }
            ),
        )
        .await;

    Json(serde_json::json!({
        "success": true,
        "enabled": new_val
    }))
}

#[derive(Deserialize)]
struct PlaySongRequest {
    song: String,
}

async fn play_song_handler(
    State(state): State<AppState>,
    Json(payload): Json<PlaySongRequest>,
) -> impl IntoResponse {
    let query = payload.song.trim();
    if query.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Empty song query"})),
        );
    }

    state.player.play_smart_or_query(&state.ytmusic, &state.soundcloud, query, &state.led).await;
    state
        .logs
        .push("INFO", &format!("Web: запуск трека '{}'", query))
        .await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "track": query,
            "state": "playing"
        })),
    )
}

#[derive(Deserialize)]
struct MusicSearchQuery {
    q: String,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

async fn music_search_handler(
    State(state): State<AppState>,
    Query(params): Query<MusicSearchQuery>,
) -> impl IntoResponse {
    let q = params.q.trim();
    if q.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Empty search query" })),
        )
            .into_response();
    }

    let source = params.source.unwrap_or_else(|| "all".to_string());
    let limit = params.limit.unwrap_or(12);

    match source.to_lowercase().as_str() {
        "ytmusic" | "youtube" | "yt" => {
            match state.ytmusic.search(q, limit).await {
                Ok(tracks) => Json(serde_json::json!({
                    "success": true,
                    "query": q,
                    "source": "ytmusic",
                    "results": tracks,
                }))
                .into_response(),
                Err(e) => {
                    warn!("YT Music search for '{}' failed: {}", q, e);
                    Json(serde_json::json!({
                        "success": false,
                        "error": e,
                        "source": "ytmusic",
                        "results": [],
                    }))
                    .into_response()
                }
            }
        }
        "soundcloud" | "sc" => {
            match state.soundcloud.search_tracks(q, limit).await {
                Ok(tracks) => Json(serde_json::json!({
                    "success": true,
                    "query": q,
                    "source": "soundcloud",
                    "results": tracks,
                }))
                .into_response(),
                Err(e) => {
                    warn!("SoundCloud search for '{}' failed: {}", q, e);
                    Json(serde_json::json!({
                        "success": false,
                        "error": e,
                        "source": "soundcloud",
                        "results": [],
                    }))
                    .into_response()
                }
            }
        }
        "radio" => {
            match state.soundcloud.search_fallback_streams(q, limit).await {
                Ok(tracks) => Json(serde_json::json!({
                    "success": true,
                    "query": q,
                    "source": "radio",
                    "results": tracks,
                }))
                .into_response(),
                Err(e) => {
                    warn!("Radio search for '{}' failed: {}", q, e);
                    Json(serde_json::json!({
                        "success": false,
                        "error": e,
                        "source": "radio",
                        "results": [],
                    }))
                    .into_response()
                }
            }
        }
        _ => {
            let yt_limit = (limit + 1) / 2;
            let sc_limit = limit / 2;
            let yt_tracks = state.ytmusic.search(q, yt_limit).await.unwrap_or_default();
            let sc_tracks = state.soundcloud.search_tracks(q, sc_limit).await.unwrap_or_default();

            let mut combined = Vec::new();
            let max_len = std::cmp::max(yt_tracks.len(), sc_tracks.len());
            for i in 0..max_len {
                if let Some(yt) = yt_tracks.get(i) {
                    combined.push(serde_json::to_value(yt).unwrap_or_default());
                }
                if let Some(sc) = sc_tracks.get(i) {
                    combined.push(serde_json::to_value(sc).unwrap_or_default());
                }
                if combined.len() >= limit {
                    break;
                }
            }

            Json(serde_json::json!({
                "success": true,
                "query": q,
                "source": "all",
                "results": combined,
            }))
            .into_response()
        }
    }
}

#[derive(Deserialize)]
struct SoundCloudPlayRequest {
    #[serde(default)]
    song: Option<String>,
    #[serde(default)]
    track_id: Option<u64>,
}

async fn soundcloud_play_handler(
    State(state): State<AppState>,
    Json(payload): Json<SoundCloudPlayRequest>,
) -> impl IntoResponse {
    state.tts.stop(&state.led);

    if let Some(tid) = payload.track_id {
        match state.soundcloud.resolve_track_by_id(tid).await {
            Ok(stream) => {
                let dur_secs = stream.duration_ms / 1000;
                state
                    .player
                    .play_direct_stream(
                        &stream.stream_url,
                        &stream.track_title,
                        &stream.artist,
                        stream.artwork_url,
                        dur_secs,
                        stream.is_hls,
                        0,
                        &state.led,
                    )
                    .await;
                state
                    .logs
                    .push(
                        "INFO",
                        &format!("SoundCloud: запущен трек #{} '{}'", tid, stream.track_title),
                    )
                    .await;
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "success": true,
                        "track": stream.track_title,
                        "artist": stream.artist,
                        "state": "playing"
                    })),
                )
                    .into_response();
            }
            Err(e) => {
                warn!("Failed to resolve track #{}: {}", tid, e);
            }
        }
    }

    let query = payload.song.unwrap_or_else(|| "Lofi chill beats".to_string());
    if query.contains("soundcloud.com/") {
        match state.soundcloud.resolve_permalink(&query).await {
            Ok(stream) => {
                let dur_secs = stream.duration_ms / 1000;
                state
                    .player
                    .play_direct_stream(
                        &stream.stream_url,
                        &stream.track_title,
                        &stream.artist,
                        stream.artwork_url,
                        dur_secs,
                        stream.is_hls,
                        0,
                        &state.led,
                    )
                    .await;
                state
                    .logs
                    .push(
                        "INFO",
                        &format!("SoundCloud: запущен трек по ссылке '{}'", stream.track_title),
                    )
                    .await;
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "success": true,
                        "track": stream.track_title,
                        "artist": stream.artist,
                        "state": "playing"
                    })),
                )
                    .into_response();
            }
            Err(e) => {
                warn!("Failed to resolve SoundCloud permalink '{}': {}", query, e);
            }
        }
    }

    if (query.starts_with("http://") || query.starts_with("https://"))
        && !query.contains("youtube.com/")
        && !query.contains("youtu.be/")
    {
        state
            .player
            .play_direct_stream(
                &query,
                "Аудио поток",
                "Прямая ссылка / Радио",
                None,
                0,
                query.contains(".m3u8"),
                0,
                &state.led,
            )
            .await;
        state
            .logs
            .push("INFO", &format!("Stream: запуск прямого потока '{}'", query))
            .await;
        return (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "track": "Аудио поток",
                "artist": "Прямая ссылка",
                "state": "playing"
            })),
        )
            .into_response();
    }

    state
        .player
        .play_soundcloud_or_query(&state.soundcloud, &query, &state.led)
        .await;
    state
        .logs
        .push("INFO", &format!("SoundCloud: поиск и запуск '{}'", query))
        .await;

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "track": query,
            "state": "playing"
        })),
    )
        .into_response()
}

// -----------------------------------------------------------------------------
// YouTube Music Personalized & Streaming Handlers
// -----------------------------------------------------------------------------

#[derive(Deserialize)]
pub struct YtPlayRequest {
    pub video_id: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub duration_seconds: Option<u64>,
}

#[derive(Deserialize)]
pub struct YtCookiePayload {
    pub cookie: String,
}

#[derive(Deserialize)]
pub struct LimitQuery {
    pub limit: Option<usize>,
}

async fn ytmusic_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.ytmusic.get_status().await;
    Json(serde_json::json!({
        "success": true,
        "status": status,
    }))
}

async fn ytmusic_cookie_handler(
    State(state): State<AppState>,
    Json(payload): Json<YtCookiePayload>,
) -> impl IntoResponse {
    match state.ytmusic.update_cookie(payload.cookie).await {
        Ok(_) => {
            let status = state.ytmusic.get_status().await;
            (StatusCode::OK, Json(serde_json::json!({
                "success": true,
                "status": status,
            })))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false,
            "error": e,
        }))),
    }
}

async fn ytmusic_home_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.ytmusic.get_home().await {
        Ok(sections) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "sections": sections,
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false,
            "error": e,
            "sections": [],
        }))).into_response(),
    }
}

async fn ytmusic_liked_handler(
    State(state): State<AppState>,
    Query(params): Query<LimitQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50);
    match state.ytmusic.get_liked_songs(limit).await {
        Ok(tracks) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "tracks": tracks,
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false,
            "error": e,
            "tracks": [],
        }))).into_response(),
    }
}

async fn ytmusic_autoplay_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    match state.ytmusic.get_autoplay_tracks(&id).await {
        Ok(tracks) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "video_id": id,
            "tracks": tracks,
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false,
            "error": e,
            "tracks": [],
        }))).into_response(),
    }
}

async fn ytmusic_playlists_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.ytmusic.get_library_playlists().await {
        Ok(playlists) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "playlists": playlists,
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false,
            "error": e,
            "playlists": [],
        }))).into_response(),
    }
}

async fn ytmusic_playlist_tracks_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    match state.ytmusic.get_playlist_tracks(&id).await {
        Ok(tracks) => (StatusCode::OK, Json(serde_json::json!({
            "success": true,
            "playlist_id": id,
            "tracks": tracks,
        }))).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
            "success": false,
            "error": e,
            "tracks": [],
        }))).into_response(),
    }
}

fn extract_youtube_video_id(input: &str) -> &str {
    let s = input.trim();
    if let Some(idx) = s.find("v=") {
        let rest = &s[idx + 2..];
        let end = rest.find(['&', '?', '#', '/', ' ']).unwrap_or(rest.len());
        &rest[..end]
    } else if let Some(idx) = s.find("youtu.be/") {
        let rest = &s[idx + 9..];
        let end = rest.find(['&', '?', '#', '/', ' ']).unwrap_or(rest.len());
        &rest[..end]
    } else if let Some(idx) = s.find("/shorts/") {
        let rest = &s[idx + 8..];
        let end = rest.find(['&', '?', '#', '/', ' ']).unwrap_or(rest.len());
        &rest[..end]
    } else {
        s
    }
}

async fn ytmusic_play_handler(
    State(state): State<AppState>,
    Json(payload): Json<YtPlayRequest>,
) -> impl IntoResponse {
    state.tts.stop(&state.led);

    let raw_vid = payload.video_id.trim();
    let vid = extract_youtube_video_id(raw_vid);
    if vid.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Empty video_id",
        }))).into_response();
    }

    match state.ytmusic.resolve_stream_info(vid).await {
        Ok(info) => {
            let title = if !payload.title.is_empty() && payload.title != "YouTube Music Track" {
                payload.title
            } else {
                info.title.unwrap_or_else(|| "YouTube Music Track".to_string())
            };

            let artist = if !payload.artist.is_empty() && payload.artist != "YouTube Music" {
                payload.artist
            } else {
                info.artist.unwrap_or_else(|| "YouTube Music".to_string())
            };

            let dur_secs = payload.duration_seconds
                .or(info.duration_seconds)
                .unwrap_or(210);

            let artwork = payload.artwork_url.or(info.artwork_url);

            state.player.play_direct_stream(
                &info.stream_url,
                &title,
                &artist,
                artwork.clone(),
                dur_secs,
                false,
                0,
                &state.led,
            ).await;

            *state.player.last_yt_id.write().await = Some(vid.to_string());
            if state.player.is_autoplay() {
                let yt_cl = state.ytmusic.clone();
                let q_cl = state.player.queue.clone();
                let vid_str = vid.to_string();
                tokio::spawn(async move {
                    if let Ok(recs) = yt_cl.get_autoplay_tracks(&vid_str).await {
                        let filtered: Vec<_> = recs.into_iter().filter(|x| x.id != vid_str).collect();
                        info!("⚡ Autoplay: preloaded {} next tracks for '{}'", filtered.len(), vid_str);
                        *q_cl.write().await = filtered;
                    }
                });
            }

            state.logs.push("INFO", &format!("YouTube Music: запуск '{}' - '{}' ({})", title, artist, vid)).await;

            (StatusCode::OK, Json(serde_json::json!({
                "success": true,
                "title": title,
                "artist": artist,
                "duration_seconds": dur_secs,
                "artwork_url": artwork,
                "stream_url": info.stream_url,
            }))).into_response()
        }
        Err(e) => {
            let title = if payload.title.is_empty() { "YouTube Music Track".to_string() } else { payload.title };
            let artist = if payload.artist.is_empty() { "YouTube Music".to_string() } else { payload.artist };

            warn!("YouTube Music direct resolve error for {}: {}. Searching alternative YouTube Music match...", vid, e);
            // Search YouTube Music for alternative video ID of the same track
            let mut alt_stream = None;
            let query = format!("{} {}", title, artist);
            if let Ok(results) = state.ytmusic.search(&query, 3).await {
                for alt_track in results {
                    if alt_track.id != vid {
                        if let Ok(alt_info) = state.ytmusic.resolve_stream_info(&alt_track.id).await {
                            alt_stream = Some((alt_info, alt_track.id));
                            break;
                        }
                    }
                }
            }

            if let Some((alt_info, alt_vid)) = alt_stream {
                let dur_secs = payload.duration_seconds
                    .or(alt_info.duration_seconds)
                    .unwrap_or(210);

                let artwork = payload.artwork_url.or(alt_info.artwork_url);

                state.player.play_direct_stream(
                    &alt_info.stream_url,
                    &title,
                    &artist,
                    artwork.clone(),
                    dur_secs,
                    false,
                    0,
                    &state.led,
                ).await;

                state.logs.push("INFO", &format!("YouTube Music: запуск альтернативного YouTube-потока '{}' ({})", title, alt_vid)).await;

                (StatusCode::OK, Json(serde_json::json!({
                    "success": true,
                    "title": title,
                    "artist": artist,
                    "stream_url": alt_info.stream_url,
                    "fallback": false,
                }))).into_response()
            } else {
                error!("YouTube Music resolve error for {}: {}", vid, e);
                state.logs.push("ERROR", &format!("YouTube Music: не удалось получить поток для '{}': {}", title, e)).await;
                (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                    "success": false,
                    "error": format!("Не удалось получить поток YouTube для выбранного трека '{}': {}", title, e),
                }))).into_response()
            }
        }
    }
}

async fn ytmusic_search_handler(
    State(state): State<AppState>,
    Query(params): Query<MusicSearchQuery>,
) -> impl IntoResponse {
    let q = params.q.trim();
    if q.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Empty search query" })),
        )
            .into_response();
    }
    let limit = params.limit.unwrap_or(20);
    match state.ytmusic.search(q, limit).await {
        Ok(tracks) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "success": true,
                "source": "ytmusic",
                "query": q,
                "results": tracks,
            })),
        )
            .into_response(),
        Err(e) => {
            warn!("YT Music search for '{}' failed: {}", q, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": e,
                    "source": "ytmusic",
                    "results": [],
                })),
            )
                .into_response()
        }
    }
}

fn resolve_song_target(query: &str) -> String {
    let q_lower = query.trim().to_lowercase();
    if q_lower.starts_with("http://") || q_lower.starts_with("https://") {
        return query.trim().to_string();
    }

    // Popular streaming presets
    match q_lower.as_str() {
        "radio record" | "record" | "радио рекорд" | "рекорд" => {
            "http://air2.radiorecord.ru:805/rr_320".to_string()
        }
        "lofi" | "lofi chill" | "лофай" => {
            "http://stream.zeno.fm/f3wvbbqmdg8uv".to_string()
        }
        "deep house" | "deep" | "дип хаус" => {
            "http://air2.radiorecord.ru:805/deep_320".to_string()
        }
        "relax fm" | "relax" | "релакс" => {
            "http://ic7.101.ru:8000/v13_1".to_string()
        }
        "rock" | "classic rock" | "рок" => {
            "http://air2.radiorecord.ru:805/rock_320".to_string()
        }
        "nature" | "rain" | "дождь" | "звуки дождя" => {
            "http://stream.zeno.fm/f3wvbbqmdg8uv".to_string()
        }
        _ => query.trim().to_string(),
    }
}

#[derive(Deserialize)]
struct LedRequest {
    #[serde(default)]
    action: Option<String>,
    #[serde(default)]
    color: Option<[u8; 3]>,
    #[serde(default)]
    duration_ms: Option<u64>,
}

async fn set_led_handler(
    State(state): State<AppState>,
    Json(payload): Json<LedRequest>,
) -> impl IntoResponse {
    let action = payload.action.as_deref().unwrap_or("set");
    match action {
        "clear" | "off" => {
            state.led.clear();
            state.logs.push("INFO", "Web: подсветка выключена").await;
        }
        "aura" | "music" => {
            state.led.set_music_aura();
            state.logs.push("INFO", "Web: установлена музыкальная аура").await;
        }
        "blink" => {
            let [r, g, b] = payload.color.unwrap_or([255, 255, 255]);
            let dur = payload.duration_ms.unwrap_or(200);
            state.led.blink(r, g, b, dur);
        }
        _ => {
            if let Some([r, g, b]) = payload.color {
                state.led.set_rgb(r, g, b);
                state
                    .logs
                    .push("INFO", &format!("Web: установлен цвет LED [{}, {}, {}]", r, g, b))
                    .await;
            }
        }
    }
    let cur = state.led.get_rgb();
    Json(serde_json::json!({
        "success": true,
        "color": cur
    }))
}

#[derive(Deserialize)]
struct SpeakRequest {
    text: String,
    #[serde(default)]
    color: Option<[u8; 3]>,
}

async fn post_speak_handler(
    State(state): State<AppState>,
    Json(payload): Json<SpeakRequest>,
) -> impl IntoResponse {
    let text = payload.text.trim();
    if text.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Empty text"})),
        );
    }

    let color = payload.color.unwrap_or([0, 150, 255]);
    state
        .logs
        .push("INFO", &format!("Web: воспроизведение фразы: '{}'", text))
        .await;

    let tts = state.tts.clone();
    let led = state.led.clone();
    let player = state.player.clone();
    let text_clone = text.to_string();

    tokio::spawn(async move {
        if player.get_state().await == PlaybackState::Playing {
            player.pause(&led).await;
        }
        if let Err(e) = tts.speak(&text_clone, color, &led).await {
            error!("TTS Speak failed: {}", e);
        }
    });

    (StatusCode::OK, Json(serde_json::json!({"success": true})))
}

async fn trigger_mic_handler(State(state): State<AppState>) -> impl IntoResponse {
    // Send CenterHold to simulate holding center button
    let _ = state.btn_tx.send(StationButtonEvent::CenterHold).await;
    state
        .logs
        .push("INFO", "Web: запущен микрофон Станции для голосовой команды (4 сек)")
        .await;

    Json(serde_json::json!({
        "success": true,
        "message": "Microphone recording started on speaker"
    }))
}

#[derive(Deserialize)]
struct VirtualButtonRequest {
    button: String,
}

async fn virtual_button_handler(
    State(state): State<AppState>,
    Json(payload): Json<VirtualButtonRequest>,
) -> impl IntoResponse {
    let ev = match payload.button.to_lowercase().as_str() {
        "volumeup" | "vol_up" | "+" => StationButtonEvent::VolumeUp,
        "volumedown" | "vol_down" | "-" => StationButtonEvent::VolumeDown,
        "centertap" | "tap" | "play_pause" => StationButtonEvent::CenterTap,
        "centerhold" | "hold" | "voice" => StationButtonEvent::CenterHold,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Unknown button"})),
            );
        }
    };

    let _ = state.btn_tx.send(ev).await;
    state
        .logs
        .push("INFO", &format!("Web: нажата виртуальная кнопка {:?}", ev))
        .await;

    (StatusCode::OK, Json(serde_json::json!({"success": true})))
}

#[derive(Deserialize)]
struct AskRequest {
    prompt: String,
    #[serde(default)]
    speak: Option<bool>,
    #[serde(default)]
    session_id: Option<String>,
}

async fn post_ask_handler(
    State(state): State<AppState>,
    Json(payload): Json<AskRequest>,
) -> impl IntoResponse {
    handle_prompt(&state, &payload.prompt, payload.speak.unwrap_or(true), payload.session_id, true).await
}

async fn handle_prompt(
    state: &AppState,
    prompt_text: &str,
    speak: bool,
    session_id_req: Option<String>,
    allow_actions: bool,
) -> (StatusCode, Json<serde_json::Value>) {
    let prompt = prompt_text.trim();
    if prompt.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Empty prompt"})),
        );
    }

    state
        .logs
        .push("INFO", &format!("Web DeepSeek запрос: '{}'", prompt))
        .await;

    // 1-hour session rule & context resolution:
    let (session_id, history, is_new) = state
        .chat
        .prepare_session_for_prompt(session_id_req.as_deref())
        .await;

    // Context composition (Home Assistant + Weather + Timers/Alarms)
    let mut context_parts = Vec::new();
    let ha_context = state.homeassistant.get_entities_summary().await;
    if !ha_context.is_empty() {
        context_parts.push(ha_context);
    }
    let weather_context = state.weather.get_summary().await;
    if !weather_context.is_empty() {
        context_parts.push(weather_context);
    }
    let timer_context = state.timer_alarm.get_summary().await;
    if !timer_context.is_empty() {
        context_parts.push(timer_context);
    }
    let effective_context = context_parts.join("\n\n");

    match state.deepseek.query_with_context(prompt, &history, &effective_context).await {
        Ok(resp) => {
            state.led.set_rgb(resp.color[0], resp.color[1], resp.color[2]);

            // Execute smart home, timer and player actions only if allowed (POST requests)
            if allow_actions {
                // Execute any Home Assistant API calls returned by LLM
                for ha_call in &resp.ha_calls {
                    info!("Web LLM calling Home Assistant: {}/{} data={:?}", ha_call.domain, ha_call.service, ha_call.service_data);
                    if let Err(e) = state.homeassistant.call_service(&ha_call.domain, &ha_call.service, ha_call.service_data.clone()).await {
                        error!("Failed to execute HA call {}/{}: {}", ha_call.domain, ha_call.service, e);
                    }
                }

                // Execute Timer calls returned strictly by LLM
                if let Some(t_call) = &resp.timer_call {
                    state.timer_alarm.process_timer_call(t_call).await;
                }

                // Execute Alarm calls returned strictly by LLM
                if let Some(a_call) = &resp.alarm_call {
                    state.timer_alarm.process_alarm_call(a_call).await;
                }

                // Stop alarm/timer chime if requested or if ringing
                if resp.action == "stop" || state.timer_alarm.is_ringing() {
                    state.timer_alarm.stop_ringing();
                }

                match resp.action.as_str() {
                    "stop" => state.player.stop(&state.led).await,
                    "resume" => state.player.resume(&state.led).await,
                    _ => {}
                }
            }

            state
                .logs
                .push(
                    "INFO",
                    &format!(
                        "DeepSeek ответ [чат {}]: '{}' (действие: {}, ha_calls: {})",
                        session_id, resp.text, resp.action, resp.ha_calls.len()
                    ),
                )
                .await;

            // Record turn in persistent chat history
            state
                .chat
                .record_turn(
                    &session_id,
                    prompt,
                    &resp.text,
                    &resp.action,
                    resp.color,
                    &resp.song,
                    resp.ha_calls.clone(),
                )
                .await;

            // Sequential TTS Speech -> Music Playback (eliminates ALSA EBUSY contention)
            let should_speak = speak && !resp.text.is_empty();
            let should_play = allow_actions && resp.action == "play";

            if should_speak || should_play {
                let song_name = if resp.song.is_empty() {
                    "Lofi chill beats".to_string()
                } else {
                    resp.song.clone()
                };
                let yt = state.ytmusic.clone();
                let sc = state.soundcloud.clone();
                let pl = state.player.clone();
                let tts = state.tts.clone();
                let l = state.led.clone();
                let text = resp.text.clone();
                let color = resp.color;

                tokio::spawn(async move {
                    if should_speak {
                        if let Err(e) = tts.speak(&text, color, &l).await {
                            error!("TTS speak error: {}", e);
                        }
                    }
                    if should_play {
                        pl.play_smart_or_query(&yt, &sc, &song_name, &l).await;
                    }
                });
            }

            let mut val = serde_json::to_value(&resp).unwrap_or_else(|_| serde_json::json!({}));
            if let Some(obj) = val.as_object_mut() {
                obj.insert("session_id".to_string(), serde_json::json!(session_id));
                obj.insert("is_new_session".to_string(), serde_json::json!(is_new));
            }

            (StatusCode::OK, Json(val))
        }
        Err(e) => {
            state
                .logs
                .push("ERROR", &format!("Ошибка DeepSeek: {}", e))
                .await;
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "text": "Ошибка при обработке запроса к нейросети.",
                    "color": [255, 50, 0],
                    "action": "none",
                    "error": e,
                    "session_id": session_id
                })),
            )
        }
    }
}

async fn get_config_handler(State(state): State<AppState>) -> impl IntoResponse {
    let mut cfg = state.config.read().await.clone();
    // Mask sensitive keys for browser display
    if !cfg.deepseek.api_key.is_empty() {
        cfg.deepseek.api_key = mask_key(&cfg.deepseek.api_key);
    }
    if !cfg.deepseek.openrouter_api_key.is_empty() {
        cfg.deepseek.openrouter_api_key = mask_key(&cfg.deepseek.openrouter_api_key);
    }
    if !cfg.deepseek.groq_api_key.is_empty() {
        cfg.deepseek.groq_api_key = mask_key(&cfg.deepseek.groq_api_key);
    }
    if !cfg.deepseek.openai_api_key.is_empty() {
        cfg.deepseek.openai_api_key = mask_key(&cfg.deepseek.openai_api_key);
    }
    if !cfg.deepseek.mistral_api_key.is_empty() {
        cfg.deepseek.mistral_api_key = mask_key(&cfg.deepseek.mistral_api_key);
    }
    if !cfg.deepseek.anthropic_api_key.is_empty() {
        cfg.deepseek.anthropic_api_key = mask_key(&cfg.deepseek.anthropic_api_key);
    }
    if !cfg.deepseek.gemini_api_key.is_empty() {
        cfg.deepseek.gemini_api_key = mask_key(&cfg.deepseek.gemini_api_key);
    }
    if !cfg.deepseek.perplexity_api_key.is_empty() {
        cfg.deepseek.perplexity_api_key = mask_key(&cfg.deepseek.perplexity_api_key);
    }
    if !cfg.deepseek.together_api_key.is_empty() {
        cfg.deepseek.together_api_key = mask_key(&cfg.deepseek.together_api_key);
    }
    if !cfg.stt.api_key.is_empty() {
        cfg.stt.api_key = mask_key(&cfg.stt.api_key);
    }
    if !cfg.stt.openrouter_api_key.is_empty() {
        cfg.stt.openrouter_api_key = mask_key(&cfg.stt.openrouter_api_key);
    }
    if !cfg.stt.openai_api_key.is_empty() {
        cfg.stt.openai_api_key = mask_key(&cfg.stt.openai_api_key);
    }
    if !cfg.stt.groq_api_key.is_empty() {
        cfg.stt.groq_api_key = mask_key(&cfg.stt.groq_api_key);
    }
    if !cfg.stt.deepgram_api_key.is_empty() {
        cfg.stt.deepgram_api_key = mask_key(&cfg.stt.deepgram_api_key);
    }
    if !cfg.stt.gemini_api_key.is_empty() {
        cfg.stt.gemini_api_key = mask_key(&cfg.stt.gemini_api_key);
    }
    if !cfg.tts.openrouter.api_key.is_empty() {
        cfg.tts.openrouter.api_key = mask_key(&cfg.tts.openrouter.api_key);
    }
    if !cfg.tts.elevenlabs.api_key.is_empty() {
        cfg.tts.elevenlabs.api_key = mask_key(&cfg.tts.elevenlabs.api_key);
    }
    if !cfg.tts.cloudflare.api_token.is_empty() {
        cfg.tts.cloudflare.api_token = mask_key(&cfg.tts.cloudflare.api_token);
    }
    if !cfg.tts.openai.api_key.is_empty() {
        cfg.tts.openai.api_key = mask_key(&cfg.tts.openai.api_key);
    }
    if !cfg.tts.deepgram.api_key.is_empty() {
        cfg.tts.deepgram.api_key = mask_key(&cfg.tts.deepgram.api_key);
    }
    if !cfg.tts.cartesia.api_key.is_empty() {
        cfg.tts.cartesia.api_key = mask_key(&cfg.tts.cartesia.api_key);
    }
    if !cfg.deepseek.cloudflare_api_token.is_empty() {
        cfg.deepseek.cloudflare_api_token = mask_key(&cfg.deepseek.cloudflare_api_token);
    }
    if !cfg.homeassistant.token.is_empty() {
        cfg.homeassistant.token = mask_key(&cfg.homeassistant.token);
    }
    if !cfg.accounts.ytmusic_cookie.is_empty() {
        cfg.accounts.ytmusic_cookie = "Cookie установлен [Скрыто]".to_string();
    }
    if !cfg.accounts.spotify_client_secret.is_empty() {
        cfg.accounts.spotify_client_secret = mask_key(&cfg.accounts.spotify_client_secret);
    }
    if !cfg.accounts.spotify_refresh_token.is_empty() {
        cfg.accounts.spotify_refresh_token = mask_key(&cfg.accounts.spotify_refresh_token);
    }
    if !cfg.accounts.soundcloud_token.is_empty() {
        cfg.accounts.soundcloud_token = mask_key(&cfg.accounts.soundcloud_token);
    }
    if !cfg.proxy.socks5_url.is_empty() {
        if let Ok(mut u) = url::Url::parse(&cfg.proxy.socks5_url) {
            if u.password().is_some() {
                let _ = u.set_password(Some("***"));
                cfg.proxy.socks5_url = u.to_string();
            }
        }
    }
    Json(cfg)
}

#[derive(Deserialize)]
struct TestDnsRequest {
    #[serde(default)]
    host: Option<String>,
}

async fn get_dns_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await;
    let active_nameservers = crate::services::dns::get_active_nameservers().await;
    Json(serde_json::json!({
        "custom_enabled": cfg.dns.custom_enabled,
        "provider": cfg.dns.provider,
        "primary_dns": cfg.dns.primary_dns,
        "secondary_dns": cfg.dns.secondary_dns,
        "active_nameservers": active_nameservers,
    }))
}

async fn test_dns_handler(Json(payload): Json<TestDnsRequest>) -> impl IntoResponse {
    let host = payload.host.as_deref().unwrap_or("cloudflare.com");
    match crate::services::dns::test_dns_resolution(host).await {
        Ok((ip, latency_ms)) => Json(serde_json::json!({
            "success": true,
            "host": host,
            "resolved_ip": ip,
            "latency_ms": latency_ms,
            "message": format!("Успешно разрешено: {} -> {} ({} мс)", host, ip, latency_ms)
        })),
        Err(e) => Json(serde_json::json!({
            "success": false,
            "host": host,
            "error": e,
            "message": format!("Ошибка разрешения: {}", e)
        })),
    }
}

async fn get_assistant_config_handler(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await;

    let llm_prov = cfg.deepseek.provider.trim().to_lowercase();
    let (llm_key, llm_model, llm_account_id, llm_custom_url) = match llm_prov.as_str() {
        "openrouter" => (
            if !cfg.deepseek.openrouter_api_key.is_empty() {
                cfg.deepseek.openrouter_api_key.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            if !cfg.deepseek.openrouter_model.is_empty() {
                cfg.deepseek.openrouter_model.clone()
            } else {
                cfg.deepseek.model.clone()
            },
            None,
            None,
        ),
        "cloudflare" => (
            if !cfg.deepseek.cloudflare_api_token.is_empty() {
                cfg.deepseek.cloudflare_api_token.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            cfg.deepseek.cloudflare_model.clone(),
            Some(cfg.deepseek.cloudflare_account_id.clone()),
            None,
        ),
        "custom" | "local" => (
            cfg.deepseek.api_key.as_str(),
            if !cfg.deepseek.custom_model.is_empty() {
                cfg.deepseek.custom_model.clone()
            } else {
                cfg.deepseek.model.clone()
            },
            None,
            Some(cfg.deepseek.custom_base_url.clone()),
        ),
        "gemini" => (
            if !cfg.deepseek.gemini_api_key.is_empty() {
                cfg.deepseek.gemini_api_key.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            if !cfg.deepseek.gemini_model.is_empty() {
                cfg.deepseek.gemini_model.clone()
            } else {
                "gemini-flash-lite-latest".to_string()
            },
            None,
            None,
        ),
        "groq" => (
            if !cfg.deepseek.groq_api_key.is_empty() {
                cfg.deepseek.groq_api_key.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            if !cfg.deepseek.groq_model.is_empty() {
                cfg.deepseek.groq_model.clone()
            } else {
                "llama-3.3-70b-versatile".to_string()
            },
            None,
            None,
        ),
        "openai" => (
            if !cfg.deepseek.openai_api_key.is_empty() {
                cfg.deepseek.openai_api_key.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            if !cfg.deepseek.openai_model.is_empty() {
                cfg.deepseek.openai_model.clone()
            } else {
                "gpt-4o-mini".to_string()
            },
            None,
            None,
        ),
        "mistral" => (
            if !cfg.deepseek.mistral_api_key.is_empty() {
                cfg.deepseek.mistral_api_key.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            if !cfg.deepseek.mistral_model.is_empty() {
                cfg.deepseek.mistral_model.clone()
            } else {
                "mistral-small-latest".to_string()
            },
            None,
            None,
        ),
        "anthropic" => (
            if !cfg.deepseek.anthropic_api_key.is_empty() {
                cfg.deepseek.anthropic_api_key.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            if !cfg.deepseek.anthropic_model.is_empty() {
                cfg.deepseek.anthropic_model.clone()
            } else {
                "claude-3-5-haiku-latest".to_string()
            },
            None,
            None,
        ),
        "perplexity" => (
            if !cfg.deepseek.perplexity_api_key.is_empty() {
                cfg.deepseek.perplexity_api_key.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            if !cfg.deepseek.perplexity_model.is_empty() {
                cfg.deepseek.perplexity_model.clone()
            } else {
                "sonar".to_string()
            },
            None,
            None,
        ),
        "together" => (
            if !cfg.deepseek.together_api_key.is_empty() {
                cfg.deepseek.together_api_key.as_str()
            } else {
                cfg.deepseek.api_key.as_str()
            },
            if !cfg.deepseek.together_model.is_empty() {
                cfg.deepseek.together_model.clone()
            } else {
                "meta-llama/Llama-3.3-70B-Instruct-Turbo".to_string()
            },
            None,
            None,
        ),
        _ => (
            cfg.deepseek.api_key.as_str(),
            cfg.deepseek.model.clone(),
            None,
            None,
        ),
    };

    let stt_prov = cfg.stt.provider.trim().to_lowercase();
    let (stt_key, stt_model, stt_account_id, stt_custom_url) = match stt_prov.as_str() {
        "gemini" => (
            if !cfg.stt.gemini_api_key.is_empty() {
                cfg.stt.gemini_api_key.as_str()
            } else if !cfg.deepseek.gemini_api_key.is_empty() {
                cfg.deepseek.gemini_api_key.as_str()
            } else {
                cfg.stt.api_key.as_str()
            },
            if !cfg.stt.gemini_model.is_empty() {
                cfg.stt.gemini_model.clone()
            } else {
                "gemini-flash-lite-latest".to_string()
            },
            None,
            None,
        ),
        "openrouter" => (
            if !cfg.stt.openrouter_api_key.is_empty() {
                cfg.stt.openrouter_api_key.as_str()
            } else {
                cfg.stt.api_key.as_str()
            },
            cfg.stt.openrouter_model.clone(),
            None,
            None,
        ),
        "custom" | "local" => (
            cfg.stt.api_key.as_str(),
            cfg.stt.model.clone(),
            None,
            Some(cfg.stt.custom_url.clone()),
        ),
        _ => (
            cfg.stt.api_key.as_str(),
            cfg.stt.model.clone(),
            Some(cfg.stt.account_id.clone()),
            None,
        ),
    };

    let tts_prov = cfg.tts.provider.trim().to_lowercase();
    let (tts_key, tts_model, tts_voice, tts_account_id, tts_custom_url, tts_extra) = match tts_prov.as_str() {
        "openrouter" => (
            if !cfg.tts.openrouter.api_key.is_empty() {
                cfg.tts.openrouter.api_key.as_str()
            } else {
                cfg.deepseek.openrouter_api_key.as_str()
            },
            cfg.tts.openrouter.model.clone(),
            Some(cfg.tts.openrouter.voice.clone()),
            None,
            None,
            cfg.tts.openrouter.extra_payload.clone().or_else(|| cfg.tts.extra_payload.clone()),
        ),
        "cloudflare" => (
            if !cfg.tts.cloudflare.api_token.is_empty() {
                cfg.tts.cloudflare.api_token.as_str()
            } else {
                cfg.stt.api_key.as_str()
            },
            cfg.tts.cloudflare.model.clone(),
            Some(if !cfg.tts.cloudflare.voice_id.is_empty() {
                cfg.tts.cloudflare.voice_id.clone()
            } else {
                "21m00Tcm4TlvDq8ikWAM".to_string()
            }),
            Some(if !cfg.tts.cloudflare.account_id.is_empty() {
                cfg.tts.cloudflare.account_id.clone()
            } else {
                cfg.stt.account_id.clone()
            }),
            None,
            cfg.tts.cloudflare.extra_payload.clone().or_else(|| cfg.tts.extra_payload.clone()),
        ),
        "custom" | "local" => (
            "",
            "local".to_string(),
            None,
            None,
            Some(cfg.tts.custom.url.clone()),
            cfg.tts.extra_payload.clone(),
        ),
        "openai" => (
            cfg.tts.openai.api_key.as_str(),
            cfg.tts.openai.model.clone(),
            Some(cfg.tts.openai.voice.clone()),
            None,
            None,
            cfg.tts.extra_payload.clone(),
        ),
        "elevenlabs" => (
            cfg.tts.elevenlabs.api_key.as_str(),
            cfg.tts.elevenlabs.model_id.clone(),
            Some(cfg.tts.elevenlabs.voice_id.clone()),
            None,
            None,
            cfg.tts.extra_payload.clone(),
        ),
        _ => (
            "",
            "google".to_string(),
            None,
            None,
            None,
            cfg.tts.extra_payload.clone(),
        ),
    };

    let resp = AssistantConfigResponse {
        llm: AssistantStageInfo {
            provider: if llm_prov.is_empty() { "openrouter".to_string() } else { llm_prov },
            model: llm_model,
            has_key: !llm_key.is_empty(),
            key_masked: mask_key(llm_key),
            account_id: llm_account_id,
            voice: None,
            language: None,
            endpoint_url: llm_custom_url,
            extra_payload: cfg.deepseek.extra_payload.clone(),
        },
        stt: AssistantStageInfo {
            provider: if stt_prov.is_empty() { "cloudflare".to_string() } else { stt_prov },
            model: stt_model,
            has_key: !stt_key.is_empty(),
            key_masked: mask_key(stt_key),
            account_id: stt_account_id,
            voice: None,
            language: Some(cfg.stt.language.clone()),
            endpoint_url: stt_custom_url,
            extra_payload: cfg.stt.extra_payload.clone(),
        },
        tts: AssistantStageInfo {
            provider: if tts_prov.is_empty() { "google".to_string() } else { tts_prov },
            model: tts_model,
            has_key: !tts_key.is_empty(),
            key_masked: mask_key(tts_key),
            account_id: tts_account_id,
            voice: tts_voice,
            language: Some("ru".to_string()),
            endpoint_url: tts_custom_url,
            extra_payload: tts_extra,
        },
        system_prompt: cfg.deepseek.system_prompt.clone(),
        temperature: cfg.deepseek.temperature,
        max_tokens: cfg.deepseek.max_tokens,
    };

    (StatusCode::OK, Json(resp))
}

async fn post_assistant_config_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateAssistantConfigPayload>,
) -> impl IntoResponse {
    // 1. Validate extra_payload for all stages if present
    if let Some(ref llm) = payload.llm {
        if let Some(ref extra) = llm.extra_payload {
            if !extra.is_object() && !extra.is_null() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Invalid LLM extra_payload: must be a JSON object"
                    })),
                );
            }
        }
    }
    if let Some(ref stt) = payload.stt {
        if let Some(ref extra) = stt.extra_payload {
            if !extra.is_object() && !extra.is_null() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Invalid STT extra_payload: must be a JSON object"
                    })),
                );
            }
        }
    }
    if let Some(ref tts) = payload.tts {
        if let Some(ref extra) = tts.extra_payload {
            if !extra.is_object() && !extra.is_null() {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": "Invalid TTS extra_payload: must be a JSON object"
                    })),
                );
            }
        }
    }

    // 2. Apply updates
    let mut cfg = state.config.write().await;

    // LLM stage
    if let Some(ref llm) = payload.llm {
        if let Some(ref prov) = llm.provider {
            let p = prov.trim().to_lowercase();
            if !p.is_empty() {
                cfg.deepseek.provider = p;
            }
        }
        if let Some(ref m) = llm.model {
            let m_clean = m.trim();
            if !m_clean.is_empty() {
                match cfg.deepseek.provider.as_str() {
                    "openrouter" => cfg.deepseek.openrouter_model = m_clean.to_string(),
                    "cloudflare" => cfg.deepseek.cloudflare_model = m_clean.to_string(),
                    "custom" | "local" => cfg.deepseek.custom_model = m_clean.to_string(),
                    "gemini" => cfg.deepseek.gemini_model = m_clean.to_string(),
                    "groq" => cfg.deepseek.groq_model = m_clean.to_string(),
                    "openai" => cfg.deepseek.openai_model = m_clean.to_string(),
                    "mistral" => cfg.deepseek.mistral_model = m_clean.to_string(),
                    "anthropic" => cfg.deepseek.anthropic_model = m_clean.to_string(),
                    "perplexity" => cfg.deepseek.perplexity_model = m_clean.to_string(),
                    "together" => cfg.deepseek.together_model = m_clean.to_string(),
                    _ => cfg.deepseek.model = m_clean.to_string(),
                }
            }
        }
        if let Some(ref k) = llm.api_key {
            let k_clean = k.trim();
            if !k_clean.is_empty() && !is_masked(k_clean) {
                match cfg.deepseek.provider.as_str() {
                    "openrouter" => cfg.deepseek.openrouter_api_key = k_clean.to_string(),
                    "cloudflare" => cfg.deepseek.cloudflare_api_token = k_clean.to_string(),
                    "gemini" => cfg.deepseek.gemini_api_key = k_clean.to_string(),
                    "groq" => cfg.deepseek.groq_api_key = k_clean.to_string(),
                    "openai" => cfg.deepseek.openai_api_key = k_clean.to_string(),
                    "mistral" => cfg.deepseek.mistral_api_key = k_clean.to_string(),
                    "anthropic" => cfg.deepseek.anthropic_api_key = k_clean.to_string(),
                    "perplexity" => cfg.deepseek.perplexity_api_key = k_clean.to_string(),
                    "together" => cfg.deepseek.together_api_key = k_clean.to_string(),
                    _ => cfg.deepseek.api_key = k_clean.to_string(),
                }
            }
        }
        if let Some(ref acc) = llm.account_id {
            if !acc.trim().is_empty() {
                cfg.deepseek.cloudflare_account_id = acc.trim().to_string();
            }
        }
        if let Some(ref url) = llm.endpoint_url {
            cfg.deepseek.custom_base_url = url.trim().to_string();
        }
        if let Some(ref extra) = llm.extra_payload {
            cfg.deepseek.extra_payload = if extra.is_null() { None } else { Some(extra.clone()) };
        }
    }

    // STT stage
    if let Some(ref stt) = payload.stt {
        if let Some(ref prov) = stt.provider {
            let p = prov.trim().to_lowercase();
            if !p.is_empty() {
                cfg.stt.provider = p;
            }
        }
        if let Some(ref m) = stt.model {
            let m_clean = m.trim();
            if !m_clean.is_empty() {
                if cfg.stt.provider == "openrouter" {
                    cfg.stt.openrouter_model = m_clean.to_string();
                } else if cfg.stt.provider == "gemini" {
                    cfg.stt.gemini_model = m_clean.to_string();
                } else {
                    cfg.stt.model = m_clean.to_string();
                }
            }
        }
        if let Some(ref k) = stt.api_key {
            let k_clean = k.trim();
            if !k_clean.is_empty() && !is_masked(k_clean) {
                if cfg.stt.provider == "openrouter" {
                    cfg.stt.openrouter_api_key = k_clean.to_string();
                } else if cfg.stt.provider == "gemini" {
                    cfg.stt.gemini_api_key = k_clean.to_string();
                } else {
                    cfg.stt.api_key = k_clean.to_string();
                }
            }
        }
        if let Some(ref acc) = stt.account_id {
            if !acc.trim().is_empty() {
                cfg.stt.account_id = acc.trim().to_string();
            }
        }
        if let Some(ref lang) = stt.language {
            if !lang.trim().is_empty() {
                cfg.stt.language = lang.trim().to_string();
            }
        }
        if let Some(ref url) = stt.endpoint_url {
            cfg.stt.custom_url = url.trim().to_string();
        }
        if let Some(ref extra) = stt.extra_payload {
            cfg.stt.extra_payload = if extra.is_null() { None } else { Some(extra.clone()) };
        }
    }

    // TTS stage
    if let Some(ref tts) = payload.tts {
        if let Some(ref prov) = tts.provider {
            let p = prov.trim().to_lowercase();
            if !p.is_empty() {
                cfg.tts.provider = p;
            }
        }
        if let Some(ref m) = tts.model {
            let m_clean = m.trim();
            if !m_clean.is_empty() {
                match cfg.tts.provider.as_str() {
                    "openrouter" => cfg.tts.openrouter.model = m_clean.to_string(),
                    "cloudflare" => cfg.tts.cloudflare.model = m_clean.to_string(),
                    "openai" => cfg.tts.openai.model = m_clean.to_string(),
                    "elevenlabs" => cfg.tts.elevenlabs.model_id = m_clean.to_string(),
                    "deepgram" => cfg.tts.deepgram.model = m_clean.to_string(),
                    _ => {}
                }
            }
        }
        if let Some(ref v) = tts.voice {
            let v_clean = v.trim();
            if !v_clean.is_empty() {
                match cfg.tts.provider.as_str() {
                    "openrouter" => cfg.tts.openrouter.voice = v_clean.to_string(),
                    "cloudflare" => cfg.tts.cloudflare.voice_id = v_clean.to_string(),
                    "openai" => cfg.tts.openai.voice = v_clean.to_string(),
                    "elevenlabs" => cfg.tts.elevenlabs.voice_id = v_clean.to_string(),
                    _ => {}
                }
            }
        }
        if let Some(ref k) = tts.api_key {
            let k_clean = k.trim();
            if !k_clean.is_empty() && !is_masked(k_clean) {
                match cfg.tts.provider.as_str() {
                    "openrouter" => cfg.tts.openrouter.api_key = k_clean.to_string(),
                    "cloudflare" => cfg.tts.cloudflare.api_token = k_clean.to_string(),
                    "openai" => cfg.tts.openai.api_key = k_clean.to_string(),
                    "elevenlabs" => cfg.tts.elevenlabs.api_key = k_clean.to_string(),
                    _ => {}
                }
            }
        }
        if let Some(ref acc) = tts.account_id {
            if !acc.trim().is_empty() {
                cfg.tts.cloudflare.account_id = acc.trim().to_string();
            }
        }
        if let Some(ref url) = tts.endpoint_url {
            cfg.tts.custom.url = url.trim().to_string();
        }
        if let Some(ref extra) = tts.extra_payload {
            let opt_extra = if extra.is_null() { None } else { Some(extra.clone()) };
            cfg.tts.extra_payload = opt_extra.clone();
            cfg.tts.openrouter.extra_payload = opt_extra.clone();
            cfg.tts.cloudflare.extra_payload = opt_extra;
        }
    }

    if let Some(ref sp) = payload.system_prompt {
        cfg.deepseek.system_prompt = sp.clone();
    }
    if let Some(temp) = payload.temperature {
        cfg.deepseek.temperature = temp;
    }
    if let Some(toks) = payload.max_tokens {
        cfg.deepseek.max_tokens = toks;
    }

    // 3. Persist to disk
    let save_path = if Path::new("/data").exists() {
        "/data/config.json"
    } else {
        "config.json"
    };

    if let Ok(json_str) = serde_json::to_string_pretty(&*cfg) {
        let _ = tokio::fs::write(save_path, json_str).await;
        state
            .logs
            .push(
                "INFO",
                &format!("Assistant config saved to {} with extra_payload support", save_path),
            )
            .await;
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "status": "saved",
            "path": save_path
        })),
    )
}

fn is_masked(val: &str) -> bool {
    let t = val.trim();
    t.starts_with('*') || t.contains("...")
}

fn mask_key(k: &str) -> String {
    let trimmed = k.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let chars: Vec<char> = trimmed.chars().collect();
    if chars.len() > 8 {
        let prefix: String = chars[..4].iter().collect();
        let suffix: String = chars[chars.len() - 4..].iter().collect();
        format!("{}...{}", prefix, suffix)
    } else {
        "***".to_string()
    }
}

#[derive(Deserialize)]
struct UpdateConfigRequest {
    #[serde(default)]
    llm_provider: Option<String>,
    #[serde(default)]
    deepseek_api_key: Option<String>,
    #[serde(default)]
    deepseek_model: Option<String>,
    #[serde(default)]
    openrouter_api_key: Option<String>,
    #[serde(default)]
    openrouter_model: Option<String>,
    #[serde(default)]
    groq_api_key: Option<String>,
    #[serde(default)]
    groq_model: Option<String>,
    #[serde(default)]
    openai_api_key: Option<String>,
    #[serde(default)]
    openai_model: Option<String>,
    #[serde(default)]
    mistral_api_key: Option<String>,
    #[serde(default)]
    mistral_model: Option<String>,
    #[serde(default)]
    anthropic_api_key: Option<String>,
    #[serde(default)]
    anthropic_model: Option<String>,
    #[serde(default)]
    gemini_api_key: Option<String>,
    #[serde(default)]
    gemini_model: Option<String>,
    #[serde(default)]
    perplexity_api_key: Option<String>,
    #[serde(default)]
    perplexity_model: Option<String>,
    #[serde(default)]
    together_api_key: Option<String>,
    #[serde(default)]
    together_model: Option<String>,
    #[serde(default)]
    custom_llm_base_url: Option<String>,
    #[serde(default)]
    custom_llm_model: Option<String>,
    #[serde(default)]
    system_prompt: Option<String>,
    #[serde(default)]
    stt_provider: Option<String>,
    #[serde(default)]
    cloudflare_account_id: Option<String>,
    #[serde(default)]
    cloudflare_api_key: Option<String>,
    #[serde(default)]
    stt_model: Option<String>,
    #[serde(default)]
    openrouter_stt_api_key: Option<String>,
    #[serde(default)]
    openrouter_stt_model: Option<String>,
    #[serde(default)]
    openai_stt_api_key: Option<String>,
    #[serde(default)]
    openai_stt_model: Option<String>,
    #[serde(default)]
    groq_stt_api_key: Option<String>,
    #[serde(default)]
    groq_stt_model: Option<String>,
    #[serde(default)]
    deepgram_stt_api_key: Option<String>,
    #[serde(default)]
    deepgram_stt_model: Option<String>,
    #[serde(default)]
    custom_stt_url: Option<String>,
    #[serde(default)]
    tts_provider: Option<String>,
    #[serde(default)]
    openrouter_tts_api_key: Option<String>,
    #[serde(default)]
    openrouter_tts_model: Option<String>,
    #[serde(default)]
    openrouter_tts_voice: Option<String>,
    #[serde(default)]
    elevenlabs_api_key: Option<String>,
    #[serde(default)]
    elevenlabs_model_id: Option<String>,
    #[serde(default)]
    elevenlabs_voice_id: Option<String>,
    #[serde(default)]
    cloudflare_tts_account_id: Option<String>,
    #[serde(default)]
    cloudflare_tts_api_token: Option<String>,
    #[serde(default)]
    cloudflare_tts_model: Option<String>,
    #[serde(default)]
    openai_tts_api_key: Option<String>,
    #[serde(default)]
    openai_tts_model: Option<String>,
    #[serde(default)]
    openai_tts_voice: Option<String>,
    #[serde(default)]
    deepgram_tts_api_key: Option<String>,
    #[serde(default)]
    deepgram_tts_model: Option<String>,
    #[serde(default)]
    cartesia_tts_api_key: Option<String>,
    #[serde(default)]
    cartesia_tts_model_id: Option<String>,
    #[serde(default)]
    cartesia_tts_voice_id: Option<String>,
    #[serde(default)]
    custom_tts_url: Option<String>,
    #[serde(default)]
    proxy_enabled: Option<bool>,
    #[serde(default)]
    socks5_enabled: Option<bool>,
    #[serde(default)]
    proxy_socks5_url: Option<String>,
    #[serde(default)]
    socks5_url: Option<String>,
    #[serde(default)]
    dns_custom_enabled: Option<bool>,
    #[serde(default)]
    dns_provider: Option<String>,
    #[serde(default)]
    dns_primary: Option<String>,
    #[serde(default)]
    dns_secondary: Option<String>,
    #[serde(default)]
    cloudflare_llm_account_id: Option<String>,
    #[serde(default)]
    cloudflare_llm_api_token: Option<String>,
    #[serde(default)]
    cloudflare_llm_model: Option<String>,
    #[serde(default)]
    llm_extra_payload: Option<serde_json::Value>,
    #[serde(default)]
    openrouter_llm_extra_payload: Option<serde_json::Value>,
    #[serde(default)]
    cloudflare_llm_extra_payload: Option<serde_json::Value>,
    #[serde(default)]
    stt_extra_payload: Option<serde_json::Value>,
    #[serde(default)]
    cloudflare_stt_extra_payload: Option<serde_json::Value>,
    #[serde(default)]
    tts_extra_payload: Option<serde_json::Value>,
    #[serde(default)]
    openrouter_tts_extra_payload: Option<serde_json::Value>,
    #[serde(default)]
    cloudflare_tts_extra_payload: Option<serde_json::Value>,
    #[serde(default)]
    cloudflare_tts_voice_id: Option<String>,
    #[serde(default)]
    cloudflare_tts_lang: Option<String>,
}

async fn post_config_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateConfigRequest>,
) -> impl IntoResponse {
    let mut cfg = state.config.write().await;

    if let Some(acc) = payload.cloudflare_llm_account_id {
        cfg.deepseek.cloudflare_account_id = acc.trim().to_string();
    }
    if let Some(tok) = payload.cloudflare_llm_api_token {
        if !is_masked(tok.trim()) {
            cfg.deepseek.cloudflare_api_token = tok.trim().to_string();
        }
    }
    if let Some(m) = payload.cloudflare_llm_model {
        if !m.trim().is_empty() {
            cfg.deepseek.cloudflare_model = m.trim().to_string();
        }
    }
    if let Some(extra) = payload.cloudflare_llm_extra_payload.or(payload.openrouter_llm_extra_payload).or(payload.llm_extra_payload) {
        cfg.deepseek.extra_payload = if extra.is_null() { None } else { Some(extra) };
    }
    if let Some(extra) = payload.cloudflare_stt_extra_payload.or(payload.stt_extra_payload) {
        cfg.stt.extra_payload = if extra.is_null() { None } else { Some(extra) };
    }
    if let Some(extra) = payload.cloudflare_tts_extra_payload {
        cfg.tts.cloudflare.extra_payload = if extra.is_null() { None } else { Some(extra) };
    }
    if let Some(extra) = payload.openrouter_tts_extra_payload.or(payload.tts_extra_payload) {
        let opt = if extra.is_null() { None } else { Some(extra) };
        cfg.tts.extra_payload = opt.clone();
        cfg.tts.openrouter.extra_payload = opt;
    }
    if let Some(v) = payload.cloudflare_tts_voice_id {
        if !v.trim().is_empty() {
            cfg.tts.cloudflare.voice_id = v.trim().to_string();
        }
    }
    if let Some(l) = payload.cloudflare_tts_lang {
        if !l.trim().is_empty() {
            cfg.tts.cloudflare.lang = l.trim().to_string();
        }
    }

    if let Some(prov) = payload.llm_provider {
        if !prov.trim().is_empty() {
            cfg.deepseek.provider = prov.trim().to_lowercase();
        }
    }
    if let Some(key) = payload.openrouter_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.openrouter_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.openrouter_model {
        if !m.trim().is_empty() {
            cfg.deepseek.openrouter_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.groq_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.groq_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.groq_model {
        if !m.trim().is_empty() {
            cfg.deepseek.groq_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.openai_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.openai_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.openai_model {
        if !m.trim().is_empty() {
            cfg.deepseek.openai_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.mistral_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.mistral_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.mistral_model {
        if !m.trim().is_empty() {
            cfg.deepseek.mistral_model = m.trim().to_string();
        }
    }
    if let Some(url) = payload.custom_llm_base_url {
        cfg.deepseek.custom_base_url = url.trim().to_string();
    }
    if let Some(m) = payload.custom_llm_model {
        if !m.trim().is_empty() {
            cfg.deepseek.custom_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.deepseek_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.deepseek_model {
        if !m.trim().is_empty() {
            cfg.deepseek.model = m.trim().to_string();
        }
    }
    if let Some(sp) = payload.system_prompt {
        cfg.deepseek.system_prompt = sp;
    }
    if let Some(key) = payload.openai_tts_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.tts.openai.api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.openai_tts_model {
        if !m.trim().is_empty() {
            cfg.tts.openai.model = m.trim().to_string();
        }
    }
    if let Some(v) = payload.openai_tts_voice {
        if !v.trim().is_empty() {
            cfg.tts.openai.voice = v.trim().to_string();
        }
    }
    if let Some(acc) = payload.cloudflare_account_id {
        if !acc.trim().is_empty() {
            cfg.stt.account_id = acc.trim().to_string();
        }
    }
    if let Some(key) = payload.cloudflare_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.stt.api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.stt_model {
        if !m.trim().is_empty() {
            cfg.stt.model = m.trim().to_string();
        }
    }
    if let Some(tp) = payload.tts_provider {
        if !tp.trim().is_empty() {
            cfg.tts.provider = tp.trim().to_lowercase();
        }
    }
    if let Some(key) = payload.elevenlabs_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.tts.elevenlabs.api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.elevenlabs_model_id {
        if !m.trim().is_empty() {
            cfg.tts.elevenlabs.model_id = m.trim().to_string();
        }
    }
    if let Some(v) = payload.elevenlabs_voice_id {
        if !v.trim().is_empty() {
            cfg.tts.elevenlabs.voice_id = v.trim().to_string();
        }
    }
    if let Some(acc) = payload.cloudflare_tts_account_id {
        if !acc.trim().is_empty() {
            cfg.tts.cloudflare.account_id = acc.trim().to_string();
        }
    }
    if let Some(tok) = payload.cloudflare_tts_api_token {
        let trimmed = tok.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.tts.cloudflare.api_token = trimmed.to_string();
        }
    }
    if let Some(m) = payload.cloudflare_tts_model {
        if !m.trim().is_empty() {
            cfg.tts.cloudflare.model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.anthropic_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.anthropic_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.anthropic_model {
        if !m.trim().is_empty() {
            cfg.deepseek.anthropic_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.gemini_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.gemini_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.gemini_model {
        if !m.trim().is_empty() {
            cfg.deepseek.gemini_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.perplexity_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.perplexity_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.perplexity_model {
        if !m.trim().is_empty() {
            cfg.deepseek.perplexity_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.together_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.deepseek.together_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.together_model {
        if !m.trim().is_empty() {
            cfg.deepseek.together_model = m.trim().to_string();
        }
    }
    if let Some(sp) = payload.stt_provider {
        if !sp.trim().is_empty() {
            cfg.stt.provider = sp.trim().to_lowercase();
        }
    }
    if let Some(key) = payload.openrouter_stt_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.stt.openrouter_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.openrouter_stt_model {
        if !m.trim().is_empty() {
            cfg.stt.openrouter_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.openrouter_tts_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.tts.openrouter.api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.openrouter_tts_model {
        if !m.trim().is_empty() {
            cfg.tts.openrouter.model = m.trim().to_string();
        }
    }
    if let Some(v) = payload.openrouter_tts_voice {
        if !v.trim().is_empty() {
            cfg.tts.openrouter.voice = v.trim().to_string();
        }
    }
    if let Some(key) = payload.openai_stt_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.stt.openai_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.openai_stt_model {
        if !m.trim().is_empty() {
            cfg.stt.openai_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.groq_stt_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.stt.groq_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.groq_stt_model {
        if !m.trim().is_empty() {
            cfg.stt.groq_model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.deepgram_stt_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.stt.deepgram_api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.deepgram_stt_model {
        if !m.trim().is_empty() {
            cfg.stt.deepgram_model = m.trim().to_string();
        }
    }
    if let Some(u) = payload.custom_stt_url {
        cfg.stt.custom_url = u.trim().to_string();
    }
    if let Some(key) = payload.deepgram_tts_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.tts.deepgram.api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.deepgram_tts_model {
        if !m.trim().is_empty() {
            cfg.tts.deepgram.model = m.trim().to_string();
        }
    }
    if let Some(key) = payload.cartesia_tts_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !is_masked(trimmed) {
            cfg.tts.cartesia.api_key = trimmed.to_string();
        }
    }
    if let Some(m) = payload.cartesia_tts_model_id {
        if !m.trim().is_empty() {
            cfg.tts.cartesia.model_id = m.trim().to_string();
        }
    }
    if let Some(v) = payload.cartesia_tts_voice_id {
        if !v.trim().is_empty() {
            cfg.tts.cartesia.voice_id = v.trim().to_string();
        }
    }
    if let Some(u) = payload.custom_tts_url {
        cfg.tts.custom.url = u.trim().to_string();
    }
    if let Some(p_en) = payload.socks5_enabled.or(payload.proxy_enabled) {
        cfg.proxy.enabled = p_en;
    }
    if let Some(p_url) = payload.socks5_url.or(payload.proxy_socks5_url) {
        let trimmed = p_url.trim().to_string();
        if !trimmed.is_empty() {
            if trimmed.contains("***") {
                if let (Ok(mut new_u), Ok(old_u)) = (url::Url::parse(&trimmed), url::Url::parse(&cfg.proxy.socks5_url)) {
                    if new_u.password() == Some("***") {
                        if let Some(old_pwd) = old_u.password() {
                            let _ = new_u.set_password(Some(old_pwd));
                            cfg.proxy.socks5_url = new_u.to_string();
                        }
                    }
                }
            } else {
                cfg.proxy.socks5_url = trimmed;
            }
        }
    }
    if let Some(d_en) = payload.dns_custom_enabled {
        cfg.dns.custom_enabled = d_en;
    }
    if let Some(prov) = payload.dns_provider {
        if !prov.trim().is_empty() {
            cfg.dns.provider = prov.trim().to_string();
        }
    }
    if let Some(prim) = payload.dns_primary {
        let trimmed = prim.trim().to_string();
        if !trimmed.is_empty() {
            cfg.dns.primary_dns = trimmed;
        }
    }
    if let Some(sec) = payload.dns_secondary {
        let trimmed = sec.trim().to_string();
        if !trimmed.is_empty() {
            cfg.dns.secondary_dns = trimmed;
        }
    }

    // Apply DNS changes to system
    let _ = crate::services::dns::apply_dns(&cfg.dns).await;

    // Save to /data/config.json if path exists, otherwise config.json
    let save_path = if Path::new("/data").exists() {
        "/data/config.json"
    } else {
        "config.json"
    };

    if let Ok(json_str) = serde_json::to_string_pretty(&*cfg) {
        let _ = tokio::fs::write(save_path, json_str).await;
        state
            .logs
            .push("INFO", &format!("Конфигурация успешно сохранена в {}", save_path))
            .await;
    }

    Json(serde_json::json!({
        "status": "saved",
        "path": save_path
    }))
}

async fn get_logs_handler(State(state): State<AppState>) -> impl IntoResponse {
    let logs = state.logs.get_all().await;
    Json(serde_json::json!({ "logs": logs }))
}

async fn clear_logs_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.logs.clear().await;
    Json(serde_json::json!({ "status": "cleared" }))
}

// ---------------------------------------------------------------------------
// System Diagnostics & Network helpers
// ---------------------------------------------------------------------------

pub fn get_local_ip() -> String {
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(local_addr) = socket.local_addr() {
                let ip = local_addr.ip().to_string();
                if ip != "0.0.0.0" {
                    return ip;
                }
            }
        }
    }
    "127.0.0.1".to_string()
}

fn get_cpu_temp() -> f32 {
    if let Ok(raw) = std::fs::read_to_string("/sys/class/thermal/thermal_zone0/temp") {
        if let Ok(milli) = raw.trim().parse::<f32>() {
            return (milli / 1000.0 * 10.0).round() / 10.0;
        }
    }
    44.0
}

fn get_cpu_load_percent() -> u8 {
    if let Ok(raw) = std::fs::read_to_string("/proc/loadavg") {
        if let Some(first) = raw.split_whitespace().next() {
            if let Ok(load1) = first.parse::<f32>() {
                // Amlogic A113X has 4 Cortex-A53 cores
                let pct = (load1 / 4.0 * 100.0).clamp(1.0, 100.0);
                return pct.round() as u8;
            }
        }
    }
    8
}

fn get_mem_stats() -> (u64, u64) {
    if let Ok(meminfo) = std::fs::read_to_string("/proc/meminfo") {
        let mut total = 0u64;
        let mut avail = 0u64;
        for line in meminfo.lines() {
            if line.starts_with("MemTotal:") {
                total = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            } else if line.starts_with("MemAvailable:") {
                avail = line
                    .split_whitespace()
                    .nth(1)
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);
            }
        }
        let total_mb = total / 1024;
        let used_mb = total.saturating_sub(avail) / 1024;
        return (used_mb, total_mb);
    }
    (115, 238)
}

fn get_current_time_str() -> String {
    if let Ok(duration) = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH) {
        let secs = duration.as_secs();
        let hours = (secs / 3600) % 24;
        let mins = (secs / 60) % 60;
        let s = secs % 60;
        format!("{:02}:{:02}:{:02}", hours, mins, s)
    } else {
        "00:00:00".to_string()
    }
}

// ---------------------------------------------------------------------------
// Wi-Fi Web Handlers
// ---------------------------------------------------------------------------

async fn get_wifi_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.network.get_wifi_status();
    Json(status)
}

async fn scan_wifi_handler(State(state): State<AppState>) -> impl IntoResponse {
    let net = state.network.clone();
    let res = tokio::task::spawn_blocking(move || net.scan_wifi())
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match res {
        Ok(list) => (
            StatusCode::OK,
            Json(serde_json::json!({ "success": true, "networks": list })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e, "networks": [] })),
        ),
    }
}

#[derive(Deserialize)]
struct WifiConnectPayload {
    ssid: String,
    psk: Option<String>,
}

async fn connect_wifi_handler(
    State(state): State<AppState>,
    Json(payload): Json<WifiConnectPayload>,
) -> impl IntoResponse {
    let psk = payload.psk.unwrap_or_default();
    match state.network.connect_wifi(&payload.ssid, &psk) {
        Ok(msg) => {
            state.logs.push("INFO", &format!("Wi-Fi connection requested for SSID '{}'", payload.ssid)).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "message": msg })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

async fn restart_wifi_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.network.restart_wifi() {
        Ok(msg) => {
            state.logs.push("INFO", "Wi-Fi restart initiated").await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "message": msg })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

async fn get_saved_wifi_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.network.get_saved_wifi_networks() {
        Ok(list) => (
            StatusCode::OK,
            Json(serde_json::json!({ "success": true, "networks": list })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e, "networks": [] })),
        ),
    }
}

#[derive(Deserialize)]
struct WifiNetworkIdPayload {
    id: u32,
}

async fn select_saved_wifi_handler(
    State(state): State<AppState>,
    Json(payload): Json<WifiNetworkIdPayload>,
) -> impl IntoResponse {
    match state.network.select_saved_wifi_network(payload.id) {
        Ok(msg) => {
            state
                .logs
                .push(
                    "INFO",
                    &format!("Switched to saved Wi-Fi network id #{}", payload.id),
                )
                .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "message": msg })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

async fn remove_saved_wifi_handler(
    State(state): State<AppState>,
    Json(payload): Json<WifiNetworkIdPayload>,
) -> impl IntoResponse {
    match state.network.remove_saved_wifi_network(payload.id) {
        Ok(msg) => {
            state
                .logs
                .push(
                    "INFO",
                    &format!("Removed saved Wi-Fi network id #{}", payload.id),
                )
                .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "message": msg })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

// ---------------------------------------------------------------------------
// Wi-Fi 88x2cs Power Saving Web Handlers
// ---------------------------------------------------------------------------

async fn get_wifi_powersave_handler(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.network.get_powersave_config();
    Json(cfg)
}

#[derive(Deserialize)]
struct SetPowerSavePayload {
    param: Option<String>,
    name: Option<String>,
    value: String,
}

async fn set_wifi_powersave_handler(
    State(state): State<AppState>,
    Json(payload): Json<SetPowerSavePayload>,
) -> impl IntoResponse {
    let param_name = payload.param.or(payload.name).unwrap_or_default();
    if param_name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "success": false, "error": "Parameter name required" })),
        );
    }

    match state.network.set_powersave_param(&param_name, &payload.value) {
        Ok(()) => {
            state.logs.push(
                "INFO",
                &format!("88x2cs parameter '{}' updated to '{}'", param_name, payload.value),
            ).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "message": format!("Param '{}' set to '{}'", param_name, payload.value) })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

async fn optimize_wifi_powersave_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.network.optimize_powersave() {
        Ok(()) => {
            state.logs.push(
                "INFO",
                "Wi-Fi 88x2cs power saving disabled (rtw_power_mgnt=0, rtw_ips_mode=0, rtw_smart_ps=0). Stable connection maintained.",
            ).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "message": "Power saving disabled successfully (rtw_power_mgnt=0, rtw_ips_mode=0, rtw_smart_ps=0)"
                })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

// ---------------------------------------------------------------------------
// Bluetooth Web Handlers
// ---------------------------------------------------------------------------

async fn get_bluetooth_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.network.get_bluetooth_status();
    Json(status)
}

#[derive(Deserialize)]
struct BtTogglePayload {
    powered: bool,
}

async fn toggle_bluetooth_handler(
    State(state): State<AppState>,
    Json(payload): Json<BtTogglePayload>,
) -> impl IntoResponse {
    match state.network.toggle_bluetooth_power(payload.powered) {
        Ok(()) => {
            state.logs.push("INFO", &format!("Bluetooth power changed to {}", payload.powered)).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "powered": payload.powered })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

#[derive(Deserialize)]
struct BtDiscoverablePayload {
    discoverable: bool,
}

async fn set_bluetooth_discoverable_handler(
    State(state): State<AppState>,
    Json(payload): Json<BtDiscoverablePayload>,
) -> impl IntoResponse {
    match state
        .network
        .set_bluetooth_discoverable(payload.discoverable)
    {
        Ok(()) => {
            state
                .logs
                .push(
                    "INFO",
                    &format!(
                        "Bluetooth discoverable (speaker pairing mode) set to {}",
                        payload.discoverable
                    ),
                )
                .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "discoverable": payload.discoverable })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

async fn scan_bluetooth_handler(State(state): State<AppState>) -> impl IntoResponse {
    let net = state.network.clone();
    let res = tokio::task::spawn_blocking(move || net.scan_bluetooth())
        .await
        .unwrap_or_else(|e| Err(e.to_string()));
    match res {
        Ok(list) => (
            StatusCode::OK,
            Json(serde_json::json!({ "success": true, "devices": list })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e, "devices": [] })),
        ),
    }
}

#[derive(Deserialize)]
struct BtDeviceActionPayload {
    address: String,
}

async fn connect_bluetooth_handler(
    State(state): State<AppState>,
    Json(payload): Json<BtDeviceActionPayload>,
) -> impl IntoResponse {
    let net = state.network.clone();
    let addr = payload.address.clone();
    let res = tokio::task::spawn_blocking(move || net.connect_bluetooth(&addr)).await;
    match res {
        Ok(Ok(out)) => {
            state.logs.push("INFO", &format!("Connected to Bluetooth device {}", payload.address)).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "output": out })),
            )
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": format!("Task failed: {}", e) })),
        ),
    }
}

async fn disconnect_bluetooth_handler(
    State(state): State<AppState>,
    Json(payload): Json<BtDeviceActionPayload>,
) -> impl IntoResponse {
    let net = state.network.clone();
    let addr = payload.address.clone();
    let res = tokio::task::spawn_blocking(move || net.disconnect_bluetooth(&addr)).await;
    match res {
        Ok(Ok(out)) => {
            state.logs.push("INFO", &format!("Disconnected from Bluetooth device {}", payload.address)).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "output": out })),
            )
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": format!("Task failed: {}", e) })),
        ),
    }
}

async fn pair_bluetooth_handler(
    State(state): State<AppState>,
    Json(payload): Json<BtDeviceActionPayload>,
) -> impl IntoResponse {
    let net = state.network.clone();
    let addr = payload.address.clone();
    let res = tokio::task::spawn_blocking(move || net.pair_bluetooth(&addr)).await;
    match res {
        Ok(Ok(out)) => {
            state
                .logs
                .push(
                    "INFO",
                    &format!("Paired with Bluetooth device {}", payload.address),
                )
                .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "output": out })),
            )
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": format!("Task failed: {}", e) })),
        ),
    }
}

async fn remove_bluetooth_handler(
    State(state): State<AppState>,
    Json(payload): Json<BtDeviceActionPayload>,
) -> impl IntoResponse {
    let net = state.network.clone();
    let addr = payload.address.clone();
    let res = tokio::task::spawn_blocking(move || net.remove_bluetooth(&addr)).await;
    match res {
        Ok(Ok(out)) => {
            state
                .logs
                .push(
                    "INFO",
                    &format!("Removed Bluetooth device {}", payload.address),
                )
                .await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "output": out })),
            )
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": format!("Task failed: {}", e) })),
        ),
    }
}

// ---------------------------------------------------------------------------
// Terminal Command Execution Web Handler
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TerminalRunPayload {
    command: String,
}

async fn terminal_run_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(payload): Json<TerminalRunPayload>,
) -> axum::response::Response {
    // SEC-01: CSRF & Drive-By RCE protection. Require X-Station-Request header.
    if !headers.contains_key("x-station-request") {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "success": false,
                "exit_code": -1,
                "stdout": "",
                "stderr": "Forbidden: missing X-Station-Request security header",
                "output": "Forbidden: missing X-Station-Request security header"
            })),
        )
            .into_response();
    }

    let cmd = payload.command.trim();
    if cmd.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "exit_code": -1,
                "stdout": "",
                "stderr": "Command cannot be empty",
                "output": "Command cannot be empty"
            })),
        )
            .into_response();
    }

    state.logs.push("WARN", &format!("Terminal execution: '{}'", cmd)).await;

    let res = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio::process::Command::new("sh").args(["-c", cmd]).output(),
    )
    .await;

    match res {
        Ok(Ok(o)) => {
            let mut stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let mut stderr = String::from_utf8_lossy(&o.stderr).to_string();
            // Cap output to 64 KB to protect 238MB RAM
            if stdout.len() > 65536 {
                stdout.truncate(65536);
                stdout.push_str("\n... [вывод обрезан до 64 КБ]");
            }
            if stderr.len() > 65536 {
                stderr.truncate(65536);
                stderr.push_str("\n... [вывод обрезан до 64 КБ]");
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": o.status.success(),
                    "exit_code": o.status.code().unwrap_or(-1),
                    "stdout": stdout,
                    "stderr": stderr,
                    "output": if stdout.is_empty() { stderr } else { stdout },
                })),
            )
                .into_response()
        }
        Ok(Err(e)) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "exit_code": -1,
                "stdout": "",
                "stderr": e.to_string(),
                "output": format!("Execution error: {}", e),
            })),
        )
            .into_response(),
        Err(_) => (
            StatusCode::GATEWAY_TIMEOUT,
            Json(serde_json::json!({
                "success": false,
                "exit_code": -1,
                "stdout": "",
                "stderr": "Command timed out after 15 seconds",
                "output": "Command timed out after 15 seconds",
            })),
        )
            .into_response(),
    }
}

// ---------------------------------------------------------------------------
// Dual Boot & System Switching Handlers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DualBootStatusResponse {
    active: bool,
    remaining_seconds: u32,
    timeout_seconds: u32,
    current_os: &'static str,
    default_mode: String,
}

async fn get_dualboot_status_handler() -> impl IntoResponse {
    let active = Path::new("/tmp/dualboot_active").exists();
    let remaining_seconds = if active {
        std::fs::read_to_string("/tmp/dualboot_remaining")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(0)
    } else {
        0
    };
    let default_mode = std::fs::read_to_string("/data/dualboot_mode")
        .unwrap_or_else(|_| "choice".to_string())
        .trim()
        .to_string();

    Json(DualBootStatusResponse {
        active,
        remaining_seconds,
        timeout_seconds: 7,
        current_os: "station_daemon",
        default_mode,
    })
}

#[derive(Deserialize)]
struct DualBootChooseRequest {
    choice: String,
}

async fn post_dualboot_choose_handler(
    Json(payload): Json<DualBootChooseRequest>,
) -> impl IntoResponse {
    match payload.choice.as_str() {
        "daemon" => {
            let _ = std::fs::write("/tmp/dualboot_stay_daemon", "1");
            let _ = std::fs::remove_file("/tmp/dualboot_active");
            Json(serde_json::json!({
                "success": true,
                "message": "Выбран Station Daemon. Автопереключение на Квазар отменено."
            }))
        }
        "quasar" => {
            let _ = std::fs::write("/tmp/dualboot_switch_quasar", "1");
            tokio::spawn(async {
                tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                let _ = std::process::Command::new("sh")
                    .arg("-c")
                    .arg("/data/dualboot.sh --quasar")
                    .spawn();
            });
            Json(serde_json::json!({
                "success": true,
                "message": "Переключение на Яндекс Квазар (Алиса)..."
            }))
        }
        _ => Json(serde_json::json!({
            "success": false,
            "error": "Неверный выбор (ожидается 'daemon' или 'quasar')"
        })),
    }
}

async fn post_switch_quasar_handler() -> impl IntoResponse {
    tokio::spawn(async {
        let _ = std::process::Command::new("killall").args(["-9", "arecord", "aplay", "mpg123", "ffmpeg"]).output();
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let _ = std::process::Command::new("sh")
            .arg("-c")
            .arg("/data/dualboot.sh --quasar")
            .spawn();
    });
    Json(serde_json::json!({
        "success": true,
        "message": "Остановка Station Daemon и запуск Яндекс Квазара (Алиса)..."
    }))
}

#[derive(Deserialize)]
struct DualBootModePayload {
    mode: String,
}

async fn post_dualboot_mode_handler(
    Json(payload): Json<DualBootModePayload>,
) -> impl IntoResponse {
    let mode = payload.mode.trim();
    if mode == "choice" || mode == "daemon" || mode == "quasar" {
        if let Err(e) = std::fs::write("/data/dualboot_mode", mode) {
            return Json(serde_json::json!({
                "success": false,
                "error": format!("Не удалось сохранить режим: {}", e)
            }));
        }
        Json(serde_json::json!({
            "success": true,
            "mode": mode
        }))
    } else {
        Json(serde_json::json!({
            "success": false,
            "error": "Режим должен быть 'choice', 'daemon' или 'quasar'"
        }))
    }
}

// ---------------------------------------------------------------------------
// Chat History & Multi-turn Session Handlers
// ---------------------------------------------------------------------------

async fn get_chats_handler(State(state): State<AppState>) -> impl IntoResponse {
    let list = state.chat.list_sessions().await;
    Json(serde_json::json!({
        "success": true,
        "chats": list
    }))
}

async fn get_chat_by_id_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    if let Some(chat) = state.chat.get_session(&id).await {
        (
            StatusCode::OK,
            Json(serde_json::json!({ "success": true, "chat": chat })),
        )
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({ "success": false, "error": "Чат не найден" })),
        )
    }
}

#[derive(Deserialize)]
struct CreateChatRequest {
    title: Option<String>,
}

async fn post_new_chat_handler(
    State(state): State<AppState>,
    payload: Option<Json<CreateChatRequest>>,
) -> impl IntoResponse {
    let title = payload.and_then(|p| p.0.title);
    let session = state.chat.create_session(title.as_deref()).await;
    Json(serde_json::json!({
        "success": true,
        "chat": session
    }))
}

async fn delete_chat_by_id_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let deleted = state.chat.delete_session(&id).await;
    Json(serde_json::json!({
        "success": deleted,
        "id": id
    }))
}

async fn delete_all_chats_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.chat.clear_all_sessions().await;
    Json(serde_json::json!({
        "success": true,
        "message": "Все чаты успешно удалены"
    }))
}

// ==========================================
// Home Assistant Handlers
// ==========================================

async fn get_hass_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.homeassistant.check_status().await {
        Ok(res) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "data": res })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "status": "error", "error": e })),
        ),
    }
}

async fn get_hass_entities_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.homeassistant.get_entities().await {
        Ok(entities) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "entities": entities })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "status": "error", "error": e })),
        ),
    }
}

async fn post_hass_call_handler(
    State(state): State<AppState>,
    Json(req): Json<HassCallServiceRequest>,
) -> impl IntoResponse {
    match state
        .homeassistant
        .call_service(&req.domain, &req.service, req.service_data)
        .await
    {
        Ok(res) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "result": res })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "status": "error", "error": e })),
        ),
    }
}

async fn post_hass_process_handler(
    State(state): State<AppState>,
    Json(req): Json<HassProcessRequest>,
) -> impl IntoResponse {
    match state.homeassistant.process_conversation(&req.text).await {
        Ok(speech) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "ok", "speech": speech })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "status": "error", "error": e })),
        ),
    }
}

// ============================================================================
// Weather & Timers/Alarms REST Handlers
// ============================================================================

#[derive(Deserialize)]
struct WeatherQuery {
    #[serde(default)]
    city: Option<String>,
}

#[derive(Deserialize)]
struct UpdateWeatherConfigRequest {
    city: Option<String>,
    latitude: Option<f64>,
    longitude: Option<f64>,
    timezone: Option<String>,
}

async fn get_weather_handler(
    State(state): State<AppState>,
    Query(query): Query<WeatherQuery>,
) -> impl IntoResponse {
    let res = if let Some(city) = query.city.as_deref().filter(|s| !s.trim().is_empty()) {
        state.weather.get_weather_for_city(city).await
    } else {
        state.weather.get_current_weather().await
    };

    match res {
        Ok(report) => (StatusCode::OK, Json(serde_json::to_value(report).unwrap_or_else(|_| serde_json::json!({})))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        ),
    }
}

async fn post_weather_config_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateWeatherConfigRequest>,
) -> impl IntoResponse {
    let mut cfg = state.config.write().await;
    if let Some(c) = payload.city {
        cfg.weather.city = c;
    }
    if let Some(lat) = payload.latitude {
        cfg.weather.latitude = lat;
    }
    if let Some(lon) = payload.longitude {
        cfg.weather.longitude = lon;
    }
    if let Some(tz) = payload.timezone {
        cfg.weather.timezone = tz;
    }
    save_app_config(&*cfg).await;
    let saved_weather = cfg.weather.clone();
    drop(cfg);

    (StatusCode::OK, Json(serde_json::json!({ "status": "ok", "weather": saved_weather })))
}

// ----------------- Timers -----------------

#[derive(Deserialize)]
struct CreateTimerRequest {
    #[serde(alias = "duration_secs")]
    duration: u64,
    #[serde(default)]
    label: Option<String>,
}

async fn get_timers_handler(State(state): State<AppState>) -> impl IntoResponse {
    let timers = state.timer_alarm.list_timers().await;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let items: Vec<serde_json::Value> = timers
        .into_iter()
        .map(|t| {
            let rem = t.remaining_secs(now);
            serde_json::json!({
                "id": t.id,
                "label": t.label,
                "duration_secs": t.duration_secs,
                "remaining_secs": rem,
                "created_at": t.created_at,
                "ends_at": t.ends_at,
                "is_ringing": t.is_ringing,
            })
        })
        .collect();

    Json(serde_json::json!({ "timers": items }))
}

async fn post_timer_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateTimerRequest>,
) -> impl IntoResponse {
    let timer = state.timer_alarm.add_timer(payload.duration, payload.label.as_deref()).await;
    (StatusCode::CREATED, Json(serde_json::to_value(timer).unwrap_or_else(|_| serde_json::json!({}))))
}

async fn cancel_all_timers_handler(State(state): State<AppState>) -> impl IntoResponse {
    let count = state.timer_alarm.cancel_all_timers().await;
    Json(serde_json::json!({ "status": "ok", "cancelled": count }))
}

async fn delete_timer_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let success = state.timer_alarm.cancel_timer(&id).await;
    Json(serde_json::json!({ "status": if success { "ok" } else { "not_found" } }))
}

// ----------------- Alarms -----------------

#[derive(Deserialize)]
struct CreateAlarmRequest {
    time: String,
    #[serde(default)]
    label: Option<String>,
    #[serde(default)]
    repeat: Option<String>,
}

#[derive(Deserialize)]
struct UpdateAlarmRequest {
    enabled: bool,
}

async fn get_alarms_handler(State(state): State<AppState>) -> impl IntoResponse {
    let alarms = state.timer_alarm.list_alarms().await;
    Json(serde_json::json!({ "alarms": alarms }))
}

async fn post_alarm_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateAlarmRequest>,
) -> impl IntoResponse {
    match state.timer_alarm.add_alarm(&payload.time, payload.label.as_deref(), payload.repeat.as_deref()).await {
        Ok(alarm) => (StatusCode::CREATED, Json(serde_json::to_value(alarm).unwrap_or_else(|_| serde_json::json!({})))),
        Err(e) => (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": e }))),
    }
}

async fn put_alarm_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
    Json(payload): Json<UpdateAlarmRequest>,
) -> impl IntoResponse {
    let success = state.timer_alarm.toggle_alarm(&id, payload.enabled).await;
    Json(serde_json::json!({ "status": if success { "ok" } else { "not_found" } }))
}

async fn delete_alarm_handler(
    State(state): State<AppState>,
    AxumPath(id): AxumPath<String>,
) -> impl IntoResponse {
    let success = state.timer_alarm.delete_alarm(&id).await;
    Json(serde_json::json!({ "status": if success { "ok" } else { "not_found" } }))
}

async fn post_alarm_stop_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.timer_alarm.stop_ringing();
    Json(serde_json::json!({ "status": "ok" }))
}

// ==============================================================================
// STREAMING SINKS (AirPlay, Spotify Connect, Snapcast)
// ==============================================================================

async fn save_app_config(cfg: &AppConfig) {
    let save_path = if Path::new("/data").exists() {
        "/data/config.json"
    } else {
        "config.json"
    };
    if let Ok(json_str) = serde_json::to_string_pretty(cfg) {
        let _ = tokio::fs::write(save_path, json_str).await;
    }
}

async fn get_streaming_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.streaming.get_status().await;
    Json(status)
}

#[derive(Deserialize)]
struct UpdateStreamingConfigRequest {
    #[serde(default)]
    pub airplay_enabled: Option<bool>,
    #[serde(default)]
    pub airplay_name: Option<String>,
    #[serde(default)]
    pub spotify_enabled: Option<bool>,
    #[serde(default)]
    pub spotify_name: Option<String>,
    #[serde(default)]
    pub snapcast_enabled: Option<bool>,
    #[serde(default)]
    pub snapcast_server: Option<String>,
    #[serde(default)]
    pub snapcast_port: Option<u16>,
    #[serde(default)]
    pub bluetooth_sink_enabled: Option<bool>,
}

async fn post_streaming_config_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateStreamingConfigRequest>,
) -> impl IntoResponse {
    {
        let mut cfg = state.config.write().await;
        let s = &mut cfg.streaming;
        if let Some(v) = payload.airplay_enabled { s.airplay_enabled = v; }
        if let Some(v) = payload.airplay_name { s.airplay_name = v; }
        if let Some(v) = payload.spotify_enabled { s.spotify_enabled = v; }
        if let Some(v) = payload.spotify_name { s.spotify_name = v; }
        if let Some(v) = payload.snapcast_enabled { s.snapcast_enabled = v; }
        if let Some(v) = payload.snapcast_server { s.snapcast_server = v; }
        if let Some(v) = payload.snapcast_port { s.snapcast_port = v; }
        if let Some(v) = payload.bluetooth_sink_enabled { s.bluetooth_sink_enabled = v; }
        save_app_config(&*cfg).await;
    }
    state.streaming.sync_services().await;

    let status = state.streaming.get_status().await;
    Json(serde_json::json!({
        "success": true,
        "status": status,
    }))
}

async fn restart_streaming_handler(State(state): State<AppState>) -> impl IntoResponse {
    state.streaming.restart_services().await;
    let status = state.streaming.get_status().await;
    Json(serde_json::json!({
        "success": true,
        "status": status,
    }))
}

// ==============================================================================
// BLUETOOTH A2DP SINK
// ==============================================================================

async fn enable_bt_sink_handler(State(state): State<AppState>) -> impl IntoResponse {
    {
        let mut cfg = state.config.write().await;
        cfg.streaming.bluetooth_sink_enabled = true;
        save_app_config(&*cfg).await;
    }
    state.streaming.sync_services().await;
    Json(serde_json::json!({
        "success": true,
        "discoverable": true,
        "message": "Bluetooth включен в режим обнаружения (A2DP Sink). Выберите 'Yandex Station' на телефоне.",
    }))
}

async fn disable_bt_sink_handler(State(state): State<AppState>) -> impl IntoResponse {
    {
        let mut cfg = state.config.write().await;
        cfg.streaming.bluetooth_sink_enabled = false;
        save_app_config(&*cfg).await;
    }
    state.streaming.sync_services().await;
    Json(serde_json::json!({
        "success": true,
        "discoverable": false,
    }))
}

// ==============================================================================
// SOUND PACKS
// ==============================================================================

async fn get_sound_packs_handler(State(state): State<AppState>) -> impl IntoResponse {
    let active_pack = {
        let cfg = state.config.read().await;
        cfg.soundpack.active_pack.clone()
    };
    let packs = list_available_packs();
    let packs_json: Vec<_> = packs.into_iter().map(|p| {
        serde_json::json!({
            "id": p.id,
            "name": p.name,
            "description": p.description,
            "active": p.id == active_pack,
        })
    }).collect();

    Json(serde_json::json!({
        "active_pack": active_pack,
        "packs": packs_json,
    }))
}

#[derive(Deserialize)]
struct SetSoundPackRequest {
    pack: String,
}

async fn set_active_sound_pack_handler(
    State(state): State<AppState>,
    Json(payload): Json<SetSoundPackRequest>,
) -> impl IntoResponse {
    {
        let mut cfg = state.config.write().await;
        cfg.soundpack.active_pack = payload.pack.clone();
        save_app_config(&*cfg).await;
    }

    let card = {
        let cfg = state.config.read().await;
        cfg.audio.alsa_card.clone()
    };
    let pack = payload.pack.clone();
    tokio::task::spawn_blocking(move || {
        play_sound(&pack, "activation", &card);
    });

    Json(serde_json::json!({
        "success": true,
        "active_pack": payload.pack,
    }))
}

#[derive(Deserialize)]
struct PreviewSoundQuery {
    #[serde(default = "default_preview_pack")]
    pack: String,
    #[serde(default = "default_preview_type")]
    sound: String,
}
fn default_preview_pack() -> String { "classic".to_string() }
fn default_preview_type() -> String { "activation".to_string() }

async fn preview_sound_handler(
    Query(query): Query<PreviewSoundQuery>,
) -> impl IntoResponse {
    let wav_bytes = synthesize_sound(&query.pack, &query.sound);
    (
        [
            (axum::http::header::CONTENT_TYPE, "audio/wav"),
            (axum::http::header::CACHE_CONTROL, "no-cache"),
        ],
        wav_bytes,
    )
}

// ==============================================================================
// HOME ASSISTANT MEDIA PLAYER API
// ==============================================================================

#[derive(Deserialize)]
struct HaPlayMediaRequest {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    query: Option<String>,
}

async fn media_player_play_handler(
    State(state): State<AppState>,
    Json(payload): Json<HaPlayMediaRequest>,
) -> axum::response::Response {
    state.tts.stop(&state.led);

    if let Some(url) = payload.url {
        let trimmed = url.trim();
        // SEC-02: Validate stream URL scheme (strictly http:// or https://, no newlines, no local file://)
        if (!trimmed.starts_with("http://") && !trimmed.starts_with("https://"))
            || trimmed.contains('\n')
            || trimmed.contains('\r')
        {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "success": false,
                    "error": "Only HTTP/HTTPS media stream URLs are permitted"
                })),
            )
                .into_response();
        }
        let title = payload.title.as_deref().unwrap_or("Стрим");
        let artist = payload.artist.as_deref().unwrap_or("Home Assistant");
        state.player.play_direct_stream(trimmed, title, artist, None, 0, false, 0, &state.led).await;
    } else if let Some(query) = payload.query {
        state.player.play_smart_or_query(&state.ytmusic, &state.soundcloud, &query, &state.led).await;
    }

    let ha = state.homeassistant.clone();
    let player = state.player.clone();
    let vol = state.audio.get_volume() as f32 / 10.0;
    tokio::spawn(async move {
        let (st, track, artist, art, dur, pos) = player.get_snapshot().await;
        let _ = ha.sync_media_player_state(&st, vol, &track, &artist, art.as_deref(), dur, pos).await;
    });

    (StatusCode::OK, Json(serde_json::json!({ "success": true, "state": "playing" }))).into_response()
}

async fn sync_ha_media_player_handler(State(state): State<AppState>) -> impl IntoResponse {
    let ha = state.homeassistant.clone();
    let player = state.player.clone();
    let vol = state.audio.get_volume() as f32 / 10.0;
    let (st, track, artist, art, dur, pos) = player.get_snapshot().await;
    let res = ha.sync_media_player_state(&st, vol, &track, &artist, art.as_deref(), dur, pos).await;

    match res {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({ "success": true, "synced": true }))),
        Err(e) => (StatusCode::BAD_GATEWAY, Json(serde_json::json!({ "success": false, "error": e }))),
    }
}



