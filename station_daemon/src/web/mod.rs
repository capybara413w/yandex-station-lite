use crate::config::{
    AppConfig, AssistantConfigResponse, AssistantStageInfo, UpdateAssistantConfigPayload,
};
use crate::hardware::{AudioHardware, LedController, NetworkHardware, StationButtonEvent};
use crate::services::{
    DeepSeekService, PlaybackState, PlayerService, SoundCloudService, SttService, TtsService,
    VisualizerService, YtMusicService,
};
use axum::{
    extract::{Path as AxumPath, Query, State},
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
        .route("/ask", get(get_ask_handler))
        // Config management
        .route("/api/config", get(get_config_handler).post(post_config_handler))
        .route(
            "/api/config/assistant",
            get(get_assistant_config_handler).post(post_assistant_config_handler),
        )
        // Logs
        .route("/api/logs", get(get_logs_handler))
        .route("/api/logs/system", get(get_logs_handler))
        .route("/api/logs/system/clear", post(clear_logs_handler))
        // Wi-Fi Management
        .route("/api/wifi/status", get(get_wifi_status_handler))
        .route("/api/wifi/scan", get(scan_wifi_handler).post(scan_wifi_handler))
        .route("/api/wifi/connect", post(connect_wifi_handler))
        .route("/api/wifi/restart", post(restart_wifi_handler))
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
            "/api/bluetooth/scan",
            get(scan_bluetooth_handler).post(scan_bluetooth_handler),
        )
        .route("/api/bluetooth/connect", post(connect_bluetooth_handler))
        .route(
            "/api/bluetooth/disconnect",
            post(disconnect_bluetooth_handler),
        )
        // Terminal command execution
        .route("/api/terminal/run", post(terminal_run_handler))
        .route("/api/command", post(terminal_run_handler))
        // Static assets fallback if /data/station_web exists
        .nest_service("/static", ServeDir::new(static_dir))
        .layer(CorsLayer::permissive())
        .with_state(state)
}

