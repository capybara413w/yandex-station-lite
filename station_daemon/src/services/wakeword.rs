use crate::config::AppConfig;
use crate::hardware::LedController;
use crate::services::player::PlayerService;
use crate::services::stt::{pcm_to_wav, SttService};
use crate::services::tts::TtsService;
use std::collections::VecDeque;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Extracts wake-word trigger and optional one-shot command from transcribed text.
///
/// Returns:
/// - `None`: No wake-word detected.
/// - `Some(None)`: Wake-word detected alone without command (e.g. "Алиса", "Алиса?").
/// - `Some(Some(cmd))`: Wake-word detected with a one-shot command (e.g. "Алиса, включи рок" -> `Some("включи рок")`).
pub fn extract_wakeword(text: &str, target_phrase: &str) -> Option<Option<String>> {
    let lower = text.to_lowercase();
    let cleaned: String = lower
        .chars()
        .map(|c| if c.is_alphanumeric() || c.is_whitespace() { c } else { ' ' })
        .collect();
    let words: Vec<&str> = cleaned.split_whitespace().collect();
    if words.is_empty() {
        return None;
    }

    let default_aliases = ["алиса", "алис", "алисочка", "alisa", "alice"];
    let target = target_phrase.trim().to_lowercase();

    let mut found_idx = None;
    for (i, word) in words.iter().enumerate() {
        if (!target.is_empty() && *word == target) || default_aliases.contains(word) {
            found_idx = Some(i);
            break;
        }
    }

    let idx = found_idx?;
    let remainder_words = &words[(idx + 1)..];
    if remainder_words.is_empty() {
        Some(None)
    } else {
        let cmd = remainder_words.join(" ");
        Some(Some(cmd))
    }
}

/// Ensures the authentic Yandex Station spotter chime WAV is converted and ready in /tmp/spotter_start.wav.
pub fn ensure_chime_prepared() -> Result<String, String> {
    let wav_path = "/tmp/spotter_start.wav";
    if Path::new(wav_path).exists() {
        return Ok(wav_path.to_string());
    }

    let opus_src = "/system/vendor/quasar/sounds/common/spotter_start.opus";
    if !Path::new(opus_src).exists() {
        return Err(format!("Spotter chime not found at {}", opus_src));
    }

    let ffmpeg_bin = if Path::new("/opt/bin/ffmpeg").exists() {
        "/opt/bin/ffmpeg"
    } else if Path::new("/usr/bin/ffmpeg").exists() {
        "/usr/bin/ffmpeg"
    } else {
        "ffmpeg"
    };

    let status = Command::new(ffmpeg_bin)
        .args([
            "-y",
            "-i", opus_src,
            "-ar", "48000",
            "-ac", "2",
            "-f", "wav",
            wav_path,
        ])
        .output()
        .map_err(|e| format!("Failed to convert spotter chime: {}", e))?;

    if status.status.success() && Path::new(wav_path).exists() {
        Ok(wav_path.to_string())
    } else {
        Err(format!("ffmpeg chime conversion failed: {:?}", status))
    }
}

/// Plays the authentic Yandex Station spotter chime to ALSA playback device.
pub fn play_spotter_chime(playback_card: &str) {
    let _ = ensure_chime_prepared();
    let wav_path = "/tmp/spotter_start.wav";
    if Path::new(wav_path).exists() {
        let aplay_bin = if Path::new("/opt/bin/aplay").exists() {
            "/opt/bin/aplay"
        } else if Path::new("/usr/bin/aplay").exists() {
            "/usr/bin/aplay"
        } else {
            "aplay"
        };

        let _ = Command::new(aplay_bin)
            .args(["-D", playback_card, "-q", wav_path])
            .output();
    }
}

pub struct WakeWordService {
    pub enabled: Arc<AtomicBool>,
    pub phrase: Arc<RwLock<String>>,
    pub is_processing: Arc<AtomicBool>,
    record_device: String,
    playback_card: String,
    sample_rate: u32,
    stt: Arc<SttService>,
    tts: Arc<TtsService>,
    player: Arc<PlayerService>,
    led: Arc<LedController>,
}

