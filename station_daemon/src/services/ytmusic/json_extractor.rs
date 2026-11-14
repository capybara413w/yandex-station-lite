// Based on YTerMusic by ccgauche (https://github.com/ccgauche/ytermusic)
// Adapted for station_daemon (Yandex Station Lite) under GNU GPL v3.0

use std::fmt::Display;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialOrd, Eq, Ord, PartialEq, Hash, Serialize, Deserialize)]
pub struct YoutubeMusicPlaylistRef {
    pub name: String,
    pub subtitle: String,
    pub browse_id: String,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, PartialOrd, Eq, Ord, PartialEq, Hash, Serialize, Deserialize)]
pub struct YoutubeMusicVideoRef {
    pub title: String,
    pub author: String,
    pub album: String,
    pub video_id: String,
    pub duration: String,
    pub artwork_url: Option<String>,
}

impl Display for YoutubeMusicVideoRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} | {}", self.author, self.title)
    }
}

#[derive(Debug, Clone, PartialOrd, Eq, Ord, PartialEq, Hash, Serialize, Deserialize)]
pub struct Continuation {
    pub(crate) continuation: String,
    pub(crate) click_tracking_params: String,
}

/// Applies recursively the `transformer` function to the given json value
/// and returns the transformed values.
pub(crate) fn from_json<T: PartialEq>(
    json: &Value,
    transformer: impl Fn(&Value) -> Option<T>,
) -> Vec<T> {
    fn inner_crawl<T: PartialEq>(
        value: &Value,
        results: &mut Vec<T>,
        transformer: &impl Fn(&Value) -> Option<T>,
    ) {
        if let Some(e) = transformer(value) {
            if !results.contains(&e) {
                results.push(e);
            }
            return;
        }
        match value {
            Value::Array(a) => {
                for x in a {
                    inner_crawl(x, results, transformer);
                }
            }
            Value::Object(a) => {
                for x in a.values() {
                    inner_crawl(x, results, transformer);
                }
            }
            _ => (),
        }
    }
    let mut results = Vec::new();
    inner_crawl(json, &mut results, &transformer);
    results
}

