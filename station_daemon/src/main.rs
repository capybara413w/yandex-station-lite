mod config;
mod hardware;
mod services;
mod web;

use config::AppConfig;
use hardware::{
    AudioHardware, ButtonsListener, LedController, NetworkHardware, StationButtonEvent,
};
use services::{
    play_assistant_chime, play_sound, ChatService, DeepSeekService, HomeAssistantService,
    PlayerService, ProxyBridge, SoundCloudService, StreamingService, SttService,
    TimerAlarmService, TtsService, VisualizerService, WakeWordService, WeatherService,
    YtMusicService,
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

    // Free any stale ALSA captures or playbacks
    let _ = std::process::Command::new("killall").args(["-9", "arecord", "aplay"]).output();

    // Check CLI arguments for Dual Boot controls
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--quasar") {
        info!("CLI flag --quasar detected: launching Yandex Quasar...");
        let _ = std::process::Command::new("/data/dualboot.sh")
            .arg("--quasar")
            .status();
        return Ok(());
    }
    if args.iter().any(|a| a == "--skip-choice" || a == "--daemon") {
        info!("CLI flag --skip-choice: skipping dualboot countdown window, staying in Station Daemon.");
        let _ = std::fs::write("/tmp/dualboot_stay_daemon", "1");
        let _ = std::fs::remove_file("/tmp/dualboot_active");
    }

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

    // Apply DNS configuration on boot
    if let Err(e) = crate::services::dns::apply_dns(&initial_config.dns).await {
        warn!("Could not apply DNS configuration on startup: {}", e);
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
    let homeassistant = Arc::new(HomeAssistantService::new(config.clone()));
    let soundcloud = Arc::new(SoundCloudService::new(config.clone()));
    let ytmusic = Arc::new(YtMusicService::new(config.clone()));
    let stt = Arc::new(SttService::new(
        initial_config.audio.record_device.clone(),
        initial_config.audio.sample_rate,
        config.clone(),
        &initial_config.proxy,
    ));
    let (player_instance, autoplay_rx) = PlayerService::new(config.clone());
    let player = Arc::new(player_instance);
    PlayerService::start_autoplay_worker(player.clone(), ytmusic.clone(), led.clone(), autoplay_rx);
    let tts = Arc::new(TtsService::new(config.clone(), &initial_config.proxy));
    let visualizer = Arc::new(VisualizerService::new(&initial_config.hardware.led_sysfs_base));
    visualizer.clone().start(tts.clone());

    let chat_storage_path = if Path::new("/data").exists() {
        "/data/chat_history.json"
    } else {
        "chat_history.json"
    };
    let chat = Arc::new(ChatService::new(chat_storage_path));

    let weather = Arc::new(WeatherService::new(config.clone()));
    let alarms_path = if Path::new("/data").exists() {
        "/data/station_alarms.json"
    } else {
        "station_alarms.json"
    };
    let timer_alarm = TimerAlarmService::new(alarms_path, Some(tts.clone()), Some(led.clone()));

    // 5. Assistant Turn Execution Engine (Triggered by Wake-Word or CenterHold)
    let (assistant_tx, mut assistant_rx) = mpsc::channel::<Option<String>>(16);

    let wakeword = Arc::new(WakeWordService::new(
        &initial_config,
        stt.clone(),
        tts.clone(),
        player.clone(),
        led.clone(),
    ));
    wakeword.clone().start_worker(assistant_tx.clone());

    let led_ref = led.clone();
    let stt_ref = stt.clone();
    let ds_ref = deepseek.clone();
    let player_ref = player.clone();
    let sc_ref = soundcloud.clone();
    let yt_ref = ytmusic.clone();
    let tts_ref = tts.clone();
    let chat_ref = chat.clone();
    let ww_ref = wakeword.clone();
    let ha_ref = homeassistant.clone();
    let weather_ref = weather.clone();
    let timer_alarm_ref = timer_alarm.clone();

    tokio::spawn(async move {
        while let Some(initial_prompt) = assistant_rx.recv().await {
            let h = tokio::spawn(execute_assistant_turn(
                initial_prompt,
                led_ref.clone(),
                stt_ref.clone(),
                ds_ref.clone(),
                ha_ref.clone(),
                weather_ref.clone(),
                timer_alarm_ref.clone(),
                player_ref.clone(),
                sc_ref.clone(),
                yt_ref.clone(),
                tts_ref.clone(),
                chat_ref.clone(),
                ww_ref.clone(),
            ));
            if let Err(e) = h.await {
                error!("Voice assistant turn caught error or panic: {:?}", e);
                ww_ref.set_processing(false);
            }
        }
    });

    // 6. Button Event Processing Loop — station/buttons_daemon.sh
    let (btn_tx, mut btn_rx) = mpsc::channel::<StationButtonEvent>(32);
    buttons.start(btn_tx.clone());

    let audio_clone = audio.clone();
    let led_clone = led.clone();
    let player_clone = player.clone();
    let tts_clone = tts.clone();
    let timer_alarm_clone = timer_alarm.clone();
    let assistant_tx_clone = assistant_tx.clone();
    let ww_clone = wakeword.clone();
    let alsa_card_str = initial_config.audio.alsa_card.clone();
    let config_btn = config.clone();

    tokio::spawn(async move {
        while let Some(event) = btn_rx.recv().await {
            // Signal dualboot supervisor that a physical hardware button was pressed!
            if Path::new("/tmp/dualboot_active").exists() {
                let _ = std::fs::write("/tmp/dualboot_stay_daemon", "1");
                let _ = std::fs::remove_file("/tmp/dualboot_active");
                info!("DualBoot: hardware button event detected ({:?}) -> cancelled auto-switch, staying in Station Daemon!", event);
            }

            match event {
                // Volume Up (+)
                StationButtonEvent::VolumeUp => {
                    if timer_alarm_clone.is_ringing() {
                        timer_alarm_clone.stop_ringing();
                        led_clone.blink(0, 255, 100, 100);
                        continue;
                    }
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
                    if timer_alarm_clone.is_ringing() {
                        timer_alarm_clone.stop_ringing();
                        led_clone.blink(0, 255, 100, 100);
                        continue;
                    }
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
                    // If timer/alarm is ringing -> stop ringing immediately!
                    if timer_alarm_clone.is_ringing() {
                        info!("CenterTap stopped ringing timer/alarm");
                        timer_alarm_clone.stop_ringing();
                        led_clone.blink(0, 255, 100, 100);
                        continue;
                    }
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
                    // 1. Release ALSA mic device (hw:0,1) immediately from wakeword capture
                    ww_clone.pause_and_stop_capture();
                    // 2. Pause music if playing
                    player_clone.pause(&led_clone).await;
                    // 3. Illuminate Alice Purple
                    led_clone.set_rgb(180, 0, 255);
                    // 4. Play soundpack assistant chime
                    let active_pack = config_btn.read().await.soundpack.active_pack.clone();
                    play_sound(&active_pack, "activation", &alsa_card_str);
                    // 5. Trigger assistant turn with VAD
                    let _ = assistant_tx_clone.send(None).await;
                }
            }
        }
    });

    // 7. Initialize Streaming Receivers (AirPlay, Spotify Connect, Snapcast)
    let streaming = Arc::new(StreamingService::new(config.clone()));
    streaming.sync_services().await;

    // 8. Launch Axum HTTP Server & Web Control Panel
    let app_state = web::AppState {
        config: config.clone(),
        audio: audio.clone(),
        led: led.clone(),
        network: network.clone(),
        deepseek: deepseek.clone(),
        player: player.clone(),
        soundcloud: soundcloud.clone(),
        ytmusic: ytmusic.clone(),
        stt: stt.clone(),
        tts: tts.clone(),
        visualizer: visualizer.clone(),
        btn_tx: btn_tx.clone(),
        logs: logs.clone(),
        start_time: std::time::Instant::now(),
        chat: chat.clone(),
        wakeword: wakeword.clone(),
        homeassistant: homeassistant.clone(),
        weather: weather.clone(),
        timer_alarm: timer_alarm.clone(),
        streaming: streaming.clone(),
    };

    let router = web::create_router(app_state);
    let addr: SocketAddr = format!("{}:{}", bind_address, server_port)
        .parse()
        .unwrap_or_else(|_| SocketAddr::from(([0, 0, 0, 0], 5000)));

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

/// Helper: Converts STT error string into a clear, understandable Russian message and LED color
fn humanize_stt_error(e: &str) -> (&'static str, [u8; 3]) {
    let lower = e.to_lowercase();
    if lower.contains("таймаут") || lower.contains("речь не обнаружена") || lower.contains("timeout") || lower.contains("no speech") {
        ("Я вас не услышала. Пожалуйста, повторите.", [255, 140, 0])
    } else if lower.contains("записано 0") || lower.contains("empty") || lower.contains("0 байт") {
        ("Не удалось записать звук с микрофона.", [255, 100, 0])
    } else if lower.contains("busy") || lower.contains("занято") {
        ("Микрофон сейчас занят другой задачей.", [255, 100, 0])
    } else if lower.contains("401") || lower.contains("403") || lower.contains("unauthorized") || lower.contains("forbidden") || lower.contains("api key") {
        ("Ошибка распознавания речи: неверный API-ключ. Проверьте настройки в веб-панели.", [255, 0, 0])
    } else if lower.contains("429") || lower.contains("quota") || lower.contains("rate limit") || lower.contains("credit") || lower.contains("insufficient") {
        ("Превышен лимит распознавания речи или закончился баланс.", [255, 0, 0])
    } else if lower.contains("connect") || lower.contains("network") || lower.contains("dns") || lower.contains("proxy") || lower.contains("сеть") || lower.contains("timed out") {
        ("Не удалось связаться с сервисом распознавания речи. Проверьте интернет.", [255, 50, 50])
    } else {
        ("Не удалось распознать речь. Попробуйте ещё раз.", [255, 50, 50])
    }
}

/// Helper: Converts LLM / DeepSeek error string into a clear, understandable Russian message and LED color
fn humanize_llm_error(e: &str) -> (&'static str, [u8; 3]) {
    let lower = e.to_lowercase();
    if lower.contains("401") || lower.contains("unauthorized") || lower.contains("invalid api key") || lower.contains("bearer token") || lower.contains("api key is empty") {
        ("Ошибка авторизации нейросети: неверный API-ключ. Проверьте ключ в настройках веб-панели.", [255, 0, 0])
    } else if lower.contains("402") || lower.contains("payment") || lower.contains("insufficient") || lower.contains("credit") || lower.contains("balance") {
        ("На балансе нейросети закончились средства. Пополните счёт в вашем аккаунте.", [255, 0, 0])
    } else if lower.contains("429") || lower.contains("rate limit") || lower.contains("too many requests") || lower.contains("quota") {
        ("Превышен лимит запросов к нейросети. Пожалуйста, подождите минуту и повторите.", [255, 80, 0])
    } else if lower.contains("500") || lower.contains("502") || lower.contains("503") || lower.contains("504") || lower.contains("bad gateway") || lower.contains("service unavailable") {
        ("Сервер нейросети временно перегружен или недоступен. Попробуйте чуть позже.", [255, 50, 50])
    } else if lower.contains("timed out") || lower.contains("timeout") || lower.contains("таймаут") {
        ("Время ожидания ответа от нейросети истекло. Повторите запрос.", [255, 50, 50])
    } else if lower.contains("connect") || lower.contains("network") || lower.contains("dns") || lower.contains("proxy") || lower.contains("connection refused") {
        ("Не удалось связаться с нейросетью. Проверьте подключение к интернету или прокси.", [255, 50, 50])
    } else {
        ("Произошла ошибка при обращении к нейросети. Попробуйте позже.", [255, 0, 0])
    }
}

/// Executes a complete Assistant Turn:
/// - If `initial_prompt` is `Some(cmd)`, uses the one-shot voice command directly (skips recording).
/// - If `initial_prompt` is `None` (CenterHold or standalone Wake-Word), records speech using real-time VAD.
/// - Queries LLM with multi-turn chat history.
/// - Speaks answer via Native TTS.
/// - Clearly informs user via speech and LED if any error occurs at any stage.
/// - Executes music actions (play/stop/resume).
async fn execute_assistant_turn(
    initial_prompt: Option<String>,
    led_ref: Arc<LedController>,
    stt_ref: Arc<SttService>,
    ds_ref: Arc<DeepSeekService>,
    ha_ref: Arc<HomeAssistantService>,
    weather_ref: Arc<WeatherService>,
    timer_alarm_ref: Arc<TimerAlarmService>,
    player_ref: Arc<PlayerService>,
    sc_ref: Arc<SoundCloudService>,
    yt_ref: Arc<YtMusicService>,
    tts_ref: Arc<TtsService>,
    chat_ref: Arc<ChatService>,
    ww_ref: Arc<WakeWordService>,
) {
    ww_ref.set_processing(true);

    let recognized_text = match initial_prompt {
        Some(cmd) if !cmd.trim().is_empty() => {
            led_ref.set_rgb(220, 150, 0); // Amber: processing one-shot command
            cmd
        }
        _ => {
            // Standalone wake word or touch hold:
            // Record & transcribe speech using dynamic VAD
            match stt_ref.record_and_transcribe(&led_ref).await {
                Ok(text) => {
                    let trimmed = text.trim();
                    if trimmed.is_empty() {
                        warn!("STT returned empty text");
                        let err_msg = "Не удалось разобрать речь. Повторите, пожалуйста.";
                        led_ref.blink(255, 140, 0, 200);
                        let _ = tts_ref.speak(err_msg, [255, 140, 0], &led_ref).await;
                        ww_ref.set_processing(false);
                        return;
                    }
                    text
                }
                Err(e) => {
                    warn!("STT recording/transcription failed: {}", e);
                    let (err_msg, color) = humanize_stt_error(&e);
                    led_ref.blink(color[0], color[1], color[2], 200);
                    let _ = tts_ref.speak(err_msg, color, &led_ref).await;
                    ww_ref.set_processing(false);
                    return;
                }
            }
        }
    };

    info!("Voice command recognized: '{}'", recognized_text);

    // Multi-turn session & 1-hour rule context
    let (session_id, history, _) = chat_ref.prepare_session_for_prompt(None).await;

    // 1. Fetch live Contexts: Home Assistant, Weather, Timers & Alarms (ALL queries go strictly through LLM)
    let mut context_parts = Vec::new();
    let ha_context = ha_ref.get_entities_summary().await;
    if !ha_context.is_empty() {
        context_parts.push(ha_context);
    }
    let weather_context = weather_ref.get_summary().await;
    if !weather_context.is_empty() {
        context_parts.push(weather_context);
    }
    let timer_context = timer_alarm_ref.get_summary().await;
    if !timer_context.is_empty() {
        context_parts.push(timer_context);
    }
    let effective_context = context_parts.join("\n\n");

    // 2. Send query to LLM with history and live combined context
    match ds_ref.query_with_context(&recognized_text, &history, &effective_context).await {
        Ok(resp) => {
            info!("LLM reply: '{}' (action: {}, ha_calls: {})", resp.text, resp.action, resp.ha_calls.len());

            // Execute any Home Assistant API calls returned strictly by LLM
            for ha_call in &resp.ha_calls {
                info!("LLM calling Home Assistant: {}/{} data={:?}", ha_call.domain, ha_call.service, ha_call.service_data);
                if let Err(e) = ha_ref.call_service(&ha_call.domain, &ha_call.service, ha_call.service_data.clone()).await {
                    error!("Failed to execute HA call {}/{}: {}", ha_call.domain, ha_call.service, e);
                }
            }

            // Execute Timer calls returned strictly by LLM
            if let Some(t_call) = &resp.timer_call {
                timer_alarm_ref.process_timer_call(t_call).await;
            }

            // Execute Alarm calls returned strictly by LLM
            if let Some(a_call) = &resp.alarm_call {
                timer_alarm_ref.process_alarm_call(a_call).await;
            }

            // Stop alarm/timer chime if requested or if ringing
            if resp.action == "stop" || timer_alarm_ref.is_ringing() {
                timer_alarm_ref.stop_ringing();
            }

            let turn_action = if !resp.ha_calls.is_empty() {
                format!("ha_calls:{}", resp.ha_calls.len())
            } else {
                resp.action.clone()
            };

            chat_ref
                .record_turn(
                    &session_id,
                    &recognized_text,
                    &resp.text,
                    &turn_action,
                    resp.color,
                    &resp.song,
                    resp.ha_calls.clone(),
                )
                .await;

            // Native Speech Synthesis (TTS) & Breathing Animation
            if !resp.text.is_empty() {
                if let Err(e) = tts_ref.speak(&resp.text, resp.color, &led_ref).await {
                    error!("TTS playback failed: {}", e);
                    for _ in 0..3 {
                        led_ref.blink(255, 0, 0, 150);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                    }
                }
            }

            // Handle Music Actions via YouTube Music (100% exact match)
            match resp.action.as_str() {
                "play" => {
                    let song = if resp.song.is_empty() {
                        "Lofi chill beats"
                    } else {
                        &resp.song
                    };
                    player_ref.play_smart_or_query(&yt_ref, &sc_ref, song, &led_ref).await;
                }
                "stop" => player_ref.stop(&led_ref).await,
                "resume" => player_ref.resume(&led_ref).await,
                _ => {}
            }
        }
        Err(e) => {
            error!("DeepSeek query failed: {}", e);
            let (err_msg, color) = humanize_llm_error(&e);
            chat_ref
                .record_turn(
                    &session_id,
                    &recognized_text,
                    &format!("⚠️ {}", err_msg),
                    "error",
                    color,
                    "",
                    Vec::new(),
                )
                .await;
            led_ref.blink(color[0], color[1], color[2], 200);
            let _ = tts_ref.speak(err_msg, color, &led_ref).await;
        }
    }

    ww_ref.set_processing(false);
}
