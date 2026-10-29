use crate::config::ProxyConfig;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{info, warn};

const DEFAULT_CLIENT_ID: &str = "xk6BGxLCsQJ5Ss0bH6sRdsZhts3gTNaU";

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

pub struct SoundCloudService {
    client: Client,
    client_id: Arc<RwLock<String>>,
}

impl SoundCloudService {
    pub fn new(proxy: &ProxyConfig) -> Self {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36");

        if proxy.enabled && !proxy.socks5_url.is_empty() {
            if let Ok(p) = reqwest::Proxy::all(&proxy.socks5_url) {
                builder = builder.proxy(p);
            }
        }

        let client = builder.build().unwrap_or_else(|_| Client::new());

        Self {
            client,
            client_id: Arc::new(RwLock::new(DEFAULT_CLIENT_ID.to_string())),
        }
    }

    pub async fn get_client_id(&self) -> String {
        self.client_id.read().await.clone()
    }

    /// Dynamically extracts a fresh client_id from soundcloud.com script assets if needed
    pub async fn refresh_client_id(&self) -> Result<String, String> {
        info!("Refreshing SoundCloud client_id from soundcloud.com web assets...");
        let resp = self
            .client
            .get("https://soundcloud.com")
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
            if let Ok(js_res) = self.client.get(&script_url).send().await {
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

    /// Searches tracks on SoundCloud API v2
    pub async fn search_tracks(&self, query: &str, limit: usize) -> Result<Vec<SoundCloudTrack>, String> {
        let q = query.trim();
        if q.is_empty() {
            return Ok(Vec::new());
        }

        let cid = self.get_client_id().await;
        let url = format!(
            "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit={}",
            urlencoding::encode(q),
            cid,
            limit.clamp(1, 25)
        );

        let mut res = self.client.get(&url).send().await;

        // If unauthorized or forbidden, try refreshing client ID once
        if let Ok(ref r) = res {
            if r.status().as_u16() == 401 || r.status().as_u16() == 403 {
                warn!("SoundCloud returned {}, refreshing client_id...", r.status());
                if let Ok(new_id) = self.refresh_client_id().await {
                    let retry_url = format!(
                        "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit={}",
                        urlencoding::encode(q),
                        new_id,
                        limit.clamp(1, 25)
                    );
                    res = self.client.get(&retry_url).send().await;
                }
            }
        }

        let response = res.map_err(|e| format!("SoundCloud search request error: {}", e))?;
        if !response.status().is_success() {
            return Err(format!("SoundCloud search failed with status {}", response.status()));
        }

        let json_val: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse SoundCloud JSON: {}", e))?;

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
                });
            }
        }

        info!("SoundCloud search for '{}' returned {} tracks", q, tracks.len());
        Ok(tracks)
    }

    /// Resolves best playable stream URL (progressive MP3 or HLS) for a search query
    pub async fn resolve_best_stream(&self, query: &str) -> Result<ResolvedStream, String> {
        let q = query.trim();
        let cid = self.get_client_id().await;

        let search_url = format!(
            "https://api-v2.soundcloud.com/search/tracks?q={}&client_id={}&limit=5",
            urlencoding::encode(q),
            cid
        );

        let res = self
            .client
            .get(&search_url)
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
                            if let Ok(stream_res) = self.client.get(&stream_req_url).send().await {
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
                            if let Ok(stream_res) = self.client.get(&stream_req_url).send().await {
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
        let cid = self.get_client_id().await;
        let track_url = format!(
            "https://api-v2.soundcloud.com/tracks/{}?client_id={}",
            track_id, cid
        );

        let res = self
            .client
            .get(&track_url)
            .send()
            .await
            .map_err(|e| format!("SoundCloud track fetch error: {}", e))?;

        let track: serde_json::Value = res
            .json()
            .await
            .map_err(|e| format!("SoundCloud JSON error: {}", e))?;

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
                        if let Ok(stream_res) = self.client.get(&stream_req_url).send().await {
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
                        if let Ok(stream_res) = self.client.get(&stream_req_url).send().await {
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

        Err(format!("Track {} has no accessible streams", track_id))
    }
}
