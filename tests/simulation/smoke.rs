use std::sync::Arc;

use async_trait::async_trait;
use cognitive::services::consolidation::ConsolidationCandidate;
use cognitive::services::extraction::BatchExtractionResult;
use cognitive::types::{MemoryOp, Observation};
use cognitive::{ConsolidationHandler, ExtractionHandler};
use simulator::harness::SimulationHarness;
use simulator::scenario::Scenario;

// ── Mock handlers ────────────────────────────────────────────────────

/// Mock extraction handler that returns empty results (no facts extracted).
/// This keeps the smoke test fast and free of LLM dependencies.
struct MockExtractionHandler;

#[async_trait]
impl ExtractionHandler for MockExtractionHandler {
    async fn extract_facts_batch(
        &self,
        _observations: &[Observation],
    ) -> common::Result<BatchExtractionResult> {
        Ok(BatchExtractionResult {
            extractions: vec![],
            fallback_indices: vec![],
        })
    }
}

/// Mock consolidation handler that returns Noop for every candidate.
struct MockConsolidationHandler;

#[async_trait]
impl ConsolidationHandler for MockConsolidationHandler {
    async fn decide_batch(
        &self,
        candidates: &[ConsolidationCandidate],
    ) -> common::Result<Vec<MemoryOp>> {
        Ok(candidates.iter().map(|_| MemoryOp::Noop).collect())
    }
}

// ── Smoke test scenario (7 days) ────────────────────────────────────

const SMOKE_SCENARIO_TOML: &str = r#"
[persona]
name = "smoke_test_user"
timezone = "UTC"
language = "en"
seed = 42

[persona.messages_per_day]
onboarding = 3
routine = 3
power_user = 3
shift = 3

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "engineer" },
]

[persona.phases.onboarding]
duration_days = 3
correction_rate = 0.2
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.5
tool_action_rate = 0.3

[persona.phases.routine]
duration_days = 2
correction_rate = 0.1
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.1
tool_action_rate = 0.5

[persona.phases.power_user]
duration_days = 1
correction_rate = 0.05
topic_weights = { tasks = 0.5, chat = 0.5 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.7

[persona.phases.behavior_shift]
duration_days = 1
correction_rate = 0.15
shift_description = "switches focus"
new_facts = [{ subject = "user", predicate = "learning", object = "Python" }]
topic_weights = { tasks = 0.3, notes = 0.4, chat = 0.3 }
new_fact_introduction_rate = 0.4
tool_action_rate = 0.5

[[checkpoints]]
at_day = 7
assertions = [
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.0 },
]
"#;

#[tokio::test]
async fn smoke_test_7_day_simulation() {
    let scenario = Scenario::from_toml(SMOKE_SCENARIO_TOML).unwrap();
    assert_eq!(scenario.total_days(), 7);

    let extraction: Arc<dyn ExtractionHandler> = Arc::new(MockExtractionHandler);
    let consolidation: Arc<dyn ConsolidationHandler> = Arc::new(MockConsolidationHandler);

    let harness = SimulationHarness::new(scenario, extraction, consolidation)
        .await
        .unwrap();

    let report = harness.run().await.unwrap();

    assert!(
        report.summary.total_messages > 0,
        "expected at least 1 message, got {}",
        report.summary.total_messages
    );
    assert!(
        !report.metric_timeline.is_empty(),
        "metric timeline should not be empty"
    );
    assert!(
        report.wall_time_secs < 60.0,
        "smoke test should finish in under 60s, took {:.2}s",
        report.wall_time_secs
    );
}

#[tokio::test]
async fn scenario_12mo_parses() {
    let toml_content =
        include_str!("scenarios/software_engineer_12mo.toml");
    let scenario = Scenario::from_toml(toml_content).unwrap();

    assert_eq!(scenario.persona.name, "software_engineer_vn");
    assert_eq!(scenario.total_days(), 269);
    assert_eq!(scenario.checkpoints.len(), 4);
    assert_eq!(scenario.persona.profile.known_facts.len(), 6);
}
