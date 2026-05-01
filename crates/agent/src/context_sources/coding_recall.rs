//! Coding-memory recall context source — injects recall snippets into the
//! system prompt when the channel is coding.

use async_trait::async_trait;
use coding_memory::recall::CodingRecallService;
use context_engine::source::{ContextSource, SourceContext};
use std::sync::Arc;

pub struct CodingRecallContextSource {
    service: Arc<CodingRecallService>,
}

impl CodingRecallContextSource {
    pub fn new(service: Arc<CodingRecallService>) -> Self {
        Self { service }
    }
}

#[async_trait]
impl ContextSource for CodingRecallContextSource {
    fn name(&self) -> &str {
        "coding_recall"
    }

    fn priority(&self) -> u8 {
        50
    }

    async fn provide(&self, ctx: &SourceContext) -> Option<String> {
        if ctx.channel != common::CODING_CHANNEL {
            return None;
        }
        let query = ctx.message.as_deref().unwrap_or("");
        if query.trim().is_empty() {
            return None;
        }
        let repo_id = ctx.project_id.as_deref();
        let resp = self
            .service
            .recall_index(query, repo_id, None, None, 6)
            .await
            .ok()?;
        if resp.results.is_empty() {
            return None;
        }
        let snippets: Vec<String> = resp
            .results
            .iter()
            .take(6)
            .map(|e| format!("- {}: {}", e.kind, e.title))
            .collect();
        let body = format!(
            "## Coding-memory recall (top {} snippets)\n{}",
            snippets.len(),
            snippets.join("\n")
        );
        Some(body)
    }

    fn estimated_tokens(&self) -> usize {
        800
    }
}
