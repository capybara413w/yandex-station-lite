use crate::services::tts::TtsService;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::net::UdpSocket;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VisualizerConfig {
    pub enabled: bool,
    pub mode: String,          // "mono" (single color, default) or "rgb"
    pub color: [u8; 3],        // default [255, 255, 255] (all pure white)
    pub bass_cutoff_hz: f32,   // default 500.0 (100.0 .. 1000.0)
    pub high_cutoff_hz: f32,   // default 3500.0 (2000.0 .. 6000.0)
    pub speed: String,         // "fast", "normal", "smooth"
    pub sensitivity: f32,      // default 1.3 (0.5 .. 3.0)
}

impl Default for VisualizerConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: "mono".to_string(), // Single color / white by default
            color: [255, 255, 255],   // White
            bass_cutoff_hz: 500.0,    // 500Hz cutoff (captures kicks, sub-bass, and bassline)
            high_cutoff_hz: 3500.0,   // 3500Hz cutoff for crisp trebles & hi-hats
            speed: "fast".to_string(), // Snappy 60+ Hz updates
            sensitivity: 1.3,
        }
    }
}

pub struct VisualizerService {
    enabled: Arc<AtomicBool>,
    sysfs_base: String,
    current_levels: Arc<RwLock<[u8; 3]>>, // [bass, mid, high] (0-255)
    config: Arc<RwLock<VisualizerConfig>>,
}

