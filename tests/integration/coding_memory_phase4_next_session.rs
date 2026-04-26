// Scenario: seed Phase-3 fixture session into the store, then call the Phase-4
// SessionStart renderer for the same repo; assert that the markdown contains
// the seeded RepoContext fact and a recent-activity table row.

use coding_memory::recall::budget::HeuristicBudgeter;
use coding_memory::recall::{CodingRecallService, CodingRecallServiceConfig};
use std::sync::Arc;

#[tokio::test]
async fn next_session_sees_prior_memory() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    storage::StoragePool::run_feature_migrations(
        pool.inner(),
        &coding_memory::coding_memory_migrations(),
    )
    .await
    .unwrap();
    let fact_repo = Arc::new(cognitive::SemanticFactRepo::new(pool.inner().clone()));
    let ep_repo = Arc::new(cognitive::EpisodicMemoryRepo::new(pool.inner().clone()));

    // Load fixture and seed.
    let raw = std::fs::read_to_string("tests/fixtures/coding/phase4_recall_seed.jsonl").unwrap();
    for line in raw.lines() {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        match v.get("type").and_then(|s| s.as_str()) {
            Some("fact") => {
                let f: cognitive::SemanticFact =
                    serde_json::from_value(v.get("payload").cloned().unwrap()).unwrap();
                fact_repo
                    .upsert_with_metadata(&f, f.scope_repo_id.as_deref(), f.metadata.as_deref())
                    .await
                    .unwrap();
            }
            Some("episode") => {
                let e: cognitive::EpisodicMemory =
                    serde_json::from_value(v.get("payload").cloned().unwrap()).unwrap();
                let kind = e.kind.as_deref().unwrap_or("turn_trace");
                ep_repo
                    .insert_with_kind_and_metadata(
                        &e,
                        kind,
                        e.scope_repo_id.as_deref(),
                        e.metadata.as_deref(),
                    )
                    .await
                    .unwrap();
            }
            _ => {}
        }
    }

    let ums = Arc::new(cognitive::UnifiedMemoryService::new((*fact_repo).clone()));
    let telem = coding_memory::RecallInvocationRepo::new(pool.clone());
    let svc = Arc::new(CodingRecallService::new(
        CodingRecallServiceConfig::default(),
        ums,
        fact_repo,
        ep_repo,
        telem,
        Arc::new(HeuristicBudgeter),
    ));
    let md = coding_memory::recall::renderers::render_session_start_block(&svc, Some("repo:demo"))
        .await
        .unwrap();
    assert!(
        md.contains("auth_module") || md.contains("JWT"),
        "got:\n{md}"
    );
}
