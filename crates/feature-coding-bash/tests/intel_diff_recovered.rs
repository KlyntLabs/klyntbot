//! Phase 2.3b: Recovered transition when prior failed.

use feature_coding_bash::intelligence::{diff_against_prior, KindTransition};
use jiff::Timestamp;
use storage::{repos::{BashJobRepo, BashJobRow}, StoragePool};

fn passed_row(id: &str) -> BashJobRow {
    BashJobRow {
        id: id.into(),
        session_id: "s1".into(),
        agent_id: "a1".into(),
        description: "desc".into(),
        command: "cargo test".into(),
        command_key: "k".into(),
        cwd: "/".into(),
        timeout_ms: 60_000,
        silent_completion: false,
        status: "Completed".into(),
        exit_code: Some(0),
        failure_kind: None,
        failure_detail: None,
        failure_extracted: None,
        started_at: Timestamp::from_millisecond(1_700_000_000_000).unwrap(),
        finished_at: Some(Timestamp::from_millisecond(1_700_000_001_000).unwrap()),
        total_bytes_emitted: 0,
        bisect_count: 0,
        log_path: format!("/tmp/{id}.log"),
        final_path: None,
        last_polled_at: None,
        last_seen_offset: 0,
    }
}

fn failed_row(id: &str) -> BashJobRow {
    BashJobRow {
        failure_kind: Some("TestFailure".into()),
        failure_extracted: Some(serde_json::json!({"failed_test_names":["A"]}).to_string()),
        status: "Failed".into(),
        exit_code: Some(101),
        ..passed_row(id)
    }
}

#[tokio::test]
async fn recovered_transition_when_prior_failed() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let m = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(&m.sql).execute(pool.inner()).await.unwrap();
    let repo = BashJobRepo::new(pool.inner().clone());

    repo.insert(&failed_row("a")).await.unwrap();
    let curr = passed_row("b");
    repo.insert(&curr).await.unwrap();

    let prior = repo.find_prior_by_command_key("s1", "k", "b").await.unwrap().unwrap();
    let diff = diff_against_prior(&prior, &curr);

    assert_eq!(
        diff.kind_transition,
        KindTransition::Recovered { prior_kind: "TestFailure".into() }
    );
}
