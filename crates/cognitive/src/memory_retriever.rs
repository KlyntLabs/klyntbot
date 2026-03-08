use std::sync::Arc;

use async_trait::async_trait;
use context_engine::memory_retriever::{MemoryEntry, MemoryRetriever};

use crate::conversation_recall::ConversationRecallService;

/// Implements `MemoryRetriever` by delegating to `ConversationRecallService`.
///
/// Plugs into `ContextEngine::with_memory_retriever()` to inject conversation
/// recall into the message list during context assembly.
pub struct CognitiveMemoryRetriever {
    recall: Arc<ConversationRecallService>,
}

impl CognitiveMemoryRetriever {
    pub fn new(recall: Arc<ConversationRecallService>) -> Self {
        Self { recall }
    }
}

#[async_trait]
impl MemoryRetriever for CognitiveMemoryRetriever {
    async fn retrieve(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        match self
            .recall
            .search(query, limit, self.recall.config().default_threshold)
            .await
        {
            Ok(results) => results
                .into_iter()
                .map(|r| MemoryEntry {
                    id: r.id,
                    content: r.content,
                    score: r.score,
                })
                .collect(),
            Err(e) => {
                tracing::warn!("Conversation recall search failed: {e}");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retriever_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<CognitiveMemoryRetriever>();
    }
}
