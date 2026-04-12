//! Session management for conversation history.

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex as StdMutex};
use tokio::sync::Mutex as TokioMutex;
use tracing::{debug, error, warn};
use uuid::Uuid;

use common::{KlyntbotError, Result};

/// Generate a unique message ID (UUID v4)
fn generate_message_id() -> String {
    Uuid::new_v4().to_string()
}

/// A conversation session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Session key (usually channel:chat_id)
    pub key: String,

    /// Message history
    pub messages: Vec<SessionMessage>,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Session metadata
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,

    /// Optional squad ID — when set, this session uses multi-persona squad execution.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub squad_id: Option<String>,

    /// Messages remaining before next keyword correction can fire (0 = ready).
    #[serde(default)]
    pub correction_cooldown: u32,
}

impl Session {
    /// Create a new session
    pub fn new(key: impl Into<String>) -> Self {
        let now = Utc::now();
        Self {
            key: key.into(),
            messages: Vec::new(),
            created_at: now,
            updated_at: now,
            metadata: HashMap::new(),
            squad_id: None,
            correction_cooldown: 0,
        }
    }

    /// Add a message to the session
    pub fn add_message(&mut self, role: impl Into<String>, content: impl Into<String>) {
        self.add_message_with_request_id(role, content, None);
    }

    /// Add a message with an optional request ID for correlation
    pub fn add_message_with_request_id(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        request_id: Option<String>,
    ) {
        self.messages.push(SessionMessage {
            id: generate_message_id(),
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
            request_id,
            tool_calls: None,
            metadata: None,
        });
        self.updated_at = Utc::now();
        self.trim_if_needed();
    }

    /// Add a message with full structured data (tool calls, metadata).
    pub fn add_structured_message(
        &mut self,
        role: impl Into<String>,
        content: impl Into<String>,
        request_id: Option<String>,
        tool_calls: Option<serde_json::Value>,
        metadata: Option<serde_json::Value>,
    ) {
        self.messages.push(SessionMessage {
            id: generate_message_id(),
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
            request_id,
            tool_calls,
            metadata,
        });
        self.updated_at = Utc::now();
        self.trim_if_needed();
    }

    /// Get recent message history for LLM context
    pub fn get_history(&self, max_messages: usize) -> &[SessionMessage] {
        let start = self.messages.len().saturating_sub(max_messages);
        &self.messages[start..]
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
        self.updated_at = Utc::now();
    }

    /// Trim in-memory messages if they exceed the threshold.
    /// Keeps the most recent `IN_MEMORY_TRIM_KEEP` messages.
    fn trim_if_needed(&mut self) {
        if self.messages.len() > IN_MEMORY_TRIM_THRESHOLD {
            let drain_count = self.messages.len() - IN_MEMORY_TRIM_KEEP;
            self.messages.drain(..drain_count);
        }
    }

    /// Validate session integrity and auto-repair issues.
    /// Returns the number of repairs made.
    pub fn validate_and_repair(&mut self) -> usize {
        let mut repairs = 0;

        // Remove orphaned tool messages (tool message without preceding assistant tool_calls)
        let mut i = 0;
        while i < self.messages.len() {
            if self.messages[i].role == "tool" {
                let has_nearby_tool_call = self.messages[..i]
                    .iter()
                    .rev()
                    .take(10)
                    .any(|m| m.role == "assistant" && m.tool_calls.is_some());

                if !has_nearby_tool_call {
                    tracing::debug!(
                        "Removing orphaned tool message at index {} in session {}",
                        i,
                        self.key
                    );
                    self.messages.remove(i);
                    repairs += 1;
                    continue;
                }
            }
            i += 1;
        }

        // Ensure timestamps are monotonically increasing
        let mut last_ts = chrono::DateTime::<chrono::Utc>::MIN_UTC;
        for msg in &mut self.messages {
            if msg.timestamp < last_ts {
                msg.timestamp = last_ts + chrono::Duration::milliseconds(1);
                repairs += 1;
            }
            last_ts = msg.timestamp;
        }

        if repairs > 0 {
            tracing::info!("Session '{}' repaired: {} fixes applied", self.key, repairs);
        }

        repairs
    }
}

