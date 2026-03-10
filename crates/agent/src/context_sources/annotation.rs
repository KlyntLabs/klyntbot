//! Injects active annotations into the system prompt.

use async_trait::async_trait;
use cognitive::repos::AnnotationRepo;
use cognitive::types::PRIORITY_CRITICAL;
use context_engine::source::{ContextSource, SourceContext};

pub struct AnnotationContextSource {
    repo: AnnotationRepo,
}

impl AnnotationContextSource {
    pub fn new(repo: AnnotationRepo) -> Self {
        Self { repo }
    }
}

#[async_trait]
impl ContextSource for AnnotationContextSource {
    fn name(&self) -> &str {
        "annotations"
    }

    /// Priority between RetrievedMemory (70) and CompressedHistory (30).
    fn priority(&self) -> u8 {
        50
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let critical = self
            .repo
            .get_by_min_priority(PRIORITY_CRITICAL)
            .await
            .ok()?;

        if critical.is_empty() {
            return None;
        }

        let mut text = "[Active Annotations]\n".to_string();
        for ann in &critical {
            text.push_str(&format!(
                "- [{}] {}: {}\n",
                ann.target_type, ann.target_id, ann.content
            ));
        }

        // Batch-increment access counts (single query instead of N)
        let ids: Vec<&str> = critical.iter().map(|a| a.id.as_str()).collect();
        let _ = self.repo.increment_access_batch(&ids).await;

        Some(text)
    }
}
