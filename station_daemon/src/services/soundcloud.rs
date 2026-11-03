use crate::config::AppConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

const DEFAULT_CLIENT_ID: &str = "swtEGNKrP2w6Z5B8DVvAurWCwNQBlgMd";
const BACKUP_CLIENT_IDS: &[&str] = &[
    "swtEGNKrP2w6Z5B8DVvAurWCwNQBlgMd",
    "xk6BGxLCsQJ5Ss0bH6sRdsZhts3gTNaU",
    "a3e059563d7fd3372b49b37f00a00bcf",
    "2t9loNfh0ekOfbnLeBYjdO50ExQ0edIF",
    "iZIs9mchVcX5lhVR1EzGCcyddKZaPjai",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoundCloudTrack {
    pub id: u64,
    pub title: String,
    pub artist: String,
    pub duration_ms: u64,
    pub artwork_url: Option<String>,
    pub permalink_url: String,
    #[serde(default)]
    pub stream_url: Option<String>,
    #[serde(default)]
    pub is_hls: bool,
    #[serde(default)]
    pub source: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedStream {
    pub track_title: String,
    pub artist: String,
    pub stream_url: String,
    pub is_hls: bool,
    pub artwork_url: Option<String>,
    #[serde(default)]
    pub duration_ms: u64,
}

fn clean_yt_artist_string(raw: &str) -> String {
    let mut parts = Vec::new();
    for p in raw.split('•') {
        let trimmed = p.trim();
        let lower = trimmed.to_lowercase();
        if trimmed.is_empty()
            || lower.contains("view")
            || lower.contains("просмотр")
            || lower.contains(':')
            || lower == "video"
            || lower == "song"
            || lower == "track"
        {
            continue;
        }
        parts.push(trimmed);
    }
    if parts.is_empty() {
        "YouTube Music".to_string()
    } else {
        parts.join(" ")
    }
}

pub struct SoundCloudService {
    config: Arc<RwLock<AppConfig>>,
    client_id: Arc<RwLock<String>>,
}

impl SoundCloudService {
    pub fn new(config: Arc<RwLock<AppConfig>>) -> Self {
        Self {
            config,
            client_id: Arc::new(RwLock::new(DEFAULT_CLIENT_ID.to_string())),
        }
    }

    /// Dynamically builds a reqwest Client respecting the currently configured SOCKS5 proxy
    pub async fn build_client(&self) -> Client {
        let cfg = self.config.read().await;
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(8))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36");

        if cfg.proxy.enabled {
            let local_proxy = "http://127.0.0.1:8889";
            if let Ok(p) = reqwest::Proxy::all(local_proxy) {
                builder = builder.proxy(p);
            }
        }

        builder.build().unwrap_or_else(|_| Client::new())
    }

    pub async fn get_client_id(&self) -> String {
        self.client_id.read().await.clone()
    }

    /// Dynamically extracts a fresh client_id from soundcloud.com script assets if needed
    pub async fn refresh_client_id(&self) -> Result<String, String> {
        info!("Refreshing SoundCloud client_id from soundcloud.com web assets...");
        let client = self.build_client().await;
        let resp = client
            .get("https://soundcloud.com")
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .send()
            .await
            .map_err(|e| format!("Failed to fetch soundcloud.com: {}", e))?;

        let html = resp.text().await.map_err(|e| e.to_string())?;

        // Find asset scripts: https://a-v2.sndcdn.com/assets/...js
        let mut script_urls = Vec::new();
        for part in html.split('"') {
            if part.starts_with("https://a-v2.sndcdn.com/assets/") && part.ends_with(".js") {
                script_urls.push(part.to_string());
            }
        }

        // Search script assets for client_id:"..."
        for script_url in script_urls.into_iter().rev().take(10) {
            if let Ok(js_res) = client.get(&script_url).send().await {
                if let Ok(js_text) = js_res.text().await {
                    if let Some(idx) = js_text.find("client_id:\"") {
                        let sub = &js_text[idx + 11..];
                        if let Some(end_idx) = sub.find('"') {
                            let found_id = &sub[..end_idx];
                            if found_id.len() >= 20 && !found_id.contains(".com") {
                                info!("Discovered fresh SoundCloud client_id: {}", found_id);
                                let mut lock = self.client_id.write().await;
                                *lock = found_id.to_string();
                                return Ok(found_id.to_string());
                            }
                        }
                    }
                }
            }
        }

        let current = self.get_client_id().await;
        Ok(current)
    }

    /// Searches tracks on SoundCloud API v2 with rotating client_id pool and browser headers
    pub async fn search_tracks(&self, query: &str, limit: usize) -> Result<Vec<SoundCloudTrack>, String> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }

        let client = self.build_client().await;
        let mut last_error = String::new();

        // 1. Try with active client_id, then try fallback client IDs from pool
        let mut cids_to_try = vec![self.get_client_id().await];
        for b_cid in BACKUP_CLIENT_IDS {
            if !cids_to_try.iter().any(|c| c == *b_cid) {
                cids_to_try.push(b_cid.to_string());
            }
        }

        for (idx, cid) in cids_to_try.iter().enumerate() {
            let url = format!(
                "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit={}",
                urlencoding::encode(q),
                cid,
                limit.clamp(1, 25)
            );

            match client
                .get(&url)
                .header("Accept", "application/json, text/javascript, */*; q=0.01")
                .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
                .header("Origin", "https://soundcloud.com")
                .header("Referer", "https://soundcloud.com/")
                .send()
                .await
            {
                Ok(resp) => {
                    let status = resp.status();
                    if status.is_success() {
                        if let Ok(json_val) = resp.json::<serde_json::Value>().await {
                            let mut tracks = Vec::new();
                            if let Some(collection) = json_val["collection"].as_array() {
                                for item in collection {
                                    let id = item["id"].as_u64().unwrap_or(0);
                                    if id == 0 {
                                        continue;
                                    }
                                    let title = item["title"].as_str().unwrap_or("Unknown Title").to_string();
                                    let artist = item["user"]["username"]
                                        .as_str()
                                        .unwrap_or("Unknown Artist")
                                        .to_string();
                                    let duration_ms = item["duration"].as_u64().unwrap_or(0);
                                    let artwork_url = item["artwork_url"].as_str().map(|s| s.to_string());
                                    let permalink_url = item["permalink_url"]
                                        .as_str()
                                        .unwrap_or("")
                                        .to_string();

                                    tracks.push(SoundCloudTrack {
                                        id,
                                        title,
                                        artist,
                                        duration_ms,
                                        artwork_url,
                                        permalink_url,
                                        stream_url: None,
                                        is_hls: false,
                                        source: Some("soundcloud".to_string()),
                                    });
                                }
                            }

                            // If a backup client_id succeeded, promote it to active
                            if idx > 0 {
                                let mut lock = self.client_id.write().await;
                                *lock = cid.clone();
                            }

                            info!("SoundCloud search for '{}' returned {} tracks (cid: {})", q, tracks.len(), cid);
                            return Ok(tracks);
                        }
                    } else if status.as_u16() == 401 || status.as_u16() == 403 {
                        last_error = format!("SoundCloud HTTP {}", status);
                        continue;
                    } else {
                        last_error = format!("SoundCloud HTTP {}", status);
                    }
                }
                Err(e) => {
                    last_error = e.to_string();
                    warn!("SoundCloud request attempt with cid {} failed: {}", cid, e);
                }
            }
        }

        // 2. Fallback: try scraping fresh client_id
        if let Ok(fresh_id) = self.refresh_client_id().await {
            let retry_url = format!(
                "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit={}",
                urlencoding::encode(q),
                fresh_id,
                limit.clamp(1, 25)
            );
            if let Ok(resp) = client
                .get(&retry_url)
                .header("Accept", "application/json, text/javascript, */*; q=0.01")
                .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
                .header("Origin", "https://soundcloud.com")
                .header("Referer", "https://soundcloud.com/")
                .send()
                .await
            {
                if resp.status().is_success() {
                    if let Ok(json_val) = resp.json::<serde_json::Value>().await {
                        let mut tracks = Vec::new();
                        if let Some(collection) = json_val["collection"].as_array() {
                            for item in collection {
                                let id = item["id"].as_u64().unwrap_or(0);
                                if id == 0 {
                                    continue;
                                }
                                let title = item["title"].as_str().unwrap_or("Unknown Title").to_string();
                                let artist = item["user"]["username"]
                                    .as_str()
                                    .unwrap_or("Unknown Artist")
                                    .to_string();
                                let duration_ms = item["duration"].as_u64().unwrap_or(0);
                                let artwork_url = item["artwork_url"].as_str().map(|s| s.to_string());
                                let permalink_url = item["permalink_url"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string();

                                tracks.push(SoundCloudTrack {
                                    id,
                                    title,
                                    artist,
                                    duration_ms,
                                    artwork_url,
                                    permalink_url,
                                    stream_url: None,
                                    is_hls: false,
                                    source: Some("soundcloud".to_string()),
                                });
                            }
                        }
                        return Ok(tracks);
                    }
                }
            }
        }

        // 3. Resilient Fallback: If SoundCloud is blocked by ISP in the region, query public audio directory
        if let Ok(fallback_tracks) = self.search_fallback_streams(q, limit).await {
            if !fallback_tracks.is_empty() {
                info!("SoundCloud unreachable; providing {} resilient stream tracks for '{}'", fallback_tracks.len(), q);
                return Ok(fallback_tracks);
            }
        }

        Err(format!("SoundCloud API недоступен ({}) — включите SOCKS5 прокси или используйте радио/прямой URL", last_error))
    }

    /// Resilient fallback search against public audio directories (Radio-Browser / open streams)
    pub async fn search_fallback_streams(&self, query: &str, limit: usize) -> Result<Vec<SoundCloudTrack>, String> {
        let client = self.build_client().await;
        let url = format!(
            "https://de1.api.radio-browser.info/json/stations/byname/{}?limit={}",
            urlencoding::encode(query),
            limit.clamp(1, 15)
        );

        if let Ok(res) = client.get(&url).timeout(std::time::Duration::from_secs(4)).send().await {
            if res.status().is_success() {
                if let Ok(arr) = res.json::<serde_json::Value>().await {
                    if let Some(stations) = arr.as_array() {
                        let mut tracks = Vec::new();
                        for (idx, st) in stations.iter().enumerate() {
                            let name = st["name"].as_str().unwrap_or("Radio Stream").trim();
                            let stream_url = st["url_resolved"].as_str().or_else(|| st["url"].as_str()).unwrap_or("");
                            if stream_url.is_empty() || name.is_empty() {
                                continue;
                            }
                            let tags = st["tags"].as_str().unwrap_or("Live Radio");
                            let icon = st["favicon"].as_str().filter(|s| !s.is_empty()).map(|s| s.to_string());

                            tracks.push(SoundCloudTrack {
                                id: 9000000 + idx as u64,
                                title: name.to_string(),
                                artist: format!("Online Radio • {}", tags),
                                duration_ms: 0,
                                artwork_url: icon,
                                permalink_url: stream_url.to_string(),
                                stream_url: Some(stream_url.to_string()),
                                is_hls: stream_url.contains(".m3u8"),
                                source: Some("radio".to_string()),
                            });
                        }
                        if !tracks.is_empty() {
                            return Ok(tracks);
                        }
                    }
                }
            }
        }

        Ok(Vec::new())
    }

    /// Searches tracks on YouTube Music (WEB_REMIX Innertube API)
    pub async fn search_ytmusic_tracks(&self, query: &str, limit: usize) -> Result<Vec<SoundCloudTrack>, String> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }

        let client = self.build_client().await;
        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": "1.20231201.01.00"
                }
            },
            "query": q
        });

        let res = client
            .post("https://music.youtube.com/youtubei/v1/search")
            .header("Content-Type", "application/json")
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36")
            .json(&body)
            .timeout(std::time::Duration::from_secs(6))
            .send()
            .await
            .map_err(|e| format!("YT Music request error: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("YT Music HTTP {}", res.status()));
        }

        let data: serde_json::Value = res.json().await.map_err(|e| format!("YT Music JSON error: {}", e))?;
        let mut results = Vec::new();

        let list = data
            .pointer("/contents/tabbedSearchResultsRenderer/tabs/0/tabRenderer/content/sectionListRenderer/contents")
            .and_then(|v| v.as_array());

        if let Some(sections) = list {
            // 1. Top card
            if let Some(card) = sections.get(0).and_then(|s| s.get("musicCardShelfRenderer")) {
                let title = card.pointer("/title/runs/0/text").and_then(|v| v.as_str()).unwrap_or("");
                let sub = card
                    .pointer("/subtitle/runs")
                    .and_then(|v| v.as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .unwrap_or_default();
                let clean_sub = clean_yt_artist_string(&sub);
                let vid = card
                    .pointer("/onTap/watchEndpoint/videoId")
                    .or_else(|| card.pointer("/buttons/0/buttonRenderer/command/watchEndpoint/videoId"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let thumb = card
                    .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
                    .and_then(|v| v.as_array())
                    .and_then(|arr| arr.last())
                    .and_then(|t| t.get("url"))
                    .and_then(|u| u.as_str())
                    .map(|s| s.to_string());

                if !title.is_empty() && !vid.is_empty() {
                    results.push(SoundCloudTrack {
                        id: 0,
                        title: title.to_string(),
                        artist: if clean_sub.is_empty() { "YouTube Music".to_string() } else { clean_sub },
                        duration_ms: 0,
                        artwork_url: thumb,
                        permalink_url: format!("https://music.youtube.com/watch?v={}", vid),
                        stream_url: None,
                        is_hls: false,
                        source: Some("ytmusic".to_string()),
                    });
                }
            }

            // 2. Shelf items
            for sec in sections {
                if let Some(item) = sec.pointer("/itemSectionRenderer/contents/0/musicResponsiveListItemRenderer") {
                    let vid = item
                        .pointer("/playlistItemData/videoId")
                        .or_else(|| item.pointer("/overlay/musicItemThumbnailOverlayRenderer/content/musicPlayButtonRenderer/playNavigationEndpoint/watchEndpoint/videoId"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if vid.is_empty() {
                        continue;
                    }

                    let cols = item.get("flexColumns").and_then(|v| v.as_array());
                    let title = cols
                        .and_then(|c| c.get(0))
                        .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs/0/text"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if title.is_empty() {
                        continue;
                    }

                    let artist = cols
                        .and_then(|c| c.get(1))
                        .and_then(|c| c.pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs"))
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.get("text").and_then(|t| t.as_str()))
                                .collect::<Vec<_>>()
                                .join("")
                        })
                        .unwrap_or_default();
                    let clean_artist = clean_yt_artist_string(&artist);

                    let thumb = item
                        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.last())
                        .and_then(|t| t.get("url"))
                        .and_then(|u| u.as_str())
                        .map(|s| s.to_string());

                    results.push(SoundCloudTrack {
                        id: 0,
                        title: title.to_string(),
                        artist: if clean_artist.is_empty() { "YouTube Music".to_string() } else { clean_artist },
                        duration_ms: 0,
                        artwork_url: thumb,
                        permalink_url: format!("https://music.youtube.com/watch?v={}", vid),
                        stream_url: None,
                        is_hls: false,
                        source: Some("ytmusic".to_string()),
                    });

                    if results.len() >= limit {
                        break;
                    }
                }
            }
        }

        info!("YouTube Music search for '{}' returned {} tracks", q, results.len());
        Ok(results)
    }

    /// Unified multi-source search: supports "all", "ytmusic", "soundcloud", "radio"
    pub async fn search_multi_source(&self, query: &str, source: &str, limit: usize) -> Result<Vec<SoundCloudTrack>, String> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }

        match source.to_lowercase().as_str() {
            "ytmusic" | "youtube" | "yt" => {
                self.search_ytmusic_tracks(q, limit).await
            }
            "soundcloud" | "sc" => {
                self.search_tracks(q, limit).await
            }
            "radio" => {
                self.search_fallback_streams(q, limit).await
            }
            _ => {
                // "all" - search YouTube Music and SoundCloud in parallel
                let yt_res = self.search_ytmusic_tracks(q, limit.clamp(4, 15)).await.unwrap_or_default();
                let sc_res = self.search_tracks(q, limit.clamp(4, 15)).await.unwrap_or_default();
                
                let mut combined = Vec::new();
                let max_len = std::cmp::max(yt_res.len(), sc_res.len());
                for i in 0..max_len {
                    if let Some(t) = yt_res.get(i) {
                        combined.push(t.clone());
                    }
                    if let Some(t) = sc_res.get(i) {
                        combined.push(t.clone());
                    }
                    if combined.len() >= limit {
                        break;
                    }
                }

                if combined.is_empty() {
                    let radio_res = self.search_fallback_streams(q, limit).await.unwrap_or_default();
                    combined.extend(radio_res);
                }
                Ok(combined)
            }
        }
    }

    /// Resolves best playable stream URL (progressive MP3 or HLS) for a search query
    pub async fn resolve_best_stream(&self, query: &str) -> Result<ResolvedStream, String> {
        let q = query.trim();
        let client = self.build_client().await;
        let cid = self.get_client_id().await;

        let search_url = format!(
            "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit=5",
            urlencoding::encode(q),
            cid
        );

        let res = client
            .get(&search_url)
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .header("Origin", "https://soundcloud.com")
            .header("Referer", "https://soundcloud.com/")
            .send()
            .await
            .map_err(|e| format!("SoundCloud search error: {}", e))?;

        let json_val: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("SoundCloud JSON error: {}", e))?;

        let collection = json_val["collection"]
            .as_array()
            .ok_or_else(|| "No collection in SoundCloud response".to_string())?;

        if collection.is_empty() {
            return Err(format!("No tracks found on SoundCloud for '{}'", q));
        }

        // Priority Pass 1: Look for Progressive MP3 across all search results (plays natively with 0% CPU overhead)
        for track in collection {
            let title = track["title"].as_str().unwrap_or(q).to_string();
            let artist = track["user"]["username"].as_str().unwrap_or("").to_string();
            let artwork_url = track["artwork_url"].as_str().map(|s| s.to_string());
            let track_auth = track["track_authorization"].as_str().unwrap_or("");
            let duration_ms = track["duration"].as_u64().unwrap_or(0);

            if let Some(transcodings) = track["media"]["transcodings"].as_array() {
                for tr in transcodings {
                    let protocol = tr["format"]["protocol"].as_str().unwrap_or("");
                    let mime = tr["format"]["mime_type"].as_str().unwrap_or("");
                    if protocol == "progressive" && (mime.contains("mpeg") || mime.contains("mp3")) {
                        if let Some(tr_url) = tr["url"].as_str() {
                            let stream_req_url = format!(
                                "{}?client_id={}&track_authorization={}",
                                tr_url, cid, track_auth
                            );
                            if let Ok(stream_res) = client
                                .get(&stream_req_url)
                                .header("Accept", "application/json, text/javascript, */*; q=0.01")
                                .header("Origin", "https://soundcloud.com")
                                .header("Referer", "https://soundcloud.com/")
                                .send()
                                .await
                            {
                                if let Ok(s_json) = stream_res.json::<serde_json::Value>().await {
                                    if let Some(final_url) = s_json["url"].as_str() {
                                        info!(
                                            "Resolved Progressive MP3 stream for '{}' (by {})",
                                            title, artist
                                        );
                                        return Ok(ResolvedStream {
                                            track_title: title,
                                            artist,
                                            stream_url: final_url.to_string(),
                                            is_hls: false,
                                            artwork_url,
                                            duration_ms,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Priority Pass 2: Fallback to HLS stream for top candidate
        for track in collection {
            let title = track["title"].as_str().unwrap_or(q).to_string();
            let artist = track["user"]["username"].as_str().unwrap_or("").to_string();
            let artwork_url = track["artwork_url"].as_str().map(|s| s.to_string());
            let track_auth = track["track_authorization"].as_str().unwrap_or("");
            let duration_ms = track["duration"].as_u64().unwrap_or(0);

            if let Some(transcodings) = track["media"]["transcodings"].as_array() {
                for tr in transcodings {
                    let protocol = tr["format"]["protocol"].as_str().unwrap_or("");
                    if protocol == "hls" {
                        if let Some(tr_url) = tr["url"].as_str() {
                            let stream_req_url = format!(
                                "{}?client_id={}&track_authorization={}",
                                tr_url, cid, track_auth
                            );
                            if let Ok(stream_res) = client
                                .get(&stream_req_url)
                                .header("Accept", "application/json, text/javascript, */*; q=0.01")
                                .header("Origin", "https://soundcloud.com")
                                .header("Referer", "https://soundcloud.com/")
                                .send()
                                .await
                            {
                                if let Ok(s_json) = stream_res.json::<serde_json::Value>().await {
                                    if let Some(final_url) = s_json["url"].as_str() {
                                        info!(
                                            "Resolved HLS stream for '{}' (by {})",
                                            title, artist
                                        );
                                        return Ok(ResolvedStream {
                                            track_title: title,
                                            artist,
                                            stream_url: final_url.to_string(),
                                            is_hls: true,
                                            artwork_url,
                                            duration_ms,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(format!("Could not resolve playable stream for '{}'", q))
    }

    /// Resolves stream for a specific track by its SoundCloud track ID
    pub async fn resolve_track_by_id(&self, track_id: u64) -> Result<ResolvedStream, String> {
        let client = self.build_client().await;
        let cid = self.get_client_id().await;
        let track_url = format!(
            "https://api-v2.soundcloud.com/tracks/{}?client_id={}",
            track_id, cid
        );

        let res = client
            .get(&track_url)
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .header("Origin", "https://soundcloud.com")
            .header("Referer", "https://soundcloud.com/")
            .send()
            .await
            .map_err(|e| format!("SoundCloud track fetch error: {}", e))?;

        let track: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("SoundCloud JSON error: {}", e))?;

        Self::extract_stream_from_track(&track, cid.as_str(), &client).await
    }

    /// Resolves stream for a SoundCloud track by its permalink URL (e.g. https://soundcloud.com/artist/title)
    pub async fn resolve_permalink(&self, permalink_url: &str) -> Result<ResolvedStream, String> {
        let client = self.build_client().await;
        let cid = self.get_client_id().await;
        let resolve_url = format!(
            "https://api-v2.soundcloud.com/resolve?url={}&client_id={}",
            urlencoding::encode(permalink_url),
            cid
        );

        let res = client
            .get(&resolve_url)
            .header("Accept", "application/json, text/javascript, */*; q=0.01")
            .header("Accept-Language", "ru-RU,ru;q=0.9,en-US;q=0.8,en;q=0.7")
            .header("Origin", "https://soundcloud.com")
            .header("Referer", "https://soundcloud.com/")
            .send()
            .await
            .map_err(|e| format!("SoundCloud resolve error: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("SoundCloud resolve HTTP {}", res.status()));
        }

        let track: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("SoundCloud JSON error: {}", e))?;

        Self::extract_stream_from_track(&track, cid.as_str(), &client).await
    }

    /// Helper to extract progressive MP3 or HLS stream from a SoundCloud track object
    pub async fn extract_stream_from_track(
        track: &serde_json::Value,
        cid: &str,
        client: &Client,
    ) -> Result<ResolvedStream, String> {
        let title = track["title"].as_str().unwrap_or("SoundCloud Track").to_string();
        let artist = track["user"]["username"].as_str().unwrap_or("").to_string();
        let artwork_url = track["artwork_url"].as_str().map(|s| s.to_string());
        let track_auth = track["track_authorization"].as_str().unwrap_or("");
        let duration_ms = track["duration"].as_u64().unwrap_or(0);

        if let Some(transcodings) = track["media"]["transcodings"].as_array() {
            // Check progressive first
            for tr in transcodings {
                let protocol = tr["format"]["protocol"].as_str().unwrap_or("");
                let mime = tr["format"]["mime_type"].as_str().unwrap_or("");
                if protocol == "progressive" && (mime.contains("mpeg") || mime.contains("mp3")) {
                    if let Some(tr_url) = tr["url"].as_str() {
                        let stream_req_url = format!(
                            "{}?client_id={}&track_authorization={}",
                            tr_url, cid, track_auth
                        );
                        if let Ok(stream_res) = client
                            .get(&stream_req_url)
                            .header("Accept", "application/json, text/javascript, */*; q=0.01")
                            .header("Origin", "https://soundcloud.com")
                            .header("Referer", "https://soundcloud.com/")
                            .send()
                            .await
                        {
                            if let Ok(s_json) = stream_res.json::<serde_json::Value>().await {
                                if let Some(final_url) = s_json["url"].as_str() {
                                    return Ok(ResolvedStream {
                                        track_title: title,
                                        artist,
                                        stream_url: final_url.to_string(),
                                        is_hls: false,
                                        artwork_url,
                                        duration_ms,
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Check HLS second
            for tr in transcodings {
                let protocol = tr["format"]["protocol"].as_str().unwrap_or("");
                if protocol == "hls" {
                    if let Some(tr_url) = tr["url"].as_str() {
                        let stream_req_url = format!(
                            "{}?client_id={}&track_authorization={}",
                            tr_url, cid, track_auth
                        );
                        if let Ok(stream_res) = client
                            .get(&stream_req_url)
                            .header("Accept", "application/json, text/javascript, */*; q=0.01")
                            .header("Origin", "https://soundcloud.com")
                            .header("Referer", "https://soundcloud.com/")
                            .send()
                            .await
                        {
                            if let Ok(s_json) = stream_res.json::<serde_json::Value>().await {
                                if let Some(final_url) = s_json["url"].as_str() {
                                    return Ok(ResolvedStream {
                                        track_title: title,
                                        artist,
                                        stream_url: final_url.to_string(),
                                        is_hls: true,
                                        artwork_url,
                                        duration_ms,
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(format!("Track '{}' has no accessible audio streams", title))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_ytmusic_search() {
        let mut cfg = AppConfig::default();
        cfg.proxy.enabled = true;
        cfg.proxy.socks5_url = "socks5://station:kiserStation2026@13.141.51.5:31080".to_string();
        let svc = SoundCloudService::new(Arc::new(RwLock::new(cfg)));
        let res = svc.search_ytmusic_tracks("Linkin Park Numb", 5).await;
        println!("search_ytmusic_tracks result: {:?}", res);
        assert!(res.is_ok());
        let tracks = res.unwrap();
        println!("tracks count: {}", tracks.len());
        for t in &tracks {
            println!("- {} by {}", t.title, t.artist);
        }
    }
}
