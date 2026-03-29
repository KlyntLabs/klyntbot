//! MemoryEchoProvider implementation — wires Mirror (Tier 2) + recall (Tier 3).

use std::sync::Arc;

use async_trait::async_trait;
use context_engine::MemoryRetriever;
use voice_engine::MemoryEchoProvider;

/// Minimum relevance score for a Tier 3 memory recall result to surface as an echo.
const MIN_RECALL_SCORE: f64 = 0.4;

/// App-level memory echo provider that tries Mirror snippets first,
/// then falls back to conversation recall via `MemoryRetriever`.
pub struct AppMemoryEchoProvider {
    mirror: Option<Arc<cognitive::mirror::MirrorFacade>>,
    memory_retriever: Option<Arc<dyn MemoryRetriever>>,
}

impl AppMemoryEchoProvider {
    pub fn new(
        mirror: Option<Arc<cognitive::mirror::MirrorFacade>>,
        memory_retriever: Option<Arc<dyn MemoryRetriever>>,
    ) -> Self {
        Self {
            mirror,
            memory_retriever,
        }
    }
}

#[async_trait]
impl MemoryEchoProvider for AppMemoryEchoProvider {
    async fn lookup(&self, partial_text: &str, _learning_active: bool) -> Option<String> {
        // Tier 2: Mirror-powered snippet (embedding similarity)
        if let Some(ref facade) = self.mirror {
            if let Some(snippet) = facade.get_recent_voice_relevant_snippet(partial_text).await {
                return Some(snippet);
            }
        }

        // Tier 3: Conversation recall via MemoryRetriever (cognitive facts + past messages)
        if let Some(ref retriever) = self.memory_retriever {
            let entries = retriever.retrieve(partial_text, 1).await;
            if let Some(entry) = entries.first() {
                if entry.score >= MIN_RECALL_SCORE {
                    return Some(entry.content.clone());
                }
            }
        }

        None
    }
}
