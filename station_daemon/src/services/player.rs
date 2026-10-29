use crate::hardware::LedController;
use crate::services::soundcloud::SoundCloudService;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlaybackState {
    Playing,
    Paused,
    Stopped,
}

fn get_mpg123_bin() -> &'static str {
    if Path::new("/opt/bin/mpg123").exists() {
        "/opt/bin/mpg123"
    } else if Path::new("/data/opt/bin/mpg123").exists() {
        "/data/opt/bin/mpg123"
    } else if Path::new("/usr/bin/mpg123").exists() {
        "/usr/bin/mpg123"
    } else {
        "mpg123"
    }
}

fn get_ffmpeg_bin() -> Option<&'static str> {
    if Path::new("/opt/bin/ffmpeg").exists() {
        Some("/opt/bin/ffmpeg")
    } else if Path::new("/data/opt/bin/ffmpeg").exists() {
        Some("/data/opt/bin/ffmpeg")
    } else if Path::new("/usr/bin/ffmpeg").exists() {
        Some("/usr/bin/ffmpeg")
    } else if Path::new("/bin/ffmpeg").exists() {
        Some("/bin/ffmpeg")
    } else if Command::new("ffmpeg")
        .arg("-version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        Some("ffmpeg")
    } else {
        None
    }
}

pub struct PlayerService {
    state: Arc<RwLock<PlaybackState>>,
    current_track: Arc<RwLock<String>>,
    current_artist: Arc<RwLock<String>>,
    current_artwork: Arc<RwLock<Option<String>>>,
    current_stream_url: Arc<RwLock<String>>,
    current_duration_secs: Arc<RwLock<u64>>,
    base_offset_secs: Arc<RwLock<u64>>,
    playback_start: Arc<RwLock<Option<Instant>>>,
    is_hls: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    is_busy: Arc<AtomicBool>,
}

impl PlayerService {
    pub fn new() -> Self {
        let _ = fs::remove_file("/tmp/music_playing");
        let _ = fs::remove_file("/tmp/music_paused");

        Self {
            state: Arc::new(RwLock::new(PlaybackState::Stopped)),
            current_track: Arc::new(RwLock::new("Ожидание команды...".to_string())),
            current_artist: Arc::new(RwLock::new(String::new())),
            current_artwork: Arc::new(RwLock::new(None)),
            current_stream_url: Arc::new(RwLock::new(String::new())),
            current_duration_secs: Arc::new(RwLock::new(0)),
            base_offset_secs: Arc::new(RwLock::new(0)),
            playback_start: Arc::new(RwLock::new(None)),
            is_hls: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            is_busy: Arc::new(AtomicBool::new(false)),
        }
    }

    pub async fn get_state(&self) -> PlaybackState {
        *self.state.read().await
    }

    pub async fn get_current_track(&self) -> String {
        self.current_track.read().await.clone()
    }

    pub async fn get_current_artist(&self) -> String {
        self.current_artist.read().await.clone()
    }

    pub async fn get_current_artwork(&self) -> Option<String> {
        self.current_artwork.read().await.clone()
    }

    /// Returns (position_secs, duration_secs) of the currently playing track
    pub async fn get_position_and_duration(&self) -> (u64, u64) {
        let state = *self.state.read().await;
        let base = *self.base_offset_secs.read().await;
        let duration = *self.current_duration_secs.read().await;

        let pos = match state {
            PlaybackState::Playing => {
                if let Some(start) = *self.playback_start.read().await {
                    base + start.elapsed().as_secs()
                } else {
                    base
                }
            }
            PlaybackState::Paused => base,
            PlaybackState::Stopped => 0,
        };

        let clamped_pos = if duration > 0 {
            pos.min(duration)
        } else {
            pos
        };

        (clamped_pos, duration)
    }

    /// Cleanly kills running playback processes and allows the Linux ALSA driver
    /// 80ms to completely release the hw:0,0 hardware PCM device.
    async fn clean_stop_current_process(&self) {
        info!("Stopping current audio decoders (SIGTERM -> SIGKILL)...");

        // 1. Send SIGTERM first so decoders can close audio devices cleanly
        let _ = Command::new("killall")
            .args(["-TERM", "mpg123", "ffmpeg"])
            .output();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // 2. Send SIGKILL to ensure no zombie or hung processes remain
        let _ = Command::new("killall")
            .args(["-9", "mpg123", "ffmpeg", "play.sh"])
            .output();

        let _ = fs::remove_file("/tmp/music_paused");
        let _ = fs::remove_file("/tmp/music_playing");

        // 3. Essential ALSA hardware drain pause (80ms):
        // Prevents ALSA "Device or resource busy" (EBUSY) when the next decoder opens hw:0,0
        tokio::time::sleep(std::time::Duration::from_millis(80)).await;
    }