impl WakeWordService {
    pub fn new(
        config: &AppConfig,
        stt: Arc<SttService>,
        tts: Arc<TtsService>,
        player: Arc<PlayerService>,
        led: Arc<LedController>,
    ) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(config.audio.wakeword_enabled)),
            phrase: Arc::new(RwLock::new(config.audio.wakeword_phrase.clone())),
            is_processing: Arc::new(AtomicBool::new(false)),
            record_device: config.audio.record_device.clone(),
            playback_card: config.audio.alsa_card.clone(),
            sample_rate: config.audio.sample_rate,
            stt,
            tts,
            player,
            led,
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::Relaxed);
    }

    pub async fn get_phrase(&self) -> String {
        self.phrase.read().await.clone()
    }

    pub async fn set_phrase(&self, phrase: String) {
        *self.phrase.write().await = phrase;
    }

    pub fn is_processing(&self) -> bool {
        self.is_processing.load(Ordering::Relaxed)
    }

    pub fn set_processing(&self, val: bool) {
        self.is_processing.store(val, Ordering::Relaxed);
    }

    /// Starts the continuous background listener for the "Алиса" wake-word.
    /// Runs in a blocking worker thread, tracking RMS energy and triggering on speech.
    pub fn start_worker(
        self: Arc<Self>,
        trigger_tx: mpsc::Sender<Option<String>>,
    ) {
        tokio::task::spawn_blocking(move || {
            use std::io::Read;

            let arecord_bin = if Path::new("/opt/bin/arecord").exists() {
                "/opt/bin/arecord"
            } else if Path::new("/usr/bin/arecord").exists() {
                "/usr/bin/arecord"
            } else {
                "arecord"
            };

            let rate_str = self.sample_rate.to_string();
            // 50ms chunk at 16000Hz mono 16-bit = 1600 bytes
            const CHUNK_SIZE: usize = 1600;
            let mut chunk = vec![0u8; CHUNK_SIZE];

            info!(
                "Wake-word background worker started. Listening on {} (rate: {})...",
                self.record_device, self.sample_rate
            );

            // Pre-convert chime on startup so it's instantly available without delay
            let _ = ensure_chime_prepared();

            let rt_handle = tokio::runtime::Handle::current();

            loop {
                // If disabled, pause
                if !self.enabled.load(Ordering::Relaxed) {
                    std::thread::sleep(Duration::from_millis(500));
                    continue;
                }

                // If assistant is processing, TTS is speaking, or /tmp/listen.lock is held, wait
                if self.is_processing.load(Ordering::Relaxed)
                    || self.tts.is_speaking()
                    || Path::new("/tmp/listen.lock").exists()
                {
                    std::thread::sleep(Duration::from_millis(200));
                    continue;
                }

                // Spawn arecord process for continuous background microphone capture
                let mut child = match Command::new(arecord_bin)
                    .args([
                        "-D", &self.record_device,
                        "-f", "S16_LE",
                        "-r", &rate_str,
                        "-c", "1",
                        "-t", "raw",
                        "-q",
                    ])
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                {
                    Ok(c) => c,
                    Err(e) => {
                        warn!("Wake-word arecord spawn error: {}. Retrying in 2s...", e);
                        std::thread::sleep(Duration::from_secs(2));
                        continue;
                    }
                };

                let mut stdout = match child.stdout.take() {
                    Some(s) => s,
                    None => {
                        let _ = child.kill();
                        let _ = child.wait();
                        std::thread::sleep(Duration::from_secs(1));
                        continue;
                    }
                };

                let mut pre_roll: VecDeque<Vec<u8>> = VecDeque::with_capacity(8); // ~350ms pre-roll
                let mut speech_buffer: Vec<u8> = Vec::with_capacity(16000 * 2 * 4); // up to 4s speech
                let mut speech_active = false;
                let mut silence_chunks: usize = 0;
                let mut noise_floor_sum: f64 = 0.0;
                let mut noise_samples: usize = 0;

                loop {
                    // Check if external conditions interrupted listening
                    if !self.enabled.load(Ordering::Relaxed)
                        || self.is_processing.load(Ordering::Relaxed)
                        || self.tts.is_speaking()
                        || Path::new("/tmp/listen.lock").exists()
                    {
                        break;
                    }

                    if stdout.read_exact(&mut chunk).is_err() {
                        break;
                    }

                    // Calculate RMS
                    let mut sum_sq: f64 = 0.0;
                    for i in 0..(CHUNK_SIZE / 2) {
                        let s = i16::from_le_bytes([chunk[i * 2], chunk[i * 2 + 1]]) as f64;
                        sum_sq += s * s;
                    }
                    let rms = (sum_sq / (CHUNK_SIZE / 2) as f64).sqrt();

                    if !speech_active {
                        if noise_samples < 20 {
                            noise_floor_sum += rms;
                            noise_samples += 1;
                        } else {
                            // Moving average noise floor
                            let curr_floor = noise_floor_sum / noise_samples as f64;
                            noise_floor_sum = curr_floor * 0.95 + rms * 0.05;
                            noise_samples = 1;
                        }

                        let noise_floor = if noise_samples > 0 {
                            noise_floor_sum / noise_samples as f64
                        } else {
                            150.0
                        };

                        let speech_threshold = (noise_floor * 1.85).clamp(420.0, 1600.0);

                        if pre_roll.len() >= 7 {
                            pre_roll.pop_front();
                        }
                        pre_roll.push_back(chunk.clone());

                        if rms > speech_threshold {
                            // Speech onset detected!
                            speech_active = true;
                            silence_chunks = 0;
                            speech_buffer.clear();
                            for pr in pre_roll.drain(..) {
                                speech_buffer.extend_from_slice(&pr);
                            }
                            speech_buffer.extend_from_slice(&chunk);
                        }
                    } else {
                        // Speech active
                        speech_buffer.extend_from_slice(&chunk);

                        let noise_floor = noise_floor_sum / noise_samples.max(1) as f64;
                        let silence_threshold = ((noise_floor * 1.85).clamp(420.0, 1600.0)) * 0.65;

                        if rms < silence_threshold {
                            silence_chunks += 1;
                        } else {
                            silence_chunks = 0;
                        }

                        let total_speech_chunks = speech_buffer.len() / CHUNK_SIZE;

                        // Stop phrase capture if user paused for ~650ms (13 chunks) OR phrase reached 3.5s (70 chunks)
                        if silence_chunks >= 13 || total_speech_chunks >= 70 {
                            speech_active = false;

                            // Minimum phrase length: at least 7 chunks (350ms) to avoid single clicks
                            if total_speech_chunks >= 7 {
                                let wav = pcm_to_wav(&speech_buffer, self.sample_rate, 1, 16);
                                let phrase_target = rt_handle.block_on(self.get_phrase());

                                // Transcribe audio segment
                                let transcribe_res = rt_handle.block_on(self.stt.transcribe_wav(&wav));

                                if let Ok(transcribed) = transcribe_res {
                                    let clean_trans = transcribed.trim();
                                    if !clean_trans.is_empty() {
                                        debug!("Wake-word listener heard: '{}'", clean_trans);

                                        if let Some(cmd_opt) = extract_wakeword(clean_trans, &phrase_target) {
                                            info!(
                                                "*** WAKE-WORD DETECTED: '{}' (One-shot command: {:?}) ***",
                                                clean_trans, cmd_opt
                                            );

                                            // Mark processing active to prevent concurrent triggers
                                            self.is_processing.store(true, Ordering::SeqCst);

                                            // Stop arecord capture immediately
                                            let _ = child.kill();
                                            let _ = child.wait();

                                            // 1. Pause music if playing
                                            rt_handle.block_on(self.player.pause(&self.led));

                                            // 2. Set Alice Purple LED (180, 0, 255)
                                            self.led.set_rgb(180, 0, 255);

                                            // 3. Play authentic spotter chime
                                            play_spotter_chime(&self.playback_card);

                                            // 4. Send trigger to assistant engine
                                            let _ = rt_handle.block_on(trigger_tx.send(cmd_opt));

                                            // Exit child loop so we wait until processing is finished
                                            break;
                                        }
                                    }
                                }
                            }

                            speech_buffer.clear();
                            pre_roll.clear();
                        }
                    }
                }

                let _ = child.kill();
                let _ = child.wait();
                std::thread::sleep(Duration::from_millis(100));
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_wakeword_standalone() {
        assert_eq!(extract_wakeword("Алиса", "алиса"), Some(None));
        assert_eq!(extract_wakeword("алиса?", "алиса"), Some(None));
        assert_eq!(extract_wakeword("Алис!", "алиса"), Some(None));
        assert_eq!(extract_wakeword("Алисочка...", "алиса"), Some(None));
        assert_eq!(extract_wakeword("Alice", "алиса"), Some(None));
        assert_eq!(extract_wakeword("Alisa", "алиса"), Some(None));
    }

    #[test]
    fn test_extract_wakeword_one_shot() {
        assert_eq!(
            extract_wakeword("Алиса, включи рок", "алиса"),
            Some(Some("включи рок".to_string()))
        );
        assert_eq!(
            extract_wakeword("Эй, Алиса, какая погода на улице?", "алиса"),
            Some(Some("какая погода на улице".to_string()))
        );
        assert_eq!(
            extract_wakeword("Алиса стоп", "алиса"),
            Some(Some("стоп".to_string()))
        );
    }

    #[test]
    fn test_extract_wakeword_no_match() {
        assert_eq!(extract_wakeword("Привет, как дела?", "алиса"), None);
        assert_eq!(extract_wakeword("музыка играет громко", "алиса"), None);
        assert_eq!(extract_wakeword("", "алиса"), None);
    }
}
