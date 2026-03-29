//! MemoryEchoProvider implementation — wires Mirror (Tier 2) + recall (Tier 3).

use std::sync::Arc;

use async_trait::async_trait;
use voice_engine::MemoryEchoProvider;

/// App-level memory echo provider that tries Mirror snippets first,
/// then falls back to episodic memory recall.
pub struct AppMemoryEchoProvider {
    mirror: Option<Arc<cognitive::mirror::MirrorFacade>>,
}

impl AppMemoryEchoProvider {
    pub fn new(mirror: Option<Arc<cognitive::mirror::MirrorFacade>>) -> Self {
        Self { mirror }
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

        // Tier 3: Would use ContextEngine::recall_relevant here.
        // For now, Tier 2 is the primary echo source. Tier 3 recall
        // can be added by injecting ContextEngine and calling
        // memory_retriever.retrieve(partial_text, 1) when available.
        None
    }
}
