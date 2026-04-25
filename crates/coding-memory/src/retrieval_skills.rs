//! C3 retrieval-skill registry — invoked by the failure-state-aware
//! retrieval probe when coverage_score falls below threshold.
//!
//! Closed set of 5 skills at Phase 4. Effectiveness tracked by EMA and
//! published as `DomainEvent::RetrievalSkillApplied`.

use crate::error::NotImplementedInPhase;
use async_trait::async_trait;
use common::{KlyntbotError, Result};

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
}

/// Outcome of a retrieval skill application.
#[derive(Debug, Clone)]
pub struct EscalationOutcome {
    /// Was coverage raised above threshold?
    pub succeeded: bool,
    /// New coverage score after applying.
    pub coverage_after: f32,
    /// Additional context produced (stringified).
    pub added_context: String,
}

/// Retrieval skill — the unit of C3 escalation.
#[async_trait]
pub trait RetrievalSkill: Send + Sync {
    /// Skill name used in telemetry + effectiveness EMA.
    fn name(&self) -> &'static str;

    /// Short description for UI surfaces.
    fn description(&self) -> &'static str;

    /// Apply the skill against an escalation context. Phase 4.
    async fn apply(&self, ctx: &EscalationContext) -> Result<EscalationOutcome>;

    /// Current EMA-updated effectiveness (0.0 – 1.0).
    fn effectiveness_score(&self) -> f32;
}

macro_rules! phase_stub_skill {
    ($struct_name:ident, $n:expr, $d:expr) => {
        /// Phase 4 stub.
        #[derive(Debug, Default)]
        pub struct $struct_name;

        #[async_trait]
        impl RetrievalSkill for $struct_name {
            fn name(&self) -> &'static str {
                $n
            }
            fn description(&self) -> &'static str {
                $d
            }
            async fn apply(&self, _ctx: &EscalationContext) -> Result<EscalationOutcome> {
                Err(phase(4))
            }
            fn effectiveness_score(&self) -> f32 {
                0.5
            }
        }
    };
}

phase_stub_skill!(
    QueryRewriter,
    "query_rewriter",
    "PRF + multi-query expansion; 3 rewrites, RRF-merge."
);
phase_stub_skill!(
    QueryDecomposer,
    "query_decomposer",
    "Split compound queries into 2-4 sub-queries."
);
phase_stub_skill!(
    EvidenceFocuser,
    "evidence_focuser",
    "Cross-encoder rerank on top-20 to identify top 5."
);
phase_stub_skill!(
    RawEventEscalator,
    "raw_event_escalator",
    "Bypass summaries; use provenance pointers to raw events."
);
phase_stub_skill!(
    CausalContextExpander,
    "causal_context_expander",
    "Walk memory_causal_edges from top-k; surface chains."
);

fn phase(p: u8) -> KlyntbotError {
    KlyntbotError::NotImplemented(format!("{:?}", NotImplementedInPhase::new(p)))
}
