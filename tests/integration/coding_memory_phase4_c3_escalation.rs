use async_trait::async_trait;
use coding_memory::retrieval_skills::{
    BudgetTier, EscalationContext, EscalationOutcome, RetrievalSkill, RetrievalSkillRegistry,
};
use std::sync::Arc;

struct LiftSkill;

#[async_trait]
impl RetrievalSkill for LiftSkill {
    fn name(&self) -> &'static str {
        "lift"
    }
    fn description(&self) -> &'static str {
        "raises coverage"
    }
    fn tier(&self) -> BudgetTier {
        BudgetTier::Fast
    }
    async fn apply(&self, _: &EscalationContext) -> common::Result<EscalationOutcome> {
        Ok(EscalationOutcome {
            succeeded: true,
            coverage_after: 0.7,
            added_context: String::new(),
            added_ids: vec![],
        })
    }
}

#[tokio::test]
async fn escalation_lifts_coverage_and_bumps_ema() {
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let reg = RetrievalSkillRegistry::new(vec![Arc::new(LiftSkill)], bus);
    let before = reg.effectiveness_of("lift").await;
    assert!(
        (before - 0.5).abs() < f32::EPSILON,
        "initial EMA should be 0.5, got {before}"
    );

    let ctx = EscalationContext {
        query: "test".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::Fast,
        repo: None,
    };
    let out = reg.escalate(&ctx).await.unwrap();
    assert!(out.final_outcome.succeeded);
    assert!(out.final_outcome.coverage_after > 0.5);

    let after = reg.effectiveness_of("lift").await;
    assert!(
        after > before,
        "EMA should bump after success, got before={before} after={after}"
    );
}
