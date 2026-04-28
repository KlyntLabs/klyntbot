//! Phase-6 exit gate: commit deleting a function invalidates anchored facts within 60 s.

use coding_ingest::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use coding_ingest::git_invalidation::GitInvalidationHandler;
use coding_memory::git_invalidation::GitInvalidationHandlerImpl;
use jiff::Timestamp;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use storage::StoragePool;
use tempfile::TempDir;
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

fn init_repo_with_two_commits(dir: &TempDir) -> (String, String) {
    let path = dir.path();
    let repo = git2::Repository::init(path).unwrap();
    let sig = git2::Signature::now("test", "t@t").unwrap();

    std::fs::write(path.join("a.rs"), "fn foo() {}\nfn bar() {}\n").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(std::path::Path::new("a.rs")).unwrap();
    idx.write().unwrap();
    let tree_oid = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let parent_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "init", &tree, &[])
        .unwrap();

    std::fs::write(path.join("a.rs"), "fn bar() {}\n").unwrap();
    let mut idx = repo.index().unwrap();
    idx.add_path(std::path::Path::new("a.rs")).unwrap();
    idx.write().unwrap();
    let tree_oid = idx.write_tree().unwrap();
    let tree = repo.find_tree(tree_oid).unwrap();
    let parent = repo.find_commit(parent_oid).unwrap();
    let head_oid = repo
        .commit(Some("HEAD"), &sig, &sig, "drop foo", &tree, &[&parent])
        .unwrap();

    (head_oid.to_string(), parent_oid.to_string())
}

#[tokio::test(flavor = "multi_thread")]
async fn end_to_end_within_60s() {
    let dir = TempDir::new().unwrap();
    let (head, parent) = init_repo_with_two_commits(&dir);

    let pool = fresh_pool().await;
    let fact_id = Uuid::new_v4().to_string();
    let metadata = serde_json::json!({
        "anchoredSymbols":[{
            "filePath":"a.rs","symbol":"foo","kind":"function",
            "gitHash": parent, "byteSpan": null
        }]
    });
    sqlx::query(
        "INSERT INTO semantic_facts \
         (id, domain, subject, predicate, object, confidence, memory_type, scope_repo_id, metadata, valid_from) \
         VALUES (?1, 'code', 'foo', 'is', 'a fn', 0.9, 'fact', 'repo:test', ?2, datetime('now'))",
    )
    .bind(&fact_id)
    .bind(metadata.to_string())
    .execute(pool.inner())
    .await
    .unwrap();

    let handler = GitInvalidationHandlerImpl::new(
        pool.clone(),
        Arc::new(coding_memory::TreeSitterExtractor::new()),
    );
    let event = AgentEvent::V1(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: format!("git:{head}"),
        turn_id: None,
        cwd: dir.path().to_path_buf(),
        repo: None,
        occurred_at: Timestamp::now(),
        kind: EventKind::GitCommit {
            commit_hash: head,
            parent_hash: Some(parent),
            repo_root: dir.path().to_path_buf(),
            changed_files: vec![PathBuf::from("a.rs")],
        },
    });

    tokio::time::timeout(Duration::from_secs(60), handler.handle(&event))
        .await
        .expect("completed within 60s")
        .expect("handler succeeded");

    let valid_until: Option<String> =
        sqlx::query_scalar("SELECT valid_until FROM semantic_facts WHERE id = ?1")
            .bind(&fact_id)
            .fetch_one(pool.inner())
            .await
            .unwrap();
    assert!(
        valid_until.is_some(),
        "expected fact invalidated within 60s"
    );
}
