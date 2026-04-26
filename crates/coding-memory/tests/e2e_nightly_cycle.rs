//! e2e — nightly cycle on seeded data.
//!
//! Seeds episodic memories + procedural rules, runs all 4 coding phases
//! with mock handlers, and asserts at least one artifact-related change.

use async_trait::async_trait;
use coding_memory::reforge::{
    selective_delete::SelectiveDeleteSignal,
    synth_handler::{CodingSynthesisHandler, RuleArtifactsHandler},
    types::{
        CodingPhaseHandlers, CodingSynthesisInput, CodingSynthesisOutput, PromoteAction,
        RuleArtifactInput, RuleArtifactOutput,
    },
    CodingSynthesisPhase, CrossSessionDedup, RuleArtifactGenerationPhase,
};
use jiff::Timestamp;
use storage::StoragePool;
use tempfile::tempdir;

struct MockSynthesis;

#[async_trait]
impl CodingSynthesisHandler for MockSynthesis {
    async fn synthesize_coding(
        &self,
        _input: &CodingSynthesisInput,
    ) -> common::Result<CodingSynthesisOutput> {
        Ok(CodingSynthesisOutput {
            actions: vec![PromoteAction::ExtractPattern {
                repo_id: Some("r1".into()),
                rule: "Always use early return".into(),
                confidence: 0.85,
                supporting: vec![],
            }],
            narrative: "mock synthesis".into(),
        })
    }
}

struct MockArtifacts;

#[async_trait]
impl RuleArtifactsHandler for MockArtifacts {
    async fn synthesize_artifact(
        &self,
        _input: &RuleArtifactInput,
    ) -> common::Result<RuleArtifactOutput> {
        Ok(RuleArtifactOutput {
            body: "## Architecture\n- Use early return\n".into(),
            section_labels: vec!["Architecture".into()],
        })
    }
}

#[tokio::test]
async fn nightly_cycle_produces_artifact_change() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    // Seed episodic memory (fix_attempt in last 24h).
    sqlx::query(
        "INSERT INTO episodic_memories \
         (id, domain, content, importance, occurred_at, recorded_at, stability, \
          access_count, scope_type, kind, scope_repo_id) \
         VALUES ('ep1', 'code', 'fix attempt', 0.5, ?1, ?1, 1.0, 0, 'code', 'fix_attempt', 'r1')",
    )
    .bind(Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    // Seed a high-confidence semantic fact for the repo.
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, subject, predicate, object, confidence, domain, scope_type, scope_repo_id, \
          valid_from, memory_type) \
         VALUES ('f1','repo:r1','language','rust',0.9,'work','code','r1',?1,'fact')",
    )
    .bind(Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let dir = tempdir().unwrap();
    let repo_root = dir.path().join("repo1");
    std::fs::create_dir_all(&repo_root).unwrap();
    std::env::set_var(
        "KLYNTBOT_REPO_PATHS_TEST_OVERRIDE",
        format!(r#"{{"r1":"{}"}}"#, repo_root.display()),
    );

    let fact_repo = cognitive::SemanticFactRepo::new(pool.inner().clone());
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.inner().clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(pool.inner().clone());
    let co_act = cognitive::CoActivationRepo::new(pool.inner().clone());
    let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
    let session_summary_repo = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let selective_delete_log =
        coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());
    let pattern_effectiveness_log =
        coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(
            pool.clone(),
        );

    let synth = MockSynthesis;
    let artifacts = MockArtifacts;

    let handlers = CodingPhaseHandlers {
        synthesis: Some(&synth),
        rule_artifacts: Some(&artifacts),
        fact_repo: &fact_repo,
        episodic_repo: &ep_repo,
        rule_repo: &rule_repo,
        co_activation_repo: &co_act,
        utilization_repo: &utilization,
        session_summary_repo: &session_summary_repo,
        selective_delete_log: &selective_delete_log,
        pattern_effectiveness_log: &pattern_effectiveness_log,
        bus: None,
    };

    let rule_before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM procedural_rules")
        .fetch_one(pool.inner())
        .await
        .unwrap();

    let synth_applied = CodingSynthesisPhase::run(&handlers).await.unwrap();
    let artifact_applied = RuleArtifactGenerationPhase::run(&handlers, &["claude_md".into()])
        .await
        .unwrap();
    let delete_applied = SelectiveDeleteSignal::apply(&pool, &selective_delete_log)
        .await
        .unwrap();
    let dedup_applied = CrossSessionDedup::run(&fact_repo, 0.92).await.unwrap();

    let rule_after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM procedural_rules")
        .fetch_one(pool.inner())
        .await
        .unwrap();

    let claude_md = repo_root.join("CLAUDE.md");
    let file_written = claude_md.exists();

    assert!(
        synth_applied > 0
            || artifact_applied > 0
            || delete_applied > 0
            || dedup_applied > 0
            || file_written
            || rule_after.0 > rule_before.0,
        "expected at least one artifact-related change"
    );

    // Stronger assertion: synthesis added a rule OR rule-artifacts wrote a file.
    assert!(
        rule_after.0 > rule_before.0 || file_written,
        "expected rule count to increase or artifact file to be written"
    );
}
