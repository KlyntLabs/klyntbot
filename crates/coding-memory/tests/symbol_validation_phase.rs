use coding_memory::causal::CausalEdgeRepo;
use coding_memory::reforge::symbol_validation::SymbolValidationPhase;
use coding_memory::TreeSitterExtractor;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use std::collections::HashMap;
use std::sync::Arc;
use storage::StoragePool;

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

#[tokio::test]
async fn deletes_fact_when_symbol_missing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.rs");
    std::fs::write(&path, "fn other() {}\n").unwrap();

    let pool = fresh_pool().await;

    let fact_id = uuid::Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "anchoredSymbols":[{
            "filePath": "a.rs",
            "symbol":"foo",
            "kind":"function",
            "gitHash":"abc",
            "byteSpan": null
        }]
    });
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, domain, subject, predicate, object, confidence, memory_type, scope_repo_id, metadata, valid_from) \
         VALUES (?1, 'code', 'foo', 'is', 'broken', 0.8, 'fact', 'repo:test', ?2, datetime('now'))",
    )
    .bind(&fact_id)
    .bind(metadata.to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let mut roots = HashMap::new();
    roots.insert("repo:test".into(), dir.path().to_path_buf());

    let phase = SymbolValidationPhase::new(
        Arc::new(SemanticFactRepo::new(pool.inner().clone())),
        Arc::new(EpisodicMemoryRepo::new(pool.inner().clone())),
        Arc::new(TreeSitterExtractor::new()),
        roots,
        Arc::new(CausalEdgeRepo::new(pool.clone())),
    );
    let outcome = phase.run().await.unwrap();
    assert_eq!(outcome.invalidated, 1);
    assert_eq!(outcome.marked_stale, 0);

    let valid_until: Option<String> =
        sqlx::query_scalar("SELECT valid_until FROM semantic_facts WHERE id = ?1")
            .bind(&fact_id)
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert!(valid_until.is_some(), "expected valid_until set");
}

#[tokio::test]
async fn marks_stale_when_symbol_present_but_file_changed() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("a.rs");
    std::fs::write(&path, "fn foo() { /* edited */ }\n").unwrap();

    let pool = fresh_pool().await;
    let fact_id = uuid::Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "anchoredSymbols":[{
            "filePath": "a.rs",
            "symbol":"foo",
            "kind":"function",
            "gitHash":"abc",
            "byteSpan": null
        }]
    });
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, domain, subject, predicate, object, confidence, memory_type, scope_repo_id, metadata, valid_from) \
         VALUES (?1, 'code', 'foo', 'is', 'fine', 0.8, 'fact', 'repo:test', ?2, datetime('now'))",
    )
    .bind(&fact_id)
    .bind(metadata.to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let mut roots = HashMap::new();
    roots.insert("repo:test".into(), dir.path().to_path_buf());

    let phase = SymbolValidationPhase::new(
        Arc::new(SemanticFactRepo::new(pool.inner().clone())),
        Arc::new(EpisodicMemoryRepo::new(pool.inner().clone())),
        Arc::new(TreeSitterExtractor::new()),
        roots,
        Arc::new(CausalEdgeRepo::new(pool.clone())),
    );
    let outcome = phase.run().await.unwrap();
    assert_eq!(outcome.invalidated, 0);
    assert_eq!(outcome.marked_stale, 1);

    let metadata: String = sqlx::query_scalar("SELECT metadata FROM semantic_facts WHERE id = ?1")
        .bind(&fact_id)
        .fetch_one(pool.inner())
        .await
        .unwrap();
    let parsed: serde_json::Value = metadata.parse().unwrap();
    assert_eq!(parsed["status"], "needs_review");
}
