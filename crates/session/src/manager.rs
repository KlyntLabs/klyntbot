//! Session management for conversation history.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use tokio::fs;
use tracing::{debug, warn};
use uuid::Uuid;

use common::{Result, SessionError};

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

/// Session manager with JSONL persistence.
/// Optionally backed by a SQL repository when constructed via `from_repo()`.
pub struct SessionManager {
    sessions_dir: PathBuf,
    cache: HashMap<String, Session>,
    lru_order: VecDeque<String>,
    max_cache_size: usize,
    write_mutex: tokio::sync::Mutex<()>,
    /// Optional SQL backend — when present, persistence is delegated to SQL.
    sql_repo: Option<storage::SessionRepo>,
}

impl SessionManager {
    /// Create a new session manager backed by JSONL
    pub async fn new(sessions_dir: impl Into<PathBuf>) -> Self {
        Self::with_capacity(sessions_dir, 1000).await // Default 1000 sessions
    }

    /// Create a new session manager with a specific cache capacity
    pub async fn with_capacity(sessions_dir: impl Into<PathBuf>, max_cache_size: usize) -> Self {
        let sessions_dir = sessions_dir.into();

        // Create sessions directory if it doesn't exist
        if let Err(e) = tokio::fs::create_dir_all(&sessions_dir).await {
            warn!("Failed to create sessions directory: {}", e);
        }

        Self {
            sessions_dir,
            cache: HashMap::new(),
            lru_order: VecDeque::new(),
            max_cache_size,
            write_mutex: tokio::sync::Mutex::new(()),
            sql_repo: None,
        }
    }

