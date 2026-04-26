//! Tests for skill evolver detection.

use coding_memory::skill_evolver::detect_candidates;
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
async fn detect_returns_empty_when_no_rules() {
    let pool = setup_pool().await;
    let candidates = detect_candidates(&pool, "repo1").await.unwrap();
    assert!(candidates.is_empty());
}

#[tokio::test]
async fn detect_skips_low_confidence_rules() {
    let pool = setup_pool().await;
    sqlx::query(
        "INSERT INTO procedural_rules (id, domain, rule_text, confidence, source, metadata, scope_repo_id)
         VALUES ('r1', 'coding', 'low conf', 0.5, 'observed', '{\"kind\":\"workflow_pattern\"}', 'repo1')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let candidates = detect_candidates(&pool, "repo1").await.unwrap();
    assert!(candidates.is_empty());
}

#[tokio::test]
async fn detect_skips_non_workflow_patterns() {
    let pool = setup_pool().await;
    sqlx::query(
        "INSERT INTO procedural_rules (id, domain, rule_text, confidence, source, metadata, scope_repo_id)
         VALUES ('r1', 'coding', 'failure pattern', 0.9, 'observed', '{\"kind\":\"failure_pattern\"}', 'repo1')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let candidates = detect_candidates(&pool, "repo1").await.unwrap();
    assert!(candidates.is_empty());
}

#[tokio::test]
async fn detect_skips_already_expressed_rules() {
    let pool = setup_pool().await;
    sqlx::query(
        "INSERT INTO procedural_rules (id, domain, rule_text, confidence, source, metadata, scope_repo_id)
         VALUES ('r1', 'coding', 'already expressed', 0.9, 'observed', '{\"kind\":\"workflow_pattern\"}', 'repo1')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    sqlx::query(
        "INSERT INTO skill_versions (id, skill_name, version, file_path, content, source, scope, scope_repo_id, source_pattern_id, status)
         VALUES ('sv1', 'skill1', 1, '/tmp/skill.md', 'content', 'evolver', 'project', 'repo1', 'r1', 'active')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let candidates = detect_candidates(&pool, "repo1").await.unwrap();
    assert!(candidates.is_empty());
}

#[tokio::test]
async fn detect_returns_workflow_patterns() {
    let pool = setup_pool().await;
    sqlx::query(
        "INSERT INTO procedural_rules (id, domain, rule_text, confidence, source, metadata, scope_repo_id)
         VALUES ('r1', 'coding', 'Use early return', 0.85, 'observed', '{\"kind\":\"workflow_pattern\"}', 'repo1')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let candidates = detect_candidates(&pool, "repo1").await.unwrap();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].id, "r1");
    assert_eq!(candidates[0].rule_text, "Use early return");
    assert!(candidates[0].confidence >= 0.7);
}
