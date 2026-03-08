//! Conversation recall handler trait and types for semantic memory search.
//!
//! The `ConversationRecallHandler` trait provides dependency inversion for
//! the recall pipeline (defined in tools crate L4, implemented in agent crate L5
//! delegating to `cognitive::ConversationRecallService`).

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single conversation recall search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallSearchResult {
    pub id: String,
    pub session_key: String,
    pub role: String,
    pub content_preview: String,
    pub content_full: String,
    pub score: f64,
    pub created_at: DateTime<Utc>,
}

/// Filter for purging conversation embeddings.
#[derive(Debug, Clone)]
pub enum PurgeFilter {
    /// Delete all embeddings for a specific session.
    BySessionKey(String),
    /// Delete all embeddings embedded before a specific date.
    Before(DateTime<Utc>),
    /// Delete all embeddings.
    All,
}

/// Status of the conversation recall system.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationRecallStatus {
    pub total_embeddings: usize,
    pub is_available: bool,
}

/// Interface for conversation recall operations.
///
/// Defined in `tools` (L4) for use by `MemoryTool`.
/// Implemented in `agent` (L5) delegating to `cognitive::ConversationRecallService`.
#[async_trait]
pub trait ConversationRecallHandler: Send + Sync {
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> common::Result<()>;

    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> common::Result<Vec<RecallSearchResult>>;

    async fn purge(&self, filter: PurgeFilter) -> common::Result<usize>;

    async fn status(&self) -> common::Result<ConversationRecallStatus>;

    fn is_available(&self) -> bool;
}
