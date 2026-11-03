mod config;
mod hardware;
mod services;
mod web;

use config::AppConfig;
use hardware::{
    AudioHardware, ButtonsListener, LedController, NetworkHardware, StationButtonEvent,
};
use services::{
    DeepSeekService, PlayerService, ProxyBridge, SoundCloudService, SttService, TtsService,
    VisualizerService,
};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{error, info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Set execution PATH for Entware utilities (/opt/bin, /opt/sbin)
    let path = std::env::var("PATH").unwrap_or_default();
    std::env::set_var("PATH", format!("/data/opt/bin:/data/opt/sbin:/opt/bin:/opt/sbin:{}", path));

    // 1. Initialize lightweight tracing subscriber
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(tracing_subscriber::fmt::layer().compact())
        .init();

    info!("=== Starting Yandex Station Lite Autonomous Daemon (aarch64) ===");

    // 2. Load configuration
    let config_path = if Path::new("/data/config.json").exists() {
        "/data/config.json"
    } else if Path::new("config.json").exists() {
        "config.json"
    } else {
        "config.example.json"
    };

    let initial_config = AppConfig::load_or_default(config_path);
    let server_port = initial_config.server.port;
    let bind_address = initial_config.server.bind_address.clone();

    info!(
        "Config loaded. Server port: {}, Mixer: {}, Input device: {}",
        server_port, initial_config.audio.mixer_control_name, initial_config.hardware.input_event_device
    );

    // 3. Initialize Hardware Controllers
    let audio = Arc::new(AudioHardware::new(&initial_config.audio));
    let led = Arc::new(LedController::new(&initial_config.hardware));
    let buttons = ButtonsListener::new(&initial_config.hardware);
    let network = Arc::new(NetworkHardware::new());

    // Disable Wi-Fi power saving immediately so RTL8822CS never enters sleep mode
    if let Err(e) = network.optimize_powersave() {
        warn!("Could not optimize 88x2cs Wi-Fi power saving on startup: {}", e);
    }

    // Initial hardware setup: apply default volume step and startup LED glow
    audio.set_volume(initial_config.audio.initial_volume);
    led.set_rgb(0, 150, 255); // Calming cyan startup aura

    // Shared thread-safe config & log buffer for web control panel
    let config = Arc::new(tokio::sync::RwLock::new(initial_config.clone()));
    let logs = Arc::new(web::LogBuffer::new());

    // 4. Initialize Core Native Services (Player, SoundCloud, STT, DeepSeek, Native TTS, Proxy Bridge)
    let proxy_bridge = Arc::new(ProxyBridge::new(config.clone()));
    proxy_bridge.start(8889);

    let deepseek = Arc::new(DeepSeekService::new(config.clone(), &initial_config.proxy));
    let soundcloud = Arc::new(SoundCloudService::new(config.clone()));
    let stt = Arc::new(SttService::new(
        initial_config.audio.record_device.clone(),
        initial_config.audio.sample_rate,
        initial_config.stt.clone(),
        &initial_config.proxy,
    ));
    let player = Arc::new(PlayerService::new(config.clone()));
    let tts = Arc::new(TtsService::new(config.clone(), &initial_config.proxy));
    let visualizer = Arc::new(VisualizerService::new(&initial_config.hardware.led_sysfs_base));
    visualizer.clone().start(tts.clone());

    // 5. Button Event Processing Loop — station/buttons_daemon.sh
    let (btn_tx, mut btn_rx) = mpsc::channel::<StationButtonEvent>(32);
    buttons.start(btn_tx.clone());

    let audio_clone = audio.clone();
    let led_clone = led.clone();
    let player_clone = player.clone();
    let soundcloud_clone = soundcloud.clone();
    let stt_clone = stt.clone();
    let deepseek_clone = deepseek.clone();
    let tts_clone = tts.clone();

    tokio::spawn(async move {
        while let Some(event) = btn_rx.recv().await {
            match event {
                // Volume Up (+)
                StationButtonEvent::VolumeUp => {
                    let (new_vol, changed) = audio_clone.adjust_volume(1);
                    if changed {
                        info!("Hardware Event: VolumeUp -> step {}/10", new_vol);
                        led_clone.blink(0, 255, 100, 100);
                    } else {
                        info!("Hardware Event: VolumeUp -> MAX 10");
                        led_clone.blink(255, 0, 0, 100);
                    }
                }
                // Volume Down (-)
                StationButtonEvent::VolumeDown => {
                    let (new_vol, changed) = audio_clone.adjust_volume(-1);
                    if changed {
                        info!("Hardware Event: VolumeDown -> step {}/10", new_vol);
                        led_clone.blink(255, 140, 0, 100);
                    } else {
                        info!("Hardware Event: VolumeDown -> MIN 1");
                        led_clone.blink(255, 0, 0, 100);
                    }
                }
                // Center Touch Sensor: Short Tap (< 0.7 sec)
                StationButtonEvent::CenterTap => {
                    info!("Hardware Event: CenterTap (Play/Pause Toggle / Interrupt Speech)");
                    // If native TTS is actively speaking -> interrupt speech immediately
                    if tts_clone.is_speaking() {
                        tts_clone.stop(&led_clone);
                        led_clone.blink(255, 140, 0, 100);
                    } else {
                        player_clone.toggle_play_pause(&led_clone).await;
                    }
                }
                // Center Touch Sensor: Long Hold (>= 0.7 sec)
                StationButtonEvent::CenterHold => {
                    info!("Hardware Event: CenterHold >= 700ms (Voice Assistant Trigger)");

                    let led_ref = led_clone.clone();
                    let stt_ref = stt_clone.clone();
                    let ds_ref = deepseek_clone.clone();
                    let player_ref = player_clone.clone();
                    let sc_ref = soundcloud_clone.clone();
                    let tts_ref = tts_clone.clone();

                    tokio::spawn(async move {
                        // 1. Record & transcribe speech
                        match stt_ref.record_and_transcribe(&led_ref).await {
                            Ok(recognized_text) => {
                                info!("Voice command recognized: '{}'", recognized_text);

                                // 2. Send query to DeepSeek
                                match ds_ref.query(&recognized_text).await {
                                    Ok(resp) => {
                                        info!("DeepSeek reply: '{}' (action: {})", resp.text, resp.action);

                                        // 3. Native Speech Synthesis (TTS) & Breathing Animation
                                        if !resp.text.is_empty() {
                                            let _ = tts_ref.speak(&resp.text, resp.color, &led_ref).await;
                                        }

                                        // 4. Handle Music Actions via SoundCloud
                                        match resp.action.as_str() {
                                            "play" => {
                                                let song = if resp.song.is_empty() {
                                                    "Lofi chill beats"
                                                } else {
                                                    &resp.song
                                                };
                                                player_ref.play_soundcloud_or_query(&sc_ref, song, &led_ref).await;
                                            }
                                            "stop" => player_ref.stop(&led_ref).await,
                                            "resume" => player_ref.resume(&led_ref).await,
                                            _ => {}
                                        }
                                    }
                                    Err(e) => {
                                        error!("DeepSeek query failed: {}", e);
                                        led_ref.blink(255, 0, 0, 200);
                                    }
                                }
                            }
                            Err(e) => {
                                warn!("STT recording/transcription failed: {}", e);
                                led_ref.blink(255, 140, 0, 200);
                            }
                        }
                    });
                }
            }
        }
    });

    // 6. Launch Axum HTTP Server & Web Control Panel
    let app_state = web::AppState {
        config: config.clone(),
        audio: audio.clone(),
        led: led.clone(),
        network: network.clone(),
        deepseek: deepseek.clone(),
        player: player.clone(),
        soundcloud: soundcloud.clone(),
        stt: stt.clone(),
        tts: tts.clone(),
        visualizer: visualizer.clone(),
        btn_tx: btn_tx.clone(),
        logs: logs.clone(),
        start_time: std::time::Instant::now(),
    };

    let router = web::create_router(app_state);
    let addr: SocketAddr = format!("{}:{}", bind_address, server_port)
        .parse()
        .unwrap_or_else(|_| "0.0.0.0:5000".parse().unwrap());

    let local_ip = web::get_local_ip();
    info!("╔════════════════════════════════════════════════════════════════════╗");
    info!("║   YANDEX STATION LITE AUTONOMOUS DAEMON (A113X aarch64)            ║");
    info!("║                                                                    ║");
    info!("║   ► ВЕБ-ПАНЕЛЬ УПРАВЛЕНИЯ В ЛОКАЛЬНОЙ СЕТИ:                        ║");
    info!("║     http://{}:{}                                       ║", local_ip, server_port);
    info!("║     http://0.0.0.0:{}                                                ║", server_port);
    info!("║                                                                    ║");
    info!("║   Подключайтесь из браузера с телефона, ноутбука или планшета!     ║");
    info!("╚════════════════════════════════════════════════════════════════════╝");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
