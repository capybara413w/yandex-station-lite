// Based on YTerMusic by ccgauche (https://github.com/ccgauche/ytermusic)
// Adapted for station_daemon (Yandex Station Lite) under GNU GPL v3.0

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, COOKIE, USER_AGENT};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha1::{Digest, Sha1};
use tokio::sync::RwLock;
use tracing::debug;

use super::json_extractor::{
    extract_playlist_info, from_json, get_continuation, get_playlist, get_playlist_panel_video,
    get_playlist_search, get_video, get_video_from_album, Continuation, YoutubeMusicPlaylistRef,
    YoutubeMusicVideoRef,
};
use super::string_utils::StringUtils;

const YTM_DOMAIN: &str = "https://music.youtube.com";
const DEFAULT_API_KEY: &str = "AIzaSyC9XL3ZjWddXya6X74dJoCTL-WEYFDNX30";
const DEFAULT_CLIENT_VERSION: &str = "1.20240701.01.00";

#[derive(Debug, Clone, PartialOrd, Eq, Ord, PartialEq, Hash, Serialize, Deserialize)]
pub struct SearchResults {
    pub videos: Vec<YoutubeMusicVideoRef>,
    pub playlists: Vec<YoutubeMusicPlaylistRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackStreamInfo {
    pub stream_url: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_seconds: Option<u64>,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, PartialOrd, Eq, Ord, PartialEq, Hash)]
pub enum Endpoint {
    MusicLikedPlaylists,
    MusicHome,
    MusicLibraryLanding,
    Playlist(String),
    Search(String),
}

impl Endpoint {
    fn get_key(&self) -> String {
        match self {
            Endpoint::MusicLikedPlaylists => "browseId".to_owned(),
            Endpoint::MusicLibraryLanding => "browseId".to_owned(),
            Endpoint::Playlist(_) => "browseId".to_owned(),
            Endpoint::MusicHome => "browseId".to_owned(),
            Endpoint::Search(_) => "query".to_owned(),
        }
    }

    fn get_param(&self) -> String {
        match self {
            Endpoint::MusicLikedPlaylists => "FEmusic_liked_playlists".to_owned(),
            Endpoint::MusicLibraryLanding => "FEmusic_library_landing".to_owned(),
            Endpoint::Playlist(id) => {
                let s = id.trim();
                if s.starts_with("VL") || s.starts_with("FE") || s.starts_with("MPRE") {
                    s.to_owned()
                } else {
                    format!("VL{}", s)
                }
            }
            Endpoint::Search(query) => query.to_owned(),
            Endpoint::MusicHome => "FEmusic_home".to_owned(),
        }
    }

    fn get_route(&self) -> String {
        match self {
            Endpoint::MusicLikedPlaylists => "browse".to_owned(),
            Endpoint::MusicLibraryLanding => "browse".to_owned(),
            Endpoint::Playlist(_) => "browse".to_owned(),
            Endpoint::Search(_) => "search".to_owned(),
            Endpoint::MusicHome => "browse".to_owned(),
        }
    }
}

pub struct YoutubeMusicInstance {
    sapisid: Option<String>,
    innertube_api_key: String,
    client_version: String,
    cookies: String,
    account_id: Option<String>,
    client: Client,
    visitor_data: Arc<RwLock<Option<String>>>,
    sts: Arc<RwLock<Option<u64>>>,
}

