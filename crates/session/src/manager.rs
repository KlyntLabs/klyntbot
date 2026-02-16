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
        self.messages.push(SessionMessage {
            id: generate_message_id(),
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
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
}

/// Session manager with JSONL persistence
pub struct SessionManager {
    sessions_dir: PathBuf,
    cache: HashMap<String, Session>,
    lru_order: VecDeque<String>,
    max_cache_size: usize,
}

impl SessionManager {
    /// Create a new session manager
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
                    let _ = self.save(&session).await;
                    debug!("Evicted session from cache: {}", old_key);
                }
            }
        }

        // On cache miss: load or create, then move key into cache (no extra clone)
        if !self.cache.contains_key(&key) {
            let session = match self.load(&key).await {
                Ok(s) => s,
                Err(_) => {
                    debug!("Creating new session: {}", key);
                    Session::new(key.clone())
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

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let value: serde_json::Value = serde_json::from_str(line)
                .map_err(|e| SessionError::LoadFailed(format!("Failed to parse line: {}", e)))?;

            if let Some(msg_type) = value.get("_type").and_then(|v| v.as_str()) {
                if msg_type == "metadata" {
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
            key: key.to_string(),
            messages,
            created_at: created_at.unwrap_or_else(Utc::now),
            updated_at: updated_at.unwrap_or_else(Utc::now),
            metadata,
        })
    }

    /// Save a session to disk
    pub async fn save(&self, session: &Session) -> Result<()> {
        let path = self.session_path(&session.key);

        debug!("Saving session to: {}", path.display());

        let mut content = String::new();

        // Write metadata line
        let metadata_line = serde_json::json!({
            "_type": "metadata",
            "created_at": session.created_at.to_rfc3339(),
            "updated_at": session.updated_at.to_rfc3339(),
            "metadata": session.metadata,
        });
        content.push_str(&serde_json::to_string(&metadata_line).map_err(SessionError::Json)?);
        content.push('\n');

        // Write message lines
        for msg in &session.messages {
            content.push_str(&serde_json::to_string(msg).map_err(SessionError::Json)?);
            content.push('\n');
        }

        fs::write(&path, content).await.map_err(|e| {
            SessionError::SaveFailed(format!("Failed to write {}: {}", path.display(), e))
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
                if let Ok(key) = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| s.replace('_', ":"))
                    .ok_or_else(|| SessionError::LoadFailed("Invalid filename".to_string()))
                {
                    // Try to read metadata
                    if let Ok(session) = self.load(&key).await {
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
}
