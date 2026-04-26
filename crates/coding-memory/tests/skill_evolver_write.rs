//! Tests for skill evolver write functions.

use coding_memory::reforge::types::ProjectSkillSpec;
use coding_memory::skill_evolver::write::{
    next_version_for, render_skill_body, sanitize, write_skill_md, JournalArgs, SkillWriteOutcome,
};
use storage::StoragePool;
use tempfile::TempDir;

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

#[test]
fn sanitize_creates_kebab_case_slug() {
    assert_eq!(sanitize("Hello World"), "hello-world");
    assert_eq!(sanitize("Use early return!"), "use-early-return");
    assert_eq!(sanitize("  Multiple   Spaces  "), "multiple-spaces");
    assert_eq!(sanitize("UPPERCASE"), "uppercase");
}

#[test]
fn render_skill_body_renders_markdown() {
    let spec = ProjectSkillSpec {
        skill_id: "test-skill".into(),
        repo_id: "repo1".into(),
        name: "Test Skill".into(),
        description: "A test skill".into(),
        when_to_use: vec!["when a".into(), "when b".into()],
        procedure: "Step 1\nStep 2".into(),
        references: vec![uuid::Uuid::nil()],
        anchored_symbols: vec![],
        effectiveness: 0.8,
    };
    let body = render_skill_body(&spec);
    assert!(body.contains("# Test Skill"));
    assert!(body.contains("## Description"));
    assert!(body.contains("A test skill"));
    assert!(body.contains("## When to Use"));
    assert!(body.contains("- when a"));
    assert!(body.contains("## Procedure"));
    assert!(body.contains("Step 1"));
    assert!(body.contains("## References"));
}

#[tokio::test]
async fn next_version_for_starts_at_one() {
    let pool = setup_pool().await;
    let v = next_version_for(&pool, "new-skill").await.unwrap();
    assert_eq!(v, 1);
}

#[tokio::test]
async fn next_version_for_increments() {
    let pool = setup_pool().await;
    sqlx::query(
        "INSERT INTO skill_versions (id, skill_name, version, file_path, content, source)
         VALUES ('v1', 'my-skill', 1, '/a', 'c', 'test')",
    )
    .execute(pool.inner())
    .await
    .unwrap();

    let v = next_version_for(&pool, "my-skill").await.unwrap();
    assert_eq!(v, 2);
}

#[tokio::test]
async fn write_skill_md_creates_file() {
    let pool = setup_pool().await;
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("skill.md");

    let spec = ProjectSkillSpec {
        skill_id: "test-skill".into(),
        repo_id: "repo1".into(),
        name: "Test Skill".into(),
        description: "Desc".into(),
        when_to_use: vec![],
        procedure: "Do thing".into(),
        references: vec![],
        anchored_symbols: vec![],
        effectiveness: 0.8,
    };

    let outcome = write_skill_md(
        &pool,
        &JournalArgs {
            repo_id: "repo1".into(),
            skill_path: path.clone(),
            cycle_id: "c1".into(),
            spec,
        },
    )
    .await
    .unwrap();

    match outcome {
        SkillWriteOutcome::Written { version, path: p } => {
            assert_eq!(version, 1);
            assert_eq!(p, path);
            assert!(path.exists());
            let content = std::fs::read_to_string(&path).unwrap();
            assert!(content.contains("Test Skill"));
            assert!(content.contains("klyntbot:managed:start"));
            assert!(content.contains("klyntbot:managed:end"));
        }
        SkillWriteOutcome::Skipped { .. } => panic!("expected written"),
    }
}
