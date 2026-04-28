//! Invariant — no coding reforge phase reduces episodic memory row count.

use coding_memory::reforge::{
    selective_delete::SelectiveDeleteSignal, types::CodingPhaseHandlers, CodingSynthesisPhase,
    CrossSessionDedup, RuleArtifactGenerationPhase,
};
use proptest::prelude::*;
use storage::StoragePool;

async fn seed_episodics(pool: &StoragePool, count: usize) {
    for i in 0..count {
        sqlx::query(
            "INSERT INTO episodic_memories \
             (id, domain, content, importance, occurred_at, recorded_at, stability, \
              access_count, scope_type, kind, scope_repo_id) \
             VALUES (?1, 'code', ?2, 0.5, ?3, ?3, 1.0, 0, 'code', 'fix_attempt', 'r1')",
        )
        .bind(format!("ep_{i}"))
        .bind(format!("attempt {i}"))
        .bind(jiff::Timestamp::now().to_string())
        .execute(pool.inner())
        .await
        .unwrap();
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(16))]

    #[test]
    fn reforge_never_deletes_episodics(n in 0usize..=50) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let pool = StoragePool::connect_in_memory().await.unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
                .await
                .unwrap();
            StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
                .await
                .unwrap();

            seed_episodics(&pool, n).await;
            let before: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
                .fetch_one(pool.inner())
                .await
                .unwrap();

            let fact_repo = cognitive::SemanticFactRepo::new(pool.inner().clone());
            let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.inner().clone());
            let rule_repo = cognitive::ProceduralRuleRepo::new(pool.inner().clone());
            let co_act = cognitive::CoActivationRepo::new(pool.inner().clone());
            let utilization = coding_memory::recall::telemetry::RecallInvocationRepo::new(pool.clone());
            let session_summary_repo = coding_memory::reforge::SessionSummaryRepo::new(pool.clone());
            let selective_delete_log = coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());
            let pattern_effectiveness_log =
                coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(pool.clone());

            let handlers = CodingPhaseHandlers {
                synthesis: None,
                rule_artifacts: None,
                fact_repo: &fact_repo,
                episodic_repo: &ep_repo,
                rule_repo: &rule_repo,
                co_activation_repo: &co_act,
                utilization_repo: &utilization,
                session_summary_repo: &session_summary_repo,
                selective_delete_log: &selective_delete_log,
                pattern_effectiveness_log: &pattern_effectiveness_log,
                bus: None,
                causal_repo: None,
                symbol_extractor: None,
                repo_roots: &Default::default(),
            };

            CodingSynthesisPhase::run(&handlers).await.unwrap();
            RuleArtifactGenerationPhase::run(&handlers, &[]).await.unwrap();
            SelectiveDeleteSignal::apply(&pool, &selective_delete_log).await.unwrap();
            CrossSessionDedup::run(&fact_repo, 0.92, None).await.unwrap();

            let after: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM episodic_memories")
                .fetch_one(pool.inner())
                .await
                .unwrap();

            prop_assert!(after.0 >= before.0, "Reforge deleted {} rows", before.0 - after.0);
            Ok(())
        }).unwrap();
    }
}
