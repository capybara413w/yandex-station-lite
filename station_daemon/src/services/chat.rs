use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;
use tracing::{error, info, warn};

pub const CHAT_INACTIVITY_TIMEOUT_SECS: u64 = 3600; // 1 hour

static MSG_COUNTER: AtomicU64 = AtomicU64::new(1);

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn generate_id(prefix: &str) -> String {
    let now = now_secs();
    let seq = MSG_COUNTER.fetch_add(1, Ordering::Relaxed);
    let random_part = (now.wrapping_mul(31) ^ seq.wrapping_mul(17)) & 0xffffff;
    format!("{}_{}_{:06x}", prefix, now, random_part)
}

fn truncate_title(prompt: &str, max_len: usize) -> String {
    let clean = prompt.trim().lines().next().unwrap_or("").trim();
    if clean.is_empty() {
        return "Новый диалог".to_string();
    }
    let chars: Vec<char> = clean.chars().collect();
    if chars.len() <= max_len {
        clean.to_string()
    } else {
        let mut t: String = chars[..max_len].iter().collect();
        t.push_str("...");
        t
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageItem {
    pub id: String,
    pub role: String, // "user" or "assistant"
    pub text: String,
    pub timestamp: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<[u8; 3]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub song: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ha_calls: Option<Vec<crate::services::deepseek::HaCall>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    #[serde(default)]
    pub messages: Vec<ChatMessageItem>,
    #[serde(default)]
    pub is_deleted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatSessionSummary {
    pub id: String,
    pub title: String,
    pub created_at: u64,
    pub updated_at: u64,
    pub message_count: usize,
    pub last_message: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct ChatStore {
    #[serde(default)]
    sessions: Vec<ChatSession>,
}

pub struct ChatService {
    storage_path: PathBuf,
    store: Arc<RwLock<ChatStore>>,
}

impl ChatService {
    pub fn new<P: AsRef<Path>>(storage_path: P) -> Self {
        let path = storage_path.as_ref().to_path_buf();
        let store = Self::load_from_disk(&path);
        Self {
            storage_path: path,
            store: Arc::new(RwLock::new(store)),
        }
    }

    fn load_from_disk(path: &Path) -> ChatStore {
        if path.exists() {
            match std::fs::read_to_string(path) {
                Ok(content) => match serde_json::from_str::<ChatStore>(&content) {
                    Ok(mut store) => {
                        // Purge deleted sessions to conserve RAM and flash memory
                        store.sessions.retain(|s| !s.is_deleted);
                        if store.sessions.len() > 25 {
                            store.sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
                            store.sessions.truncate(25);
                        }
                        // Truncate messages in each session if needed
                        for s in store.sessions.iter_mut() {
                            if s.messages.len() > 40 {
                                let excess = s.messages.len() - 40;
                                s.messages.drain(0..excess);
                            }
                        }
                        info!(
                            "Loaded {} active chat sessions from {}",
                            store.sessions.len(),
                            path.display()
                        );
                        return store;
                    }
                    Err(e) => {
                        warn!("Failed to parse chat store from {}: {}, starting empty", path.display(), e);
                    }
                },
                Err(e) => {
                    warn!("Failed to read chat store from {}: {}, starting empty", path.display(), e);
                }
            }
        }
        ChatStore::default()
    }

    async fn persist_store(&self) {
        let store = self.store.read().await;
        match serde_json::to_string_pretty(&*store) {
            Ok(json_data) => {
                if let Some(parent) = self.storage_path.parent() {
                    let _ = tokio::fs::create_dir_all(parent).await;
                }
                if let Err(e) = tokio::fs::write(&self.storage_path, json_data).await {
                    error!("Failed to write chat store to {}: {}", self.storage_path.display(), e);
                }
            }
            Err(e) => {
                error!("Failed to serialize chat store: {}", e);
            }
        }
    }

    /// Prepares a chat session according to the 1-hour rule:
    /// - If `requested_session_id` is supplied and exists (not deleted), checks if < 1h elapsed since last message.
    ///   If < 1h, continues it with context. If >= 1h, starts a new chat.
    /// - If `requested_session_id` is None, finds the most recently updated non-deleted session.
    ///   If < 1h elapsed, continues it with context. If >= 1h, starts a new chat.
    /// - Deleted sessions are strictly ignored.
    ///
    /// Returns: (session_id, previous_messages_for_context, is_new_session)
    pub async fn prepare_session_for_prompt(
        &self,
        requested_session_id: Option<&str>,
    ) -> (String, Vec<ChatMessageItem>, bool) {
        let now = now_secs();
        let mut store = self.store.write().await;

        let candidate_session_id = if let Some(req_id) = requested_session_id {
            if let Some(s) = store.sessions.iter().find(|s| s.id == req_id && !s.is_deleted) {
                Some(s.id.clone())
            } else {
                None
            }
        } else {
            // Find most recently active non-deleted session
            store
                .sessions
                .iter()
                .filter(|s| !s.is_deleted)
                .max_by_key(|s| {
                    s.messages
                        .last()
                        .map(|m| m.timestamp)
                        .unwrap_or(s.updated_at)
                })
                .map(|s| s.id.clone())
        };

        if let Some(cand_id) = candidate_session_id {
            if let Some(s) = store.sessions.iter().find(|s| s.id == cand_id) {
                let last_ts = s
                    .messages
                    .last()
                    .map(|m| m.timestamp)
                    .unwrap_or(s.updated_at);
                let elapsed = now.saturating_sub(last_ts);

                if elapsed < CHAT_INACTIVITY_TIMEOUT_SECS {
                    info!(
                        "Chat session '{}' is active (last message {}s ago < 1h). Attaching {} context messages.",
                        s.id,
                        elapsed,
                        s.messages.len()
                    );
                    return (s.id.clone(), s.messages.clone(), false);
                } else {
                    info!(
                        "Chat session '{}' expired (last message {}s ago >= 1h). Starting fresh chat session.",
                        s.id, elapsed
                    );
                }
            }
        }

        // Start new chat session (prune oldest if at limit)
        if store.sessions.len() >= 25 {
            store.sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            store.sessions.truncate(24);
        }

        let new_id = generate_id("chat");
        let new_session = ChatSession {
            id: new_id.clone(),
            title: "Новый диалог".to_string(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            is_deleted: false,
        };

        info!("Created new chat session: '{}'", new_id);
        store.sessions.push(new_session);
        drop(store);
        self.persist_store().await;

        (new_id, Vec::new(), true)
    }

    /// Records a completed user query and assistant response into the session.
    pub async fn record_turn(
        &self,
        session_id: &str,
        user_prompt: &str,
        assistant_text: &str,
        action: &str,
        color: [u8; 3],
        song: &str,
        ha_calls: Vec<crate::services::deepseek::HaCall>,
    ) {
        let now = now_secs();
        let mut store = self.store.write().await;

        let session_opt = store
            .sessions
            .iter_mut()
            .find(|s| s.id == session_id && !s.is_deleted);

        if let Some(session) = session_opt {
            // Update title if needed
            if session.title == "Новый диалог" || session.messages.is_empty() {
                session.title = truncate_title(user_prompt, 38);
            }

            // Append User message
            session.messages.push(ChatMessageItem {
                id: generate_id("msg_u"),
                role: "user".to_string(),
                text: user_prompt.trim().to_string(),
                timestamp: now,
                action: None,
                color: None,
                song: None,
                ha_calls: None,
            });

            // Append Assistant message
            session.messages.push(ChatMessageItem {
                id: generate_id("msg_a"),
                role: "assistant".to_string(),
                text: assistant_text.trim().to_string(),
                timestamp: now,
                action: if action.is_empty() || action == "none" {
                    None
                } else {
                    Some(action.to_string())
                },
                color: Some(color),
                song: if song.is_empty() {
                    None
                } else {
                    Some(song.to_string())
                },
                ha_calls: if ha_calls.is_empty() {
                    None
                } else {
                    Some(ha_calls)
                },
            });

            // Cap messages to prevent memory leak and huge JSON
            if session.messages.len() > 40 {
                let excess = session.messages.len() - 40;
                session.messages.drain(0..excess);
            }

            session.updated_at = now;
        } else {
            warn!("Chat session '{}' not found to record turn", session_id);
        }

        drop(store);
        self.persist_store().await;
    }

    /// Returns summary of all non-deleted chat sessions sorted by updated_at descending.
    pub async fn list_sessions(&self) -> Vec<ChatSessionSummary> {
        let now = now_secs();
        let store = self.store.read().await;

        let mut list: Vec<ChatSessionSummary> = store
            .sessions
            .iter()
            .filter(|s| !s.is_deleted)
            .map(|s| {
                let last_ts = s
                    .messages
                    .last()
                    .map(|m| m.timestamp)
                    .unwrap_or(s.updated_at);
                let is_active = now.saturating_sub(last_ts) < CHAT_INACTIVITY_TIMEOUT_SECS;
                let last_message = s.messages.last().map(|m| m.text.clone());

                ChatSessionSummary {
                    id: s.id.clone(),
                    title: s.title.clone(),
                    created_at: s.created_at,
                    updated_at: s.updated_at,
                    message_count: s.messages.len(),
                    last_message,
                    is_active,
                }
            })
            .collect();

        // Sort newest updated first
        list.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        list
    }

    /// Retrieves full session details if not deleted.
    pub async fn get_session(&self, id: &str) -> Option<ChatSession> {
        let store = self.store.read().await;
        store
            .sessions
            .iter()
            .find(|s| s.id == id && !s.is_deleted)
            .cloned()
    }

    /// Creates an explicit new empty chat session.
    pub async fn create_session(&self, title: Option<&str>) -> ChatSession {
        let now = now_secs();
        let mut store = self.store.write().await;

        if store.sessions.len() >= 25 {
            store.sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
            store.sessions.truncate(24);
        }

        let session = ChatSession {
            id: generate_id("chat"),
            title: title
                .map(|t| truncate_title(t, 38))
                .unwrap_or_else(|| "Новый диалог".to_string()),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            is_deleted: false,
        };

        let result = session.clone();
        store.sessions.push(session);
        drop(store);
        self.persist_store().await;

        result
    }

    /// Deletes a session by removing it from memory and disk.
    pub async fn delete_session(&self, id: &str) -> bool {
        let mut store = self.store.write().await;
        let before_len = store.sessions.len();
        store.sessions.retain(|s| s.id != id);
        if store.sessions.len() < before_len {
            drop(store);
            self.persist_store().await;
            info!("Deleted and pruned chat session '{}'", id);
            true
        } else {
            false
        }
    }

    /// Clears all sessions completely.
    pub async fn clear_all_sessions(&self) {
        let mut store = self.store.write().await;
        store.sessions.clear();
        drop(store);
        self.persist_store().await;
        info!("Cleared all chat sessions");
    }
}
