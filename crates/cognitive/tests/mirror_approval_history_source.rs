use cognitive::mirror::sources::approval_history::ApprovalHistorySource;
use storage::repos::CodingApprovalHistoryRepo;
use storage::StoragePool;
use tools_core::events::ToolEvent;

#[tokio::test]
async fn records_resolved_approval_into_history_repo() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = CodingApprovalHistoryRepo::new(pool.inner().clone());
    let source = ApprovalHistorySource::new(repo.clone());

    // Synthesize a paired ApprovalRequested + ApprovalResolved as a fake stream
    let req = ToolEvent::ApprovalRequested {
        request_id: "r1".into(),
        tool: "bash".into(),
        args_hash: "hash-of-bash-args".into(),
        layer: "ask".into(),
        rule_matched: None,
        mirror_history: None,
        sandbox_summary: "none".into(),
        requires_user_input: true,
        args: Some(serde_json::json!({"command":"git status"})),
        cwd: Some("/tmp".into()),
        layer_reason: Some("ask".into()),
    };
    let res = ToolEvent::ApprovalResolved {
        request_id: "r1".into(),
        decision: "allow".into(),
        decision_reason: "user".into(),
        latency_ms: 10,
        persisted_rule: None,
        decided_by: "user".into(),
    };
    source.observe(&req, "test-repo").await;
    source.observe(&res, "test-repo").await;
    let s = repo
        .summary("bash", "hash-of-bash-args", "test-repo")
        .await
        .unwrap();
    assert_eq!(s.approval_count, 1);
}
