pub mod network;

pub use network::{
    BluetoothStatus, BtDeviceInfo, NetworkHardware, WifiApInfo, WifiPowerSaveConfig,
    WifiPowerSaveParam, WifiStatus,
};

use crate::config::{AudioConfig, HardwareConfig};
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};

// ---------------------------------------------------------------------------
// Buttons & Sensors (evdev) — station/buttons_daemon.sh specification
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StationButtonEvent {
    VolumeUp,
    VolumeDown,
    CenterTap,
    CenterHold,
}

pub struct ButtonsListener {
    device_path: String,
}

impl ButtonsListener {
    pub fn new(config: &HardwareConfig) -> Self {
        Self {
            device_path: config.input_event_device.clone(),
        }
    }

    /// Spawns the evdev buttons listener loop for /dev/input/event1 in a dedicated OS thread.
    /// - Volume (+): 115 (0x73, KEY_VOLUMEUP)
    /// - Volume (-): 114 (0x72, KEY_VOLUMEDOWN)
    /// - Center sensor: 582 (0x246, KEY_VOICECOMMAND)
    ///   * Pressed (value == 1): begins 700ms timer
    ///   * >= 700ms held: triggers CenterHold (Voice assistant trigger)
    ///   * Released (value == 0) < 700ms: triggers CenterTap (Play/Pause)
    pub fn start(self, tx: mpsc::Sender<StationButtonEvent>) {
        // Ensure stale /tmp/listen.lock from previous aborted sessions is cleaned up immediately
        let _ = std::fs::remove_file("/tmp/listen.lock");

        std::thread::Builder::new()
            .name("evdev-buttons".into())
            .spawn(move || {
                info!("Starting hardware buttons listener on {}", self.device_path);

                if !Path::new(&self.device_path).exists() {
                    warn!(
                        "Input device {} not available. Buttons listener running in stub mode.",
                        self.device_path
                    );
                    return;
                }

                match evdev::Device::open(&self.device_path) {
                    Ok(mut device) => {
                        info!("Successfully opened evdev device: {:?}", device.name());

                        let held = Arc::new(AtomicBool::new(false));
                        let long_triggered = Arc::new(AtomicBool::new(false));
                        let mut current_cancel_token: Option<Arc<AtomicBool>> = None;

                        loop {
                            match device.fetch_events() {
                                Ok(events) => {
                                    for ev in events {
                                        if ev.event_type() == evdev::EventType::KEY {
                                            let code = ev.code();
                                            let value = ev.value();

                                            match code {
                                                // Volume Up: 115 (0x73)
                                                115 if value == 1 => {
                                                    let _ = tx.blocking_send(StationButtonEvent::VolumeUp);
                                                }
                                                // Volume Down: 114 (0x72)
                                                114 if value == 1 => {
                                                    let _ = tx.blocking_send(StationButtonEvent::VolumeDown);
                                                }
                                                // Center Touch Sensor: 582 (0x246, KEY_VOICECOMMAND)
                                                582 => {
                                                    if value == 1 {
                                                        // Ignore if mic recording is active
                                                        if Path::new("/tmp/listen.lock").exists() {
                                                            continue;
                                                        }

                                                        held.store(true, Ordering::SeqCst);
                                                        long_triggered.store(false, Ordering::SeqCst);

                                                        // Invalidate previous timer if still alive
                                                        if let Some(token) = current_cancel_token.take() {
                                                            token.store(true, Ordering::SeqCst);
                                                        }

                                                        let cancel_token = Arc::new(AtomicBool::new(false));
                                                        current_cancel_token = Some(cancel_token.clone());

                                                        let tx_timer = tx.clone();
                                                        let held_timer = held.clone();
                                                        let long_timer = long_triggered.clone();

                                                        // Dedicated lightweight hold timer thread (700 ms)
                                                        std::thread::Builder::new()
                                                            .name("center-hold-timer".into())
                                                            .spawn(move || {
                                                                std::thread::sleep(std::time::Duration::from_millis(700));
                                                                if !cancel_token.load(Ordering::SeqCst)
                                                                    && held_timer.load(Ordering::SeqCst)
                                                                {
                                                                    long_timer.store(true, Ordering::SeqCst);
                                                                    let _ = tx_timer.blocking_send(StationButtonEvent::CenterHold);
                                                                }
                                                            })
                                                            .ok();
                                                    } else if value == 0 {
                                                        // Cancel active hold timer
                                                        if let Some(token) = current_cancel_token.take() {
                                                            token.store(true, Ordering::SeqCst);
                                                        }

                                                        if held.load(Ordering::SeqCst) {
                                                            held.store(false, Ordering::SeqCst);
                                                            if !long_triggered.load(Ordering::SeqCst) {
                                                                // Short tap -> Play/Pause
                                                                let _ = tx.blocking_send(StationButtonEvent::CenterTap);
                                                            }
                                                            long_triggered.store(false, Ordering::SeqCst);
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                Err(e) => {
                                    error!("Error reading evdev events: {}", e);
                                    std::thread::sleep(std::time::Duration::from_millis(500));
                                }
                            }
                        }
                    }
                    Err(e) => {
                        warn!("Failed to open evdev device {}: {}", self.device_path, e);
                    }
                }
            })
            .expect("Failed to spawn evdev-buttons thread");
    }
}

// ---------------------------------------------------------------------------
// LED Ring Controller (Sysfs /sys/class/leds/micro:...) — station/led.sh
// ---------------------------------------------------------------------------

pub struct LedController {
    sysfs_base: String,
    rings_count: usize,
    current_color: Arc<std::sync::RwLock<[u8; 3]>>,
}

impl LedController {
    pub fn new(config: &HardwareConfig) -> Self {
        Self {
            sysfs_base: config.led_sysfs_base.clone(),
            // Exactly 3 RGB rings on Yandex Station Lite (ring1, ring2, ring3)
            rings_count: config.led_rings_count.min(3),
            current_color: Arc::new(std::sync::RwLock::new([0, 0, 0])),
        }
    }

    /// Sets RGB color across all 3 rings (indices 1, 2, 3).
    pub fn set_rgb(&self, red: u8, green: u8, blue: u8) {
        if let Ok(mut c) = self.current_color.write() {
            *c = [red, green, blue];
        }
        for i in 1..=self.rings_count {
            self.write_brightness("red", i, red);
            self.write_brightness("green", i, green);
            self.write_brightness("blue", i, blue);
        }
    }

    /// Returns the currently set RGB color.
    pub fn get_rgb(&self) -> [u8; 3] {
        self.current_color.read().map(|c| *c).unwrap_or([0, 0, 0])
    }

    /// Turn off all 3 LED rings.
    pub fn clear(&self) {
        self.set_rgb(0, 0, 0);
    }

    /// Default music background aura (45 red, 0 green, 64 blue).
    pub fn set_music_aura(&self) {
        self.set_rgb(45, 0, 64);
    }

    /// Blinks LED color for duration_ms, then restores previous state
    /// (music aura if playing, otherwise off) matching station/buttons_daemon.sh blink().
    pub fn blink(&self, red: u8, green: u8, blue: u8, duration_ms: u64) {
        let base = self.sysfs_base.clone();
        let rings = self.rings_count;

        tokio::spawn(async move {
            for i in 1..=rings {
                let _ = fs::write(format!("{}/micro:red:ring{}/brightness", base, i), red.to_string());
                let _ = fs::write(format!("{}/micro:green:ring{}/brightness", base, i), green.to_string());
                let _ = fs::write(format!("{}/micro:blue:ring{}/brightness", base, i), blue.to_string());
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(duration_ms)).await;

            // If music is playing -> restore music aura (45, 0, 64), otherwise turn off (0, 0, 0)
            let is_music_playing = Path::new("/tmp/music_playing").exists();
            let (r, g, b) = if is_music_playing { (45, 0, 64) } else { (0, 0, 0) };

            for i in 1..=rings {
                let _ = fs::write(format!("{}/micro:red:ring{}/brightness", base, i), r.to_string());
                let _ = fs::write(format!("{}/micro:green:ring{}/brightness", base, i), g.to_string());
                let _ = fs::write(format!("{}/micro:blue:ring{}/brightness", base, i), b.to_string());
            }
        });
    }

    fn write_brightness(&self, color: &str, ring: usize, val: u8) {
        let path = format!("{}/micro:{}:ring{}/brightness", self.sysfs_base, color, ring);
        if Path::new(&path).exists() {
            let _ = fs::write(&path, val.to_string());
        }
    }
}

// ---------------------------------------------------------------------------
// Hardware Volume (TAS5805M Digital Amplifier) — station/buttons_daemon.sh
// ---------------------------------------------------------------------------

pub struct AudioHardware {
    current_volume_step: Arc<AtomicU8>,
    mixer_name: String,
}

impl AudioHardware {
    pub fn new(config: &AudioConfig) -> Self {
        // Read initial volume step from /data/volume if valid (1..=10), otherwise use config or 6
        let initial_step = if let Ok(s) = fs::read_to_string("/data/volume") {
            s.trim().parse::<u8>().unwrap_or(config.initial_volume).clamp(1, 10)
        } else {
            config.initial_volume.clamp(1, 10)
        };

        Self {
            current_volume_step: Arc::new(AtomicU8::new(initial_step)),
            mixer_name: config.mixer_control_name.clone(),
        }
    }

    /// Maps discrete volume level (1-10) to exact TAS5805M amplifier register value
    /// according to station/buttons_daemon.sh:
    ///  1 -> 117 (-45.0 dB)
    ///  2 -> 129 (-39.0 dB)
    ///  3 -> 141 (-33.0 dB)
    ///  4 -> 152 (-27.5 dB)
    ///  5 -> 162 (-22.5 dB)
    ///  6 -> 172 (-17.5 dB, default)
    ///  7 -> 182 (-12.5 dB)
    ///  8 -> 191 (-8.0 dB)
    ///  9 -> 201 (-3.0 dB)
    ///  10 -> 210 (+1.5 dB)
    pub fn map_volume_to_tas5805m(step: u8) -> (u8, &'static str) {
        match step {
            1 => (117, "-45.0 dB"),
            2 => (129, "-39.0 dB"),
            3 => (141, "-33.0 dB"),
            4 => (152, "-27.5 dB"),
            5 => (162, "-22.5 dB"),
            6 => (172, "-17.5 dB"),
            7 => (182, "-12.5 dB"),
            8 => (191, "-8.0 dB"),
            9 => (201, "-3.0 dB"),
            10 => (210, "+1.5 dB"),
            _ => (172, "-17.5 dB"),
        }
    }

    pub fn get_volume(&self) -> u8 {
        self.current_volume_step.load(Ordering::SeqCst)
    }

    /// Sets volume step (1-10), writes to /data/volume, and updates TAS5805M mixer register.
    pub fn set_volume(&self, step: u8) -> u8 {
        let clamped = step.clamp(1, 10);
        self.current_volume_step.store(clamped, Ordering::SeqCst);
        let (raw_val, db_str) = Self::map_volume_to_tas5805m(clamped);

        // Persist to /data/volume
        let _ = fs::write("/data/volume", format!("{}\n", clamped));

        // Apply via amixer sset / cset
        let amixer_bin = if Path::new("/opt/bin/amixer").exists() {
            "/opt/bin/amixer"
        } else if Path::new("/usr/bin/amixer").exists() {
            "/usr/bin/amixer"
        } else {
            "amixer"
        };
        let _ = std::process::Command::new(amixer_bin)
            .args(["sset", &self.mixer_name, &raw_val.to_string()])
            .output();

        info!(
            "Hardware Volume set to step {}/10 -> TAS5805M register: {} ({})",
            clamped, raw_val, db_str
        );

        clamped
    }

    /// Adjust volume by +1 or -1 step.
    /// Returns (new_step, changed):
    /// - If changed is true: step was increased or decreased.
    /// - If changed is false: already at boundary (10 max or 1 min).
    pub fn adjust_volume(&self, delta: i8) -> (u8, bool) {
        let cur = self.get_volume();
        if delta > 0 {
            if cur < 10 {
                let next = self.set_volume(cur + 1);
                (next, true)
            } else {
                (cur, false)
            }
        } else if delta < 0 {
            if cur > 1 {
                let next = self.set_volume(cur - 1);
                (next, true)
            } else {
                (cur, false)
            }
        } else {
            (cur, false)
        }
    }
}
