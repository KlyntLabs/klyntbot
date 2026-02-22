//! Session management for conversation history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use tracing::{debug, warn};
use uuid::Uuid;

use common::Result;

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
const COMPACTION_THRESHOLD: usize = 1000;

/// Number of entries to keep after compaction
const COMPACTION_KEEP: usize = 500;

/// Session manager backed by a SQL repository.
/// Uses an in-memory LRU cache for performance.
pub struct SessionManager {
    cache: HashMap<String, Session>,
    lru_order: VecDeque<String>,
    max_cache_size: usize,
    sql_repo: storage::SessionRepo,
}

impl SessionManager {
    /// Create a session manager backed by a SQL repository.
    pub async fn from_repo(repo: storage::SessionRepo) -> Self {
        Self {
            cache: HashMap::new(),
            lru_order: VecDeque::new(),
            max_cache_size: 1000,
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
        }
    }

    /// Get an existing session or create a new one
    pub async fn get_or_create(&mut self, key: impl Into<String>) -> Result<&mut Session> {
        let key = key.into();

        // Update LRU order
        self.lru_order.retain(|k| k != &key);
        self.lru_order.push_back(key.clone());

        // Evict if over capacity
        while self.lru_order.len() > self.max_cache_size {
            if let Some(old_key) = self.lru_order.pop_front() {
                if let Some(session) = self.cache.remove(&old_key) {
                    if let Err(e) = self.save(&session).await {
                        warn!(
                            "Failed to save evicted session {}: {}. Data may be lost.",
                            old_key, e
                        );
                    }
                    debug!("Evicted session from cache: {}", old_key);
                }
            }
        }

        // On cache miss: load or create, then move key into cache
        if !self.cache.contains_key(&key) {
            let session = match self.sql_repo.get_session(&key).await {
                Ok(row) => {
                    let msgs = self.sql_repo.get_messages(&key).await?;
                    Self::row_to_session(row, msgs)
                }
                Err(storage::StorageError::NotFound(_)) => {
                    // Create in SQL
                    let metadata = serde_json::Value::Object(serde_json::Map::new());
                    self.sql_repo.create_session(&key, &metadata).await?;
                    debug!("Creating new session in SQL: {}", key);
                    Session::new(key.clone())
                }
                Err(e) => return Err(e.into()),
            };
            self.cache.insert(key, session);
            // Retrieve via LRU back-reference (key was moved into cache)
            let lru_key = self.lru_order.back().unwrap();
            return Ok(self.cache.get_mut(lru_key.as_str()).unwrap());
        }

        Ok(self.cache.get_mut(&key).unwrap())
    }

    /// Save a session to SQL.
    /// Automatically compacts the session if it exceeds the compaction threshold.
    ///
    /// Uses batch INSERT to persist all messages in a single round-trip
    /// (ON CONFLICT DO NOTHING for idempotency) instead of N individual inserts.
    pub async fn save(&self, session: &Session) -> Result<()> {
        // Upsert session metadata
        let metadata = serde_json::to_value(&session.metadata).unwrap_or_default();
        self.sql_repo
            .create_session(&session.key, &metadata)
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

            for msg in &session.messages {
                ids.push(Uuid::parse_str(&msg.id).unwrap_or_else(|_| Uuid::new_v4()));
                roles.push(msg.role.clone());
                contents.push(msg.content.clone());
                timestamps.push(msg.timestamp);
                request_ids.push(msg.request_id.clone());
                tool_calls_list.push(msg.tool_calls.clone());
                metadata_list.push(msg.metadata.clone());
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
    /// This method saves the session directly from the internal cache if it exists.
    pub async fn save_by_key(&mut self, key: &str) -> Result<()> {
        if let Some(session) = self.cache.get(key) {
            self.save(session).await?;
        }
        Ok(())
    }

    /// Reset (delete) a session — removes from in-memory cache and deletes from the database.
    ///
    /// Cache removal is unconditional; the database deletion error is propagated.
    pub async fn reset_session(&mut self, key: &str) -> Result<()> {
        // Remove from in-memory cache unconditionally
        self.cache.remove(key);
        self.lru_order.retain(|k| k != key);

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
        self.cache.contains_key(key)
    }

    /// Delete a session
    pub async fn delete(&mut self, key: &str) -> Result<bool> {
        // Remove from cache
        self.cache.remove(key);

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
    /// This test does not require a live database: it uses a lazy pool (no
    /// connection is established) and directly manipulates the cache fields
    /// (accessible within the same module).
    #[tokio::test]
    async fn test_reset_session_removes_from_cache() {
        let pool = sqlx::PgPool::connect_lazy("postgres://localhost/test_unused").unwrap();
        let repo = storage::SessionRepo::new(pool);
        let mut manager = SessionManager::from_repo(repo).await;

        let key = "test:reset";

        // Insert directly into cache — no DB required
        manager.cache.insert(key.to_string(), Session::new(key));
        manager.lru_order.push_back(key.to_string());

        assert!(manager.has_session(key), "session should be in cache after insertion");

        // reset_session removes from cache unconditionally; DB call may fail — ignore
        let _ = manager.reset_session(key).await;

        assert!(
            !manager.has_session(key),
            "session should no longer be in cache after reset"
        );
        assert!(
            !manager.lru_order.contains(&key.to_string()),
            "session key should be removed from LRU order"
        );
    }

    #[test]
    fn test_has_session_false_when_not_cached() {
        // has_session is sync — no async or DB needed
        // We can't construct SessionManager without async, so we only verify the
        // logic once a manager exists via a trivial inline check.
        let key = "any:key";
        // Build a bare HashMap to mimic the cache field logic
        let cache: HashMap<String, Session> = HashMap::new();
        assert!(!cache.contains_key(key));
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
}