impl YoutubeMusicInstance {
    /// Creates a new YoutubeMusicInstance inspired by ytermusic's ytpapi2 architecture
    pub async fn new(
        cookie_str: &str,
        account_id: Option<String>,
        proxy_url: Option<String>,
    ) -> Result<Self, String> {
        let mut builder = Client::builder()
            .timeout(std::time::Duration::from_secs(20))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36");

        if let Some(ref p) = proxy_url {
            if let Ok(proxy) = reqwest::Proxy::all(p) {
                builder = builder.proxy(proxy);
            }
        }

        let client = builder.build().map_err(|e| format!("Client build error: {}", e))?;

        let cookies = cookie_str.trim().to_string();
        let sapisid = Self::extract_sapisid_from_raw(&cookies);

        // Fetch homepage to extract dynamic keys (as in ytermusic)
        let (api_key, version, visitor_init, sts_init) = match client.get(YTM_DOMAIN).send().await {
            Ok(resp) if resp.status().is_success() => {
                let html = resp.text().await.unwrap_or_default();
                let key = html.between("INNERTUBE_API_KEY\":\"", "\"")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| DEFAULT_API_KEY.to_string());
                let ver = html.between("INNERTUBE_CLIENT_VERSION\":\"", "\"")
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| DEFAULT_CLIENT_VERSION.to_string());
                let vis = html.between("\"VISITOR_DATA\":\"", "\"")
                    .map(|s| s.to_string());
                let s_ts = html.between("\"STS\":", ",")
                    .or_else(|| html.between("\"STS\":", "}"))
                    .and_then(|s| s.trim().parse::<u64>().ok());
                (key, ver, vis, s_ts)
            }
            _ => (DEFAULT_API_KEY.to_string(), DEFAULT_CLIENT_VERSION.to_string(), None, None),
        };

        debug!("YTerMusic engine initialized. Key: {}, Version: {}, Authenticated: {}", api_key, version, sapisid.is_some());

        Ok(Self {
            sapisid,
            innertube_api_key: api_key,
            client_version: version,
            cookies,
            account_id,
            client,
            visitor_data: Arc::new(RwLock::new(visitor_init)),
            sts: Arc::new(RwLock::new(sts_init)),
        })
    }

    pub fn is_authenticated(&self) -> bool {
        self.sapisid.is_some() || !self.cookies.is_empty()
    }

    /// Extracts SAPISID or __Secure-3PAPISID from cookies string or json
    pub fn extract_sapisid_from_raw(cookie: &str) -> Option<String> {
        let trimmed = cookie.trim();
        if trimmed.starts_with('[') || trimmed.starts_with('{') {
            if let Ok(val) = serde_json::from_str::<serde_json::Value>(trimmed) {
                let items: Vec<&serde_json::Value> = if let Some(arr) = val.as_array() {
                    arr.iter().collect()
                } else if val.is_object() {
                    vec![&val]
                } else {
                    Vec::new()
                };

                for it in items {
                    let name = it.get("name").or_else(|| it.get("Name")).and_then(|v| v.as_str()).unwrap_or("");
                    let lower = name.to_lowercase();
                    if lower == "sapisid" || lower == "__secure-3papisid" || lower == "__secure-1papisid" {
                        if let Some(v) = it.get("value").or_else(|| it.get("Value")).and_then(|v| v.as_str()) {
                            return Some(v.trim().to_string());
                        }
                    }
                }
            }
        }

        for part in cookie.split(';') {
            let item = part.trim();
            if let Some((k, v)) = item.split_once('=') {
                let key = k.trim().to_lowercase();
                if key == "sapisid" || key == "__secure-3papisid" || key == "__secure-1papisid" {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    /// Computes SAPISIDHASH header as implemented in ytermusic / ytpapi2
    fn compute_sapi_hash(&self) -> Option<String> {
        let sapisid = self.sapisid.as_ref()?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let payload = format!("{} {} {}", now, sapisid, YTM_DOMAIN);
        let mut hasher = Sha1::new();
        hasher.update(payload.as_bytes());
        let digest_hex = hex::encode(hasher.finalize());
        Some(format!("SAPISIDHASH {}_{}", now, digest_hex))
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36"),
        );
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert("X-Origin", HeaderValue::from_static(YTM_DOMAIN));
        headers.insert("Origin", HeaderValue::from_static(YTM_DOMAIN));
        headers.insert("X-Goog-AuthUser", HeaderValue::from_static("0"));

        if !self.cookies.is_empty() {
            if let Ok(v) = HeaderValue::from_str(&self.cookies) {
                headers.insert(COOKIE, v);
            }
        }

        if let Some(auth) = self.compute_sapi_hash() {
            if let Ok(v) = HeaderValue::from_str(&auth) {
                headers.insert(AUTHORIZATION, v);
            }
        }

        headers
    }

    async fn browse_raw(
        &self,
        endpoint_route: &str,
        endpoint_key: &str,
        endpoint_param: &str,
    ) -> Result<String, String> {
        let url = format!(
            "https://music.youtube.com/youtubei/v1/{}?key={}&prettyPrint=false",
            endpoint_route, self.innertube_api_key
        );

        let body = match &self.account_id {
            Some(id) => serde_json::json!({
                "context": {
                    "client": {
                        "clientName": "WEB_REMIX",
                        "clientVersion": self.client_version,
                        "hl": "ru",
                        "gl": "RU"
                    },
                    "user": {
                        "onBehalfOfUser": id
                    }
                },
                endpoint_key: endpoint_param
            }),
            None => serde_json::json!({
                "context": {
                    "client": {
                        "clientName": "WEB_REMIX",
                        "clientVersion": self.client_version,
                        "hl": "ru",
                        "gl": "RU"
                    }
                },
                endpoint_key: endpoint_param
            }),
        };

        let res = self.client
            .post(&url)
            .headers(self.build_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Network request failed: {}", e))?;

        if !res.status().is_success() {
            let st = res.status();
            let txt = res.text().await.unwrap_or_default();
            return Err(format!("Innertube HTTP {}: {}", st, txt));
        }

        res.text().await.map_err(|e| format!("Failed to read response text: {}", e))
    }

    async fn browse(
        &self,
        endpoint: &Endpoint,
        continuations: bool,
    ) -> Result<(Value, Vec<Continuation>), String> {
        let raw = self.browse_raw(
            &endpoint.get_route(),
            &endpoint.get_key(),
            &endpoint.get_param(),
        ).await?;

        let json: Value = serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {}", e))?;
        if json.get("error").is_some() {
            return Err(format!("YouTube Music API error: {:?}", json.get("error")));
        }

        let conts = if continuations {
            from_json(&json, get_continuation)
        } else {
            Vec::new()
        };

        Ok((json, conts))
    }

    async fn browse_continuation_raw(&self, c: &Continuation) -> Result<String, String> {
        let url = format!(
            "https://music.youtube.com/youtubei/v1/browse?ctoken={}&continuation={}&type=next&itct={}&key={}&prettyPrint=false",
            c.continuation, c.continuation, c.click_tracking_params, self.innertube_api_key
        );

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": self.client_version,
                    "hl": "ru",
                    "gl": "RU"
                }
            }
        });

        let res = self.client
            .post(&url)
            .headers(self.build_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Continuation request failed: {}", e))?;

        res.text().await.map_err(|e| format!("Failed to read continuation: {}", e))
    }

    async fn browse_continuation(
        &self,
        continuation: &Continuation,
        continuations: bool,
    ) -> Result<(Value, Vec<Continuation>), String> {
        let raw = self.browse_continuation_raw(continuation).await?;
        let json: Value = serde_json::from_str(&raw).map_err(|e| format!("JSON parse error: {}", e))?;
        let new_conts = if continuations {
            from_json(&json, get_continuation)
        } else {
            Vec::new()
        };
        Ok((json, new_conts))
    }

    pub async fn get_library(
        &self,
        endpoint: &Endpoint,
        mut n_continuations: usize,
    ) -> Result<Vec<YoutubeMusicPlaylistRef>, String> {
        let (library_json, mut continuations) = self.browse(endpoint, n_continuations > 0).await?;
        let mut library = from_json(&library_json, get_playlist);

        while let Some(continuation) = continuations.pop() {
            if n_continuations == 0 {
                break;
            }
            n_continuations -= 1;
            if let Ok((json, new_conts)) = self.browse_continuation(&continuation, n_continuations > 0).await {
                continuations.extend(new_conts);
                library.extend(from_json(&json, get_playlist));
            }
        }

        Ok(library)
    }

    pub async fn get_playlist_raw(
        &self,
        playlist_id: &str,
        mut n_continuations: usize,
    ) -> Result<Vec<YoutubeMusicVideoRef>, String> {
        let (playlist_json, mut continuations) = self
            .browse(&Endpoint::Playlist(playlist_id.to_string()), n_continuations > 0)
            .await?;

        let mut videos = parse_playlist(&playlist_json);

        while let Some(continuation) = continuations.pop() {
            if n_continuations == 0 {
                break;
            }
            n_continuations -= 1;
            if let Ok((json, new_conts)) = self.browse_continuation(&continuation, n_continuations > 0).await {
                continuations.extend(new_conts);
                videos.extend(parse_playlist(&json));
            }
        }

        Ok(videos)
    }

    pub async fn search(
        &self,
        search_query: &str,
        mut n_continuations: usize,
    ) -> Result<SearchResults, String> {
        let (search_json, mut continuations) = self
            .browse(&Endpoint::Search(search_query.to_string()), n_continuations > 0)
            .await?;

        let mut videos = from_json(&search_json, get_video);
        let mut playlists = from_json(&search_json, get_playlist_search);

        while let Some(continuation) = continuations.pop() {
            if n_continuations == 0 {
                break;
            }
            n_continuations -= 1;
            if let Ok((json, new_conts)) = self.browse_continuation(&continuation, n_continuations > 0).await {
                continuations.extend(new_conts);
                videos.extend(from_json(&json, get_video));
                playlists.extend(from_json(&json, get_playlist_search));
            }
        }

        Ok(SearchResults { videos, playlists })
    }

    pub async fn get_home(&self, mut n_continuations: usize) -> Result<SearchResults, String> {
        let (home_json, mut continuations) = self
            .browse(&Endpoint::MusicHome, n_continuations > 0)
            .await?;

        let mut videos = from_json(&home_json, get_video);
        let mut playlists = from_json(&home_json, get_playlist);

        while let Some(continuation) = continuations.pop() {
            if n_continuations == 0 {
                break;
            }
            n_continuations -= 1;
            if let Ok((json, new_conts)) = self.browse_continuation(&continuation, n_continuations > 0).await {
                continuations.extend(new_conts);
                videos.extend(from_json(&json, get_video));
                playlists.extend(from_json(&json, get_playlist));
            }
        }

        Ok(SearchResults { videos, playlists })
    }

    /// Fetches automatic next recommendations (autoplay / automix queue) for a given track
    pub async fn get_autoplay_tracks(&self, video_id: &str) -> Result<Vec<YoutubeMusicVideoRef>, String> {
        let vid = video_id.trim();
        if vid.is_empty() {
            return Ok(Vec::new());
        }

        let url = format!(
            "https://music.youtube.com/youtubei/v1/next?key={}&prettyPrint=false",
            self.innertube_api_key
        );

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "WEB_REMIX",
                    "clientVersion": self.client_version,
                    "hl": "ru",
                    "gl": "RU"
                }
            },
            "videoId": vid,
            "isAutoplay": true,
            "enablePersistentPlaylistPanel": true
        });

        let res = self.client
            .post(&url)
            .headers(self.build_headers())
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("Next request failed: {}", e))?;

        if !res.status().is_success() {
            let st = res.status();
            let txt = res.text().await.unwrap_or_default();
            return Err(format!("Next HTTP {}: {}", st, txt));
        }

        let json: Value = res.json().await.map_err(|e| format!("JSON parse error: {}", e))?;
        let videos = from_json(&json, get_playlist_panel_video);
        if videos.is_empty() {
            let fallback_videos = from_json(&json, get_video);
            return Ok(fallback_videos);
        }
        Ok(videos)
    }

    /// Resolves direct audio stream URL from YouTube Innertube player API (Ad-Free)
    /// Emulates the VISIONOS player client for 100% exact playback of user-selected tracks
    /// Resolves stream URL and video metadata
    pub async fn resolve_stream_info(&self, video_id: &str) -> Result<TrackStreamInfo, String> {
        let vid = video_id.trim();
        if vid.is_empty() {
            return Err("Empty video ID".to_string());
        }

        // 1. Try with cached visitor_data and sts first
        let (cached_v, cached_s) = {
            let v = self.visitor_data.read().await.clone();
            let s = *self.sts.read().await;
            (v, s)
        };

        if let (Some(ref v_data), Some(s_ts)) = (&cached_v, cached_s) {
            if let Ok(info) = self.call_visionos_player(vid, v_data, s_ts).await {
                return Ok(info);
            }
        }

        // 2. Fetch watch page to obtain fresh tokens
        let (fresh_v, fresh_s) = self.fetch_watch_page_tokens(vid).await;
        let v_data = fresh_v
            .or(cached_v)
            .unwrap_or_else(|| "CgtFaldMb0FKTHZEWSjP68fXBjIoCgJOTBIiEh4SHAsMDg8QERITFBUWFxgZGhscHR4fICEiIyQlJicgYWLWAgrTAkN2c0JDQVlTclFFQld5Ylo2TWRvcFFRVDY2TkE3MEhxVHFBXzFPeUpNdFZyVER2c2xPc0dGc1FJYVNwNkplMkFBOVJOV096VDZDcjZLRUtWLXF2OE9jd0pRVnJmcEVkelRqOTZrVjQtdGVrNlJqQm13VmJyd0RiTlFiRFVId0VaRXdSUDVtWEN3dHdlRmJlb3JMMHhFUVlPU0JwMFNkZ3VxTjVyNTNfdXlCa25tNjlKN1huSU56WlJwbUloQTNNdFU2cFFCSXlPMjQ1Vkh5Tk9ZdG03R2NMNldnWEQwRUlRUGVza3dVaUI5Ull5T1VzMHlpZ0dNa1VCc21pZlpiX3pvODBLOTctTlJtNFdyY3ZhNkUwLVM0UHd0cGxuNndYc3VLempxZGY5M3hGVVR4bnpqX28xejYtUkhrN0Q4QUY1b0Y5NGZSSllLblRaQkRxaWdBYw%3D%3D".to_string());
        let s_ts = fresh_s.or(cached_s).unwrap_or(20760);

        *self.visitor_data.write().await = Some(v_data.clone());
        *self.sts.write().await = Some(s_ts);

        // 3. Try VISIONOS client with fresh tokens
        if let Ok(info) = self.call_visionos_player(vid, &v_data, s_ts).await {
            return Ok(info);
        }

        // 4. Fallback: ANDROID_MUSIC client
        let mut android_headers = HeaderMap::new();
        android_headers.insert(
            USER_AGENT,
            HeaderValue::from_static("com.google.android.apps.youtube.music/6.20.51 (Linux; U; Android 11) gzip"),
        );
        android_headers.insert("Content-Type", HeaderValue::from_static("application/json"));
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

        let url = format!("{}/youtubei/v1/player?key={}&prettyPrint=false", YTM_DOMAIN, self.innertube_api_key);
        if let Ok(res) = self.client.post(&url).headers(android_headers).json(&body).send().await {
            if res.status().is_success() {
                if let Ok(json) = res.json::<Value>().await {
                    if let Some(stream_url) = Self::extract_best_audio_url(&json) {
                        let title = json.pointer("/videoDetails/title").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let artist = json.pointer("/videoDetails/author").and_then(|v| v.as_str()).map(|s| s.to_string());
                        let duration_seconds = json.pointer("/videoDetails/lengthSeconds")
                            .and_then(|v| v.as_str())
                            .and_then(|s| s.parse::<u64>().ok());
                        let artwork_url = json.pointer("/videoDetails/thumbnail/thumbnails")
                            .and_then(|v| v.as_array())
                            .and_then(|arr| arr.last())
                            .and_then(|v| v.get("url"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        return Ok(TrackStreamInfo {
                            stream_url,
                            title,
                            artist,
                            duration_seconds,
                            artwork_url,
                        });
                    }
                }
            }
        }

        Err(format!("Could not extract playable stream URL for video '{}'", vid))
    }

    pub async fn resolve_stream_url(&self, video_id: &str) -> Result<String, String> {
        self.resolve_stream_info(video_id).await.map(|info| info.stream_url)
    }

    async fn call_visionos_player(&self, vid: &str, visitor_data: &str, sts: u64) -> Result<TrackStreamInfo, String> {
        let mut headers = HeaderMap::new();
        headers.insert("Content-Type", HeaderValue::from_static("application/json"));
        headers.insert(
            USER_AGENT,
            HeaderValue::from_static("Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15"),
        );
        headers.insert("X-YouTube-Client-Name", HeaderValue::from_static("101"));
        headers.insert("X-YouTube-Client-Version", HeaderValue::from_static("1.02"));
        headers.insert("Origin", HeaderValue::from_static("https://www.youtube.com"));
        if !visitor_data.is_empty() {
            if let Ok(val) = HeaderValue::from_str(visitor_data) {
                headers.insert("X-Goog-Visitor-Id", val);
            }
        }

        let body = serde_json::json!({
            "context": {
                "client": {
                    "clientName": "VISIONOS",
                    "clientVersion": "1.02",
                    "deviceMake": "Apple",
                    "deviceModel": "RealityDevice17,1",
                    "userAgent": "Mozilla/5.0 (Macintosh; Intel Mac OS X 15_7_3) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/26.0 Safari/605.1.15",
                    "osName": "visionOS",
                    "osVersion": "26.5.23O471",
                    "hl": "en",
                    "gl": "US",
                    "visitorData": visitor_data
                }
            },
            "videoId": vid,
            "playbackContext": {
                "contentPlaybackContext": {
                    "html5Preference": "HTML5_PREF_WANTS",
                    "signatureTimestamp": sts
                }
            },
            "contentCheckOk": true,
            "racyCheckOk": true
        });

        let api_url = "https://www.youtube.com/youtubei/v1/player?prettyPrint=false";
        let res = self.client.post(api_url).headers(headers).json(&body).send().await
            .map_err(|e| format!("VisionOS player network error: {}", e))?;

        if !res.status().is_success() {
            return Err(format!("VisionOS player HTTP {}", res.status()));
        }

        let json: Value = res.json().await.map_err(|e| format!("VisionOS player JSON error: {}", e))?;
        if let Some(stream_url) = Self::extract_best_audio_url(&json) {
            let title = json.pointer("/videoDetails/title").and_then(|v| v.as_str()).map(|s| s.to_string());
            let artist = json.pointer("/videoDetails/author").and_then(|v| v.as_str()).map(|s| s.to_string());
            let duration_seconds = json.pointer("/videoDetails/lengthSeconds")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok());
            let artwork_url = json.pointer("/videoDetails/thumbnail/thumbnails")
                .and_then(|v| v.as_array())
                .and_then(|arr| arr.last())
                .and_then(|v| v.get("url"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            return Ok(TrackStreamInfo {
                stream_url,
                title,
                artist,
                duration_seconds,
                artwork_url,
            });
        }

        Err("No playable audio stream in VisionOS response".to_string())
    }

    async fn fetch_watch_page_tokens(&self, vid: &str) -> (Option<String>, Option<u64>) {
        let watch_url = format!("https://www.youtube.com/watch?v={}", vid);
        if let Ok(res) = self.client.get(&watch_url)
            .header(USER_AGENT, "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/125.0.0.0 Safari/537.36")
            .header("Accept-Language", "en-US,en;q=0.9")
            .send()
            .await 
        {
            if res.status().is_success() {
                if let Ok(html) = res.text().await {
                    let v = html.between("\"VISITOR_DATA\":\"", "\"")
                        .map(|s| s.to_string());
                    let s = html.between("\"STS\":", ",")
                        .or_else(|| html.between("\"STS\":", "}"))
                        .or_else(|| html.between("\"signatureTimestamp\":", ","))
                        .or_else(|| html.between("\"signatureTimestamp\":", "}"))
                        .and_then(|str_val| str_val.trim().parse::<u64>().ok());
                    return (v, s);
                }
            }
        }
        (None, None)
    }

    /// Extracts highest bitrate progressive/adaptive audio URL from player JSON response
    fn extract_best_audio_url(json: &Value) -> Option<String> {
        let formats = json.pointer("/streamingData/adaptiveFormats")
            .or_else(|| json.pointer("/streamingData/formats"))
            .and_then(|v| v.as_array())?;

        let mut best_url: Option<String> = None;
        let mut best_score: u64 = 0;

        for fmt in formats {
            let mime = fmt["mimeType"].as_str().unwrap_or("");
            if mime.starts_with("audio/") {
                if let Some(stream_url) = fmt["url"].as_str() {
                    let itag = fmt["itag"].as_u64().unwrap_or(0);
                    let bitrate = fmt["bitrate"].as_u64().unwrap_or(0);
                    let score = match itag {
                        140 => 200_000 + bitrate,
                        251 => 190_000 + bitrate,
                        _ => bitrate,
                    };
                    if score > best_score {
                        best_score = score;
                        best_url = Some(stream_url.to_string());
                    }
                }
            }
        }

        best_url
    }
}

fn parse_playlist(playlist_json: &Value) -> Vec<YoutubeMusicVideoRef> {
    let mut videos = from_json(playlist_json, get_video);
    let info = extract_playlist_info(playlist_json);
    for mut video in from_json(playlist_json, get_video_from_album) {
        if videos.iter().any(|x| x.video_id == video.video_id) {
            continue;
        }
        if let Some((title, artist)) = info.as_ref() {
            if video.album.is_empty() {
                video.album = title.to_string();
            }
            if video.author.is_empty() {
                video.author = artist.to_string();
            }
        }
        videos.push(video);
    }
    if videos.is_empty() {
        for video in from_json(playlist_json, get_playlist_panel_video) {
            if !videos.iter().any(|x| x.video_id == video.video_id) {
                videos.push(video);
            }
        }
    }
    videos
}
