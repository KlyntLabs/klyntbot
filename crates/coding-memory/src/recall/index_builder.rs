//! Build `IndexEntry` rows from cognitive `ScoredFact` / `EpisodicMemory`.

use crate::recall::budget::TokenBudgeter;
use crate::recall::IndexEntry;
use cognitive::retrieval::ScoredFact;
use cognitive::EpisodicMemory;
use std::sync::Arc;

/// `IndexEntry` builder.
#[derive(Clone)]
pub struct IndexBuilder {
    budgeter: Arc<dyn TokenBudgeter>,
}

impl std::fmt::Debug for IndexBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IndexBuilder").finish()
    }
}

impl IndexBuilder {
    /// Construct with the default budgeter.
    #[must_use]
    pub fn new() -> Self {
        Self {
            budgeter: crate::recall::budget::default_budgeter(),
        }
    }

    /// Construct with a specific budgeter (test seam).
    #[must_use]
    pub fn with_budgeter(budgeter: Arc<dyn TokenBudgeter>) -> Self {
        Self { budgeter }
    }

    /// Convert a scored fact.
    #[must_use]
    pub fn from_scored_fact(&self, sf: &ScoredFact) -> IndexEntry {
        let f = &sf.fact;
        let kind = f.memory_type.clone();
        let title = format!("{} {} {}", f.subject, f.predicate, f.object);
        let scope = f
            .scope_repo_id
            .as_ref()
            .map(|r| format!("repo:{r}"))
            .unwrap_or_else(|| "global".to_string());
        let est = format!("{title}\n{}", f.metadata.as_deref().unwrap_or(""));
        let token_cost = self.budgeter.count(&est) as u32;
        let when = f
            .recorded_at
            .parse()
            .unwrap_or_else(|_| jiff::Timestamp::now());
        IndexEntry {
            id: f.id.parse().unwrap_or_else(|_| uuid::Uuid::nil()),
            kind,
            title: common::helpers::truncate_chars(&title, 120, "…"),
            when,
            scope,
            confidence: f.confidence as f32,
            token_cost,
        }
    }

    /// Convert an episodic memory.
    #[must_use]
    pub fn from_episode(&self, ep: &EpisodicMemory) -> IndexEntry {
        let scope = ep
            .scope_repo_id
            .as_ref()
            .map(|r| format!("repo:{r}"))
            .unwrap_or_else(|| "global".to_string());
        let est = ep.content.clone();
        let token_cost = self.budgeter.count(&est) as u32;
        let when = ep
            .occurred_at
            .parse()
            .unwrap_or_else(|_| jiff::Timestamp::now());
        IndexEntry {
            id: ep.id.parse().unwrap_or_else(|_| uuid::Uuid::nil()),
            kind: ep.kind.clone().unwrap_or_else(|| "episode".to_string()),
            title: common::helpers::truncate_chars(
                &ep.summary.clone().unwrap_or_default(),
                120,
                "…",
            ),
            when,
            scope,
            confidence: ep.importance as f32,
            token_cost,
        }
    }
}

impl Default for IndexBuilder {
    fn default() -> Self {
        Self::new()
    }
}
