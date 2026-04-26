//! e2e — skill evolution emits SKILL.md and journals a version row.

use coding_memory::skills::{ProjectSkillEvolver, ProjectSkillEvolverImpl};
use jiff::Timestamp;
use storage::StoragePool;
use tempfile::tempdir;

#[tokio::test]
async fn evolve_writes_skill_md_and_versions_row() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &cognitive::cognitive_migrations())
        .await
        .unwrap();
    StoragePool::run_feature_migrations(pool.inner(), &coding_memory::coding_memory_migrations())
        .await
        .unwrap();

    // Seed a high-confidence workflow_pattern rule.
    sqlx::query(
        "INSERT INTO procedural_rules \
         (id, domain, rule_text, confidence, source, signal_count, created_at, updated_at, \
          active, scope_type, scope_repo_id, effectiveness_score, stability, metadata) \
         VALUES ('rule1', 'code', 'Always handle Result explicitly', 0.85, 'observed', 3, \
                 ?1, ?1, 1, 'code', 'r1', 0.8, 2.0, ?2)",
    )
    .bind(Timestamp::now().to_string())
    .bind(r#"{"kind": "workflow_pattern"}"#)
    .execute(pool.inner())
    .await
    .unwrap();

    let dir = tempdir().unwrap();
    let base_dir = dir.path().join("project-skills");
    std::fs::create_dir_all(&base_dir).unwrap();

    let evolver = ProjectSkillEvolverImpl::new(pool.clone(), base_dir.clone());
    let results = evolver.evolve("r1").await.unwrap();

    assert!(!results.is_empty(), "expected at least one skill synthesis result");

    let result = &results[0];
    let skill_path = &result.skill_path;
    assert!(
        skill_path.exists(),
        "SKILL.md should exist at {}",
        skill_path.display()
    );

    let content = std::fs::read_to_string(skill_path).unwrap();
    assert!(content.contains("Always handle Result explicitly"));
    assert!(content.contains("klyntbot:managed:start"));
    assert!(content.contains("klyntbot:managed:end"));

    // Assert skill_versions row inserted.
    let count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM skill_versions WHERE scope = 'project' AND scope_repo_id = 'r1'",
    )
    .fetch_one(pool.inner())
    .await
    .unwrap();
    assert!(count.0 >= 1, "expected at least one skill_versions row");
}