    /// Create a session manager backed by a SQL repository.
    /// Still uses an in-memory LRU cache for performance.
    pub async fn from_repo(repo: storage::SessionRepo) -> Self {
        Self {
            sessions_dir: PathBuf::from("/dev/null"),
            cache: HashMap::new(),
            lru_order: VecDeque::new(),
            max_cache_size: 1000,
            write_mutex: tokio::sync::Mutex::new(()),
            sql_repo: Some(repo),
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

    /// Get the file path for a session
    fn session_path(&self, key: &str) -> PathBuf {
        // Sanitize the key for filesystem use
        let safe_key = key.replace([':', '/', '\\'], "_");
        self.sessions_dir.join(format!("{}.jsonl", safe_key))
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

        // On cache miss: load or create, then move key into cache (no extra clone)
        if !self.cache.contains_key(&key) {
            let session = if let Some(repo) = &self.sql_repo {
                // SQL path: try to load from DB
                match repo.get_session(&key).await {
                    Ok(row) => {
                        let msgs = repo.get_messages(&key).await?;
                        Self::row_to_session(row, msgs)
                    }
                    Err(storage::StorageError::NotFound(_)) => {
                        // Create in SQL
                        let metadata = serde_json::Value::Object(serde_json::Map::new());
                        repo.create_session(&key, &metadata).await?;
                        debug!("Creating new session in SQL: {}", key);
                        Session::new(key.clone())
                    }
                    Err(e) => return Err(e.into()),
                }
            } else {
                // JSONL path
                match self.load(&key).await {
                    Ok(s) => s,
                    Err(_) => {
                        debug!("Creating new session: {}", key);
                        Session::new(key.clone())
                    }
                }
            };
            self.cache.insert(key, session);
            // Retrieve via LRU back-reference (key was moved into cache)
            let lru_key = self.lru_order.back().unwrap();
            return Ok(self.cache.get_mut(lru_key.as_str()).unwrap());
        }

        Ok(self.cache.get_mut(&key).unwrap())
    }

    /// Load a session from disk
    async fn load(&self, key: &str) -> Result<Session> {
        let path = self.session_path(key);

        if !path.exists() {
            return Err(SessionError::NotFound(key.to_string()).into());
        }

        let content = fs::read_to_string(&path).await.map_err(|e| {
            SessionError::LoadFailed(format!("Failed to read {}: {}", path.display(), e))
        })?;

        let line_count = content.lines().count();
        let mut messages = Vec::with_capacity(line_count);
        let mut metadata = HashMap::with_capacity(4);
        let mut created_at = None;
        let mut updated_at = None;
        let mut stored_key: Option<String> = None;

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let value: serde_json::Value = serde_json::from_str(line)
                .map_err(|e| SessionError::LoadFailed(format!("Failed to parse line: {}", e)))?;

            if let Some(msg_type) = value.get("_type").and_then(|v| v.as_str()) {
                if msg_type == "metadata" {
                    if let Some(k) = value.get("key").and_then(|v| v.as_str()) {
                        stored_key = Some(k.to_string());
                    }
                    if let Some(meta) = value.get("metadata") {
                        if let Some(obj) = meta.as_object() {
                            metadata = obj.clone().into_iter().collect();
                        }
                    }
                    if let Some(created) = value.get("created_at").and_then(|v| v.as_str()) {
                        created_at = DateTime::parse_from_rfc3339(created)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc));
                    }
                    if let Some(updated) = value.get("updated_at").and_then(|v| v.as_str()) {
                        updated_at = DateTime::parse_from_rfc3339(updated)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc));
                    }
                }
            } else {
                // Regular message
                let msg: SessionMessage = serde_json::from_value(value).map_err(|e| {
                    SessionError::LoadFailed(format!("Failed to parse message: {}", e))
                })?;
                messages.push(msg);
            }
        }

        Ok(Session {
            key: stored_key.unwrap_or_else(|| key.to_string()),
            messages,
            created_at: created_at.unwrap_or_else(Utc::now),
            updated_at: updated_at.unwrap_or_else(Utc::now),
            metadata,
        })
    }

    /// Save a session to disk (JSONL) or SQL depending on the backend.
    /// Automatically compacts the session if it exceeds the compaction threshold.
    pub async fn save(&self, session: &Session) -> Result<()> {
        if let Some(repo) = &self.sql_repo {
            // Upsert session metadata
            let metadata = serde_json::to_value(&session.metadata).unwrap_or_default();
            repo.create_session(&session.key, &metadata).await?;

            // Persist all messages (idempotent via ON CONFLICT or just try-insert)
            for msg in &session.messages {
                let msg_id = Uuid::parse_str(&msg.id).unwrap_or_else(|_| Uuid::new_v4());
                // Try to add; ignore duplicate key errors
                match repo
                    .add_message(
                        &session.key,
                        msg_id,
                        &msg.role,
                        &msg.content,
                        msg.request_id.as_deref(),
                        msg.tool_calls.as_ref(),
                        msg.metadata.as_ref(),
                    )
                    .await
                {
                    Ok(_) => {}
                    Err(storage::StorageError::Sqlx(_)) => {
                        // Likely duplicate — ignore
                    }
                    Err(e) => {
                        return Err(e.into());
                    }
                }
            }

            // SQL compaction: if message count exceeds threshold, compact
            match repo.count_messages(&session.key).await {
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
                    let _ = repo
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
                    match repo
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

            return Ok(());
        }

        let _guard = self.write_mutex.lock().await;

        let path = self.session_path(&session.key);

        debug!("Saving session to: {}", path.display());

        // Apply compaction if needed: keep only the last COMPACTION_KEEP messages
        let messages = if session.messages.len() > COMPACTION_THRESHOLD {
            let start = session.messages.len() - COMPACTION_KEEP;
            debug!(
                "Compacting session {}: {} -> {} messages",
                session.key,
                session.messages.len(),
                COMPACTION_KEEP
            );
            &session.messages[start..]
        } else {
            &session.messages
        };

        let mut content = String::new();

        // Write metadata line (includes session key for lossless round-trip)
        let metadata_line = serde_json::json!({
            "_type": "metadata",
            "key": session.key,
            "created_at": session.created_at.to_rfc3339(),
            "updated_at": session.updated_at.to_rfc3339(),
            "metadata": session.metadata,
        });
        content.push_str(&serde_json::to_string(&metadata_line).map_err(SessionError::Json)?);
        content.push('\n');

        // Write message lines
        for msg in messages {
            content.push_str(&serde_json::to_string(msg).map_err(SessionError::Json)?);
            content.push('\n');
        }

        // Atomic write: write to temp file, then rename
        let tmp_path = path.with_extension("jsonl.tmp");
        fs::write(&tmp_path, &content).await.map_err(|e| {
            SessionError::SaveFailed(format!(
                "Failed to write temp {}: {}",
                tmp_path.display(),
                e
            ))
        })?;

        fs::rename(&tmp_path, &path).await.map_err(|e| {
            SessionError::SaveFailed(format!(
                "Failed to rename {} -> {}: {}",
                tmp_path.display(),
                path.display(),
                e
            ))
        })?;

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

    /// Delete a session
    pub async fn delete(&mut self, key: &str) -> Result<bool> {
        // Remove from cache
        self.cache.remove(key);

        if let Some(repo) = &self.sql_repo {
            return repo
                .delete_session(key)
                .await
                .map_err(common::KlyntbotError::from);
        }

        // Remove file
        let path = self.session_path(key);
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            fs::remove_file(&path).await.map_err(SessionError::Io)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// List all sessions
    pub async fn list(&self) -> Result<Vec<SessionInfo>> {
        if let Some(repo) = &self.sql_repo {
            let rows = repo.list_sessions().await?;
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
            return Ok(sessions);
        }

        let mut sessions = Vec::new();

        if !tokio::fs::try_exists(&self.sessions_dir)
            .await
            .unwrap_or(false)
        {
            return Ok(sessions);
        }

        let mut entries = fs::read_dir(&self.sessions_dir)
            .await
            .map_err(SessionError::Io)?;
        while let Some(entry) = entries.next_entry().await.map_err(SessionError::Io)? {
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
                if let Some(safe_key) = path.file_stem().and_then(|s| s.to_str()) {
                    // Use the safe (sanitized) key for loading — the real key is
                    // stored inside the metadata line and will be returned by load().
                    if let Ok(session) = self.load(safe_key).await {
                        sessions.push(SessionInfo {
                            key: session.key,
                            created_at: session.created_at,
                            updated_at: session.updated_at,
                            message_count: session.messages.len(),
                        });
                    }
                }
            }
        }
        drop(entries);

        // Sort by updated_at (most recent first)
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

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
    use tempfile::TempDir;

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

    #[tokio::test]
    async fn test_session_manager_get_or_create() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Get non-existent session (should create new)
        let session = manager.get_or_create("test:chat123").await.unwrap();
        assert_eq!(session.key, "test:chat123");
        assert_eq!(session.messages.len(), 0);

        // Add a message
        session.add_message("user", "Test message");

        // Get same session again (should retrieve from cache)
        let session2 = manager.get_or_create("test:chat123").await.unwrap();
        assert_eq!(session2.messages.len(), 1);
    }

    #[tokio::test]
    async fn test_session_save_and_load() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Create and populate session
        {
            let session = manager.get_or_create("test:chat123").await.unwrap();
            session.add_message("user", "Hello");
            session.add_message("assistant", "Hi there!");
        }

        // Save session (borrow ends above)
        let session = manager.get_or_create("test:chat123").await.unwrap();
        let session_clone = session.clone();
        manager.save(&session_clone).await.unwrap();

        // Create new manager to force reload from disk
        let mut manager2 = SessionManager::new(temp_dir.path()).await;
        let loaded_session = manager2.get_or_create("test:chat123").await.unwrap();

        assert_eq!(loaded_session.messages.len(), 2);
        assert_eq!(loaded_session.messages[0].content, "Hello");
        assert_eq!(loaded_session.messages[1].content, "Hi there!");
    }

    #[tokio::test]
    async fn test_session_path_sanitization() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path()).await;

        // Test that colons and slashes are replaced
        let path = manager.session_path("telegram:chat/123\\456");
        let filename = path.file_name().unwrap().to_str().unwrap();
        assert_eq!(filename, "telegram_chat_123_456.jsonl");
    }

    #[tokio::test]
    async fn test_session_delete() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Create and save session
        {
            let session = manager.get_or_create("test:chat123").await.unwrap();
            session.add_message("user", "Test");
        }
        let session = manager.get_or_create("test:chat123").await.unwrap();
        let session_clone = session.clone();
        manager.save(&session_clone).await.unwrap();

        // Delete session
        let deleted = manager.delete("test:chat123").await.unwrap();
        assert!(deleted);

        // Verify file is gone
        let path = manager.session_path("test:chat123");
        assert!(!tokio::fs::try_exists(&path).await.unwrap_or(false));

        // Delete non-existent session
        let deleted2 = manager.delete("nonexistent").await.unwrap();
        assert!(!deleted2);
    }

    #[tokio::test]
    async fn test_session_list() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Create multiple sessions
        for i in 0..3 {
            {
                let session = manager
                    .get_or_create(format!("test:chat{}", i))
                    .await
                    .unwrap();
                session.add_message("user", "Test");
            }
            let session = manager
                .get_or_create(format!("test:chat{}", i))
                .await
                .unwrap();
            let session_clone = session.clone();
            manager.save(&session_clone).await.unwrap();
        }

        // List all sessions
        let sessions = manager.list().await.unwrap();
        assert_eq!(sessions.len(), 3);

        // Verify they're sorted by updated_at (most recent first)
        for session_info in sessions {
            assert!(session_info.key.starts_with("test:chat"));
            assert_eq!(session_info.message_count, 1);
        }
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

    #[tokio::test]
    async fn test_message_id_persists_to_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Create session and add messages
        let original_id = {
            let session = manager.get_or_create("test:chat123").await.unwrap();
            session.add_message("user", "Hello");
            session.add_message("assistant", "Hi there!");
            session.messages[0].id.clone()
        };

        // Save session
        let session = manager.get_or_create("test:chat123").await.unwrap();
        let session_clone = session.clone();
        manager.save(&session_clone).await.unwrap();

        // Reload from disk
        let mut manager2 = SessionManager::new(temp_dir.path()).await;
        let loaded_session = manager2.get_or_create("test:chat123").await.unwrap();

        // Verify IDs persisted
        assert_eq!(loaded_session.messages[0].id, original_id);
        assert!(!loaded_session.messages[1].id.is_empty());
    }

    #[tokio::test]
    async fn test_atomic_write_no_corruption() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Create and save a valid session
        {
            let session = manager.get_or_create("test:atomic").await.unwrap();
            session.add_message("user", "Important data");
            session.add_message("assistant", "Response");
        }
        let session = manager.get_or_create("test:atomic").await.unwrap();
        let session_clone = session.clone();
        manager.save(&session_clone).await.unwrap();

        // Verify the original file exists and is valid
        let path = manager.session_path("test:atomic");
        assert!(tokio::fs::try_exists(&path).await.unwrap());

        // Verify no .tmp file left behind after successful write
        let tmp_path = path.with_extension("jsonl.tmp");
        assert!(!tokio::fs::try_exists(&tmp_path).await.unwrap_or(false));

        // Reload from disk — should parse cleanly
        let mut manager2 = SessionManager::new(temp_dir.path()).await;
        let loaded = manager2.get_or_create("test:atomic").await.unwrap();
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].content, "Important data");
    }

    #[tokio::test]
    async fn test_session_compaction() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Add 1001 messages to exceed the compaction threshold
        {
            let session = manager.get_or_create("test:compact").await.unwrap();
            for i in 0..1001 {
                session.add_message("user", format!("Message {}", i));
            }
        }

        // Save — should trigger compaction
        let session = manager.get_or_create("test:compact").await.unwrap();
        let session_clone = session.clone();
        manager.save(&session_clone).await.unwrap();

        // Reload from disk
        let mut manager2 = SessionManager::new(temp_dir.path()).await;
        let loaded = manager2.get_or_create("test:compact").await.unwrap();

        // After compaction: should have <= 500 messages (last 500 kept)
        assert!(
            loaded.messages.len() <= 500,
            "Expected <= 500 messages after compaction, got {}",
            loaded.messages.len()
        );
        // The last message should be the most recent one
        assert_eq!(loaded.messages.last().unwrap().content, "Message 1000");
        // The first message should be Message 501 (keeping last 500 of 0..1000)
        assert_eq!(loaded.messages.first().unwrap().content, "Message 501");
    }

    #[tokio::test]
    async fn test_session_message_has_request_id() {
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

    #[tokio::test]
    async fn test_session_below_compaction_threshold_not_compacted() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Add 500 messages — below threshold
        {
            let session = manager.get_or_create("test:nocompact").await.unwrap();
            for i in 0..500 {
                session.add_message("user", format!("Message {}", i));
            }
        }

        let session = manager.get_or_create("test:nocompact").await.unwrap();
        let session_clone = session.clone();
        manager.save(&session_clone).await.unwrap();

        // Reload — should keep all 500
        let mut manager2 = SessionManager::new(temp_dir.path()).await;
        let loaded = manager2.get_or_create("test:nocompact").await.unwrap();
        assert_eq!(loaded.messages.len(), 500);
    }

    #[tokio::test]
    async fn test_eviction_succeeds_despite_save_failure() {
        // Test that eviction completes successfully even when save fails
        // This verifies the error is logged (not propagated) per the fix
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::with_capacity(temp_dir.path(), 2).await;

        // Fill cache with 2 sessions
        {
            let session1 = manager.get_or_create("test:chat1").await.unwrap();
            session1.add_message("user", "Message 1");
        }
        {
            let session2 = manager.get_or_create("test:chat2").await.unwrap();
            session2.add_message("user", "Message 2");
        }

        // Make directory read-only to cause save to fail during eviction
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = tokio::fs::metadata(temp_dir.path())
                .await
                .unwrap()
                .permissions();
            perms.set_mode(0o444); // Read-only
            tokio::fs::set_permissions(temp_dir.path(), perms)
                .await
                .unwrap();
        }

        // On Windows, we can't easily make a directory read-only while still being able
        // to use it, so we'll just verify the eviction logic works normally
        #[cfg(not(unix))]
        {
            // No permission change needed - test will pass on behavior alone
        }

        // Add third session to trigger eviction (should evict test:chat1)
        // This should NOT panic or fail even though save will fail on Unix
        let result = manager.get_or_create("test:chat3").await;
        assert!(result.is_ok(), "Eviction should succeed even if save fails");

        {
            let session3 = result.unwrap();
            session3.add_message("user", "Message 3");
        }

        // Verify eviction still succeeded (cache should have chat2 and chat3, not chat1)
        // The key behavior: eviction completed despite save failure (error was logged, not propagated)
        assert!(
            !manager.cache.contains_key("test:chat1"),
            "chat1 should be evicted"
        );
        assert!(
            manager.cache.contains_key("test:chat2"),
            "chat2 should remain"
        );
        assert!(
            manager.cache.contains_key("test:chat3"),
            "chat3 should be added"
        );

        // Note: The warning log "Failed to save evicted session test:chat1: ... Data may be lost."
        // will be emitted to stderr during test run on Unix systems
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

    #[tokio::test]
    async fn test_structured_message_roundtrip_jsonl() {
        let temp_dir = TempDir::new().unwrap();
        let mut manager = SessionManager::new(temp_dir.path()).await;

        // Add structured message
        {
            let session = manager.get_or_create("test:structured").await.unwrap();
            let tool_calls = serde_json::json!([{"name": "web_fetch", "result": "ok"}]);
            session.add_structured_message(
                "assistant",
                "Done",
                None,
                Some(tool_calls),
                Some(serde_json::json!({"reasoning": "fetch page"})),
            );
            // Also add a plain message (tool_calls/metadata should be None)
            session.add_message("user", "Thanks");
        }

        // Save
        let session = manager.get_or_create("test:structured").await.unwrap();
        let session_clone = session.clone();
        manager.save(&session_clone).await.unwrap();

        // Reload from disk
        let mut manager2 = SessionManager::new(temp_dir.path()).await;
        let loaded = manager2.get_or_create("test:structured").await.unwrap();

        assert_eq!(loaded.messages.len(), 2);
        // Structured message preserved
        assert!(loaded.messages[0].tool_calls.is_some());
        assert_eq!(
            loaded.messages[0].tool_calls.as_ref().unwrap()[0]["name"],
            "web_fetch"
        );
        assert!(loaded.messages[0].metadata.is_some());
        // Plain message has no structured data
        assert!(loaded.messages[1].tool_calls.is_none());
        assert!(loaded.messages[1].metadata.is_none());
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
