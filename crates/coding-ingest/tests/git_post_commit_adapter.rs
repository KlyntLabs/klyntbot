use coding_ingest::adapters::git_post_commit::GitPostCommitAdapter;
use coding_ingest::event::EventKind;

#[test]
fn parses_synthetic_payload() {
    let stdin = serde_json::json!({
        "commitHash":"abc123",
        "parentHash":"def456",
        "repoRoot":"/tmp/repo",
        "changedFiles":["src/lib.rs","src/main.rs"]
    })
    .to_string();
    let event = GitPostCommitAdapter::parse(stdin.as_bytes())
        .unwrap()
        .expect("event produced");
    let kind = match event {
        coding_ingest::event::AgentEvent::V1(v1) => v1.kind,
    };
    match kind {
        EventKind::GitCommit {
            commit_hash,
            parent_hash,
            repo_root,
            changed_files,
        } => {
            assert_eq!(commit_hash, "abc123");
            assert_eq!(parent_hash.as_deref(), Some("def456"));
            assert_eq!(repo_root.to_str(), Some("/tmp/repo"));
            assert_eq!(changed_files.len(), 2);
        }
        other => panic!("expected GitCommit, got {other:?}"),
    }
}

#[test]
fn rejects_invalid_json() {
    let result = GitPostCommitAdapter::parse(b"{not json");
    assert!(result.is_err());
}