    /// Fast seek to specific second offset in the currently playing track
    pub async fn seek(&self, target_secs: u64, led: &LedController) {
        let url = self.current_stream_url.read().await.clone();
        if url.is_empty() {
            warn!("Seek requested but no stream URL is active");
            return;
        }
        let title = self.current_track.read().await.clone();
        let artist = self.current_artist.read().await.clone();
        let artwork = self.current_artwork.read().await.clone();
        let duration = *self.current_duration_secs.read().await;
        let is_hls = self.is_hls.load(Ordering::Relaxed);

        let clamped_target = if duration > 0 {
            target_secs.min(duration.saturating_sub(1))
        } else {
            target_secs
        };

        info!(
            "Seeking to {}s (duration: {}s) for track '{}'",
            clamped_target, duration, title
        );

        // 1. Increment generation counter so previous monitor threads ignore exit signals
        let new_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // 2. Clean restart: terminate existing processes and release ALSA hw:0,0
        self.clean_stop_current_process().await;

        // 3. Update base_offset_secs and reset playback_start
        {
            let mut base_guard = self.base_offset_secs.write().await;
            *base_guard = clamped_target;
            let mut start_guard = self.playback_start.write().await;
            *start_guard = Some(Instant::now());
            let mut state_guard = self.state.write().await;
            *state_guard = PlaybackState::Playing;
        }

        // 4. Launch direct playback from the new offset
        self.spawn_playback_process(
            &url,
            &title,
            &artist,
            artwork,
            duration,
            is_hls,
            clamped_target,
            new_gen,
            led,
        )
        .await;
    }

    /// Resolves SoundCloud track or preset stream and begins playback
    pub async fn play_soundcloud_or_query(
        &self,
        soundcloud: &SoundCloudService,
        query: &str,
        led: &LedController,
    ) {
        let q = query.trim();
        if q.is_empty() {
            return;
        }

        // 1. Check if it's already an HTTP stream URL or well-known radio preset
        if q.starts_with("http://") || q.starts_with("https://") {
            self.play_direct_stream(q, q, "", None, 0, false, 0, led).await;
            return;
        }

        let q_lower = q.to_lowercase();
        let preset_url = match q_lower.as_str() {
            "radio record" | "record" | "радио рекорд" | "рекорд" => {
                Some(("http://air2.radiorecord.ru:805/rr_320", "Radio Record", "Record Dance"))
            }
            "lofi" | "lofi chill" | "лофай" => {
                Some(("http://stream.zeno.fm/f3wvbbqmdg8uv", "Lofi Chill Beats", "Lofi 24/7"))
            }
            "deep house" | "deep" | "дип хаус" => {
                Some(("http://air2.radiorecord.ru:805/deep_320", "Deep House", "Radio Record"))
            }
            "relax fm" | "relax" | "релакс" => {
                Some(("http://ic7.101.ru:8000/v13_1", "Relax FM", "Relax Media"))
            }
            "rock" | "classic rock" | "рок" => {
                Some(("http://air2.radiorecord.ru:805/rock_320", "Rock Station", "Radio Record"))
            }
            _ => None,
        };

        if let Some((url, title, artist)) = preset_url {
            self.play_direct_stream(url, title, artist, None, 0, false, 0, led).await;
            return;
        }

        // 2. Query SoundCloud
        info!("Resolving query on SoundCloud: '{}'", q);
        led.blink(255, 120, 0, 150); // Orange blink indicator for searching

        match soundcloud.resolve_best_stream(q).await {
            Ok(resolved) => {
                let dur_secs = resolved.duration_ms / 1000;
                info!(
                    "Found SoundCloud track: '{}' by '{}' (HLS: {}, duration: {}s)",
                    resolved.track_title, resolved.artist, resolved.is_hls, dur_secs
                );
                self.play_direct_stream(
                    &resolved.stream_url,
                    &resolved.track_title,
                    &resolved.artist,
                    resolved.artwork_url,
                    dur_secs,
                    resolved.is_hls,
                    0,
                    led,
                )
                .await;
            }
            Err(e) => {
                warn!("SoundCloud resolution failed for '{}': {}", q, e);
                // Fallback to lofi stream
                self.play_direct_stream(
                    "http://stream.zeno.fm/f3wvbbqmdg8uv",
                    q,
                    "SoundCloud fallback stream",
                    None,
                    0,
                    false,
                    0,
                    led,
                )
                .await;
            }
        }
    }