/// A single message in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    /// Unique message ID (UUID v4)
    #[serde(default = "generate_message_id")]
    pub id: String,

    /// Message role (system, user, assistant, tool)
    pub role: String,

    /// Message content
    pub content: String,

    /// Timestamp
    pub timestamp: DateTime<Utc>,

    /// Optional request ID for correlation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,

    /// Structured tool call data (function name, arguments, result)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<serde_json::Value>,

    /// Extensible message metadata (reasoning, content parts, etc.)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Compaction threshold: compact when entries exceed this count
const COMPACTION_THRESHOLD: usize = 200;

/// Number of entries to keep after compaction
const COMPACTION_KEEP: usize = 100;

/// In-memory trim threshold: trim the Vec when it exceeds this.
/// Must be ≤ COMPACTION_THRESHOLD so SQL compaction can still fire for
/// sessions loaded from disk with many historical messages.
const IN_MEMORY_TRIM_THRESHOLD: usize = 60;

/// Number of messages to keep after an in-memory trim.
const IN_MEMORY_TRIM_KEEP: usize = 40;

/// Session manager backed by a SQL repository.
///
/// Uses a DashMap for concurrent per-session access — each session has its own
/// `tokio::sync::Mutex`, eliminating the global write lock bottleneck.
/// LRU eviction order is tracked via a `std::sync::Mutex<IndexMap>` (O(1) promote/evict).
///
/// `SessionManager` is `Clone`: all clones share the same underlying map and repo,
/// so it can be stored directly (no `Arc<RwLock<SessionManager>>` needed).
#[derive(Clone)]
pub struct SessionManager {
    sessions: Arc<DashMap<String, Arc<TokioMutex<Session>>>>,
    lru_order: Arc<StdMutex<IndexMap<String, ()>>>,
    max_cache_size: usize,
    sql_repo: storage::SessionRepo,
}

