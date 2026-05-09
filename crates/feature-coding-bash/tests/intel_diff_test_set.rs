//! Phase 2.3b: TestFailure→TestFailure diff produces correct new/still/resolved sets.

use feature_coding_bash::intelligence::{diff_against_prior, ExtractedDiff};
use jiff::Timestamp;
use storage::{repos::{BashJobRepo, BashJobRow}, StoragePool};

fn row(id: &str, kind: &str, names: &[&str]) -> BashJobRow {
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
        status: "Failed".into(),
        exit_code: Some(101),
        failure_kind: Some(kind.into()),
        failure_detail: None,
        failure_extracted: Some(serde_json::json!({
            "failed_test_names": names,
            "n_failed": names.len(),
            "n_passed": 5,
            "n_ignored": 0,
        }).to_string()),
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

#[tokio::test]
async fn test_set_diff_via_repo_round_trip() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let m = feature_coding_bash::migrations::coding_background_jobs_migration();
    sqlx::query(&m.sql).execute(pool.inner()).await.unwrap();
    let repo = BashJobRepo::new(pool.inner().clone());

    let prior = row("a", "TestFailure", &["A", "B"]);
    let curr  = row("b", "TestFailure", &["B", "C"]);
    repo.insert(&prior).await.unwrap();
    repo.insert(&curr).await.unwrap();

    let prior_back = repo.find_prior_by_command_key("s1", "k", "b").await.unwrap().unwrap();
    let diff = diff_against_prior(&prior_back, &curr);

    match diff.extracted_diff {
        ExtractedDiff::TestSet { new_failures, still_failing, resolved } => {
            let mut new_f = new_failures.clone(); new_f.sort();
            assert_eq!(new_f, vec!["C".to_string()]);
            assert_eq!(still_failing, vec!["B".to_string()]);
            assert_eq!(resolved, vec!["A".to_string()]);
        }
        other => panic!("expected TestSet, got {other:?}"),
    }
}
