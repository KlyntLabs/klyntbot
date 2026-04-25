use storage::StoragePool;

#[tokio::test]
async fn phase5_tables_exist_after_migration() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cognitive migs");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding-memory migs");

    for table in [
        "session_summaries",
        "pattern_effectiveness_log",
        "selective_delete_log",
    ] {
        let q = format!("SELECT COUNT(*) FROM {table}");
        let row: (i64,) = sqlx::query_as(&q)
            .fetch_one(pool.inner())
            .await
            .unwrap_or_else(|e| panic!("{table}: {e}"));
        assert_eq!(row.0, 0, "{table} should be empty after fresh migration");
    }
}

#[tokio::test]
async fn mirror_snippets_has_coding_alert_columns() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .expect("cognitive migs");
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .expect("coding-memory migs");
    let row: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM pragma_table_info('mirror_snippets') \
         WHERE name IN ('coding_alert_kind', 'coding_alert_severity')",
    )
    .fetch_one(pool.inner())
    .await
    .expect("pragma");
    assert_eq!(row.0, 2);
}
