//! ConversationRecallHandlerImpl — production handler for conversation recall.
//!
//! Implements the `ConversationRecallHandler` trait defined in tools crate (L4)
//! by delegating to `ConversationRecallService` from cognitive crate (L5).

use std::sync::Arc;

use async_trait::async_trait;
use cognitive::conversation_recall::{ConversationRecallService, RecallMetadata};
use common::truncate_at_boundary;
use tools::conversation_recall::{
    ConversationRecallHandler, ConversationRecallStatus, PurgeFilter, RecallSearchResult,
};

/// Implements `ConversationRecallHandler` (from tools L4) by delegating
/// to `ConversationRecallService` (from cognitive L5).
pub struct ConversationRecallHandlerImpl {
    service: Arc<ConversationRecallService>,
}

impl ConversationRecallHandlerImpl {
    pub fn new(service: Arc<ConversationRecallService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ConversationRecallHandler for ConversationRecallHandlerImpl {
    async fn embed_message(
        &self,
        session_key: &str,
        role: &str,
        content: &str,
        message_id: &str,
    ) -> common::Result<()> {
        let metadata = RecallMetadata {
            session_key: session_key.to_string(),
            role: role.to_string(),
        };
        self.service
            .store_message(message_id, content, metadata)
            .await
    }

    async fn search(
        &self,
        query: &str,
        limit: usize,
        threshold: f64,
    ) -> common::Result<Vec<RecallSearchResult>> {
        let results = self.service.search(query, limit, threshold as f32).await?;

        Ok(results
            .into_iter()
            .map(|r| RecallSearchResult {
                id: r.id,
                session_key: r.session_key,
                role: r.role,
                content_preview: truncate_at_boundary(&r.content, 100).to_string(),
                content_full: r.content,
                score: r.score,
                created_at: r.created_at,
            })
            .collect())
    }

    async fn purge(&self, filter: PurgeFilter) -> common::Result<usize> {
        let count_before = self.service.count().await.unwrap_or(0);

        match filter {
            PurgeFilter::Before(cutoff) => {
                self.service.delete_older_than(cutoff).await?;
            }
            PurgeFilter::All => {
                self.service.delete_all().await?;
            }
            PurgeFilter::BySessionKey(key) => {
                self.service.delete_by_session_key(&key).await?;
            }
        }

        let count_after = self.service.count().await.unwrap_or(0);
        Ok(count_before.saturating_sub(count_after))
    }

    async fn status(&self) -> common::Result<ConversationRecallStatus> {
        let count = self.service.count().await.unwrap_or(0);
        Ok(ConversationRecallStatus {
            total_embeddings: count,
            is_available: true,
        })
    }

    fn is_available(&self) -> bool {
        true
    }
}
