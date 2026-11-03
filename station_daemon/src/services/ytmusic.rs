use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, ORIGIN, REFERER, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::config::AppConfig;

const YTM_BASE_URL: &str = "https://music.youtube.com";
const INNERTUBE_API_KEY: &str = "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";

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

pub struct YtMusicService {
    config: Arc<RwLock<AppConfig>>,
}

impl YtMusicService {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        Self { config }
    }

    /// Builds a proxy-aware reqwest Client
    async fn build_client(&self) -> Client {
        let cfg = self.config.read().await;
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(15));

        if cfg.proxy.enabled {
            let local_proxy = "http://127.0.0.1:8889";
            if let Ok(proxy) = reqwest::Proxy::all(local_proxy) {
                builder = builder.proxy(proxy);
            }
        }

        builder.build().unwrap_or_else(|_| Client::new())
    }

    /// Extracts SAPISID or __Secure-3PAPISID from cookie string
    fn extract_sapisid(cookie: &str) -> Option<String> {
        for part in cookie.split(';') {
            let trimmed = part.trim();
            if let Some((k, v)) = trimmed.split_once('=') {
                let key = k.trim().to_lowercase();
                if key == "sapisid" || key == "__secure-3papisid" || key == "__secure-1papisid" {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    /// Generates Google SAPISIDHASH header value
    fn generate_sapisid_hash(sapisid: &str, origin: &str) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let payload = format!("{} {} {}", now, sapisid, origin);
        let mut hasher = Sha1::new();
        hasher.update(payload.as_bytes());
        let digest_hex = hex::encode(hasher.finalize());
        format!("SAPISIDHASH {}_{}", now, digest_hex)
    }

    /// Constructs Innertube request headers with optional user authentication
    async fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static(
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36",
            ),
        );
        headers.insert(ORIGIN, HeaderValue::from_static(YTM_BASE_URL));
        headers.insert(REFERER, HeaderValue::from_static("https://music.youtube.com/"));
        headers.insert("x-origin", HeaderValue::from_static(YTM_BASE_URL));

        let cfg = self.config.read().await;
        let cookie = cfg.accounts.ytmusic_cookie.trim();
        if !cookie.is_empty() {
            if let Ok(val) = HeaderValue::from_str(cookie) {
                headers.insert(COOKIE, val);
            }

            if let Some(sapisid) = Self::extract_sapisid(cookie) {
                let auth_hash = Self::generate_sapisid_hash(&sapisid, YTM_BASE_URL);
                if let Ok(val) = HeaderValue::from_str(&auth_hash) {
                    headers.insert(AUTHORIZATION, val);
                }
            }
        }

        headers
    }

    /// Checks current account authorization status
    pub async fn get_status(&self) -> YtAccountStatus {
        let cfg = self.config.read().await;
        let cookie = cfg.accounts.ytmusic_cookie.trim();
        let has_cookie = !cookie.is_empty();
        let has_sapisid = Self::extract_sapisid(cookie).is_some();

        YtAccountStatus {
            is_authenticated: has_sapisid || has_cookie,
            has_cookie,
            user_name: None,
        }
    }

    /// Saves user cookie in configuration
    pub async fn update_cookie(&self, new_cookie: String) -> Result<(), String> {
        let mut cfg = self.config.write().await;
        cfg.accounts.ytmusic_cookie = new_cookie.trim().to_string();
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

    /// Search tracks on YouTube Music
    pub async fn search(&self, query: &str, limit: usize) -> Result<Vec<YtTrack>, String> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }

        let client = self.build_client().await;
        let headers = self.build_headers().await;

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20240701.01.00",
                    "hl": "ru",
                    "gl": "RU"
                }
            },
            "query": q
        });

        let url = format!("{}/youtubei/v1/search?alt=json&key={}", YTM_BASE_URL, INNERTUBE_API_KEY);
        let res = client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("YT Music search error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("YT Music search HTTP {}: {}", status, text));
        }

        let json: Value = res
            .json()
            .await
            .map_err(|e| format!("YT Music JSON parse error: {}", e))?;

        let mut tracks = Vec::new();

        // 1. Check Top Result shelf
        if let Some(card) = json.pointer(
            "/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicCardShelfRenderer",
        ) {
            let title = card
                .pointer("/title/runs/0/text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let artist = card
                .pointer("/subtitle/runs/0/text")
                .and_then(|v| v.as_str())
                .unwrap_or("YouTube Music");
            let vid = card
                .pointer("/buttons/0/buttonRenderer/command/watchEndpoint/videoId")
                .or_else(|| card.pointer("/onTap/watchEndpoint/videoId"))
                .and_then(|v| v.as_str())
                .unwrap_or("");

            let thumb = card
                .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
                .and_then(|t| t.as_array())
                .and_then(|arr| arr.last())
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            if !title.is_empty() && !vid.is_empty() {
                tracks.push(YtTrack {
                    id: vid.to_string(),
                    title: title.to_string(),
                    artist: artist.to_string(),
                    duration: "3:30".to_string(),
                    duration_seconds: 210,
                    artwork_url: thumb,
                    album: None,
                    source: "ytmusic".to_string(),
                });
            }
        }

        // 2. Extract results list
        let sections = json
            .pointer("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
            .and_then(|v| v.as_array());

        if let Some(secs) = sections {
            for sec in secs {
                let items = sec
                    .pointer("/musicShelfRenderer/contents")
                    .or_else(|| sec.pointer("/itemSectionRenderer/contents"))
                    .and_then(|v| v.as_array());

                if let Some(item_list) = items {
                    for it in item_list {
                        let renderer = it.get("musicResponsiveListItemRenderer");
                        if let Some(item) = renderer {
                            if let Some(track) = Self::parse_responsive_track(item) {
                                tracks.push(track);
                                if tracks.len() >= limit {
                                    break;
                                }
                            }
                        }
                    }
                }
                if tracks.len() >= limit {
                    break;
                }
            }
        }

        Ok(tracks)
    }

    /// Fetches YouTube Music personalized Home Feed ("Мой супермикс", "Быстрый выбор", etc.)
    pub async fn get_home(&self) -> Result<Vec<YtSection>, String> {
        let client = self.build_client().await;
        let headers = self.build_headers().await;

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20240701.01.00",
                    "hl": "ru",
                    "gl": "RU"
                }
            },
            "browseId": "FEmusic_home"
        });

        let url = format!("{}/youtubei/v1/browse?alt=json&key={}", YTM_BASE_URL, INNERTUBE_API_KEY);
        let res = client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("YT Music Home request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("YT Music Home HTTP {}: {}", status, text));
        }

        let json: Value = res
            .json()
            .await
            .map_err(|e| format!("YT Music Home JSON parse error: {}", e))?;

        let mut sections = Vec::new();

        let raw_sections = json
            .pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
            .and_then(|v| v.as_array());

        if let Some(secs) = raw_sections {
            for sec in secs {
                // Carousel shelf renderer
                if let Some(shelf) = sec.get("musicCarouselShelfRenderer") {
                    let title = shelf
                        .pointer("/header/musicCarouselShelfBasicHeaderRenderer/title/runs/0/text")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Рекомендации")
                        .to_string();

                    let mut items = Vec::new();
                    let mut playlists = Vec::new();

                    if let Some(contents) = shelf.get("contents").and_then(|v| v.as_array()) {
                        for item in contents {
                            if let Some(tr) = item.get("musicResponsiveListItemRenderer") {
                                if let Some(track) = Self::parse_responsive_track(tr) {
                                    items.push(track);
                                }
                            } else if let Some(two_row) = item.get("musicTwoRowItemRenderer") {
                                // Can be album or playlist or track
                                if let Some(pl) = Self::parse_two_row_playlist(two_row) {
                                    playlists.push(pl);
                                } else if let Some(track) = Self::parse_two_row_track(two_row) {
                                    items.push(track);
                                }
                            }
                        }
                    }

                    if !items.is_empty() || !playlists.is_empty() {
                        sections.push(YtSection {
                            title,
                            items,
                            playlists,
                        });
                    }
                }
            }
        }

        Ok(sections)
    }

    /// Fetches user's Liked Songs ("Понравившиеся треки")
    pub async fn get_liked_songs(&self, limit: usize) -> Result<Vec<YtTrack>, String> {
        let client = self.build_client().await;
        let headers = self.build_headers().await;

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20240701.01.00",
                    "hl": "ru",
                    "gl": "RU"
                }
            },
            "browseId": "FEmusic_liked_videos"
        });

        let url = format!("{}/youtubei/v1/browse?alt=json&key={}", YTM_BASE_URL, INNERTUBE_API_KEY);
        let res = client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Liked songs request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Liked songs HTTP {}: {}", status, text));
        }

        let json: Value = res
            .json()
            .await
            .map_err(|e| format!("Liked songs JSON parse error: {}", e))?;

        let mut tracks = Vec::new();
        let contents = json
            .pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer/contents")
            .and_then(|v| v.as_array());

        if let Some(list) = contents {
            for it in list {
                if let Some(renderer) = it.get("musicResponsiveListItemRenderer") {
                    if let Some(track) = Self::parse_responsive_track(renderer) {
                        tracks.push(track);
                        if tracks.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }

        Ok(tracks)
    }

    /// Fetches user's library playlists
    pub async fn get_library_playlists(&self) -> Result<Vec<YtPlaylist>, String> {
        let client = self.build_client().await;
        let headers = self.build_headers().await;

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20240701.01.00",
                    "hl": "ru",
                    "gl": "RU"
                }
            },
            "browseId": "FEmusic_library_playlists"
        });

        let url = format!("{}/youtubei/v1/browse?alt=json&key={}", YTM_BASE_URL, INNERTUBE_API_KEY);
        let res = client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Playlists request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Playlists HTTP {}: {}", status, text));
        }

        let json: Value = res
            .json()
            .await
            .map_err(|e| format!("Playlists JSON parse error: {}", e))?;

        let mut playlists = Vec::new();
        let items = json
            .pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/gridRenderer/items")
            .and_then(|v| v.as_array());

        if let Some(grid) = items {
            for it in grid {
                if let Some(two_row) = it.get("musicTwoRowItemRenderer") {
                    if let Some(pl) = Self::parse_two_row_playlist(two_row) {
                        playlists.push(pl);
                    }
                }
            }
        }

        Ok(playlists)
    }

    /// Fetches tracks from a specific playlist ID
    pub async fn get_playlist_tracks(&self, playlist_id: &str) -> Result<Vec<YtTrack>, String> {
        let browse_id = if playlist_id.starts_with("VL") {
            playlist_id.to_string()
        } else {
            format!("VL{}", playlist_id)
        };

        let client = self.build_client().await;
        let headers = self.build_headers().await;

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20240701.01.00",
                    "hl": "ru",
                    "gl": "RU"
                }
            },
            "browseId": browse_id
        });

        let url = format!("{}/youtubei/v1/browse?alt=json&key={}", YTM_BASE_URL, INNERTUBE_API_KEY);
        let res = client
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Playlist tracks request error: {}", e))?;

        if !res.status().is_success() {
            let status = res.status();
            let text = res.text().await.unwrap_or_default();
            return Err(format!("Playlist tracks HTTP {}: {}", status, text));
        }

        let json: Value = res
            .json()
            .await
            .map_err(|e| format!("Playlist tracks JSON parse error: {}", e))?;

        let mut tracks = Vec::new();
        let items = json
            .pointer("/contents/singleColumnBrowseResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents/0/musicPlaylistShelfRenderer/contents")
            .or_else(|| json.pointer("/contents/twoColumnBrowseResultsRenderer/secondaryContents/sectionListRenderer/contents/0/musicPlaylistShelfRenderer/contents"))
            .and_then(|v| v.as_array());

        if let Some(list) = items {
            for it in list {
                if let Some(tr) = it.get("musicResponsiveListItemRenderer") {
                    if let Some(track) = Self::parse_responsive_track(tr) {
                        tracks.push(track);
                    }
                }
            }
        }

        Ok(tracks)
    }

    /// Resolves direct raw audio stream URL from YouTube Innertube player API (Ad-Free)
    pub async fn resolve_stream_url(&self, video_id: &str) -> Result<String, String> {
        let vid = video_id.trim();
        if vid.is_empty() {
            return Err("Empty video ID".to_string());
        }

        let client = self.build_client().await;
        let mut headers = self.build_headers().await;
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("com.google.android.apps.youtube.music/6.20.51 (Linux; U; Android 11) gzip"),
        );

        // Try ANDROID_MUSIC client first for raw direct URLs
        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "ANDROID_MUSIC",
                    "clientVersion": "6.20.51",
                    "androidSdkVersion": 30,
                    "hl": "ru",
                    "gl": "RU"
                }
            },
            "videoId": vid
        });

        let url = format!("{}/youtubei/v1/player?alt=json&key={}", YTM_BASE_URL, INNERTUBE_API_KEY);
        let res = client
            .post(&url)
            .headers(headers.clone())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Player API error: {}", e))?;

        if res.status().is_success() {
            let json: Value = res.json().await.unwrap_or_default();
            if let Some(stream_url) = Self::extract_best_audio_url(&json) {
                return Ok(stream_url);
            }
        }

        // Fallback: WEB_REMIX client
        let web_body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20240701.01.00",
                    "hl": "ru",
                    "gl": "RU"
                }
            },
            "videoId": vid
        });

        let res_web = client
            .post(&url)
            .headers(headers)
            .json(&web_body)
            .send()
            .await
            .map_err(|e| format!("Player API web fallback error: {}", e))?;

        if res_web.status().is_success() {
            let json: Value = res_web.json().await.unwrap_or_default();
            if let Some(stream_url) = Self::extract_best_audio_url(&json) {
                return Ok(stream_url);
            }
        }

        Err(format!("Could not extract playable stream URL for video '{}'", vid))
    }

    /// Extracts highest bitrate progressive/adaptive audio URL from player JSON response
    fn extract_best_audio_url(json: &Value) -> Option<String> {
        let formats = json.pointer("/streamingData/adaptiveFormats")
            .or_else(|| json.pointer("/streamingData/formats"))
            .and_then(|v| v.as_array())?;

        let mut best_url: Option<String> = None;
        let mut best_bitrate: u64 = 0;

        for fmt in formats {
            let mime = fmt["mimeType"].as_str().unwrap_or("");
            if mime.starts_with("audio/") {
                if let Some(stream_url) = fmt["url"].as_str() {
                    let bitrate = fmt["bitrate"].as_u64().unwrap_or(0);
                    // Prefer Opus (audio/webm) or AAC (audio/mp4) with highest bitrate
                    if bitrate > best_bitrate {
                        best_bitrate = bitrate;
                        best_url = Some(stream_url.to_string());
                    }
                }
            }
        }

        best_url
    }

    /// Helper to parse musicResponsiveListItemRenderer
    fn parse_responsive_track(item: &Value) -> Option<YtTrack> {
        let vid = item
            .pointer("/playlistItemData/videoId")
            .or_else(|| item.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId"))
            .or_else(|| item.pointer("/navigationEndpoint/watchEndpoint/videoId"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if vid.is_empty() {
            return None;
        }

        let cols = item.get("flexColumns").and_then(|v| v.as_array())?;

        let title = cols
            .first()
            .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if title.is_empty() {
            return None;
        }

        let mut artist = "YouTube Music".to_string();
        let mut duration_str = "".to_string();
        let mut duration_secs = 0u64;

        if cols.len() > 1 {
            if let Some(runs) = cols[1]
                .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
                .and_then(|v| v.as_array())
            {
                let texts: Vec<&str> = runs
                    .iter()
                    .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
                    .collect();

                if let Some(first) = texts.first() {
                    artist = first.to_string();
                }

                // Check last run for duration (format like "3:45")
                if let Some(last) = texts.last() {
                    if last.contains(':') {
                        duration_str = last.to_string();
                        duration_secs = Self::parse_duration_string(last);
                    }
                }
            }
        }

        let thumb = item
            .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.last())
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some(YtTrack {
            id: vid.to_string(),
            title: title.to_string(),
            artist,
            duration: duration_str,
            duration_seconds: duration_secs,
            artwork_url: thumb,
            album: None,
            source: "ytmusic".to_string(),
        })
    }

    /// Helper to parse musicTwoRowItemRenderer as Playlist
    fn parse_two_row_playlist(item: &Value) -> Option<YtPlaylist> {
        let nav = item.pointer("/navigationEndpoint/browseEndpoint")?;
        let browse_id = nav.get("browseId").and_then(|v| v.as_str())?;

        if !browse_id.starts_with("VL") && !browse_id.starts_with("MPRE") && !browse_id.starts_with("FEmusic") {
            return None;
        }

        let title = item
            .pointer("/title/runs/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if title.is_empty() {
            return None;
        }

        let subtitle = item
            .pointer("/subtitle/runs/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        let thumb = item
            .pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.last())
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some(YtPlaylist {
            id: browse_id.to_string(),
            title: title.to_string(),
            subtitle: subtitle.to_string(),
            artwork_url: thumb,
            track_count: None,
        })
    }

    /// Helper to parse musicTwoRowItemRenderer as Track
    fn parse_two_row_track(item: &Value) -> Option<YtTrack> {
        let vid = item
            .pointer("/navigationEndpoint/watchEndpoint/videoId")
            .or_else(|| item.pointer("/onTap/watchEndpoint/videoId"))
            .and_then(|v| v.as_str())?;

        let title = item
            .pointer("/title/runs/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim();

        if title.is_empty() {
            return None;
        }

        let artist = item
            .pointer("/subtitle/runs/0/text")
            .and_then(|v| v.as_str())
            .unwrap_or("YouTube Music")
            .trim();

        let thumb = item
            .pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails")
            .and_then(|t| t.as_array())
            .and_then(|arr| arr.last())
            .and_then(|v| v.get("url"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        Some(YtTrack {
            id: vid.to_string(),
            title: title.to_string(),
            artist: artist.to_string(),
            duration: String::new(),
            duration_seconds: 0,
            artwork_url: thumb,
            album: None,
            source: "ytmusic".to_string(),
        })
    }

    /// Converts "MM:SS" string to seconds
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
