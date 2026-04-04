//! SkillListingSource — injects skill YAML frontmatter into system prompt.

use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use context_engine::source::{ContextSource, SourceContext};

use super::store::SkillStore;

/// Context source that formats the skill listing for the system prompt.
pub struct SkillListingSource {
    store: Arc<RwLock<SkillStore>>,
}

impl SkillListingSource {
    pub fn new(store: Arc<RwLock<SkillStore>>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl ContextSource for SkillListingSource {
    fn name(&self) -> &str {
        "skill_listing"
    }

    fn priority(&self) -> u8 {
        40 // After soul (50), before memory (30)
    }

    fn protected(&self) -> bool {
        true // Always present — skills are core context
    }

    async fn provide(&self, _ctx: &SourceContext) -> Option<String> {
        let store = self.store.read().await;
        let listing = store.format_listing();
        if listing.lines().count() <= 1 {
            None // No skills loaded
        } else {
            Some(listing)
        }
    }

    fn estimated_tokens(&self) -> usize {
        200 // ~5 skills × ~40 tokens each
    }
}
