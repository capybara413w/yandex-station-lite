use crate::config::AppConfig;
use serde::{Deserialize, Serialize};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StreamingStatus {
    pub airplay_enabled: bool,
    pub airplay_running: bool,
    pub airplay_name: String,
    pub spotify_enabled: bool,
    pub spotify_running: bool,
    pub spotify_name: String,
    pub snapcast_enabled: bool,
    pub snapcast_running: bool,
    pub snapcast_server: String,
    pub snapcast_port: u16,
    pub bluetooth_sink_enabled: bool,
    pub bluetooth_sink_running: bool,
}

pub struct StreamingService {
    config: Arc<RwLock<AppConfig>>,
    airplay_child: Arc<RwLock<Option<Child>>>,
    spotify_child: Arc<RwLock<Option<Child>>>,
    snapcast_child: Arc<RwLock<Option<Child>>>,
    bt_sink_child: Arc<RwLock<Option<Child>>>,
    is_active: Arc<AtomicBool>,
}

impl StreamingService {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        Self {
            config,
            airplay_child: Arc::new(RwLock::new(None)),
            spotify_child: Arc::new(RwLock::new(None)),
            snapcast_child: Arc::new(RwLock::new(None)),
            bt_sink_child: Arc::new(RwLock::new(None)),
            is_active: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Spawns or terminates background streaming sinks matching the active configuration.
    pub async fn sync_services(&self) {
        let (streaming_cfg, alsa_card) = {
            let cfg = self.config.read().await;
            (cfg.streaming.clone(), cfg.audio.alsa_card.clone())
        };

        // 1. AirPlay (station-airplay / shairport-sync)
        let mut ap_guard = self.airplay_child.write().await;
        if streaming_cfg.airplay_enabled {
            let needs_start = match ap_guard.as_mut() {
                Some(child) => child.try_wait().map(|s| s.is_some()).unwrap_or(true),
                None => true,
            };
            if needs_start {
                *ap_guard = Self::spawn_airplay(&streaming_cfg.airplay_name, &alsa_card);
            }
        } else if let Some(mut child) = ap_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
            info!("AirPlay receiver stopped");
        }

        // 2. Spotify Connect (librespot)
        let mut sp_guard = self.spotify_child.write().await;
        if streaming_cfg.spotify_enabled {
            let needs_start = match sp_guard.as_mut() {
                Some(child) => child.try_wait().map(|s| s.is_some()).unwrap_or(true),
                None => true,
            };
            if needs_start {
                *sp_guard = Self::spawn_spotify(&streaming_cfg.spotify_name, &alsa_card);
            }
        } else if let Some(mut child) = sp_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
            info!("Spotify Connect receiver stopped");
        }