/// Serves the web control panel:
/// The rich Material 3 web UI is compiled directly into the binary (EMBEDDED_HTML).
/// This ensures binary updates on the station immediately reflect in the browser without stale disk cache.
async fn index_handler(State(_state): State<AppState>) -> impl IntoResponse {
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
    match state.soundcloud.search_multi_source(q, &source, limit).await {
        Ok(tracks) => Json(serde_json::json!({
            "success": true,
            "query": q,
            "source": source,
            "results": tracks,
        }))
        .into_response(),
        Err(e) => {
            warn!("Music search for '{}' (source: {}) failed: {}", q, source, e);
            Json(serde_json::json!({
                "success": false,
                "error": e,
                "source": source,
                "results": [],
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

async fn ytmusic_play_handler(
    State(state): State<AppState>,
    Json(payload): Json<YtPlayRequest>,
) -> impl IntoResponse {
    state.tts.stop(&state.led);

    let vid = payload.video_id.trim();
    if vid.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({
            "success": false,
            "error": "Empty video_id",
        }))).into_response();
    }

    let title = if payload.title.is_empty() { "YouTube Music Track".to_string() } else { payload.title };
    let artist = if payload.artist.is_empty() { "YouTube Music".to_string() } else { payload.artist };

    match state.ytmusic.resolve_stream_url(vid).await {
        Ok(stream_url) => {
            let dur_secs = payload.duration_seconds.unwrap_or(210);
            state.player.play_direct_stream(
                &stream_url,
                &title,
                &artist,
                payload.artwork_url.clone(),
                dur_secs,
                false,
                0,
                &state.led,
            ).await;

            state.logs.push("INFO", &format!("YouTube Music: запуск '{}' ({})", title, vid)).await;

            (StatusCode::OK, Json(serde_json::json!({
                "success": true,
                "title": title,
                "artist": artist,
                "stream_url": stream_url,
            }))).into_response()
        }
        Err(e) => {
            warn!("YouTube Music direct resolve error for {}: {}. Attempting audio stream fallback...", vid, e);
            match state.soundcloud.resolve_best_match(&title, &artist, payload.duration_seconds).await {
                Ok(resolved) => {
                    let dur_secs = if resolved.duration_ms > 0 {
                        resolved.duration_ms / 1000
                    } else {
                        payload.duration_seconds.unwrap_or(210)
                    };
                    state.player.play_direct_stream(
                        &resolved.stream_url,
                        &title,
                        &artist,
                        payload.artwork_url.clone().or(resolved.artwork_url),
                        dur_secs,
                        resolved.is_hls,
                        0,
                        &state.led,
                    ).await;

                    state.logs.push("INFO", &format!("YouTube Music: запуск '{}' ({})", title, vid)).await;
                    (StatusCode::OK, Json(serde_json::json!({
                        "success": true,
                        "title": title,
                        "artist": artist,
                        "stream_url": resolved.stream_url,
                        "fallback": true,
                    }))).into_response()
                }
                Err(sc_err) => {
                    error!("YouTube Music resolve error for {}: {} (fallback: {})", vid, e, sc_err);
                    state.logs.push("ERROR", &format!("YouTube Music error for {}: {}", vid, e)).await;
                    (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({
                        "success": false,
                        "error": format!("YT Music error: {}. Fallback error: {}", e, sc_err),
                    }))).into_response()
                }
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
            if !k_clean.is_empty() && !k_clean.starts_with('*') {
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
                } else {
                    cfg.stt.model = m_clean.to_string();
                }
            }
        }
        if let Some(ref k) = stt.api_key {
            let k_clean = k.trim();
            if !k_clean.is_empty() && !k_clean.starts_with('*') {
                if cfg.stt.provider == "openrouter" {
                    cfg.stt.openrouter_api_key = k_clean.to_string();
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
            if !k_clean.is_empty() && !k_clean.starts_with('*') {
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
    socks5_enabled: Option<bool>,
    #[serde(default)]
    proxy_socks5_url: Option<String>,
    #[serde(default)]
    socks5_url: Option<String>,
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
        if !tok.trim().starts_with('*') {
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
    if let Some(p_en) = payload.socks5_enabled.or(payload.proxy_enabled) {
        cfg.proxy.enabled = p_en;
    }
    if let Some(p_url) = payload.socks5_url.or(payload.proxy_socks5_url) {
        let trimmed = p_url.trim().to_string();
        if !trimmed.is_empty() {
            cfg.proxy.socks5_url = trimmed;
        }
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

// ---------------------------------------------------------------------------
// Wi-Fi Web Handlers
// ---------------------------------------------------------------------------

async fn get_wifi_status_handler(State(state): State<AppState>) -> impl IntoResponse {
    let status = state.network.get_wifi_status();
    Json(status)
}

async fn scan_wifi_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.network.scan_wifi() {
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

async fn scan_bluetooth_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.network.scan_bluetooth() {
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
    match state.network.connect_bluetooth(&payload.address) {
        Ok(out) => {
            state.logs.push("INFO", &format!("Connected to Bluetooth device {}", payload.address)).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "output": out })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
        ),
    }
}

async fn disconnect_bluetooth_handler(
    State(state): State<AppState>,
    Json(payload): Json<BtDeviceActionPayload>,
) -> impl IntoResponse {
    match state.network.disconnect_bluetooth(&payload.address) {
        Ok(out) => {
            state.logs.push("INFO", &format!("Disconnected from Bluetooth device {}", payload.address)).await;
            (
                StatusCode::OK,
                Json(serde_json::json!({ "success": true, "output": out })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "success": false, "error": e })),
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
    State(_state): State<AppState>,
    Json(payload): Json<TerminalRunPayload>,
) -> impl IntoResponse {
    let out = std::process::Command::new("sh")
        .args(["-c", &payload.command])
        .output();

    match out {
        Ok(o) => {
            let stdout = String::from_utf8_lossy(&o.stdout).to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).to_string();
            Json(serde_json::json!({
                "success": o.status.success(),
                "exit_code": o.status.code().unwrap_or(-1),
                "stdout": stdout,
                "stderr": stderr,
                "output": if stdout.is_empty() { stderr } else { stdout },
            }))
        }
        Err(e) => Json(serde_json::json!({
            "success": false,
            "exit_code": -1,
            "stdout": "",
            "stderr": e.to_string(),
            "output": format!("Execution error: {}", e),
        })),
    }
}
