//! Migration applies cleanly on top of the cognitive + storage baseline.
//!
//! This seeds an in-memory SQLite pool with the base cognitive migrations
//! (which create `semantic_facts` / `episodic_memories` / `skill_versions`),
//! then runs the consolidated Phase-1 migration and asserts every new
//! column/table exists.

use coding_memory::coding_memory_migrations;
use cognitive::cognitive_migrations;
use sqlx::Row;
use storage::StoragePool;

#[tokio::test]
async fn phase1_migration_applies_over_cognitive_base() {
    let pool = StoragePool::connect_in_memory().await.expect("pool");

    storage::StoragePool::run_feature_migrations(pool.inner(), &cognitive_migrations())
        .await
        .expect("cognitive migrations");

    storage::StoragePool::run_feature_migrations(pool.inner(), &coding_memory_migrations())
        .await
        .expect("coding-memory migration");

    // New semantic_facts columns
    for col in ["scope_repo_id", "metadata", "actor_id"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('semantic_facts') \
             WHERE name = ?",
        )
        .bind(col)
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert_eq!(exists, 1, "semantic_facts missing column: {col}");
    }

    // New episodic_memories columns
    for col in ["kind", "actor_id", "scope_repo_id", "metadata"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('episodic_memories') \
             WHERE name = ?",
        )
        .bind(col)
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert_eq!(exists, 1, "episodic_memories missing column: {col}");
    }

    // New tables
    for table in [
        "ingest_event_log",
        "memory_causal_edges",
        "memory_utilization",
        "klynt_sessions",
    ] {
        let row = sqlx::query(
            "SELECT name FROM sqlite_master WHERE type='table' AND name = ?",
        )
        .bind(table)
        .fetch_optional(pool.sqlx_pool())
        .await
        .unwrap();
        assert!(row.is_some(), "missing table: {table}");
    }

    // skill_versions scope columns
    for col in ["scope", "scope_repo_id"] {
        let exists: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pragma_table_info('skill_versions') \
             WHERE name = ?",
        )
        .bind(col)
        .fetch_one(pool.inner())
        .await
        .unwrap();
        assert_eq!(exists, 1, "skill_versions missing column: {col}");
    }

    let _ = row_count_check(&pool).await;
}

async fn row_count_check(pool: &StoragePool) -> sqlx::Result<()> {
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM ingest_event_log")
        .fetch_one(pool.inner())
        .await?;
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_causal_edges")
        .fetch_one(pool.inner())
        .await?;
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM memory_utilization")
        .fetch_one(pool.inner())
        .await?;
    let _: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM klynt_sessions")
        .fetch_one(pool.inner())
        .await?;
    Ok(())
}

/// Silence clippy about the unused helper trait method.
const _ = ();