    /// Starts streaming the audio URL using the proven pipeline with full logging to terminal
    pub async fn play_direct_stream(
        &self,
        stream_url: &str,
        track_title: &str,
        artist: &str,
        artwork_url: Option<String>,
        duration_secs: u64,
        is_hls: bool,
        offset_secs: u64,
        led: &LedController,
    ) {
        let current_gen = self.generation.fetch_add(1, Ordering::SeqCst) + 1;

        // Atomically terminate any previous audio process to cleanly release ALSA hw:0,0
        self.clean_stop_current_process().await;

        {
            let mut track_guard = self.current_track.write().await;
            *track_guard = track_title.to_string();
            let mut artist_guard = self.current_artist.write().await;
            *artist_guard = artist.to_string();
            let mut art_guard = self.current_artwork.write().await;
            *art_guard = artwork_url.clone();
            let mut url_guard = self.current_stream_url.write().await;
            *url_guard = stream_url.to_string();
            let mut dur_guard = self.current_duration_secs.write().await;
            *dur_guard = duration_secs;
            let mut base_guard = self.base_offset_secs.write().await;
            *base_guard = offset_secs;
            let mut start_guard = self.playback_start.write().await;
            *start_guard = Some(Instant::now());
            self.is_hls.store(is_hls, Ordering::Relaxed);
            let mut state_guard = self.state.write().await;
            *state_guard = PlaybackState::Playing;
        }

        self.spawn_playback_process(
            stream_url,
            track_title,
            artist,
            artwork_url,
            duration_secs,
            is_hls,
            offset_secs,
            current_gen,
            led,
        )
        .await;
    }

    /// Spawns audio decoding pipeline with direct terminal output (stdout/stderr inherit)
    /// and automatic fallback for HLS seeking
    async fn spawn_playback_process(
        &self,
        stream_url: &str,
        track_title: &str,
        artist: &str,
        _artwork_url: Option<String>,
        duration_secs: u64,
        is_hls: bool,
        offset_secs: u64,
        current_gen: u64,
        led: &LedController,
    ) {
        let _ = fs::write("/tmp/music_playing", "");
        let _ = fs::remove_file("/tmp/music_paused");

        // Read volume step (1-10) -> scale = VOL * 1000 (clamp 500..=12000)
        let vol_step = fs::read_to_string("/data/volume")
            .ok()
            .and_then(|s| s.trim().parse::<u32>().ok())
            .unwrap_or(6)
            .clamp(1, 10);
        let scale = (vol_step * 1000).clamp(500, 12000);

        info!(
            "Starting SoundCloud/Audio playback: '{}' - '{}' (scale: {}, is_hls: {}, offset: {}s, duration: {}s, gen: {})",
            artist, track_title, scale, is_hls, offset_secs, duration_secs, current_gen
        );

        led.set_music_aura();

        let url = stream_url.to_string();
        let mpg123_bin = get_mpg123_bin();
        let ffmpeg_bin_opt = get_ffmpeg_bin();
        let state_clone = self.state.clone();
        let gen_clone = self.generation.clone();
        let _is_hls_stream = is_hls
            || url.contains(".m3u8")
            || url.contains("/stream/hls")
            || url.contains("mpegurl");

        tokio::task::spawn_blocking(move || {
            let spawn_start = Instant::now();
            let safe_url = url.replace('\'', "%27");

            // Build audio pipeline
            // Note: On SoundCloud HLS (fragmented MP4 .m4s), passing -ss before -i causes ffmpeg to skip
            // Build audio pipeline with simultaneous lightweight 8000Hz PCM UDP feed for 3-band LED visualizer
            let pipeline_cmd = if let Some(ffmpeg_bin) = ffmpeg_bin_opt {
                if offset_secs > 0 {
                    format!(
                        "{} -nostdin -loglevel info -nostats -i '{}' -ss {} -f mp3 - -f s16le -ac 1 -ar 16000 'udp://127.0.0.1:7777?pkt_size=512' | {} -v -a hw:0,0 -f {} -",
                        ffmpeg_bin, safe_url, offset_secs, mpg123_bin, scale
                    )
                } else {
                    format!(
                        "{} -nostdin -loglevel info -nostats -i '{}' -f mp3 - -f s16le -ac 1 -ar 16000 'udp://127.0.0.1:7777?pkt_size=512' | {} -v -a hw:0,0 -f {} -",
                        ffmpeg_bin, safe_url, mpg123_bin, scale
                    )
                }
            } else {
                let frame_arg = if offset_secs > 0 {
                    format!("-k {} ", (offset_secs as f64 * 38.28) as u64)
                } else {
                    String::new()
                };
                format!(
                    "{} -v -a hw:0,0 {}-f {} '{}'",
                    mpg123_bin, frame_arg, scale, safe_url
                )
            };

            info!("▶ Executing playback pipeline in terminal:\n{}", pipeline_cmd);

            // Inherit stdout and stderr directly so the terminal shows all decoder messages live
            let process = Command::new("/bin/sh")
                .args(["-c", &pipeline_cmd])
                .stdin(Stdio::null())
                .stdout(Stdio::inherit())
                .stderr(Stdio::inherit())
                .spawn();

            let exit_status = match process {
                Ok(mut child) => child.wait(),
                Err(e) => {
                    error!("Failed to spawn playback pipeline shell: {}", e);
                    Err(e)
                }
            };

            let total_secs = spawn_start.elapsed().as_secs();

            // Cleanup flag files
            let _ = fs::remove_file("/tmp/music_playing");
            let _ = fs::remove_file("/tmp/music_paused");

            // Generation check: only update state if NO newer seek or playback has occurred
            tokio::spawn(async move {
                if gen_clone.load(Ordering::SeqCst) == current_gen {
                    let mut st = state_clone.write().await;
                    if *st == PlaybackState::Playing {
                        *st = PlaybackState::Stopped;
                    }
                    info!(
                        "⏹ Track playback finished after {}s with exit status: {:?} (gen: {})",
                        total_secs, exit_status, current_gen
                    );
                } else {
                    info!(
                        "Discarding playback finished event (current gen: {}, task gen: {})",
                        gen_clone.load(Ordering::SeqCst),
                        current_gen
                    );
                }
            });
        });
    }