        // 3. Snapcast (snapclient)
        let mut snap_guard = self.snapcast_child.write().await;
        if streaming_cfg.snapcast_enabled {
            let needs_start = match snap_guard.as_mut() {
                Some(child) => child.try_wait().map(|s| s.is_some()).unwrap_or(true),
                None => true,
            };
            if needs_start {
                *snap_guard = Self::spawn_snapcast(&streaming_cfg.snapcast_server, streaming_cfg.snapcast_port);
            }
        } else if let Some(mut child) = snap_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
            info!("Snapcast client stopped");
        }

        // 4. Bluetooth A2DP Sink (bluealsa-aplay / bluetoothctl)
        let mut bt_guard = self.bt_sink_child.write().await;
        if streaming_cfg.bluetooth_sink_enabled {
            let needs_start = match bt_guard.as_mut() {
                Some(child) => child.try_wait().map(|s| s.is_some()).unwrap_or(true),
                None => true,
            };
            if needs_start {
                *bt_guard = Self::spawn_bt_sink(&alsa_card);
            }
        } else if let Some(mut child) = bt_guard.take() {
            let _ = child.kill();
            let _ = child.wait();
            let _ = Command::new("bluetoothctl").args(&["discoverable", "off"]).status();
            info!("Bluetooth A2DP Sink stopped");
        }
    }

    fn spawn_airplay(name: &str, alsa_card: &str) -> Option<Child> {
        // Check for station-airplay or shairport-sync
        let candidates = [
            ("/data/station-airplay", vec!["--name", name, "--alsa", alsa_card]),
            ("station-airplay", vec!["--name", name, "--alsa", alsa_card]),
            ("shairport-sync", vec!["-a", name, "--", "-d", alsa_card]),
        ];

        for (bin, args) in candidates {
            match Command::new(bin)
                .args(&args)
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => {
                    info!("AirPlay receiver started ({}) with name '{}'", bin, name);
                    return Some(child);
                }
                Err(_) => continue,
            }
        }
        warn!("No AirPlay binary found (checked station-airplay, shairport-sync)");
        None
    }

    fn spawn_spotify(name: &str, alsa_card: &str) -> Option<Child> {
        let candidates = [
            "/data/librespot",
            "/usr/bin/librespot",
            "librespot",
        ];

        for bin in candidates {
            match Command::new(bin)
                .args(&[
                    "--name", name,
                    "--backend", "alsa",
                    "--device", alsa_card,
                    "--bitrate", "320",
                    "--initial-volume", "70",
                    "--enable-volume-normalisation",
                ])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => {
                    info!("Spotify Connect receiver started ({}) with name '{}'", bin, name);
                    return Some(child);
                }
                Err(_) => continue,
            }
        }
        warn!("No librespot binary found for Spotify Connect");
        None
    }

    fn spawn_snapcast(server: &str, port: u16) -> Option<Child> {
        let candidates = [
            "/data/snapclient",
            "/usr/bin/snapclient",
            "snapclient",
        ];

        for bin in candidates {
            match Command::new(bin)
                .args(&["-h", server, "-p", &port.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => {
                    info!("Snapcast client started connecting to {}:{}", server, port);
                    return Some(child);
                }
                Err(_) => continue,
            }
        }
        warn!("No snapclient binary found for Multiroom audio");
        None
    }

    fn spawn_bt_sink(alsa_card: &str) -> Option<Child> {
        // Enable discoverability and pairing on bluez
        let _ = Command::new("bluetoothctl").args(&["power", "on"]).status();
        let _ = Command::new("bluetoothctl").args(&["discoverable", "on"]).status();
        let _ = Command::new("bluetoothctl").args(&["pairable", "on"]).status();

        // Launch bluealsa-aplay to route A2DP audio to ALSA DAC
        let candidates = [
            "/data/bluealsa-aplay",
            "/usr/bin/bluealsa-aplay",
            "bluealsa-aplay",
        ];

        for bin in candidates {
            match Command::new(bin)
                .args(&["-d", alsa_card, "--profile-a2dp", "00:00:00:00:00:00"])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
            {
                Ok(child) => {
                    info!("Bluetooth A2DP Sink audio router started ({})", bin);
                    return Some(child);
                }
                Err(_) => continue,
            }
        }
        warn!("bluealsa-aplay not found (Bluetooth discoverable mode is still active)");
        None
    }

    /// Returns the live runtime status of all streaming receivers.
    pub async fn get_status(&self) -> StreamingStatus {
        let cfg = self.config.read().await;
        let s = &cfg.streaming;

        let airplay_running = self.airplay_child.write().await.as_mut()
            .map(|c| c.try_wait().map(|st| st.is_none()).unwrap_or(false))
            .unwrap_or(false);

        let spotify_running = self.spotify_child.write().await.as_mut()
            .map(|c| c.try_wait().map(|st| st.is_none()).unwrap_or(false))
            .unwrap_or(false);

        let snapcast_running = self.snapcast_child.write().await.as_mut()
            .map(|c| c.try_wait().map(|st| st.is_none()).unwrap_or(false))
            .unwrap_or(false);

        let bluetooth_sink_running = self.bt_sink_child.write().await.as_mut()
            .map(|c| c.try_wait().map(|st| st.is_none()).unwrap_or(false))
            .unwrap_or(false);

        StreamingStatus {
            airplay_enabled: s.airplay_enabled,
            airplay_running,
            airplay_name: s.airplay_name.clone(),
            spotify_enabled: s.spotify_enabled,
            spotify_running,
            spotify_name: s.spotify_name.clone(),
            snapcast_enabled: s.snapcast_enabled,
            snapcast_running,
            snapcast_server: s.snapcast_server.clone(),
            snapcast_port: s.snapcast_port,
            bluetooth_sink_enabled: s.bluetooth_sink_enabled,
            bluetooth_sink_running,
        }
    }

    /// Stops all running streaming processes on shutdown.
    pub async fn stop_all(&self) {
        if let Some(mut c) = self.airplay_child.write().await.take() {
            let _ = c.kill();
        }
        if let Some(mut c) = self.spotify_child.write().await.take() {
            let _ = c.kill();
        }
        if let Some(mut c) = self.snapcast_child.write().await.take() {
            let _ = c.kill();
        }
        if let Some(mut c) = self.bt_sink_child.write().await.take() {
            let _ = c.kill();
            let _ = Command::new("bluetoothctl").args(&["discoverable", "off"]).status();
        }
    }
}