pub fn get_thumbnail(value: &Value) -> Option<String> {
    let thumbs = value
        .pointer("/thumbnail/musicThumbnailRenderer/thumbnail/thumbnails")
        .or_else(|| value.pointer("/thumbnailRenderer/musicThumbnailRenderer/thumbnail/thumbnails"))
        .or_else(|| value.pointer("/thumbnail/thumbnails"))
        .and_then(|t| t.as_array())?;

    thumbs.last()
        .and_then(|v| v.get("url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

/// Tries to extract a playlist from a json value.
pub(crate) fn get_playlist(value: &Value) -> Option<YoutubeMusicPlaylistRef> {
    let object = value.as_object()?;
    let title_text = get_text(object.get("title")?, true, false)?;
    let subtitle = object
        .get("subtitle")
        .and_then(|x| get_text(x, false, false))
        .unwrap_or_default();
    let browse_id = object
        .get("navigationEndpoint")
        .and_then(|x| x.get("browseEndpoint"))
        .and_then(|x| x.get("browseId"))
        .and_then(Value::as_str)?
        .to_string();
    let artwork_url = get_thumbnail(value);

    Some(YoutubeMusicPlaylistRef {
        name: title_text,
        subtitle,
        browse_id,
        artwork_url,
    })
}

pub fn get_continuation(value: &Value) -> Option<Continuation> {
    let continuation = value
        .get("nextContinuationData")
        .and_then(|x| x.get("continuation"))
        .and_then(Value::as_str)?;
    let click_tracking_params = value
        .get("nextContinuationData")
        .and_then(|x| x.get("clickTrackingParams"))
        .and_then(Value::as_str)?;
    Some(Continuation {
        continuation: continuation.to_string(),
        click_tracking_params: click_tracking_params.to_string(),
    })
}

pub fn get_playlist_search(value: &Value) -> Option<YoutubeMusicPlaylistRef> {
    let browse_id = value
        .get("navigationEndpoint")
        .and_then(|x| x.get("browseEndpoint"))
        .and_then(|x| x.get("browseId"))
        .and_then(Value::as_str)?
        .to_string();

    let titles: Vec<String> = value
        .get("flexColumns")?
        .as_array()?
        .iter()
        .flat_map(|x| {
            x.get("musicResponsiveListItemFlexColumnRenderer")
                .and_then(|x| x.get("text"))
                .and_then(|x| get_text(x, false, false))
        })
        .collect();

    let artwork_url = get_thumbnail(value);

    Some(YoutubeMusicPlaylistRef {
        name: titles.first()?.clone(),
        subtitle: titles.get(1).cloned().unwrap_or_default(),
        browse_id,
        artwork_url,
    })
}

pub fn extract_playlist_info(value: &Value) -> Option<(String, String)> {
    let header = value
        .get("header")
        .and_then(|h| h.get("musicDetailHeaderRenderer"))
        .or_else(|| value.pointer("/header/musicResponsiveHeaderRenderer"))
        .or_else(|| value.pointer("/header/musicEditablePlaylistDetailHeaderRenderer/header/musicDetailHeaderRenderer"))?;

    let title = get_text(header.get("title")?, false, false)?;
    let subtitles = header
        .get("subtitle")?
        .get("runs")?
        .as_array()?
        .iter()
        .flat_map(|x| get_text(x, false, false))
        .filter(|x| x != " • ")
        .collect::<Vec<String>>();

    Some((title, subtitles.get(1).cloned().unwrap_or_default()))
}

pub fn get_video_from_album(value: &Value) -> Option<YoutubeMusicVideoRef> {
    let video_id = value
        .get("playlistItemData")
        .and_then(|x| x.get("videoId"))
        .and_then(Value::as_str)?;

    let title: Vec<String> = value
        .get("flexColumns")?
        .as_array()?
        .iter()
        .flat_map(|x| {
            x.get("musicResponsiveListItemFlexColumnRenderer")
                .and_then(|x| x.get("text"))
                .and_then(|x| get_text(x, false, false))
        })
        .collect();

    let artwork_url = get_thumbnail(value);

    Some(YoutubeMusicVideoRef {
        title: title.first()?.clone(),
        author: String::new(),
        album: String::new(),
        video_id: video_id.to_string(),
        duration: String::new(),
        artwork_url,
    })
}

/// Tries to find a video id in the json
pub fn get_videoid(value: &Value) -> Option<String> {
    match value {
        Value::Array(e) => e.iter().find_map(get_videoid),
        Value::Object(e) => e
            .get("videoId")
            .and_then(Value::as_str)
            .map(|x| x.to_string())
            .or_else(|| e.values().find_map(get_videoid)),
        _ => None,
    }
}

/// Tries to extract a video from a json value.
pub(crate) fn get_video(value: &Value) -> Option<YoutubeMusicVideoRef> {
    let video_id = get_videoid(value)?;
    let artwork_url = get_thumbnail(value);

    let flex_columns = value
        .as_object()?
        .get("flexColumns")?
        .as_array()?;

    // Title from flexColumns[0]
    let title = flex_columns.first()
        .and_then(extract_column_runs_joined)
        .filter(|s| !s.is_empty())?;

    // Subtitle / Artist / Album / Duration from flexColumns[1]
    let (author, album, extracted_dur) = if let Some(col1) = flex_columns.get(1) {
        extract_subtitle_details(col1)
    } else {
        ("YouTube Music".to_string(), String::new(), String::new())
    };

    let duration = if !extracted_dur.is_empty() {
        extracted_dur
    } else {
        value
            .pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text/runs/0/text")
            .or_else(|| value.pointer("/fixedColumns/0/musicResponsiveListItemFixedColumnRenderer/text/simpleText"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default()
    };

    Some(YoutubeMusicVideoRef {
        video_id,
        title,
        author,
        album,
        duration,
        artwork_url,
    })
}

pub(crate) fn get_playlist_panel_video(value: &Value) -> Option<YoutubeMusicVideoRef> {
    let renderer = value.get("playlistPanelVideoRenderer")?;
    let video_id = renderer.get("videoId")?.as_str()?.to_string();
    let title = renderer.pointer("/title/runs/0/text")
        .or_else(|| renderer.pointer("/title/simpleText"))
        .and_then(|v| v.as_str())?
        .to_string();
    let author = renderer.pointer("/longBylineText/runs/0/text")
        .or_else(|| renderer.pointer("/shortBylineText/runs/0/text"))
        .and_then(|v| v.as_str())
        .unwrap_or("YouTube Music")
        .to_string();
    let duration = renderer.pointer("/lengthText/runs/0/text")
        .or_else(|| renderer.pointer("/lengthText/simpleText"))
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let artwork_url = renderer.pointer("/thumbnail/thumbnails")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.last())
        .and_then(|t| t.get("url"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    Some(YoutubeMusicVideoRef {
        video_id,
        title,
        author,
        album: String::new(),
        duration,
        artwork_url,
    })
}

fn extract_column_runs_joined(col: &Value) -> Option<String> {
    let runs = col
        .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
        .or_else(|| col.pointer("/text/runs"))
        .and_then(|v| v.as_array())?;

    let parts: Vec<&str> = runs
        .iter()
        .filter_map(|r| r.get("text").and_then(|t| t.as_str()))
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" "))
    }
}

fn extract_subtitle_details(col: &Value) -> (String, String, String) {
    let runs = col
        .pointer("/musicResponsiveListItemFlexColumnRenderer/text/runs")
        .or_else(|| col.pointer("/text/runs"))
        .and_then(|v| v.as_array());

    let runs = match runs {
        Some(r) => r,
        None => return ("YouTube Music".to_string(), String::new(), String::new()),
    };

    let mut sections: Vec<String> = Vec::new();
    let mut current_section: Vec<String> = Vec::new();

    for r in runs {
        if let Some(t) = r.get("text").and_then(|s| s.as_str()) {
            let trimmed = t.trim();
            if trimmed == "•" || trimmed == "•" {
                if !current_section.is_empty() {
                    sections.push(current_section.join(" "));
                    current_section.clear();
                }
            } else if !trimmed.is_empty() {
                current_section.push(trimmed.to_string());
            }
        }
    }
    if !current_section.is_empty() {
        sections.push(current_section.join(" "));
    }

    if sections.is_empty() {
        return ("YouTube Music".to_string(), String::new(), String::new());
    }

    // Check if first section is a type marker (Песня, Композиция, Видео, etc.)
    let s0_lower = sections[0].to_lowercase();
    let is_type_marker = matches!(
        s0_lower.as_str(),
        "песня" | "композиция" | "трек" | "видео" | "исполнитель" | "song" | "track" | "video" | "artist"
    );

    let items: Vec<String> = if is_type_marker {
        sections.into_iter().skip(1).collect()
    } else {
        sections
    };

    let mut artist = "YouTube Music".to_string();
    let mut album = String::new();
    let mut duration = String::new();

    let is_dur = |s: &str| {
        s.contains(':') && s.chars().all(|c| c.is_ascii_digit() || c == ':')
    };

    let mut remaining = Vec::new();
    for item in items {
        if duration.is_empty() && is_dur(&item) {
            duration = item;
        } else {
            remaining.push(item);
        }
    }

    if let Some(a) = remaining.get(0) {
        artist = a.clone();
    }
    if let Some(alb) = remaining.get(1) {
        let alb_lower = alb.to_lowercase();
        if !alb_lower.contains("просмотр") && !alb_lower.contains("view") {
            album = alb.clone();
        }
    }

    (artist, album, duration)
}

/// Tries to extract text from a json value.
fn get_text(value: &Value, text_clean: bool, dot: bool) -> Option<String> {
    if let Some(e) = value.as_str() {
        Some(e.to_string())
    } else {
        let obj = value.as_object()?;
        if let Some(e) = obj.get("text") {
            get_text(e, text_clean, dot)
        } else if let Some(e) = obj.get("runs") {
            let k = e
                .as_array()?
                .iter()
                .flat_map(|x| get_text(x, text_clean, dot))
                .collect::<Vec<_>>();
            if k.is_empty() {
                None
            } else {
                Some(join_clean(&k, dot))
            }
        } else {
            None
        }
    }
}

fn join_clean(strings: &[String], dot: bool) -> String {
    strings
        .iter()
        .map(|x| x.trim())
        .filter(|x| !x.is_empty())
        .collect::<Vec<_>>()
        .join(if dot { " • " } else { " " })
}
