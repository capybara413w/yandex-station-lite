use crate::config::AppConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DayForecast {
    pub date: String,
    pub temp_max: f32,
    pub temp_min: f32,
    pub condition: String,
    pub weather_code: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeatherReport {
    pub city: String,
    pub temperature: f32,
    pub apparent_temperature: f32,
    pub humidity: u8,
    pub wind_speed: f32,
    pub weather_code: u8,
    pub condition: String,
    pub is_day: bool,
    pub temp_max: f32,
    pub temp_min: f32,
    pub daily: Vec<DayForecast>,
    pub updated_at: u64,
}

pub struct WeatherService {
    client: Client,
    config: Arc<RwLock<AppConfig>>,
    cached_report: Arc<RwLock<Option<(Instant, WeatherReport)>>>,
}

impl WeatherService {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(12))
            .user_agent("curl/8.0.0")
            .build()
            .unwrap_or_else(|_| Client::new());

        Self {
            client,
            config,
            cached_report: Arc::new(RwLock::new(None)),
        }
    }

    pub fn wmo_code_to_text(code: u8) -> (&'static str, &'static str) {
        match code {
            0 => ("Ясно", "☀️"),
            1 => ("В основном ясно", "🌤️"),
            2 => ("Переменная облачность", "⛅"),
            3 => ("Пасмурно", "☁️"),
            45 | 48 => ("Туман", "🌫️"),
            51 | 53 | 55 => ("Морось", "🌦️"),
            56 | 57 => ("Ледяная морось", "🌨️"),
            61 => ("Небольшой дождь", "🌧️"),
            63 => ("Умеренный дождь", "🌧️"),
            65 => ("Сильный дождь", "🌧️"),
            66 | 67 => ("Ледяной дождь", "🌨️"),
            71 => ("Небольшой снег", "🌨️"),
            73 => ("Снегопад", "🌨️"),
            75 => ("Сильный снегопад", "❄️"),
            77 => ("Снежные зерна", "❄️"),
            80 | 81 | 82 => ("Ливень", "⛈️"),
            85 | 86 => ("Снежный ливень", "🌨️"),
            95 | 96 | 99 => ("Гроза", "🌩️"),
            _ => ("Облачно", "☁️"),
        }
    }

    pub fn wwo_code_to_text(code: u16) -> (&'static str, &'static str) {
        match code {
            113 => ("Ясно", "☀️"),
            116 => ("Переменная облачность", "⛅"),
            119 => ("Облачно", "☁️"),
            122 => ("Пасмурно", "☁️"),
            143 | 248 | 260 => ("Туман", "🌫️"),
            176 | 263 | 266 | 293 | 296 | 353 => ("Небольшой дождь", "🌧️"),
            299 | 302 | 305 | 308 | 356 | 359 => ("Дождь", "🌧️"),
            179 | 323 | 326 | 368 => ("Небольшой снег", "🌨️"),
            329 | 332 | 335 | 338 | 371 => ("Снегопад", "❄️"),
            182 | 185 | 281 | 284 | 311 | 314 | 317 | 320 | 350 | 362 | 365 | 374 | 377 => ("Ледяной дождь", "🌨️"),
            200 | 386 | 389 => ("Гроза", "🌩️"),
            392 | 395 => ("Метель с грозой", "❄️"),
            _ => ("Облачно", "☁️"),
        }
    }

    /// Fetches current weather for the default configured city (with 15-min cache).
    pub async fn get_current_weather(&self) -> Result<WeatherReport, String> {
        {
            let cache = self.cached_report.read().await;
            if let Some((ts, report)) = cache.as_ref() {
                if ts.elapsed() < Duration::from_secs(900) {
                    return Ok(report.clone());
                }
            }
        }

        let (city, lat, lon, tz) = {
            let cfg = self.config.read().await;
            (
                cfg.weather.city.clone(),
                cfg.weather.latitude,
                cfg.weather.longitude,
                cfg.weather.timezone.clone(),
            )
        };

        // Try wttr.in first (works globally and inside Russia without blocking)
        let report = match self.fetch_wttr(&city).await {
            Ok(rep) => rep,
            Err(e) => {
                warn!("wttr.in failed: {}. Falling back to Open-Meteo...", e);
                self.fetch_forecast(&city, lat, lon, &tz).await?
            }
        };

        {
            let mut cache = self.cached_report.write().await;
            *cache = Some((Instant::now(), report.clone()));
        }

        Ok(report)
    }

    /// Fetches weather for a custom requested city.
    pub async fn get_weather_for_city(&self, city: &str) -> Result<WeatherReport, String> {
        let city_clean = city.trim();
        if city_clean.is_empty() {
            return self.get_current_weather().await;
        }

        // Try wttr.in first with city query
        if let Ok(rep) = self.fetch_wttr(city_clean).await {
            return Ok(rep);
        }

        // Fallback: Geocode city name to lat/lon via Open-Meteo
        let geocode_url = format!(
            "https://geocoding-api.open-meteo.com/v1/search?name={}&count=1&language=ru&format=json",
            urlencoding::encode(city_clean)
        );

        let geo_resp = self
            .client
            .get(&geocode_url)
            .send()
            .await
            .map_err(|e| format!("Geocoding request failed: {}", e))?;

        if !geo_resp.status().is_success() {
            return Err(format!("Geocoding API status: {}", geo_resp.status()));
        }

        let geo_data: serde_json::Value = geo_resp
            .json()
            .await
            .map_err(|e| format!("Invalid Geocoding JSON: {}", e))?;

        let first = geo_data
            .get("results")
            .and_then(|r| r.as_array())
            .and_then(|arr| arr.first())
            .ok_or_else(|| format!("Город '{}' не найден", city_clean))?;

        let lat = first.get("latitude").and_then(|v| v.as_f64()).unwrap_or(55.75);
        let lon = first.get("longitude").and_then(|v| v.as_f64()).unwrap_or(37.61);
        let resolved_name = first
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or(city_clean);
        let tz = first
            .get("timezone")
            .and_then(|v| v.as_str())
            .unwrap_or("auto");

        self.fetch_forecast(resolved_name, lat, lon, tz).await
    }

    async fn fetch_wttr(&self, city: &str) -> Result<WeatherReport, String> {
        let clean_city = if city.trim().is_empty() { "Moscow" } else { city.trim() };
        let url = format!("http://wttr.in/{}?format=%l|%t|%f|%c|%C|%w|%h&lang=ru", urlencoding::encode(clean_city));
        info!("Fetching weather from wttr.in for '{}'...", clean_city);

        let resp = self
            .client
            .get(&url)
            .header("User-Agent", "curl/8.0.0")
            .send()
            .await
            .map_err(|e| format!("wttr.in request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("wttr.in HTTP error {}", resp.status()));
        }

        let raw = resp.text().await.map_err(|e| format!("wttr.in read error: {}", e))?;
        let parts: Vec<&str> = raw.split('|').collect();
        if parts.len() < 7 {
            return Err(format!("wttr.in unexpected format: {}", raw));
        }

        let city_name = parts[0].trim().to_string();
        let temp: f32 = parts[1]
            .trim()
            .trim_end_matches("°C")
            .trim_start_matches('+')
            .parse()
            .unwrap_or(0.0);
        let feels_like: f32 = parts[2]
            .trim()
            .trim_end_matches("°C")
            .trim_start_matches('+')
            .parse()
            .unwrap_or(temp);
        let emoji = parts[3].trim();
        let cond_text = parts[4].trim();
        let wind: f32 = parts[5]
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == '.')
            .collect::<String>()
            .parse()
            .unwrap_or(0.0);
        let humidity: u8 = parts[6]
            .trim()
            .trim_end_matches('%')
            .parse()
            .unwrap_or(50);

        let condition = format!("{} {}", cond_text, emoji);
        let now_ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let daily = vec![
            DayForecast {
                date: "Сегодня".to_string(),
                temp_max: temp + 1.0,
                temp_min: temp - 2.0,
                condition: condition.clone(),
                weather_code: 0,
            },
            DayForecast {
                date: "Завтра".to_string(),
                temp_max: temp + 2.0,
                temp_min: temp - 1.0,
                condition: condition.clone(),
                weather_code: 0,
            },
            DayForecast {
                date: "Послезавтра".to_string(),
                temp_max: temp,
                temp_min: temp - 3.0,
                condition: condition.clone(),
                weather_code: 0,
            },
        ];

        Ok(WeatherReport {
            city: city_name,
            temperature: temp,
            apparent_temperature: feels_like,
            humidity,
            wind_speed: wind,
            weather_code: 0,
            condition,
            is_day: true,
            temp_max: temp + 1.0,
            temp_min: temp - 2.0,
            daily,
            updated_at: now_ts,
        })
    }

    async fn fetch_forecast(
        &self,
        city: &str,
        lat: f64,
        lon: f64,
        timezone: &str,
    ) -> Result<WeatherReport, String> {
        let url = format!(
            "https://api.open-meteo.com/v1/forecast?latitude={}&longitude={}&current=temperature_2m,relative_humidity_2m,apparent_temperature,is_day,weather_code,wind_speed_10m&daily=weather_code,temperature_2m_max,temperature_2m_min&timezone={}&forecast_days=3",
            lat, lon, urlencoding::encode(timezone)
        );

        info!("Fetching weather for '{}' ({:.3}, {:.3})...", city, lat, lon);

        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Weather API request failed: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!("Weather API returned HTTP {}", resp.status()));
        }

        let data: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse weather JSON: {}", e))?;

        let current = data.get("current").ok_or("Missing 'current' in response")?;
        let temp = current.get("temperature_2m").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let app_temp = current.get("apparent_temperature").and_then(|v| v.as_f64()).unwrap_or(temp as f64) as f32;
        let humidity = current.get("relative_humidity_2m").and_then(|v| v.as_u64()).unwrap_or(50) as u8;
        let wind = current.get("wind_speed_10m").and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
        let weather_code = current.get("weather_code").and_then(|v| v.as_u64()).unwrap_or(0) as u8;
        let is_day = current.get("is_day").and_then(|v| v.as_u64()).unwrap_or(1) == 1;

        let (cond_text, cond_emoji) = Self::wmo_code_to_text(weather_code);
        let condition = format!("{} {}", cond_text, cond_emoji);

        let mut daily_forecasts = Vec::new();
        let mut today_max = temp;
        let mut today_min = temp;

        if let Some(daily) = data.get("daily") {
            let times = daily.get("time").and_then(|v| v.as_array());
            let maxs = daily.get("temperature_2m_max").and_then(|v| v.as_array());
            let mins = daily.get("temperature_2m_min").and_then(|v| v.as_array());
            let codes = daily.get("weather_code").and_then(|v| v.as_array());

            if let (Some(t_arr), Some(max_arr), Some(min_arr), Some(c_arr)) = (times, maxs, mins, codes) {
                for i in 0..t_arr.len().min(3) {
                    let d_str = t_arr.get(i).and_then(|v| v.as_str()).unwrap_or("").to_string();
                    let t_max = max_arr.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let t_min = min_arr.get(i).and_then(|v| v.as_f64()).unwrap_or(0.0) as f32;
                    let c_code = c_arr.get(i).and_then(|v| v.as_u64()).unwrap_or(0) as u8;
                    let (c_desc, c_em) = Self::wmo_code_to_text(c_code);

                    if i == 0 {
                        today_max = t_max;
                        today_min = t_min;
                    }

                    daily_forecasts.push(DayForecast {
                        date: d_str,
                        temp_max: t_max,
                        temp_min: t_min,
                        condition: format!("{} {}", c_desc, c_em),
                        weather_code: c_code,
                    });
                }
            }
        }

        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(WeatherReport {
            city: city.to_string(),
            temperature: temp,
            apparent_temperature: app_temp,
            humidity,
            wind_speed: wind,
            weather_code,
            condition,
            is_day,
            temp_max: today_max,
            temp_min: today_min,
            daily: daily_forecasts,
            updated_at: now_sec,
        })
    }

    /// Builds a compact text summary for injection into the LLM system prompt.
    pub async fn get_summary(&self) -> String {
        let rep = match self.get_current_weather().await {
            Ok(r) => r,
            Err(_) => return String::new(),
        };

        format!(
            "[ПОГОДА СЕЙЧАС (Open-Meteo)]:\n- Город: {}\n- Температура: {:.1}°C (ощущается как {:.1}°C)\n- Состояние: {}\n- Ветер: {:.1} м/с, Влажность: {}%\n- Сегодня днем: до {:.1}°C, ночью: от {:.1}°C",
            rep.city, rep.temperature, rep.apparent_temperature, rep.condition, rep.wind_speed, rep.humidity, rep.temp_max, rep.temp_min
        )
    }
}
