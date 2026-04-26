use async_trait::async_trait;
use coding_memory::retrieval_skills::{
    BudgetTier, EscalationContext, EscalationOutcome, RetrievalSkill, RetrievalSkillRegistry,
};
use std::sync::Arc;

struct TestSkill {
    name: &'static str,
    tier: BudgetTier,
    succeeds: bool,
    after: f32,
}

#[async_trait]
impl RetrievalSkill for TestSkill {
    fn name(&self) -> &'static str {
        self.name
    }
    fn description(&self) -> &'static str {
        "test"
    }
    fn tier(&self) -> BudgetTier {
        self.tier
    }
    async fn apply(&self, _: &EscalationContext) -> common::Result<EscalationOutcome> {
        Ok(EscalationOutcome {
            succeeded: self.succeeds,
            coverage_after: self.after,
            added_context: String::new(),
            added_ids: vec![],
        })
    }
}

#[tokio::test]
async fn selector_stops_on_first_success() {
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let reg = RetrievalSkillRegistry::new(
        vec![
            Arc::new(TestSkill {
                name: "fail_a",
                tier: BudgetTier::Fast,
                succeeds: false,
                after: 0.1,
            }),
            Arc::new(TestSkill {
                name: "succ_b",
                tier: BudgetTier::Fast,
                succeeds: true,
                after: 0.9,
            }),
            Arc::new(TestSkill {
                name: "skip_c",
                tier: BudgetTier::Fast,
                succeeds: true,
                after: 0.99,
            }),
        ],
        bus,
    );
    let ctx = EscalationContext {
        query: "x".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::Fast,
        repo: None,
    };
    let out = reg.escalate(&ctx).await.unwrap();
    assert!(out.skills_tried.contains(&"succ_b".to_string()));
    assert!(out.final_outcome.succeeded);
}

#[tokio::test]
async fn selector_filters_by_tier() {
    let bus = Arc::new(bus::DomainEventBus::new(64));
    let reg = RetrievalSkillRegistry::new(
        vec![Arc::new(TestSkill {
            name: "ultra_only",
            tier: BudgetTier::Ultra,
            succeeds: true,
            after: 0.99,
        })],
        bus,
    );
    let ctx = EscalationContext {
        query: "x".into(),
        coverage_score: 0.1,
        budget_tier: BudgetTier::Fast,
        repo: None,
    };
    let out = reg.escalate(&ctx).await.unwrap();
    assert!(out.skills_tried.is_empty());
}
