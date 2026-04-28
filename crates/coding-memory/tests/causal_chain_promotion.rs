use coding_memory::causal::CausalEdgeRepo;
use coding_memory::reforge::synth_handler::CodingSynthesisHandler;
use coding_memory::reforge::types::{
    CodingPhaseHandlers, CodingSynthesisInput, CodingSynthesisOutput, PromoteAction,
};
use coding_memory::reforge::CodingSynthesisPhase;
use coding_memory::scope::{CausalEdge, CausalEdgeKind};
use jiff::Timestamp;
use std::sync::Arc;
use storage::StoragePool;
use uuid::Uuid;

async fn fresh_pool() -> StoragePool {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();
    pool
}

struct StubHandler;

#[async_trait::async_trait]
impl CodingSynthesisHandler for StubHandler {
    async fn synthesize_coding(
        &self,
        input: &CodingSynthesisInput,
    ) -> common::Result<CodingSynthesisOutput> {
        let actions = input
            .repo_bundles
            .iter()
            .flat_map(|b| {
                b.causal_chains
                    .iter()
                    .map(|g| PromoteAction::PromoteToProblemSolutionPattern {
                        problem_hash: g.problem_hash.clone(),
                        solution: format!("see edges {:?}", g.edge_ids),
                        supporting_edges: g.edge_ids.clone(),
                    })
            })
            .collect();
        Ok(CodingSynthesisOutput {
            actions,
            narrative: "stub".into(),
        })
    }
}

#[tokio::test]
async fn three_chain_group_promoted_to_problem_solution_pattern() {
    let pool = fresh_pool().await;
    let edges = Arc::new(CausalEdgeRepo::new(pool.clone()));

    let mut ep_ids = Vec::new();
    for _ in 0..3 {
        let id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO episodic_memories (id, domain, content, kind, occurred_at, recorded_at, importance, scope_repo_id, metadata) \
             VALUES (?1, 'coding', ?2, 'fix_attempt', ?3, ?3, 0.5, 'repo:test', ?4)",
        )
        .bind(id.to_string())
        .bind(serde_json::json!({"sessionId":"s"}).to_string())
        .bind(Timestamp::now().to_string())
        .bind(r#"{"problemHash":"H_PROMOTE"}"#)
        .execute(pool.inner())
        .await
        .unwrap();
        ep_ids.push(id);
    }
    for (f, t) in [
        (ep_ids[0], ep_ids[1]),
        (ep_ids[1], ep_ids[2]),
        (ep_ids[0], ep_ids[2]),
    ] {
        edges
            .insert(&CausalEdge {
                id: Uuid::new_v4(),
                from_id: f,
                to_id: t,
                edge_kind: CausalEdgeKind::SharesRootCause,
                confidence: 0.6,
                inferred_at: Timestamp::now(),
            })
            .await
            .unwrap();
    }

    let fact_repo = cognitive::SemanticFactRepo::new(pool.inner().clone());
    let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.inner().clone());
    let rule_repo = cognitive::ProceduralRuleRepo::new(pool.inner().clone());
    let coact = cognitive::CoActivationRepo::new(pool.inner().clone());
    let utilization = coding_memory::RecallInvocationRepo::new(pool.clone());
    let summaries = coding_memory::SessionSummaryRepo::new(pool.clone());
    let sd_log =
        coding_memory::reforge::selective_delete::SelectiveDeleteLogRepo::new(pool.clone());
    let pe_log = coding_memory::mirror::pattern_effectiveness::PatternEffectivenessLogRepo::new(
        pool.clone(),
    );

    let handler = StubHandler;
    let handlers = CodingPhaseHandlers {
        synthesis: Some(&handler),
        rule_artifacts: None,
        fact_repo: &fact_repo,
        episodic_repo: &ep_repo,
        rule_repo: &rule_repo,
        co_activation_repo: &coact,
        utilization_repo: &utilization,
        session_summary_repo: &summaries,
        selective_delete_log: &sd_log,
        pattern_effectiveness_log: &pe_log,
        bus: None,
        causal_repo: Some(&edges),
        symbol_extractor: None,
        repo_roots: &Default::default(),
    };

    let applied = CodingSynthesisPhase::run(&handlers).await.unwrap();
    assert_eq!(applied, 1, "expected one promotion applied");

    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM procedural_rules \
         WHERE source = 'reflected' AND rule_text LIKE '%H_PROMOTE%'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert_eq!(count, 1);
}
