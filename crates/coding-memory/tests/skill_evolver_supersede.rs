//! Tests for skill evolver supersede function.

use coding_memory::skill_evolver::supersede_outdated_versions;
use storage::StoragePool;

async fn setup_pool() -> StoragePool {
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
async fn supersede_marks_older_versions() {
    let pool = setup_pool().await;
    for (id, version) in [("v1", 1), ("v2", 2), ("v3", 3)] {
        sqlx::query(
            "INSERT INTO skill_versions (id, skill_name, version, file_path, content, source, status)
             VALUES (?1, 'my-skill', ?2, '/a', 'c', 'test', 'active')",
        )
        .bind(id)
        .bind(version)
        .execute(pool.inner())
        .await
        .unwrap();
    }

    let affected = supersede_outdated_versions(&pool, "my-skill").await.unwrap();
    assert_eq!(affected, 2);

    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, version FROM skill_versions WHERE skill_name = 'my-skill' AND status = 'superseded' ORDER BY version",
    )
    .fetch_all(pool.inner())
    .await
    .unwrap();

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].1, 1);
    assert_eq!(rows[1].1, 2);

    let active: Vec<(String, i64)> = sqlx::query_as(
        "SELECT id, version FROM skill_versions WHERE skill_name = 'my-skill' AND status = 'active'",
    )
    .fetch_all(pool.inner())
    .await
    .unwrap();

    assert_eq!(active.len(), 1);
    assert_eq!(active[0].1, 3);
}

#[tokio::test]
async fn supersede_single_version_does_nothing() {
    let pool = setup_pool().await;
    sqlx::query(
        "INSERT INTO skill_versions (id, skill_name, version, file_path, content, source, status)
         VALUES ('v1', 'my-skill', 1, '/a', 'c', 'test', 'active')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let affected = supersede_outdated_versions(&pool, "my-skill").await.unwrap();
    assert_eq!(affected, 0);

    let active: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM skill_versions WHERE skill_name = 'my-skill' AND status = 'active'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();

    assert_eq!(active, 1);
}
