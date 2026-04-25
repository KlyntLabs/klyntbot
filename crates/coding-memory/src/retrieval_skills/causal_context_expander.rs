//! `CausalContextExpander` — walk `memory_causal_edges` from current top-k.
//!
//! Phase 4 ships an inert version: edges aren't auto-populated until Phase 6.
//! Once seeded, this skill surfaces matching chains with no further changes.

use super::*;
use async_trait::async_trait;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Closure returning current top-k memory ids.
pub type TopKIdsFn = Arc<dyn Fn() -> Vec<Uuid> + Send + Sync>;

/// Closure: lookup causal edges for the given subject ids.
pub type EdgeLookupFn = Arc<
    dyn Fn(Vec<Uuid>) -> Pin<Box<dyn std::future::Future<Output = common::Result<Vec<crate::scope::CausalEdge>>> + Send>>
        + Send
        + Sync,
>;

/// Skill.
pub struct CausalContextExpander {
    top_k: TopKIdsFn,
    lookup: EdgeLookupFn,
}

impl std::fmt::Debug for CausalContextExpander {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CausalContextExpander").finish()
    }
}

impl CausalContextExpander {
    /// Construct.
    #[must_use]
    pub fn new(top_k: TopKIdsFn, lookup: EdgeLookupFn) -> Self {
        Self { top_k, lookup }
    }
}

#[async_trait]
impl RetrievalSkill for CausalContextExpander {
    fn name(&self) -> &'static str { "causal_context_expander" }
    fn description(&self) -> &'static str { "Walk memory_causal_edges from top-k." }
    fn tier(&self) -> BudgetTier { BudgetTier::Ultra }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let ids = (self.top_k)();
        let edges = (self.lookup)(ids).await?;
        if edges.is_empty() {
            return Ok(EscalationOutcome {
                succeeded: false,
                coverage_after: ctx.coverage_score,
                added_context: String::new(),
                added_ids: vec![],
            });
        }
        let mut buf = String::from("# Causal chains\n\n");
        let mut added: Vec<Uuid> = Vec::new();
        for e in &edges {
            buf.push_str(&format!(
                "- {:?}: {} → {}\n",
                e.edge_kind, e.from_id, e.to_id
            ));
            added.push(e.from_id);
            added.push(e.to_id);
        }
        added.sort();
        added.dedup();
        Ok(EscalationOutcome {
            succeeded: true,
            coverage_after: ctx.coverage_score + 0.15,
            added_context: buf,
            added_ids: added,
        })
    }
}