impl SessionManager {
    /// Create a session manager backed by a SQL repository.
    ///
    /// `max_cache_size` controls how many sessions are kept in the in-memory cache.
    /// When the cache is full, the least-recently-used session is saved to SQL and evicted.
    pub async fn from_repo(repo: storage::SessionRepo, max_cache_size: usize) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            lru_order: Arc::new(StdMutex::new(IndexMap::new())),
            max_cache_size,
            sql_repo: repo,
        }
    }

    /// Convert SQL rows to a domain Session.
    fn row_to_session(
        row: storage::SessionRow,
        msg_rows: Vec<storage::SessionMessageRow>,
    ) -> Session {
        let metadata: HashMap<String, serde_json::Value> =
            serde_json::from_value(row.metadata).unwrap_or_default();
        let messages = msg_rows
            .into_iter()
            .map(|m| SessionMessage {
                id: m.id.to_string(),
                role: m.role,
                content: m.content,
                timestamp: m.timestamp,
                request_id: m.request_id,
                tool_calls: m.tool_calls,
                metadata: m.metadata,
            })
            .collect();

        Session {
            key: row.key,
            messages,
            created_at: row.created_at,
            updated_at: row.updated_at,
            metadata,
            squad_id: row.squad_id,
            correction_cooldown: 0,
        }
    }

    /// Get an existing session or create a new one.
    ///
    /// Returns an `Arc<TokioMutex<Session>>` — the caller locks it per-session.
    /// Concurrent calls for *different* keys proceed without blocking each other.
    /// When `squad_id` is `Some`, new sessions are stamped with the squad.
    pub async fn get_or_create(
        &self,
        key: impl Into<String>,
        squad_id: Option<&str>,
    ) -> Result<Arc<TokioMutex<Session>>> {
        let key = key.into();

        // Update LRU order and collect keys to evict (sync, brief hold).
        // IndexMap gives O(1) remove-by-key + insert, vs O(n) VecDeque::retain.
        let evict_keys = {
            let mut lru = self.lru_order.lock().unwrap();
            // Promote: remove existing entry (O(1) swap-remove) then re-insert at back
            lru.shift_remove(&key);
            lru.insert(key.clone(), ());

            let mut to_evict = Vec::new();
            while lru.len() > self.max_cache_size {
                if let Some((old_key, _)) = lru.shift_remove_index(0) {
                    to_evict.push(old_key);
                }
            }
            to_evict
        };

        // Handle evictions (async, LRU lock already released)
        for old_key in evict_keys {
            // Save BEFORE removing from cache to prevent data loss.
            // Clone the Arc out of the DashMap ref immediately to avoid holding
            // the shard read lock across async await points.
            let session_arc = self.sessions.get(&old_key).map(|r| r.value().clone());
            let save_result = if let Some(session_arc) = session_arc {
                // Clone session data and release the mutex immediately so concurrent
                // access to this session is not blocked during retries + sleep.
                let session_snapshot = session_arc.lock().await.clone();
                let mut saved = false;
                for attempt in 1..=3u32 {
                    match self.save(&session_snapshot).await {
                        Ok(_) => {
                            saved = true;
                            break;
                        }
                        Err(e) => {
                            warn!("Eviction save attempt {}/3 for {}: {}", attempt, old_key, e);
                            if attempt < 3 {
                                tokio::time::sleep(std::time::Duration::from_millis(
                                    100 * attempt as u64,
                                ))
                                .await;
                            }
                        }
                    }
                }
                saved
            } else {
                // Session already removed by another task — nothing to save
                true
            };

            if save_result {
                self.sessions.remove(&old_key);
                debug!("Evicted session from cache: {}", old_key);
            } else {
                // Re-add to LRU to retry eviction next time.
                // This prevents data loss at the cost of temporarily exceeding max_cache_size.
                error!(
                    "Failed to persist evicted session {} after 3 attempts, \
                     keeping in cache to prevent data loss",
                    old_key
                );
                let mut lru = self.lru_order.lock().unwrap();
                lru.insert(old_key, ());
            }
        }

        // Fast path: session already cached
        if let Some(session_arc) = self.sessions.get(&key) {
            return Ok(session_arc.clone());
        }

        // Slow path: load or create from DB
        let session = match self.sql_repo.get_session(&key).await {
            Ok(row) => {
                let msgs = self.sql_repo.get_messages(&key).await?;
                let mut session = Self::row_to_session(row, msgs);
                session.validate_and_repair();
                session
            }
            Err(storage::StorageError::NotFound(_)) => {
                let metadata = serde_json::Value::Object(serde_json::Map::new());
                self.sql_repo
                    .upsert_session(&key, &metadata, squad_id)
                    .await?;
                debug!("Creating new session in SQL: {}", key);
                let mut s = Session::new(key.clone());
                s.squad_id = squad_id.map(|s| s.to_string());
                s
            }
            Err(e) => return Err(e.into()),
        };

        let session_arc = Arc::new(TokioMutex::new(session));
        // Atomically insert if not already present (handles concurrent inserts for same key)
        let entry = self.sessions.entry(key).or_insert(session_arc);
        Ok(entry.value().clone())
    }

    /// Save a session to SQL.
    /// Automatically compacts the session if it exceeds the compaction threshold.
    ///
    /// Uses batch INSERT to persist all messages in a single round-trip
    /// (ON CONFLICT DO NOTHING for idempotency) instead of N individual inserts.
    pub async fn save(&self, session: &Session) -> Result<()> {
        // Upsert session metadata
        let metadata = serde_json::to_value(&session.metadata)
            .map_err(|e| KlyntbotError::Storage(format!("session metadata: {e}")))?;
        self.sql_repo
            .upsert_session(&session.key, &metadata, session.squad_id.as_deref())
            .await?;

        // Build batch arrays from session messages
        if !session.messages.is_empty() {
            let mut ids = Vec::with_capacity(session.messages.len());
            let mut roles = Vec::with_capacity(session.messages.len());
            let mut contents = Vec::with_capacity(session.messages.len());
            let mut timestamps = Vec::with_capacity(session.messages.len());
            let mut request_ids = Vec::with_capacity(session.messages.len());
            let mut tool_calls_list = Vec::with_capacity(session.messages.len());
            let mut metadata_list = Vec::with_capacity(session.messages.len());
            let mut persona_ids = Vec::with_capacity(session.messages.len());

            for msg in &session.messages {
                ids.push(Uuid::parse_str(&msg.id).unwrap_or_else(|_| Uuid::new_v4()));
                roles.push(msg.role.clone());
                contents.push(msg.content.clone());
                timestamps.push(msg.timestamp);
                request_ids.push(msg.request_id.clone());
                tool_calls_list.push(msg.tool_calls.clone());
                metadata_list.push(msg.metadata.clone());
                persona_ids.push(None); // persona_id not tracked in in-memory Session
            }

            self.sql_repo
                .batch_add_messages(
                    &session.key,
                    &ids,
                    &roles,
                    &contents,
                    &timestamps,
                    &request_ids,
                    &tool_calls_list,
                    &metadata_list,
                    &persona_ids,
                )
                .await?;
        }

        // SQL compaction: if message count exceeds threshold, compact
        match self.sql_repo.count_messages(&session.key).await {
            Ok(count) if count as usize > COMPACTION_THRESHOLD => {
                let removed = count as usize - COMPACTION_KEEP;
                debug!(
                    "Compacting SQL session {}: {} -> ~{} messages",
                    session.key, count, COMPACTION_KEEP
                );

                // Insert a compaction marker before deleting
                let marker_id = Uuid::new_v4();
                let marker_content =
                    format!("[Session compacted: {} older messages removed]", removed);
                let _ = self
                    .sql_repo
                    .add_message(
                        &session.key,
                        marker_id,
                        "system",
                        &marker_content,
                        None,
                        None,
                        None,
                        None, // persona_id
                    )
                    .await;

                // keep_count includes the marker we just added
                match self
                    .sql_repo
                    .compact_session(&session.key, COMPACTION_KEEP as i64)
                    .await
                {
                    Ok(deleted) => {
                        debug!(
                            "SQL compaction complete for {}: {} messages deleted",
                            session.key, deleted
                        );
                    }
                    Err(e) => {
                        warn!("SQL compaction failed for {}: {}", session.key, e);
                    }
                }
            }
            Ok(_) => {} // below threshold, no compaction needed
            Err(e) => {
                warn!("Failed to count messages for compaction check: {}", e);
            }
        }

        Ok(())
    }

    /// Save a session by key without requiring a clone.
    /// Locks the session, clones it, then persists.
    pub async fn save_by_key(&self, key: &str) -> Result<()> {
        if let Some(session_arc) = self.sessions.get(key) {
            let session = session_arc.lock().await;
            self.save(&session).await?;
        }
        Ok(())
    }

    /// Reset (delete) a session — removes from in-memory cache and deletes from the database.
    ///
    /// Cache removal is unconditional; the database deletion error is propagated.
    pub async fn reset_session(&self, key: &str) -> Result<()> {
        // Remove from in-memory cache unconditionally
        self.sessions.remove(key);
        {
            let mut lru = self.lru_order.lock().unwrap();
            lru.shift_remove(key);
        }

        // Delete from database (cascades to messages)
        self.sql_repo
            .delete_session(key)
            .await
            .map(|_| ())
            .map_err(common::KlyntbotError::from)?;

        debug!("Session reset: {}", key);
        Ok(())
    }

    /// Check whether a session exists in the in-memory cache.
    pub fn has_session(&self, key: &str) -> bool {
        self.sessions.contains_key(key)
    }

    /// Delete a session
    pub async fn delete(&self, key: &str) -> Result<bool> {
        // Remove from cache and LRU tracking
        self.sessions.remove(key);
        {
            let mut lru = self.lru_order.lock().unwrap();
            lru.shift_remove(key);
        }

        self.sql_repo
            .delete_session(key)
            .await
            .map_err(common::KlyntbotError::from)
    }

    /// List all sessions
    pub async fn list(&self) -> Result<Vec<SessionInfo>> {
        let rows = self.sql_repo.list_sessions().await?;
        // Already sorted by updated_at DESC in the query
        let sessions: Vec<SessionInfo> = rows
            .into_iter()
            .map(|r| SessionInfo {
                key: r.key,
                created_at: r.created_at,
                updated_at: r.updated_at,
                message_count: r.message_count as usize,
            })
            .collect();
        Ok(sessions)
    }

    /// Save compressed history prefix for delta compaction.
    pub async fn save_compressed_prefix(
        &self,
        session_key: &str,
        prefix_json: &str,
        through_idx: i64,
    ) -> Result<()> {
        self.sql_repo
            .save_compressed_prefix(session_key, prefix_json, through_idx)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))
    }

    /// Load compressed history prefix for delta compaction.
    pub async fn load_compressed_prefix(&self, session_key: &str) -> Result<Option<(String, i64)>> {
        self.sql_repo
            .load_compressed_prefix(session_key)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))
    }

    /// Clear compressed prefix (invalidation).
    pub async fn clear_compressed_prefix(&self, session_key: &str) -> Result<()> {
        self.sql_repo
            .clear_compressed_prefix(session_key)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))
    }
}

