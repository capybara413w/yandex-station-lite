use crate::config::AppConfig;
use crate::hardware::{AudioHardware, LedController, StationButtonEvent};
use crate::services::{
    DeepSeekService, PlaybackState, PlayerService, SoundCloudService, SttService, TtsService,
    VisualizerService,
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse},
    routing::{get, post},
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
use tracing::{error, warn};

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
    pub deepseek: Arc<DeepSeekService>,
    pub player: Arc<PlayerService>,
    pub soundcloud: Arc<SoundCloudService>,
    pub stt: Arc<SttService>,
    pub tts: Arc<TtsService>,
    pub visualizer: Arc<VisualizerService>,
    pub btn_tx: mpsc::Sender<StationButtonEvent>,
    pub logs: Arc<LogBuffer>,
    pub start_time: Instant,
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
        // Volume
        .route("/api/volume", post(set_volume_handler))
        .route("/api/player/volume", post(set_volume_handler))
        // Player actions
        .route("/api/player/action", post(player_action_handler))
        .route("/api/player/play_song", post(play_song_handler))
        .route("/api/player/seek", post(seek_handler))
        // SoundCloud integration
        .route("/api/soundcloud/search", get(soundcloud_search_handler))
        .route("/api/soundcloud/play", post(soundcloud_play_handler))
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
        .route("/ask", get(get_ask_handler))
        // Config management
        .route("/api/config", get(get_config_handler).post(post_config_handler))
        // Logs
        .route("/api/logs", get(get_logs_handler))
        .route("/api/logs/system", get(get_logs_handler))
        .route("/api/logs/system/clear", post(clear_logs_handler))
        // Static assets fallback if /data/station_web exists
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Serves the web control panel:
/// If /data/station_web/index.html exists, serves it; otherwise serves the rich embedded UI.
async fn index_handler(State(state): State<AppState>) -> impl IntoResponse {
    let cfg = state.config.read().await;
    let custom_index = Path::new(&cfg.server.web_static_dir).join("index.html");
    if custom_index.exists() {
        if let Ok(content) = tokio::fs::read_to_string(&custom_index).await {
            return Html(content);
        }
    }
    Html(EMBEDDED_HTML.to_string())
}

async fn health_handler() -> impl IntoResponse {
    let local_ip = get_local_ip();
    let (ram_used_mb, ram_total_mb) = get_mem_stats();
    Json(serde_json::json!({
        "status": "ok",
        "service": "Yandex Station Lite Autonomous Daemon",
        "arch": "aarch64",
        "target": "Amlogic AXG A113X (238MB RAM)",
        "station_ip": local_ip,
        "ram_used_mb": ram_used_mb,
        "ram_total_mb": ram_total_mb,
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
    let uptime_secs = state.start_time.elapsed().as_secs();

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

    state.player.play_soundcloud_or_query(&state.soundcloud, query, &state.led).await;
    state
        .logs
        .push("INFO", &format!("Web: SoundCloud запуск трека '{}'", query))
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
struct SoundCloudSearchQuery {
    q: String,
    #[serde(default)]
    limit: Option<usize>,
}

async fn soundcloud_search_handler(
    State(state): State<AppState>,
    Query(params): Query<SoundCloudSearchQuery>,
) -> impl IntoResponse {
    let q = params.q.trim();
    if q.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": "Empty search query" })),
        )
            .into_response();
    }

    let limit = params.limit.unwrap_or(10);
    match state.soundcloud.search_tracks(q, limit).await {
        Ok(tracks) => Json(serde_json::json!({
            "success": true,
            "query": q,
            "results": tracks,
        }))
        .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
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
    let text_clone = text.to_string();

    tokio::spawn(async move {
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
}

async fn post_ask_handler(
    State(state): State<AppState>,
    Json(payload): Json<AskRequest>,
) -> impl IntoResponse {
    handle_prompt(&state, &payload.prompt, payload.speak.unwrap_or(true)).await
}

#[derive(Deserialize)]
struct AskQuery {
    q: String,
}

async fn get_ask_handler(
    State(state): State<AppState>,
    Query(query): Query<AskQuery>,
) -> impl IntoResponse {
    handle_prompt(&state, &query.q, true).await
}

async fn handle_prompt(
    state: &AppState,
    prompt_text: &str,
    speak: bool,
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

    match state.deepseek.query(prompt).await {
        Ok(resp) => {
            state.led.set_rgb(resp.color[0], resp.color[1], resp.color[2]);

            state
                .logs
                .push(
                    "INFO",
                    &format!(
                        "DeepSeek ответ: '{}' (действие: {})",
                        resp.text, resp.action
                    ),
                )
                .await;

            if speak && !resp.text.is_empty() {
                let tts = state.tts.clone();
                let led = state.led.clone();
                let text = resp.text.clone();
                let color = resp.color;
                tokio::spawn(async move {
                    if let Err(e) = tts.speak(&text, color, &led).await {
                        error!("TTS speak error: {}", e);
                    }
                });
            }

            match resp.action.as_str() {
                "play" => {
                    let song_name = if resp.song.is_empty() {
                        "Lofi chill beats".to_string()
                    } else {
                        resp.song.clone()
                    };
                    let sc = state.soundcloud.clone();
                    let pl = state.player.clone();
                    let l = state.led.clone();
                    tokio::spawn(async move {
                        pl.play_soundcloud_or_query(&sc, &song_name, &l).await;
                    });
                }
                "stop" => state.player.stop(&state.led).await,
                "resume" => state.player.resume(&state.led).await,
                _ => {}
            }

            (StatusCode::OK, Json(serde_json::to_value(resp).unwrap()))
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
                    "error": e
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
    Json(cfg)
}

fn mask_key(k: &str) -> String {
    if k.len() > 8 {
        format!("{}...{}", &k[..4], &k[k.len() - 4..])
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
    proxy_socks5_url: Option<String>,
}

async fn post_config_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateConfigRequest>,
) -> impl IntoResponse {
    let mut cfg = state.config.write().await;

    if let Some(prov) = payload.llm_provider {
        if !prov.trim().is_empty() {
            cfg.deepseek.provider = prov.trim().to_lowercase();
        }
    }
    if let Some(key) = payload.openrouter_api_key {
        let trimmed = key.trim();
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
        if !trimmed.is_empty() && !trimmed.starts_with('*') {
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
    if let Some(p_en) = payload.proxy_enabled {
        cfg.proxy.enabled = p_en;
    }
    if let Some(p_url) = payload.proxy_socks5_url {
        cfg.proxy.socks5_url = p_url.trim().to_string();
    }

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