    /// Legacy play call matching direct string input
    pub async fn play(&self, track_url_or_query: &str, led: &LedController) {
        self.play_direct_stream(track_url_or_query, track_url_or_query, "", None, 0, false, 0, led)
            .await;
    }

    /// Pause playback via killall -STOP on mpg123 and ffmpeg
    pub async fn pause(&self, led: &LedController) {
        let (current_pos, _) = self.get_position_and_duration().await;
        {
            let mut base_guard = self.base_offset_secs.write().await;
            *base_guard = current_pos;
            let mut start_guard = self.playback_start.write().await;
            *start_guard = None;
            let mut state_guard = self.state.write().await;
            *state_guard = PlaybackState::Paused;
        }

        let _ = Command::new("killall")
            .args(["-STOP", "mpg123", "ffmpeg"])
            .output();
        let _ = fs::write("/tmp/music_paused", "");
        let _ = fs::remove_file("/tmp/music_playing");

        // Flash orange (255, 140, 0) and turn off LEDs
        led.blink(255, 140, 0, 150);
        info!("Playback paused at {}s via killall -STOP mpg123 ffmpeg", current_pos);
    }

    /// Resume playback via killall -CONT on mpg123 and ffmpeg
    pub async fn resume(&self, led: &LedController) {
        {
            let mut start_guard = self.playback_start.write().await;
            *start_guard = Some(Instant::now());
            let mut state_guard = self.state.write().await;
            *state_guard = PlaybackState::Playing;
        }

        let _ = Command::new("killall")
            .args(["-CONT", "mpg123", "ffmpeg"])
            .output();
        let _ = fs::write("/tmp/music_playing", "");
        let _ = fs::remove_file("/tmp/music_paused");

        // Flash green and restore music aura (45, 0, 64)
        led.blink(0, 255, 100, 150);
        info!("Playback resumed via killall -CONT mpg123 ffmpeg");
    }

    /// Stop playback via clean stop and reset
    pub async fn stop(&self, led: &LedController) {
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.clean_stop_current_process().await;

        {
            let mut state_guard = self.state.write().await;
            *state_guard = PlaybackState::Stopped;
            *self.base_offset_secs.write().await = 0;
            *self.current_duration_secs.write().await = 0;
            *self.playback_start.write().await = None;
        }

        led.clear();
        info!("Playback stopped, LEDs cleared");
    }

    /// Toggles play/pause state
    pub async fn toggle_play_pause(&self, led: &LedController) {
        let current = self.get_state().await;
        match current {
            PlaybackState::Playing => self.pause(led).await,
            PlaybackState::Paused => self.resume(led).await,
            PlaybackState::Stopped => {
                let track = self.get_current_track().await;
                if track != "Ожидание команды..." && !track.is_empty() {
                    self.play(&track, led).await;
                } else {
                    led.blink(80, 80, 80, 100);
                }
            }
        }
    }
}
