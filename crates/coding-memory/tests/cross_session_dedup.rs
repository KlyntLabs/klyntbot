use coding_memory::reforge::CrossSessionDedup;
use storage::StoragePool;

#[tokio::test]
async fn supersedes_high_similarity_pair_preserves_both() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    // Seed two near-duplicate facts (we simulate similarity by directly seeding
    // an embedding + the dedup helper that bypasses LanceDB in-memory).
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, subject, predicate, object, confidence, domain, scope_type, scope_id, scope_repo_id, valid_from, memory_type) \
         VALUES \
         ('f1','repo:r1','uses_framework','axum 0.7',0.85,'work','code',NULL,'r1',?1,'fact'), \
         ('f2','repo:r1','uses_framework','axum 0.7',0.9,'work','code',NULL,'r1',?2,'fact')",
    )
    .bind("2026-04-20T10:00:00Z")
    .bind("2026-04-25T10:00:00Z")
    .execute(pool.inner())
    .await
    .unwrap();

    let fact_repo = cognitive::SemanticFactRepo::new(pool.inner().clone());
    let count = CrossSessionDedup::run_test_only_exact_match(&fact_repo, 0.92)
        .await
        .expect("dedup");
    assert!(count >= 1);

    // Both rows still exist.
    let row_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM semantic_facts WHERE id IN ('f1','f2')")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert_eq!(row_count.0, 2);

    // The older row's valid_until is now non-null, and its superseded_by points to f2.
    let (until, by): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT valid_until, superseded_by FROM semantic_facts WHERE id = 'f1'")
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert!(until.is_some());
    assert_eq!(by.as_deref(), Some("f2"));
}