/// Session information for listing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub key: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        let mut session = Session::new("test:chat123");
        assert_eq!(session.key, "test:chat123");
        assert_eq!(session.messages.len(), 0);

        session.add_message("user", "Hello");
        session.add_message("assistant", "Hi there!");

        assert_eq!(session.messages.len(), 2);
        assert_eq!(session.messages[0].role, "user");
        assert_eq!(session.messages[0].content, "Hello");
        assert_eq!(session.messages[1].role, "assistant");
    }

    #[test]
    fn test_session_get_history() {
        let mut session = Session::new("test:chat123");

        // Add 10 messages
        for i in 0..10 {
            session.add_message("user", format!("Message {}", i));
        }

        // Get last 3 messages
        let history = session.get_history(3);
        assert_eq!(history.len(), 3);
        assert_eq!(history[0].content, "Message 7");
        assert_eq!(history[1].content, "Message 8");
        assert_eq!(history[2].content, "Message 9");

        // Get more than available
        let all_history = session.get_history(100);
        assert_eq!(all_history.len(), 10);
    }

    #[test]
    fn test_session_clear() {
        let mut session = Session::new("test:chat123");
        session.add_message("user", "Hello");
        session.add_message("assistant", "Hi!");
        assert_eq!(session.messages.len(), 2);

        session.clear();
        assert_eq!(session.messages.len(), 0);
    }

    #[test]
    fn test_message_id_generated() {
        let mut session = Session::new("test:chat123");
        session.add_message("user", "Hello");

        // Verify message has an ID
        assert!(!session.messages[0].id.is_empty());

        // Verify ID looks like a UUID (36 chars with hyphens)
        assert_eq!(session.messages[0].id.len(), 36);
        assert!(session.messages[0].id.contains('-'));
    }

    #[test]
    fn test_message_id_unique() {
        let mut session = Session::new("test:chat123");
        session.add_message("user", "Message 1");
        session.add_message("user", "Message 2");
        session.add_message("user", "Message 3");

        // Verify each message has a unique ID
        let id1 = &session.messages[0].id;
        let id2 = &session.messages[1].id;
        let id3 = &session.messages[2].id;

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_session_message_has_request_id() {
        let mut session = Session::new("test:reqid");
        session.add_message_with_request_id("user", "Hello", Some("req-abc-123".to_string()));

        assert_eq!(
            session.messages[0].request_id,
            Some("req-abc-123".to_string())
        );

        // Regular add_message should have None request_id
        session.add_message("assistant", "Hi");
        assert_eq!(session.messages[1].request_id, None);
    }

    #[test]
    fn test_add_structured_message() {
        let mut session = Session::new("test:structured");
        let tool_calls = serde_json::json!([{"name": "search", "arguments": {"query": "rust"}}]);
        let metadata = serde_json::json!({"reasoning": "User wants search results"});

        session.add_structured_message(
            "assistant",
            "Here are your results",
            Some("req-123".to_string()),
            Some(tool_calls.clone()),
            Some(metadata.clone()),
        );

        assert_eq!(session.messages.len(), 1);
        let msg = &session.messages[0];
        assert_eq!(msg.role, "assistant");
        assert_eq!(msg.content, "Here are your results");
        assert_eq!(msg.request_id, Some("req-123".to_string()));
        assert_eq!(msg.tool_calls, Some(tool_calls));
        assert_eq!(msg.metadata, Some(metadata));
    }

    /// Verify that `has_session` correctly reflects in-memory cache state.
    ///
    /// Uses a lazy pool (no connection is established) and directly manipulates
    /// the sessions DashMap (accessible within the same module).
    #[tokio::test]
    async fn test_reset_session_removes_from_cache() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        let repo = storage::SessionRepo::new(pool);
        let manager = SessionManager::from_repo(repo, 1000).await;

        let key = "test:reset";

        // Insert directly into cache — no DB required
        manager.sessions.insert(
            key.to_string(),
            Arc::new(TokioMutex::new(Session::new(key))),
        );
        manager
            .lru_order
            .lock()
            .unwrap()
            .insert(key.to_string(), ());

        assert!(
            manager.has_session(key),
            "session should be in cache after insertion"
        );

        // reset_session removes from cache unconditionally; DB call may fail — ignore
        let _ = manager.reset_session(key).await;

        assert!(
            !manager.has_session(key),
            "session should no longer be in cache after reset"
        );
        assert!(
            !manager
                .lru_order
                .lock()
                .unwrap()
                .contains_key(&key.to_string()),
            "session key should be removed from LRU order"
        );
    }

    #[tokio::test]
    async fn test_delete_session_removes_from_lru() {
        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        let repo = storage::SessionRepo::new(pool);
        let manager = SessionManager::from_repo(repo, 1000).await;

        let key = "test:delete-lru";

        // Insert into cache + LRU
        manager.sessions.insert(
            key.to_string(),
            Arc::new(TokioMutex::new(Session::new(key))),
        );
        manager
            .lru_order
            .lock()
            .unwrap()
            .insert(key.to_string(), ());

        assert!(manager.has_session(key));
        assert!(manager.lru_order.lock().unwrap().contains_key(key));

        // delete should remove from both sessions and lru_order
        let _ = manager.delete(key).await;

        assert!(
            !manager.has_session(key),
            "session should no longer be in cache after delete"
        );
        assert!(
            !manager.lru_order.lock().unwrap().contains_key(key),
            "session key should be removed from LRU order after delete"
        );
    }

    #[test]
    fn test_has_session_false_when_not_cached() {
        let key = "any:key";
        let sessions: DashMap<String, Arc<TokioMutex<Session>>> = DashMap::new();
        assert!(!sessions.contains_key(key));
    }

    #[test]
    fn test_add_message_has_none_structured_fields() {
        let mut session = Session::new("test:plain");
        session.add_message("user", "Hello");
        session.add_message_with_request_id("user", "World", Some("req-1".to_string()));

        // Both methods should set tool_calls and metadata to None
        assert!(session.messages[0].tool_calls.is_none());
        assert!(session.messages[0].metadata.is_none());
        assert!(session.messages[1].tool_calls.is_none());
        assert!(session.messages[1].metadata.is_none());
    }

    /// Verify SessionManager is Clone (compile-time check).
    #[test]
    fn test_session_manager_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<SessionManager>();
    }

    /// Verify SessionManager is Send + Sync (compile-time check).
    #[test]
    fn test_session_manager_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<SessionManager>();
    }

    /// Verify concurrent access to *different* sessions doesn't deadlock.
    #[tokio::test]
    async fn test_concurrent_different_sessions_no_deadlock() {
        use std::time::Duration;

        let pool = sqlx::SqlitePool::connect(":memory:").await.unwrap();
        let repo = storage::SessionRepo::new(pool);
        let manager = SessionManager::from_repo(repo, 1000).await;

        let key_a = "test:session_a";
        let key_b = "test:session_b";

        // Insert sessions directly — no DB required
        manager.sessions.insert(
            key_a.to_string(),
            Arc::new(TokioMutex::new(Session::new(key_a))),
        );
        manager.sessions.insert(
            key_b.to_string(),
            Arc::new(TokioMutex::new(Session::new(key_b))),
        );

        let arc_a = manager.sessions.get(key_a).unwrap().clone();
        let arc_b = manager.sessions.get(key_b).unwrap().clone();

        // Task 1: mutates session A
        let task1 = tokio::spawn(async move {
            let mut s = arc_a.lock().await;
            tokio::time::sleep(Duration::from_millis(10)).await;
            s.add_message("user", "task1 writes to A");
        });

        // Task 2: mutates session B concurrently
        let task2 = tokio::spawn(async move {
            let mut s = arc_b.lock().await;
            tokio::time::sleep(Duration::from_millis(10)).await;
            s.add_message("user", "task2 writes to B");
        });

        // Both should complete well within 5 seconds (no deadlock)
        let result = tokio::time::timeout(Duration::from_secs(5), async {
            let _ = task1.await;
            let _ = task2.await;
        })
        .await;

        assert!(result.is_ok(), "tasks deadlocked or timed out");
    }

    #[test]
    fn test_validate_removes_orphaned_tool_messages() {
        let mut session = Session::new("test-validate".to_string());
        session.add_message("user", "hello");
        session.add_message("assistant", "hi there");
        // Add orphaned tool result (no preceding assistant with tool_calls)
        session.add_structured_message("tool", "result data", None, None, None);

        assert_eq!(session.messages.len(), 3);
        let repairs = session.validate_and_repair();
        assert!(repairs > 0);
        assert_eq!(session.messages.len(), 2);
    }

    #[test]
    fn test_validate_preserves_valid_session() {
        let mut session = Session::new("test-valid".to_string());
        session.add_message("user", "hello");
        session.add_message("assistant", "hi");
        session.add_message("user", "thanks");

        let repairs = session.validate_and_repair();
        assert_eq!(repairs, 0);
        assert_eq!(session.messages.len(), 3);
    }

    #[test]
    fn test_validate_empty_session() {
        let mut session = Session::new("test-empty".to_string());
        let repairs = session.validate_and_repair();
        assert_eq!(repairs, 0);
    }

    #[test]
    fn test_validate_preserves_tool_message_with_tool_calls() {
        let mut session = Session::new("test-valid-tool".to_string());
        session.add_message("user", "search for rust");
        // Assistant with tool_calls
        session.add_structured_message(
            "assistant",
            "",
            None,
            Some(serde_json::json!([{"name": "search", "arguments": {"q": "rust"}}])),
            None,
        );
        // Tool response (not orphaned — preceded by assistant with tool_calls)
        session.add_structured_message("tool", "search results", None, None, None);

        assert_eq!(session.messages.len(), 3);
        let repairs = session.validate_and_repair();
        assert_eq!(repairs, 0);
        assert_eq!(session.messages.len(), 3);
    }

    #[test]
    fn test_validate_fixes_out_of_order_timestamps() {
        let mut session = Session::new("test-timestamps".to_string());
        session.add_message("user", "first");
        session.add_message("user", "second");
        session.add_message("user", "third");

        // Manually set timestamps out of order
        let base = Utc::now();
        session.messages[0].timestamp = base;
        session.messages[1].timestamp = base - chrono::Duration::seconds(10); // out of order
        session.messages[2].timestamp = base + chrono::Duration::seconds(10);

        let repairs = session.validate_and_repair();
        assert_eq!(repairs, 1);
        // The second message should now be after the first
        assert!(session.messages[1].timestamp >= session.messages[0].timestamp);
        // All timestamps should be monotonically increasing
        for i in 1..session.messages.len() {
            assert!(session.messages[i].timestamp >= session.messages[i - 1].timestamp);
        }
    }

    #[test]
    fn session_trims_messages_when_exceeding_threshold() {
        let mut session = Session::new("test");
        for i in 0..=IN_MEMORY_TRIM_THRESHOLD {
            session.add_message("user", format!("message {i}"));
        }
        assert_eq!(session.messages.len(), IN_MEMORY_TRIM_KEEP);
        assert_eq!(
            session.messages.last().unwrap().content,
            format!("message {IN_MEMORY_TRIM_THRESHOLD}")
        );
    }

    #[tokio::test]
    async fn test_eviction_preserves_session_on_save_failure() {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = storage::SessionRepo::new(pool.inner().clone());
        let manager = SessionManager::from_repo(repo, 2).await;

        let _s1 = manager.get_or_create("key-1", None).await.unwrap();
        let _s2 = manager.get_or_create("key-2", None).await.unwrap();

        {
            let s1 = manager.get_or_create("key-1", None).await.unwrap();
            let mut session = s1.lock().await;
            session.add_message("user", "important data");
        }

        let _s3 = manager.get_or_create("key-3", None).await.unwrap();

        let s1_reloaded = manager.get_or_create("key-1", None).await.unwrap();
        let session = s1_reloaded.lock().await;
        assert!(
            session
                .messages
                .iter()
                .any(|m| m.role == "user" && m.content.contains("important data")),
            "Evicted session should be recoverable from DB"
        );
    }
}