impl VisualizerService {
    pub fn new(sysfs_base: &str) -> Self {
        Self {
            enabled: Arc::new(AtomicBool::new(true)),
            sysfs_base: sysfs_base.to_string(),
            current_levels: Arc::new(RwLock::new([0, 0, 0])),
            config: Arc::new(RwLock::new(VisualizerConfig::default())),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    pub fn set_enabled(&self, val: bool) {
        self.enabled.store(val, Ordering::Relaxed);
    }

    pub async fn get_levels(&self) -> [u8; 3] {
        *self.current_levels.read().await
    }

    pub async fn get_config(&self) -> VisualizerConfig {
        let mut cfg = self.config.read().await.clone();
        cfg.enabled = self.is_enabled();
        cfg
    }

    pub async fn update_config(&self, new_cfg: VisualizerConfig) {
        self.set_enabled(new_cfg.enabled);
        let mut w = self.config.write().await;
        *w = new_cfg;
    }

    pub async fn set_color(&self, color: [u8; 3]) {
        let mut w = self.config.write().await;
        w.color = color;
        w.mode = "mono".to_string();
    }

    pub async fn set_bass_cutoff(&self, hz: f32) {
        let mut w = self.config.write().await;
        w.bass_cutoff_hz = hz.clamp(100.0, 1200.0);
    }

    /// Spawns the lightweight 3-band reactive LED visualizer listener task
    pub fn start(self: Arc<Self>, tts: Arc<TtsService>) {
        tokio::spawn(async move {
            info!("Starting lightweight reactive 3-band audio visualizer on UDP 127.0.0.1:7777");

            let socket = match UdpSocket::bind("127.0.0.1:7777").await {
                Ok(s) => s,
                Err(e) => {
                    warn!("Failed to bind UDP visualizer socket on 127.0.0.1:7777: {}", e);
                    return;
                }
            };

            let mut buf = [0u8; 1024];

            // 3-band IIR filter states
            let mut lp_state = 0.0f32;
            let mut hp_state = 0.0f32;

            // Adaptive peak levels for AGC (Automatic Gain Control)
            let mut peak_bass = 500.0f32;
            let mut peak_mid = 500.0f32;
            let mut peak_high = 500.0f32;

            // Smoothed display levels (0.0 to 255.0)
            let mut smooth_bass = 0.0f32;
            let mut smooth_mid = 0.0f32;
            let mut smooth_high = 0.0f32;

            // Cache previous LED outputs to avoid redundant sysfs writes
            // [r1, g1, b1, r2, g2, b2, r3, g3, b3]
            let mut prev_leds = [0u8; 9];

            loop {
                // Wait for audio packet or 100ms timeout
                let recv_res = tokio::time::timeout(
                    tokio::time::Duration::from_millis(100),
                    socket.recv_from(&mut buf),
                )
                .await;

                let is_music = Path::new("/tmp/music_playing").exists();
                let is_listening = Path::new("/tmp/listen.lock").exists();
                let is_speaking = tts.is_speaking();

                // If voice assistant is active (listening/speaking) -> yield and do not touch LEDs
                if is_listening || is_speaking {
                    tokio::time::sleep(tokio::time::Duration::from_millis(60)).await;
                    continue;
                }

                let cfg = self.config.read().await.clone();

                // If visualizer is disabled by user, keep default aura if playing
                if !self.enabled.load(Ordering::Relaxed) || !cfg.enabled {
                    if is_music {
                        self.apply_leds(&mut prev_leds, [45, 0, 64], [45, 0, 64], [45, 0, 64]);
                    }
                    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
                    continue;
                }

                match recv_res {
                    Ok(Ok((len, _src))) if len >= 4 => {
                        let sample_count = len / 2;
                        let mut sum_sq_bass = 0.0f32;
                        let mut sum_sq_mid = 0.0f32;
                        let mut sum_sq_high = 0.0f32;

                        // Dynamic IIR filter parameters at 16000Hz sampling rate
                        let fs = 16000.0f32;
                        let dt = 1.0f32 / fs;

                        // Low-pass filter coefficient for bass
                        let cutoff_bass = cfg.bass_cutoff_hz.clamp(100.0, 1200.0);
                        let rc_bass = 1.0f32 / (2.0f32 * std::f32::consts::PI * cutoff_bass);
                        let alpha_lp = (dt / (rc_bass + dt)).clamp(0.04, 0.85);

                        // High-pass filter coefficient for treble
                        let cutoff_high = cfg.high_cutoff_hz.clamp(1800.0, 6500.0);
                        let alpha_hp = (2.0f32 * std::f32::consts::PI * cutoff_high * dt).clamp(0.15, 0.90);

                        // Single-pass 3-band filter across 16-bit PCM samples
                        for i in 0..sample_count {
                            let b_idx = i * 2;
                            let sample_i16 = i16::from_le_bytes([buf[b_idx], buf[b_idx + 1]]);
                            let sample = sample_i16 as f32;

                            // Low-pass filter (Bass)
                            lp_state += alpha_lp * (sample - lp_state);
                            let bass = lp_state;

                            // High-pass filter (Treble)
                            hp_state += alpha_hp * (sample - hp_state);
                            let high = sample - hp_state;

                            // Mid band is residual
                            let mid = sample - bass - high;

                            sum_sq_bass += bass * bass;
                            sum_sq_mid += mid * mid;
                            sum_sq_high += high * high;
                        }

                        let count_f = sample_count as f32;
                        let sens = cfg.sensitivity.clamp(0.5, 3.0);
                        let rms_bass = (sum_sq_bass / count_f).sqrt() * sens;
                        let rms_mid = (sum_sq_mid / count_f).sqrt() * sens;
                        let rms_high = (sum_sq_high / count_f).sqrt() * sens;

                        // Dynamic AGC (Fast attack, gentle decay)
                        if rms_bass > peak_bass {
                            peak_bass = rms_bass;
                        } else {
                            peak_bass = (peak_bass * 0.985).max(300.0);
                        }

                        if rms_mid > peak_mid {
                            peak_mid = rms_mid;
                        } else {
                            peak_mid = (peak_mid * 0.985).max(300.0);
                        }

                        if rms_high > peak_high {
                            peak_high = rms_high;
                        } else {
                            peak_high = (peak_high * 0.985).max(300.0);
                        }

                        // Normalized 0.0 .. 1.0 with dynamic curve
                        let norm_bass = (rms_bass / peak_bass).clamp(0.0, 1.0).powf(1.25);
                        let norm_mid = (rms_mid / peak_mid).clamp(0.0, 1.0).powf(1.25);
                        let norm_high = (rms_high / peak_high).clamp(0.0, 1.0).powf(1.25);

                        // Response speed weighting
                        let (attack_coeff, decay_coeff) = match cfg.speed.as_str() {
                            "fast" => (0.80f32, 0.20f32),
                            "smooth" => (0.45f32, 0.55f32),
                            _ => (0.65f32, 0.35f32),
                        };

                        smooth_bass = smooth_bass * decay_coeff + (norm_bass * 255.0) * attack_coeff;
                        smooth_mid = smooth_mid * decay_coeff + (norm_mid * 255.0) * attack_coeff;
                        smooth_high = smooth_high * decay_coeff + (norm_high * 255.0) * attack_coeff;

                        let b_val = smooth_bass.round().clamp(0.0, 255.0) as u8;
                        let m_val = smooth_mid.round().clamp(0.0, 255.0) as u8;
                        let h_val = smooth_high.round().clamp(0.0, 255.0) as u8;

                        // Update current levels for Web UI monitoring
                        if let Ok(mut l) = self.current_levels.try_write() {
                            *l = [b_val, m_val, h_val];
                        }

                        // Colors:
                        // Either mono (all white [255, 255, 255] or user custom color) or RGB tricolor
                        let (r1, r2, r3) = if cfg.mode == "rgb" {
                            // Ring 1 (Bass / Low): Punchy Magenta
                            let ring1 = [b_val, 0, (b_val as f32 * 0.45) as u8];
                            // Ring 2 (Mids / Vocals): Warm Amber
                            let ring2 = [m_val, (m_val as f32 * 0.70) as u8, 0];
                            // Ring 3 (Highs / Treble): Electric Cyan
                            let ring3 = [0, (h_val as f32 * 0.75) as u8, h_val];
                            (ring1, ring2, ring3)
                        } else {
                            // All LEDs have the SAME configured color (White [255, 255, 255] by default)
                            let [cr, cg, cb] = cfg.color;
                            let scale_col = |lvl: u8| -> [u8; 3] {
                                let f = lvl as u32;
                                [
                                    ((cr as u32 * f) / 255) as u8,
                                    ((cg as u32 * f) / 255) as u8,
                                    ((cb as u32 * f) / 255) as u8,
                                ]
                            };
                            (scale_col(b_val), scale_col(m_val), scale_col(h_val))
                        };

                        self.apply_leds(&mut prev_leds, r1, r2, r3);
                    }
                    _ => {
                        // Timeout / no packets (paused, track ended, or silent section)
                        smooth_bass *= 0.75;
                        smooth_mid *= 0.75;
                        smooth_high *= 0.75;

                        if let Ok(mut l) = self.current_levels.try_write() {
                            *l = [smooth_bass as u8, smooth_mid as u8, smooth_high as u8];
                        }

                        if is_music {
                            let [cr, cg, cb] = if cfg.mode == "rgb" { [45, 0, 64] } else { cfg.color };
                            let b_aura = (smooth_bass as u32 * 30 / 255) as u8;
                            let m_aura = (smooth_mid as u32 * 30 / 255) as u8;
                            let h_aura = (smooth_high as u32 * 30 / 255) as u8;

                            let make_aura = |amt: u8| -> [u8; 3] {
                                [
                                    ((cr as u32 * amt as u32) / 255).min(255) as u8,
                                    ((cg as u32 * amt as u32) / 255).min(255) as u8,
                                    ((cb as u32 * amt as u32) / 255).min(255) as u8,
                                ]
                            };

                            let r1 = make_aura(b_aura.max(12));
                            let r2 = make_aura(m_aura.max(12));
                            let r3 = make_aura(h_aura.max(12));
                            self.apply_leds(&mut prev_leds, r1, r2, r3);
                        } else {
                            // Music stopped -> fade to off
                            self.apply_leds(&mut prev_leds, [0, 0, 0], [0, 0, 0], [0, 0, 0]);
                        }
                    }
                }
            }
        });
    }

    /// Optimized sysfs writer: only writes to file if brightness delta >= 4
    fn apply_leds(&self, prev: &mut [u8; 9], r1: [u8; 3], r2: [u8; 3], r3: [u8; 3]) {
        let targets = [
            ("red", 1, r1[0], 0),
            ("green", 1, r1[1], 1),
            ("blue", 1, r1[2], 2),
            ("red", 2, r2[0], 3),
            ("green", 2, r2[1], 4),
            ("blue", 2, r2[2], 5),
            ("red", 3, r3[0], 6),
            ("green", 3, r3[1], 7),
            ("blue", 3, r3[2], 8),
        ];

        for (color, ring, target_val, idx) in targets {
            let old_val = prev[idx];
            // Only perform sysfs file I/O if delta is perceptible (>= 4)
            if (target_val as i16 - old_val as i16).abs() >= 4 {
                prev[idx] = target_val;
                let path = format!("{}/micro:{}:ring{}/brightness", self.sysfs_base, color, ring);
                let _ = fs::write(path, target_val.to_string());
            }
        }
    }
}
