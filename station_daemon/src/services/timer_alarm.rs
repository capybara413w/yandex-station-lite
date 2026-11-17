use crate::hardware::LedController;
use crate::services::tts::TtsService;
use chrono::{Datelike, Local, Timelike};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimerItem {
    pub id: String,
    pub label: String,
    pub duration_secs: u64,
    pub created_at: u64,
    pub ends_at: u64,
    pub is_ringing: bool,
}

impl TimerItem {
    pub fn remaining_secs(&self, now: u64) -> u64 {
        if now >= self.ends_at {
            0
        } else {
            self.ends_at - now
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlarmItem {
    pub id: String,
    pub time: String, // "HH:MM"
    pub label: String,
    pub enabled: bool,
    pub repeat: String, // "once", "daily", "weekdays", "weekends"
    #[serde(default)]
    pub last_triggered_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TimerCall {
    pub action: String, // "set", "cancel", "cancel_all"
    #[serde(alias = "seconds")]
    pub duration_secs: Option<u64>,
    pub label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlarmCall {
    pub action: String, // "set", "cancel", "enable", "disable"
    pub time: Option<String>,
    pub label: Option<String>,
    pub repeat: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SavedAlarmsStore {
    #[serde(default)]
    alarms: Vec<AlarmItem>,
}

pub struct TimerAlarmService {
    timers: Arc<RwLock<Vec<TimerItem>>>,
    alarms: Arc<RwLock<Vec<AlarmItem>>>,
    storage_path: PathBuf,
    is_ringing: Arc<AtomicBool>,
    ringing_pid: Arc<AtomicU32>,
    tts: Option<Arc<TtsService>>,
    led: Option<Arc<LedController>>,
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn generate_id(prefix: &str) -> String {
    let now = now_secs();
    let rand_part = (now ^ 0x5a5a) % 10000;
    format!("{}_{}_{}", prefix, now, rand_part)
}

impl TimerAlarmService {
    pub fn new<P: AsRef<Path>>(
        storage_path: P,
        tts: Option<Arc<TtsService>>,
        led: Option<Arc<LedController>>,
    ) -> Arc<Self> {
        let p = storage_path.as_ref().to_path_buf();
        let loaded_alarms = Self::load_alarms(&p);

        let svc = Arc::new(Self {
            timers: Arc::new(RwLock::new(Vec::new())),
            alarms: Arc::new(RwLock::new(loaded_alarms)),
            storage_path: p,
            is_ringing: Arc::new(AtomicBool::new(false)),
            ringing_pid: Arc::new(AtomicU32::new(0)),
            tts,
            led,
        });

        // Start background ticker
        let svc_clone = svc.clone();
        tokio::spawn(async move {
            svc_clone.run_ticker().await;
        });

        svc
    }

    fn load_alarms(path: &Path) -> Vec<AlarmItem> {
        if path.exists() {
            if let Ok(c) = fs::read_to_string(path) {
                if let Ok(store) = serde_json::from_str::<SavedAlarmsStore>(&c) {
                    info!("Loaded {} alarms from {}", store.alarms.len(), path.display());
                    return store.alarms;
                }
            }
        }
        Vec::new()
    }

    async fn persist_alarms(&self) {
        let store = SavedAlarmsStore {
            alarms: self.alarms.read().await.clone(),
        };
        if let Ok(json) = serde_json::to_string_pretty(&store) {
            let _ = fs::write(&self.storage_path, json);
        }
    }

    // ------------------------------------------------------------------------
    // Timer Operations
    // ------------------------------------------------------------------------

    pub async fn add_timer(&self, duration_secs: u64, label: Option<&str>) -> TimerItem {
        let now = now_secs();
        let timer = TimerItem {
            id: generate_id("tm"),
            label: label
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Таймер")
                .to_string(),
            duration_secs,
            created_at: now,
            ends_at: now + duration_secs,
            is_ringing: false,
        };

        info!("Created timer: '{}' on {}s (ends at {})", timer.label, duration_secs, timer.ends_at);
        let mut list = self.timers.write().await;
        list.push(timer.clone());
        timer
    }

    pub async fn cancel_timer(&self, id: &str) -> bool {
        let mut list = self.timers.write().await;
        let orig = list.len();
        list.retain(|t| t.id != id);
        let removed = list.len() < orig;
        if removed {
            info!("Cancelled timer '{}'", id);
        }
        removed
    }

    pub async fn cancel_all_timers(&self) -> usize {
        let mut list = self.timers.write().await;
        let count = list.len();
        list.clear();
        if count > 0 {
            info!("Cancelled all {} timers", count);
        }
        count
    }

    pub async fn list_timers(&self) -> Vec<TimerItem> {
        self.timers.read().await.clone()
    }

    // ------------------------------------------------------------------------
    // Alarm Operations
    // ------------------------------------------------------------------------

    pub async fn add_alarm(&self, time_str: &str, label: Option<&str>, repeat: Option<&str>) -> Result<AlarmItem, String> {
        let time_clean = time_str.trim();
        // Validate HH:MM
        let parts: Vec<&str> = time_clean.split(':').collect();
        if parts.len() != 2 {
            return Err("Time must be in format HH:MM".to_string());
        }
        let h: u32 = parts[0].parse().map_err(|_| "Invalid hour")?;
        let m: u32 = parts[1].parse().map_err(|_| "Invalid minute")?;
        if h > 23 || m > 59 {
            return Err("Time values out of range".to_string());
        }

        let formatted_time = format!("{:02}:{:02}", h, m);
        let rep = repeat.unwrap_or("once");
        let rep_valid = match rep {
            "daily" | "weekdays" | "weekends" => rep.to_string(),
            _ => "once".to_string(),
        };

        let alarm = AlarmItem {
            id: generate_id("al"),
            time: formatted_time,
            label: label
                .filter(|s| !s.trim().is_empty())
                .unwrap_or("Будильник")
                .to_string(),
            enabled: true,
            repeat: rep_valid,
            last_triggered_date: None,
        };

        info!("Created alarm: '{}' at {} (repeat: {})", alarm.label, alarm.time, alarm.repeat);
        {
            let mut list = self.alarms.write().await;
            list.push(alarm.clone());
        }
        self.persist_alarms().await;

        Ok(alarm)
    }

    pub async fn delete_alarm(&self, id: &str) -> bool {
        let mut list = self.alarms.write().await;
        let orig = list.len();
        list.retain(|a| a.id != id);
        let removed = list.len() < orig;
        drop(list);

        if removed {
            info!("Deleted alarm '{}'", id);
            self.persist_alarms().await;
        }
        removed
    }

    pub async fn toggle_alarm(&self, id: &str, enabled: bool) -> bool {
        let mut list = self.alarms.write().await;
        if let Some(a) = list.iter_mut().find(|a| a.id == id) {
            a.enabled = enabled;
            drop(list);
            self.persist_alarms().await;
            return true;
        }
        false
    }

    pub async fn list_alarms(&self) -> Vec<AlarmItem> {
        self.alarms.read().await.clone()
    }

    // ------------------------------------------------------------------------
    // Ringing & Alarm Audio Playback
    // ------------------------------------------------------------------------

    pub fn is_ringing(&self) -> bool {
        self.is_ringing.load(Ordering::SeqCst)
    }

    pub fn stop_ringing(&self) {
        if !self.is_ringing() {
            return;
        }

        info!("Stopping alarm / timer ringing chime!");
        self.is_ringing.store(false, Ordering::SeqCst);

        let pid = self.ringing_pid.swap(0, Ordering::SeqCst);
        if pid > 0 {
            let _ = Command::new("kill").args(["-9", &pid.to_string()]).output();
        }
        let _ = Command::new("killall").args(["-9", "mpg123"]).output();

        if let Some(led) = &self.led {
            led.clear();
        }
    }

    fn trigger_ringing(
        self: Arc<Self>,
        title: String,
        is_alarm: bool,
    ) {
        if self.is_ringing() {
            return;
        }

        self.is_ringing.store(true, Ordering::SeqCst);
        info!("RINGING ACTIVATED: '{}' (alarm={})", title, is_alarm);

        let this = self.clone();
        tokio::spawn(async move {
            let sound_file = if Path::new("/system/vendor/quasar/sounds/common/timer.opus").exists() {
                "/system/vendor/quasar/sounds/common/timer.opus"
            } else if Path::new("/system/vendor/quasar/sounds/common/calls_ringing_incoming.mp3").exists() {
                "/system/vendor/quasar/sounds/common/calls_ringing_incoming.mp3"
            } else {
                "/system/vendor/quasar/sounds/common/notification_sound.opus"
            };

            let mpg_bin = if Path::new("/opt/bin/mpg123").exists() {
                "/opt/bin/mpg123"
            } else {
                "mpg123"
            };

            // Animate LED ring in vivid pulsating amber/red [255, 100, 0]
            let is_ringing_flag = this.is_ringing.clone();
            tokio::spawn(async move {
                let factors = [10, 8, 6, 3, 2, 3, 6, 8];
                while is_ringing_flag.load(Ordering::SeqCst) {
                    for &f in &factors {
                        if !is_ringing_flag.load(Ordering::SeqCst) {
                            break;
                        }
                        let cr = (255 * f / 10) as u8;
                        let cg = (100 * f / 10) as u8;
                        for i in 1..=3 {
                            let _ = fs::write(format!("/sys/class/leds/micro:red:ring{}/brightness", i), cr.to_string());
                            let _ = fs::write(format!("/sys/class/leds/micro:green:ring{}/brightness", i), cg.to_string());
                            let _ = fs::write(format!("/sys/class/leds/micro:blue:ring{}/brightness", i), "0");
                        }
                        tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
                    }
                }
            });

            // Read volume scale from /data/volume
            let vol_step = fs::read_to_string("/data/volume")
                .ok()
                .and_then(|s| s.trim().parse::<u32>().ok())
                .unwrap_or(7)
                .clamp(1, 10);
            let scale = (vol_step * 1000).clamp(1000, 15000);

            // Ring loop (up to 60 seconds)
            let start = tokio::time::Instant::now();
            let mut spoken = false;

            while this.is_ringing() && start.elapsed().as_secs() < 60 {
                // Play 1 round of timer chime
                let this_pid = this.ringing_pid.clone();
                let file_ref = sound_file.to_string();
                let scale_str = scale.to_string();

                let _ = tokio::task::spawn_blocking(move || {
                    if let Ok(mut child) = Command::new(mpg_bin)
                        .args(["-q", "-a", "hw:0,0", "-f", &scale_str, &file_ref])
                        .spawn()
                    {
                        this_pid.store(child.id(), Ordering::SeqCst);
                        let _ = child.wait();
                        this_pid.store(0, Ordering::SeqCst);
                    }
                })
                .await;

                // After first chime round, speak voice alert via TTS if available
                if !spoken && this.is_ringing() {
                    spoken = true;
                    if let (Some(tts), Some(led)) = (&this.tts, &this.led) {
                        let phrase = if is_alarm {
                            format!("Доброе утро! Будильник: {}.", title)
                        } else {
                            format!("Время вышло! Таймер завершен.")
                        };
                        let _ = tts.speak(&phrase, [255, 120, 0], led).await;
                    }
                }

                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;
            }

            this.stop_ringing();
        });
    }

    // ------------------------------------------------------------------------
    // Background Ticker (checks timers and alarms every 500ms)
    // ------------------------------------------------------------------------

    async fn run_ticker(self: Arc<Self>) {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));
        loop {
            interval.tick().await;

            let now = now_secs();

            // 1. Check Timers
            {
                let mut timers = self.timers.write().await;
                let mut triggered_label = None;
                for t in timers.iter_mut() {
                    if now >= t.ends_at && !t.is_ringing {
                        t.is_ringing = true;
                        triggered_label = Some(t.label.clone());
                        break;
                    }
                }
                // Remove expired finished timers after ringing
                timers.retain(|t| now < t.ends_at || t.is_ringing);

                if let Some(lbl) = triggered_label {
                    self.clone().trigger_ringing(lbl, false);
                }
            }

            // 2. Check Alarms
            let now_local = Local::now();
            let current_hhmm = format!("{:02}:{:02}", now_local.hour(), now_local.minute());
            let current_date = format!("{:04}-{:02}-{:02}", now_local.year(), now_local.month(), now_local.day());
            let weekday = now_local.weekday(); // Mon..Sun

            let is_weekend = matches!(weekday, chrono::Weekday::Sat | chrono::Weekday::Sun);
            let is_weekday = !is_weekend;

            let mut alarm_triggered = None;
            {
                let mut alarms = self.alarms.write().await;
                for a in alarms.iter_mut() {
                    if !a.enabled {
                        continue;
                    }
                    if a.time == current_hhmm {
                        if a.last_triggered_date.as_deref() == Some(&current_date) {
                            continue; // already fired this minute
                        }

                        let match_repeat = match a.repeat.as_str() {
                            "daily" => true,
                            "weekdays" => is_weekday,
                            "weekends" => is_weekend,
                            _ => true, // "once"
                        };

                        if match_repeat {
                            a.last_triggered_date = Some(current_date.clone());
                            if a.repeat == "once" {
                                a.enabled = false;
                            }
                            alarm_triggered = Some(a.label.clone());
                            break;
                        }
                    }
                }
            }

            if let Some(lbl) = alarm_triggered {
                self.persist_alarms().await;
                self.clone().trigger_ringing(lbl, true);
            }
        }
    }

    // ------------------------------------------------------------------------
    // LLM Context Summary & Intent Processing
    // ------------------------------------------------------------------------

    pub async fn get_summary(&self) -> String {
        let now_local = Local::now();
        let time_str = format!("{:02}:{:02}", now_local.hour(), now_local.minute());
        let now = now_secs();

        let mut lines = Vec::new();
        lines.push(format!("\n[ТЕКУЩЕЕ ВРЕМЯ, ТАЙМЕРЫ И БУДИЛЬНИКИ]:"));
        lines.push(format!("- Точное локальное время: {} (день: {:?})", time_str, now_local.weekday()));

        let timers = self.timers.read().await;
        if timers.is_empty() {
            lines.push("- Активных таймеров нет.".to_string());
        } else {
            lines.push("- Активные таймеры:".to_string());
            for t in timers.iter() {
                let rem = t.remaining_secs(now);
                let m = rem / 60;
                let s = rem % 60;
                lines.push(format!("  * \"{}\": осталось {} мин {} сек (всего {} сек)", t.label, m, s, t.duration_secs));
            }
        }

        let alarms = self.alarms.read().await;
        let enabled_alarms: Vec<_> = alarms.iter().filter(|a| a.enabled).collect();
        if enabled_alarms.is_empty() {
            lines.push("- Включенных будильников нет.".to_string());
        } else {
            lines.push("- Включенные будильники:".to_string());
            for a in enabled_alarms {
                lines.push(format!("  * {} (\"{}\", повтор: {})", a.time, a.label, a.repeat));
            }
        }

        lines.push("\nИНСТРУКЦИЯ ПО ТАЙМЕРАМ И БУДИЛЬНИКАМ:".to_string());
        lines.push("1. Если просят поставить таймер: добавь \"timer_call\": {\"action\": \"set\", \"duration_secs\": N, \"label\": \"...\"}.".to_string());
        lines.push("2. Если просят отменить/сбросить таймер: добавь \"timer_call\": {\"action\": \"cancel_all\"}.".to_string());
        lines.push("3. Если спрашивают 'сколько осталось на таймере': ответь голосом по актуальным данным выше.".to_string());
        lines.push("4. Если просят поставить будильник: добавь \"alarm_call\": {\"action\": \"set\", \"time\": \"HH:MM\", \"repeat\": \"once\"|\"daily\"|\"weekdays\"}.".to_string());
        lines.push("5. Если просят выключить будильник: добавь \"alarm_call\": {\"action\": \"disable\", \"time\": \"HH:MM\"}.".to_string());
        lines.push("6. Если звенит таймер/будильник и пользователь говорит 'стоп', 'хватит', 'выключи': поставь \"action\": \"stop\".".to_string());

        lines.join("\n")
    }

    /// Process timer_call from LLM JSON response
    pub async fn process_timer_call(&self, call: &TimerCall) -> Option<String> {
        match call.action.as_str() {
            "set" => {
                if let Some(secs) = call.duration_secs {
                    if secs > 0 {
                        let _t = self.add_timer(secs, call.label.as_deref()).await;
                        let m = secs / 60;
                        let s = secs % 60;
                        if m > 0 && s > 0 {
                            return Some(format!("Таймер установлен на {} мин {} сек", m, s));
                        } else if m > 0 {
                            return Some(format!("Таймер установлен на {} мин", m));
                        } else {
                            return Some(format!("Таймер установлен на {} сек", s));
                        }
                    }
                }
            }
            "cancel" | "cancel_all" => {
                let cnt = self.cancel_all_timers().await;
                return Some(format!("Отменено таймеров: {}", cnt));
            }
            _ => {}
        }
        None
    }

    /// Heuristic extractor for timer commands if LLM omitted timer_call
    pub fn extract_timer_from_query(query: &str) -> Option<TimerCall> {
        let lower = query.to_lowercase();
        if lower.contains("таймер") {
            if lower.contains("отмени") || lower.contains("выключи") || lower.contains("сбрось") || lower.contains("удали") || lower.contains("стоп") {
                return Some(TimerCall {
                    action: "cancel_all".to_string(),
                    duration_secs: None,
                    label: None,
                });
            }
            if lower.contains("поставь") || lower.contains("заведи") || lower.contains("включи") || lower.contains("установи") || lower.contains("напомни") {
                let mut total_secs = 0u64;
                if lower.contains("полчаса") || lower.contains("пол часа") {
                    total_secs += 1800;
                } else if lower.contains("час") && !lower.contains("сейчас") {
                    total_secs += 3600;
                }

                let words: Vec<&str> = lower.split_whitespace().collect();
                for i in 0..words.len() {
                    if let Ok(num) = words[i].parse::<u64>() {
                        if i + 1 < words.len() {
                            let next = words[i + 1];
                            if next.starts_with("мин") {
                                total_secs += num * 60;
                            } else if next.starts_with("сек") {
                                total_secs += num;
                            } else if next.starts_with("час") {
                                total_secs += num * 3600;
                            }
                        }
                    }
                }

                if total_secs > 0 {
                    return Some(TimerCall {
                        action: "set".to_string(),
                        duration_secs: Some(total_secs),
                        label: Some("Таймер".to_string()),
                    });
                }
            }
        }
        None
    }

    /// Heuristic extractor for alarm commands if LLM omitted alarm_call
    pub fn extract_alarm_from_query(query: &str) -> Option<AlarmCall> {
        let lower = query.to_lowercase();
        if lower.contains("будильник") || lower.contains("разбуди") {
            if lower.contains("отмени") || lower.contains("выключи") || lower.contains("удали") || lower.contains("сними") {
                return Some(AlarmCall {
                    action: "disable".to_string(),
                    time: None,
                    label: None,
                    repeat: None,
                });
            }
            if lower.contains("поставь") || lower.contains("заведи") || lower.contains("установи") || lower.contains("разбуди") {
                for word in lower.split_whitespace() {
                    let clean = word.trim_matches(|c: char| !c.is_numeric() && c != ':');
                    if clean.contains(':') {
                        let parts: Vec<&str> = clean.split(':').collect();
                        if parts.len() == 2 {
                            if let (Ok(h), Ok(m)) = (parts[0].parse::<u32>(), parts[1].parse::<u32>()) {
                                if h < 24 && m < 60 {
                                    return Some(AlarmCall {
                                        action: "set".to_string(),
                                        time: Some(format!("{:02}:{:02}", h, m)),
                                        label: Some("Будильник".to_string()),
                                        repeat: Some("once".to_string()),
                                    });
                                }
                            }
                        }
                    }
                }

                let words: Vec<&str> = lower.split_whitespace().collect();
                for i in 0..words.len() {
                    if let Ok(mut h) = words[i].parse::<u32>() {
                        if h < 24 {
                            if (lower.contains("вечера") || lower.contains("дня")) && h < 12 {
                                h += 12;
                            }
                            return Some(AlarmCall {
                                action: "set".to_string(),
                                time: Some(format!("{:02}:00", h)),
                                label: Some("Будильник".to_string()),
                                repeat: Some("once".to_string()),
                            });
                        }
                    }
                }
            }
        }
        None
    }

    /// Process alarm_call from LLM JSON response
    pub async fn process_alarm_call(&self, call: &AlarmCall) -> Option<String> {
        match call.action.as_str() {
            "set" => {
                if let Some(t) = &call.time {
                    if let Ok(a) = self.add_alarm(t, call.label.as_deref(), call.repeat.as_deref()).await {
                        return Some(format!("Будильник установлен на {}", a.time));
                    }
                }
            }
            "disable" | "cancel" => {
                if let Some(t) = &call.time {
                    let mut alarms = self.alarms.write().await;
                    if let Some(a) = alarms.iter_mut().find(|a| a.time == *t) {
                        a.enabled = false;
                        drop(alarms);
                        self.persist_alarms().await;
                        return Some(format!("Будильник на {} отключен", t));
                    }
                }
            }
            "enable" => {
                if let Some(t) = &call.time {
                    let mut alarms = self.alarms.write().await;
                    if let Some(a) = alarms.iter_mut().find(|a| a.time == *t) {
                        a.enabled = true;
                        drop(alarms);
                        self.persist_alarms().await;
                        return Some(format!("Будильник на {} включен", t));
                    }
                }
            }
            _ => {}
        }
        None
    }
}
