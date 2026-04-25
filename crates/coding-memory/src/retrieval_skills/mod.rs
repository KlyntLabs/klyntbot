//! C3 retrieval-skill registry — invoked when `RetrievalQualityProbe`
//! returns `Escalate`. Closed set of 5 skills wired in Phase 4.

use async_trait::async_trait;

pub mod registry;
pub mod query_rewriter;
pub mod query_decomposer;
pub mod evidence_focuser;
pub mod raw_event_escalator;
pub mod causal_context_expander;

pub use registry::{RetrievalSkillRegistry, SelectorOutcome};
pub use query_rewriter::QueryRewriter;
pub use query_decomposer::QueryDecomposer;
pub use evidence_focuser::EvidenceFocuser;
pub use raw_event_escalator::RawEventEscalator;
pub use causal_context_expander::CausalContextExpander;

/// Budget tier at which a retrieval skill can operate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetTier {
    /// Fast (default) — bounded to the original retrieval budget.
    Fast,
    /// `deep_think` — larger budget for query rewriting/decomposing.
    DeepThink,
    /// `ultra` — full escalation, bypasses summaries.
    Ultra,
}

/// Context passed to a retrieval skill's `apply`.
#[derive(Debug, Clone)]
pub struct EscalationContext {
    /// Original query.
    pub query: String,
    /// Coverage score at invocation time.
    pub coverage_score: f32,
    /// Active tier.
    pub budget_tier: BudgetTier,
    /// Optional repo scope.
    pub repo: Option<String>,
}

/// Outcome of a retrieval skill application.
#[derive(Debug, Clone)]
pub struct EscalationOutcome {
    /// Was coverage raised above threshold?
    pub succeeded: bool,
    /// New coverage score after applying.
    pub coverage_after: f32,
    /// Additional context produced (rendered).
    pub added_context: String,
    /// New ids surfaced (deduped against the original retrieval).
    pub added_ids: Vec<uuid::Uuid>,
}

/// Retrieval skill — the unit of C3 escalation.
#[async_trait]
pub trait RetrievalSkill: Send + Sync {
    /// Skill name used in telemetry + effectiveness EMA.
    fn name(&self) -> &'static str;

    /// Short description for UI surfaces.
    fn description(&self) -> &'static str;

    /// Tier this skill belongs to.
    fn tier(&self) -> BudgetTier;

    /// Apply the skill against an escalation context.
    async fn apply(
        &self,
        ctx: &EscalationContext,
    ) -> common::Result<EscalationOutcome>;
}
