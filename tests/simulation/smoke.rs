use std::sync::Arc;

use cognitive::{ConsolidationHandler, ExtractionHandler, ReflectionHandler};
use klyntbot::agent::cognitive_handlers::{
    HeuristicConsolidationHandler, HeuristicExtractionHandler, HeuristicReflectionHandler,
};
use simulator::harness::SimulationHarness;
use simulator::providers::HeuristicNarrativeHandler;
use simulator::report::SimulationReport;
use simulator::scenario::Scenario;

// ── Shared helpers ─────────────────────────────────────────────────────

fn print_checkpoints(report: &SimulationReport) {
    for cp in &report.checkpoints {
        let status = if cp.all_passed { "PASS" } else { "FAIL" };
        eprintln!("  Checkpoint day {}: {}", cp.at_day, status);
        for a in &cp.assertions {
            let mark = if a.passed { "  [x]" } else { "  [ ]" };
            eprintln!(
                "    {} {} (actual: {:?}, expected: {})",
                mark, a.description, a.actual_value, a.expected
            );
        }
    }
    for r in &report.summary.regression_alerts {
        eprintln!(
            "  REGRESSION: {} baseline={:.3} current={:.3} change={:.1}%",
            r.metric, r.baseline, r.current, r.regression_pct
        );
    }
}

async fn run_scenario(toml: &str) -> SimulationReport {
    let scenario = Scenario::from_toml(toml).unwrap();
    let extraction: Arc<dyn ExtractionHandler> = Arc::new(HeuristicExtractionHandler);
    let consolidation: Arc<dyn ConsolidationHandler> = Arc::new(HeuristicConsolidationHandler);
    let reflection: Arc<dyn ReflectionHandler> = Arc::new(HeuristicReflectionHandler);
    let narrative: Arc<dyn cognitive::mirror::NarrativeHandler> =
        Arc::new(HeuristicNarrativeHandler);
    let harness =
        SimulationHarness::new(scenario, extraction, consolidation, reflection, narrative)
            .await
            .unwrap();
    harness.run().await.unwrap()
}

// ── Smoke test scenario (7 days) ────────────────────────────────────────

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
    let report = run_scenario(SMOKE_SCENARIO_TOML).await;

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

    let last = &report.summary.final_metrics;

    // Token efficiency should vary (not fixed 150)
    assert!(
        (last.token_efficiency - 150.0).abs() > 0.01,
        "token_efficiency should vary from fixed 150, got {:.1}",
        last.token_efficiency
    );

    // Knowledge retention should be non-zero (structured extraction + guaranteed introduction)
    assert!(
        last.knowledge_retention > 0.0,
        "knowledge_retention should be > 0 after 7 days, got {:.3}",
        last.knowledge_retention
    );

    // Personalization score should be meaningful
    assert!(
        last.personalization_score > 0.2,
        "personalization_score should be > 0.2, got {:.3}",
        last.personalization_score
    );

    // Routing stability should be in a sane range
    assert!(
        last.routing_stability > 0.0 && last.routing_stability <= 1.0,
        "routing_stability should be in (0, 1], got {:.3}",
        last.routing_stability
    );

    // Routing accuracy (real SkillRouter) should be in valid range
    assert!(
        last.routing_accuracy >= 0.0 && last.routing_accuracy <= 1.0,
        "routing_accuracy should be in [0, 1], got {:.3}",
        last.routing_accuracy
    );

    // Response quality should be in valid range
    assert!(
        last.response_quality >= 0.0 && last.response_quality <= 1.0,
        "response_quality should be in [0, 1], got {:.3}",
        last.response_quality
    );

    // Salience extract rate should be populated
    assert!(
        last.salience_extract_rate >= 0.0 && last.salience_extract_rate <= 1.0,
        "salience_extract_rate should be in [0, 1], got {:.3}",
        last.salience_extract_rate
    );

    // Verify the report can be serialized without NaN/Inf from division
    let json = serde_json::to_string(&report).unwrap();
    assert!(!json.contains("NaN"), "report contains NaN values");
    assert!(
        !json.contains("Infinity"),
        "report contains Infinity values"
    );
}

