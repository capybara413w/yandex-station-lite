// Based on YTerMusic by ccgauche (https://github.com/ccgauche/ytermusic)
// Adapted for station_daemon (Yandex Station Lite) under GNU GPL v3.0

pub mod instance;
pub mod json_extractor;
pub mod string_utils;

use std::sync::Arc;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::config::AppConfig;
pub use self::instance::TrackStreamInfo;
use self::instance::{Endpoint, YoutubeMusicInstance};
use self::json_extractor::{YoutubeMusicPlaylistRef, YoutubeMusicVideoRef};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtTrack {
    pub id: String,
    pub title: String,
    pub artist: String,
    pub duration: String,
    pub duration_seconds: u64,
    pub artwork_url: Option<String>,
    pub album: Option<String>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtPlaylist {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub artwork_url: Option<String>,
    pub track_count: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtSection {
    pub title: String,
    pub items: Vec<YtTrack>,
    pub playlists: Vec<YtPlaylist>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YtAccountStatus {
    pub is_authenticated: bool,
    pub has_cookie: bool,
    pub user_name: Option<String>,
}

/// YTerMusic-based YouTube Music Service for station_daemon
/// Original author: ccgauche (https://github.com/ccgauche/ytermusic)
#[derive(Clone)]
pub struct YtMusicService {
    config: Arc<RwLock<AppConfig>>,
}

impl YtMusicService {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        Self { config }
    }

    /// Builds a YoutubeMusicInstance with current proxy and cookie settings
    async fn get_instance(&self) -> Result<YoutubeMusicInstance, String> {
        let cfg = self.config.read().await;
        let cookie = cfg.accounts.ytmusic_cookie.trim();
        let proxy_url = if cfg.proxy.enabled {
            Some("http://127.0.0.1:8889".to_string())
        } else {
            None
        };

        YoutubeMusicInstance::new(cookie, None, proxy_url).await
    }

    /// Checks current account authorization status
    pub async fn get_status(&self) -> YtAccountStatus {
        let cfg = self.config.read().await;
        let cookie = cfg.accounts.ytmusic_cookie.trim();
        let has_cookie = !cookie.is_empty();
        let has_sapisid = YoutubeMusicInstance::extract_sapisid_from_raw(cookie).is_some();

        YtAccountStatus {
            is_authenticated: has_sapisid || has_cookie,
            has_cookie,
            user_name: None,
        }
    }

    /// Saves user cookie in configuration (accepts raw string, cURL, or Cookie-Editor JSON)
    pub async fn update_cookie(&self, new_cookie: String) -> Result<(), String> {
        let mut clean = new_cookie.trim().to_string();
        if clean.starts_with('[') || clean.starts_with('{') {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&clean) {
                let items: Vec<&serde_json::Value> = if let Some(arr) = val.as_array() {
                    arr.iter().collect()
                } else if val.is_object() {
                    vec![&val]
                } else {
                    Vec::new()
                };

                let mut pairs = Vec::new();
                for it in items {
                    let name = it.get("name").or_else(|| it.get("Name")).and_then(|v| v.as_str());
                    let value = it.get("value").or_else(|| it.get("Value")).and_then(|v| v.as_str());
                    if let (Some(n), Some(v)) = (name, value) {
                        pairs.push(format!("{}={}", n, v));
                    }
                }
                if !pairs.is_empty() {
                    clean = pairs.join("; ");
                }
            }
        }

        let mut cfg = self.config.write().await;
        cfg.accounts.ytmusic_cookie = clean;
        let save_path = if std::path::Path::new("/data").exists() {
            "/data/config.json"
        } else {
            "config.json"
        };
        if let Ok(json_str) = serde_json::to_string_pretty(&*cfg) {
            let _ = tokio::fs::write(save_path, json_str).await;
        }
        Ok(())
    }

    /// Search tracks on YouTube Music via YTerMusic engine
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<YtTrack>, String> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }

        let instance = self.get_instance().await?;
        let continuations = (limit / 20).clamp(1, 4);
        let results = instance.search(q, continuations).await?;

        let mut tracks: Vec<YtTrack> = results.videos
            .into_iter()
            .map(Self::video_to_track)
            .collect();

        if tracks.len() > limit {
            tracks.truncate(limit);
        }

        Ok(tracks)
    }

    /// Fetches YouTube Music personalized / trending home feed
    pub async fn get_home(&self) -> Result<Vec<YtSection>, String> {
        let instance = self.get_instance().await?;
        let results = instance.get_home(1).await?;

        let mut sections = Vec::new();

        if !results.videos.is_empty() {
            sections.push(YtSection {
                title: "Рекомендации".to_string(),
                items: results.videos.into_iter().map(Self::video_to_track).collect(),
                playlists: Vec::new(),
            });
        }

        if !results.playlists.is_empty() {
            sections.push(YtSection {
                title: "Плейлисты и подборки".to_string(),
                items: Vec::new(),
                playlists: results.playlists.into_iter().map(Self::playlist_to_playlist).collect(),
            });
        }

        Ok(sections)
    }

    /// Fetches user Liked Songs ("VLLM" playlist in YouTube Music)
    pub async fn get_liked_songs(&self, limit: usize) -> Result<Vec<YtTrack>, String> {
        let instance = self.get_instance().await?;
        if !instance.is_authenticated() {
            return Err("Для просмотра понравившихся треков требуется авторизация YouTube Music (Cookie)".to_string());
        }

        let continuations = (limit / 25).clamp(1, 6);
        // "VLLM" is the universal YouTube Music liked music playlist browseId ("VL" + "LM")
        let videos = match instance.get_playlist_raw("VLLM", continuations).await {
            Ok(v) if !v.is_empty() => v,
            Ok(_) | Err(_) => {
                // Fallback 1: Try with "LM" (instance will ensure VL prefix)
                match instance.get_playlist_raw("LM", continuations).await {
                    Ok(v) if !v.is_empty() => v,
                    _ => {
                        // Fallback 2: Check liked playlists landing
                        match instance.get_library(&Endpoint::MusicLikedPlaylists, 2).await {
                            Ok(library) => {
                                if let Some(first) = library.first() {
                                    instance.get_playlist_raw(&first.browse_id, continuations).await.unwrap_or_default()
                                } else {
                                    Vec::new()
                                }
                            }
                            Err(_) => Vec::new(),
                        }
                    }
                }
            }
        };

        let mut tracks: Vec<YtTrack> = videos.into_iter().map(Self::video_to_track).collect();
        if tracks.len() > limit {
            tracks.truncate(limit);
        }

        if tracks.is_empty() {
            return Err("Список понравившихся треков пуст или срок действия Cookie YouTube Music истек. Обновите Cookie в веб-интерфейсе.".to_string());
        }

        Ok(tracks)
    }

    /// Fetches user saved playlists from library
    pub async fn get_library_playlists(&self) -> Result<Vec<YtPlaylist>, String> {
        let instance = self.get_instance().await?;
        if !instance.is_authenticated() {
            return Err("Для доступа к медиатеке требуется авторизация YouTube Music (Cookie)".to_string());
        }

        let playlists = instance.get_library(&Endpoint::MusicLibraryLanding, 2).await?;
        Ok(playlists.into_iter().map(Self::playlist_to_playlist).collect())
    }

    /// Fetches tracks of a specific playlist
    pub async fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<YtTrack>, String> {
        let pid = playlist_id.trim();
        if pid.is_empty() {
            return Ok(Vec::new());
        }

        let instance = self.get_instance().await?;
        let videos = instance.get_playlist_raw(pid, 2).await?;
        Ok(videos.into_iter().map(Self::video_to_track).collect())
    }

    /// Resolves direct audio stream URL and track metadata
    pub async fn resolve_stream_info(&self, video_id: &str) -> Result<TrackStreamInfo, String> {
        let instance = self.get_instance().await?;
        instance.resolve_stream_info(video_id).await
    }

    /// Resolves direct audio stream URL
    pub async fn resolve_stream_url(&self, video_id: &str) -> Result<String, String> {
        let instance = self.get_instance().await?;
        instance.resolve_stream_url(video_id).await
    }

    /// Fetches automatic recommendations / autoplay queue for a given track
    pub async fn get_autoplay_tracks(&self, video_id: &str) -> Result<Vec<YtTrack>, String> {
        let instance = self.get_instance().await?;
        let videos = instance.get_autoplay_tracks(video_id).await?;
        Ok(videos.into_iter().map(Self::video_to_track).collect())
    }

    fn video_to_track(v: YoutubeMusicVideoRef) -> YtTrack {
        let duration_seconds = Self::parse_duration_string(&v.duration);
        YtTrack {
            id: v.video_id,
            title: v.title,
            artist: v.author,
            duration: v.duration,
            duration_seconds,
            artwork_url: v.artwork_url,
            album: if v.album.is_empty() { None } else { Some(v.album) },
            source: "ytmusic".to_string(),
        }
    }

    fn playlist_to_playlist(p: YoutubeMusicPlaylistRef) -> YtPlaylist {
        YtPlaylist {
            id: p.browse_id,
            title: p.name,
            subtitle: p.subtitle,
            artwork_url: p.artwork_url,
            track_count: None,
        }
    }

    fn parse_duration_string(s: &str) -> u64 {
        let parts: Vec<&str> = s.split(':').collect();
        match parts.len() {
            2 => {
                let m: u64 = parts[0].parse().unwrap_or(0);
                let sec: u64 = parts[1].parse().unwrap_or(0);
                m * 60 + sec
            }
            3 => {
                let h: u64 = parts[0].parse().unwrap_or(0);
                let m: u64 = parts[1].parse().unwrap_or(0);
                let sec: u64 = parts[2].parse().unwrap_or(0);
                h * 3600 + m * 60 + sec
            }
            _ => 0,
        }
    }
}
