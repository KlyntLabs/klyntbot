use coding_memory::reforge::types::CodingPhaseHandlers;
use coding_memory::reforge::{RuleArtifactGenerationPhase, RuleArtifactInput, RuleArtifactOutput, RuleArtifactsHandler};
use storage::StoragePool;
use tempfile::tempdir;

struct Mock;
#[async_trait::async_trait]
impl RuleArtifactsHandler for Mock {
    async fn synthesize_artifact(
        &self,
        input: &RuleArtifactInput,
    ) -> common::Result<RuleArtifactOutput> {
        Ok(RuleArtifactOutput {
            body: format!(
                "## Architecture\n- repo: {}\n- artifact: {:?}\n",
                input.plan.repo_id, input.artifact
            ),
            section_labels: vec!["Architecture".into()],
        })
    }
}

#[tokio::test]
async fn writes_managed_block_for_each_enabled_artifact() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    let dir = tempdir().unwrap();
    let repo_root = dir.path().join("repo1");
    std::fs::create_dir_all(&repo_root).unwrap();

    // Seed a `RepoContext` fact tied to the canonical id.
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, subject, predicate, object, confidence, domain, scope_type, scope_id, \
          scope_repo_id, valid_from, memory_type, metadata) \
         VALUES ('fact-1','repo:r1','language','rust',0.9,'work','code',NULL,'r1',?1,'fact','{}')",
    )
    .bind(jiff::Timestamp::now().to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let fact_repo = cognitive::SemanticFactRepo::new(pool.inner().clone());
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.inner().clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(pool.inner().clone());
    let co_act = cognitive::CoActivationRepo::new(pool.inner().clone());
    let summaries = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
    let sd = coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());
    let pat = coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(pool.clone());

    let handler = Mock;

    // Inject a single-entry repo path map via env var convention.
    std::env::set_var(
        "KLYNTBOT_REPO_PATHS_TEST_OVERRIDE",
        format!(r#"{{"r1":"{}"}}"#, repo_root.display()),
    );

    let handlers = CodingPhaseHandlers {
        synthesis: None,
        rule_artifacts: Some(&handler),
        fact_repo: &fact_repo,
        episodic_repo: &ep_repo,
        rule_repo: &rule_repo,
        co_activation_repo: &co_act,
        utilization_repo: &utilization,
        session_summary_repo: &summaries,
        selective_delete_log: &sd,
        pattern_effectiveness_log: &pat,
        bus: None,
    };

    RuleArtifactGenerationPhase::run(&handlers, &["claude_md".into()])
        .await
        .expect("phase");

    let claude_md = repo_root.join("CLAUDE.md");
    assert!(claude_md.exists(), "CLAUDE.md should be written");
    let content = std::fs::read_to_string(claude_md).unwrap();
    assert!(content.contains("klyntbot:managed:start"));
    assert!(content.contains("Architecture"));
}