#[tokio::test]
async fn scenario_12mo_parses() {
    let toml_content = include_str!("scenarios/software_engineer_12mo.toml");
    let scenario = Scenario::from_toml(toml_content).unwrap();

    assert_eq!(scenario.persona.name, "software_engineer_vn");
    assert_eq!(scenario.total_days(), 269);
    assert_eq!(scenario.checkpoints.len(), 4);
    assert_eq!(scenario.persona.profile.known_facts.len(), 6);
}

#[tokio::test]
async fn run_software_engineer_12mo() {
    let report = run_scenario(include_str!("scenarios/software_engineer_12mo.toml")).await;

    // Write JSON report
    let output_dir = std::env::var("SIMULATION_OUTPUT_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("target/simulation"));
    let report_path = report.write_json(&output_dir).unwrap();

    // Print summary
    eprintln!("\n============================================================");
    eprintln!("  SIMULATION REPORT: {}", report.persona);
    eprintln!("============================================================");
    eprintln!("  Simulated days:     {}", report.simulated_days);
    eprintln!("  Wall time:          {:.2}s", report.wall_time_secs);
    eprintln!("  Total messages:     {}", report.summary.total_messages);
    eprintln!(
        "  Facts extracted:    {}",
        report.summary.total_facts_extracted
    );
    eprintln!(
        "  Brain versions:     {}",
        report.summary.total_brain_versions
    );
    eprintln!(
        "  Checkpoints:        {}/{} passed",
        report.checkpoints.iter().filter(|c| c.all_passed).count(),
        report.checkpoints.len()
    );
    eprintln!(
        "  Pass rate:          {:.0}%",
        report.summary.checkpoint_pass_rate * 100.0
    );
    eprintln!(
        "  Regressions:        {}",
        report.summary.regression_alerts.len()
    );
    eprintln!();

    // Print metric evolution (first, middle, last)
    let timeline = &report.metric_timeline;
    if let (Some(first), Some(last)) = (timeline.first(), timeline.last()) {
        eprintln!(
            "  Metric Evolution (day 1 → day {}):",
            report.simulated_days
        );
        eprintln!("  ─────────────────────────────────────────────");
        eprintln!(
            "  Knowledge retention:   {:.3} → {:.3}",
            first.knowledge_retention, last.knowledge_retention
        );
        eprintln!(
            "  Retrieval precision:   {:.3} → {:.3}",
            first.retrieval_precision, last.retrieval_precision
        );
        eprintln!(
            "  Correction rate:       {:.3} → {:.3}",
            first.correction_rate, last.correction_rate
        );
        eprintln!(
            "  Personalization score: {:.3} → {:.3}",
            first.personalization_score, last.personalization_score
        );
        eprintln!(
            "  Token efficiency:      {:.0} → {:.0}",
            first.token_efficiency, last.token_efficiency
        );
        eprintln!(
            "  Routing accuracy:      {:.3} → {:.3}",
            first.routing_accuracy, last.routing_accuracy
        );
        eprintln!(
            "  Response quality:      {:.3} → {:.3}",
            first.response_quality, last.response_quality
        );
        eprintln!(
            "  Salience extract rate: {:.3} → {:.3}",
            first.salience_extract_rate, last.salience_extract_rate
        );
        eprintln!(
            "  Community stability:   {:.3} → {:.3}",
            first.community_stability, last.community_stability
        );
        eprintln!(
            "  Brain version velocity:{} → {}",
            first.brain_version_velocity, last.brain_version_velocity
        );
    }

    // Print checkpoint details
    eprintln!();
    for cp in &report.checkpoints {
        let status = if cp.all_passed { "PASS" } else { "FAIL" };
        eprintln!("  Checkpoint day {}: {}", cp.at_day, status);
        for a in &cp.assertions {
            let mark = if a.passed { "  [x]" } else { "  [ ]" };
            eprintln!(
                "    {} {} (actual: {:?}, expected: {})",
                mark, a.description, a.actual_value, a.expected
            );
        }
    }

    eprintln!();
    eprintln!("  Report written to: {}", report_path.display());
    eprintln!(
        "  Verdict: {}",
        if report.passed() { "PASSED" } else { "FAILED" }
    );

    assert!(report.summary.total_messages > 0);
    assert!(report.passed(), "Simulation failed — see report above");
}

#[tokio::test]
async fn run_finance_focused_6mo() {
    let report = run_scenario(include_str!("scenarios/finance_focused_6mo.toml")).await;
    eprintln!(
        "Finance 6mo: {} msgs, {:.2}s, retention={:.3}, personalization={:.3}",
        report.summary.total_messages,
        report.wall_time_secs,
        report.summary.final_metrics.knowledge_retention,
        report.summary.final_metrics.personalization_score
    );
    eprintln!(
        "  checkpoint_pass_rate={:.2}, regressions={}",
        report.summary.checkpoint_pass_rate,
        report.summary.regression_alerts.len()
    );
    print_checkpoints(&report);
    assert!(
        report.summary.checkpoint_pass_rate >= 1.0,
        "Finance scenario checkpoints failed (pass_rate={:.2})",
        report.summary.checkpoint_pass_rate
    );
}

#[tokio::test]
async fn run_onboarding_stress_test() {
    let report = run_scenario(include_str!("scenarios/onboarding_stress_test.toml")).await;
    let first_cr = report
        .metric_timeline
        .first()
        .map(|m| m.correction_rate)
        .unwrap_or(0.0);
    let last_cr = report.summary.final_metrics.correction_rate;
    eprintln!(
        "Onboarding stress: {} msgs, {:.2}s, correction_rate={:.3}→{:.3}",
        report.summary.total_messages, report.wall_time_secs, first_cr, last_cr
    );
    eprintln!(
        "  checkpoint_pass_rate={:.2}, regressions={}",
        report.summary.checkpoint_pass_rate,
        report.summary.regression_alerts.len()
    );
    print_checkpoints(&report);
    assert!(
        report.summary.checkpoint_pass_rate >= 1.0,
        "Onboarding stress test checkpoints failed (pass_rate={:.2})",
        report.summary.checkpoint_pass_rate
    );
}

#[tokio::test]
async fn run_fact_contradiction() {
    let report = run_scenario(include_str!("scenarios/fact_contradiction.toml")).await;
    eprintln!(
        "Contradiction test: {} msgs, {:.2}s, contradictions={:.3}, superseded={}",
        report.summary.total_messages,
        report.wall_time_secs,
        report.summary.final_metrics.contradiction_detection_rate,
        report.summary.total_facts_superseded,
    );
    print_checkpoints(&report);
    assert!(
        report.summary.total_facts_superseded > 0,
        "Expected at least 1 fact supersession"
    );
    assert!(
        report.summary.checkpoint_pass_rate >= 1.0,
        "Contradiction scenario checkpoints failed (pass_rate={:.2})",
        report.summary.checkpoint_pass_rate
    );
}

#[tokio::test]
async fn run_coaching_persona() {
    let report = run_scenario(include_str!("scenarios/coaching_persona.toml")).await;
    eprintln!(
        "Coaching persona: {} msgs, {:.2}s, retention={:.3}, personalization={:.3}",
        report.summary.total_messages,
        report.wall_time_secs,
        report.summary.final_metrics.knowledge_retention,
        report.summary.final_metrics.personalization_score,
    );
    print_checkpoints(&report);
    assert!(
        report.summary.checkpoint_pass_rate >= 1.0,
        "Coaching scenario checkpoints failed (pass_rate={:.2})",
        report.summary.checkpoint_pass_rate
    );
}
