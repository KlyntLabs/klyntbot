use coding_memory::reforge::types::{
    CodingPhaseHandlers, CodingSynthesisInput, CodingSynthesisOutput, PromoteAction,
};
use coding_memory::reforge::{CodingSynthesisHandler, CodingSynthesisPhase};
use storage::StoragePool;

struct Mock(CodingSynthesisOutput);
#[async_trait::async_trait]
impl CodingSynthesisHandler for Mock {
    async fn synthesize_coding(
        &self,
        _: &CodingSynthesisInput,
    ) -> common::Result<CodingSynthesisOutput> {
        Ok(self.0.clone())
    }
}

async fn fixture() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn extract_pattern_persists_procedural_rule() {
    let pool = fixture().await;
    let handler = Mock(CodingSynthesisOutput {
        actions: vec![PromoteAction::ExtractPattern {
            repo_id: Some("r1".into()),
            rule: "always run cargo fmt before commit".into(),
            confidence: 0.85,
            supporting: vec![],
        }],
        narrative: String::new(),
    });

    let fact_repo = cognitive::SemanticFactRepo::new(pool.inner().clone());
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.inner().clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(pool.inner().clone());
    let co_act = cognitive::CoActivationRepo::new(pool.inner().clone());
    let summaries = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
    let pat_eff_repo =
        coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(
            pool.clone(),
        );
    let sd_repo =
        coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());

    let handlers = CodingPhaseHandlers {
        synthesis: Some(&handler),
        rule_artifacts: None,
        fact_repo: &fact_repo,
        episodic_repo: &ep_repo,
        rule_repo: &rule_repo,
        co_activation_repo: &co_act,
        utilization_repo: &utilization,
        session_summary_repo: &summaries,
        selective_delete_log: &sd_repo,
        pattern_effectiveness_log: &pat_eff_repo,
        bus: None,
        causal_repo: None,
        symbol_extractor: None,
        repo_roots: &Default::default(),
    };

    CodingSynthesisPhase::run(&handlers)
        .await
        .expect("phase ok");

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM procedural_rules WHERE source = 'observed' \
         AND rule_text LIKE 'always run cargo fmt%'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(count.0, 1);
}

#[tokio::test]
async fn promote_to_project_understanding_writes_semantic_fact() {
    let pool = fixture().await;
    let handler = Mock(CodingSynthesisOutput {
        actions: vec![PromoteAction::PromoteToProjectUnderstanding {
            repo_id: "r1".into(),
            subject: "repo:r1".into(),
            predicate: "uses_framework".into(),
            object: "axum 0.7".into(),
            convergence: 0.9,
        }],
        narrative: String::new(),
    });

    let fact_repo = cognitive::SemanticFactRepo::new(pool.inner().clone());
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.inner().clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(pool.inner().clone());
    let co_act = cognitive::CoActivationRepo::new(pool.inner().clone());
    let summaries = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
    let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
    let pat_eff_repo =
        coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(
            pool.clone(),
        );
    let sd_repo =
        coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());

    let handlers = CodingPhaseHandlers {
        synthesis: Some(&handler),
        rule_artifacts: None,
        fact_repo: &fact_repo,
        episodic_repo: &ep_repo,
        rule_repo: &rule_repo,
        co_activation_repo: &co_act,
        utilization_repo: &utilization,
        session_summary_repo: &summaries,
        selective_delete_log: &sd_repo,
        pattern_effectiveness_log: &pat_eff_repo,
        bus: None,
        causal_repo: None,
        symbol_extractor: None,
        repo_roots: &Default::default(),
    };

    CodingSynthesisPhase::run(&handlers)
        .await
        .expect("phase ok");

    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM semantic_facts WHERE subject = 'repo:r1' \
         AND predicate = 'uses_framework' AND object = 'axum 0.7'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(count.0, 1);
}
