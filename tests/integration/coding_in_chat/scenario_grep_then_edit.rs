use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::{
    edit::{run_for_test as edit_run, EditArgs},
    grep::GrepTool,
};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::events::ToolEvent;
use tools_core::{RoutingContext, Tool};

#[tokio::test]
async fn grep_then_edit_emits_diff() {
    let dir = tempfile::tempdir().unwrap();
    let cwd = dir.path().to_path_buf();
    std::fs::write(cwd.join("f.rs"), "fn old_name() {}\nfn keep() {}\n").unwrap();

    // grep
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let grep = GrepTool::new(cwd.clone(), privacy.clone());
    let ctx = RoutingContext::new(
        common::ChannelName::new("system"),
        common::ChatId::new("test"),
    );
    let grep_out = grep
        .execute(serde_json::json!({"pattern":"old_name"}), &ctx)
        .await
        .unwrap();
    assert!(grep_out.contains("f.rs:1:fn old_name"));

    // edit
    let pol = Arc::new(Policy::empty());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);
    edit_run(
        EditArgs {
            path: "f.rs".into(),
            old_text: "old_name".into(),
            new_text: "new_name".into(),
        },
        cwd.clone(),
        pol,
        privacy,
        Some(tx.clone()),
        bus,
        CancellationToken::new(),
        Channel::Coding,
        NonUiPolicy::Allow,
        None,
        "scenario-test".to_string(),
        None,
        None,
        false,
        0,
        0,
        "".to_string(),
        None,
    )
    .await
    .unwrap();

    // assert FileEditWithSymbols emitted with op="edit"
    drop(tx);
    let mut saw = false;
    while let Some(e) = rx.recv().await {
        if let ToolEvent::FileEditWithSymbols {
            op,
            ref path,
            ref diff_full,
            ..
        } = e
        {
            assert_eq!(op, "edit");
            assert!(path.ends_with("f.rs"));
            assert!(diff_full.contains("-fn old_name"));
            assert!(diff_full.contains("+fn new_name"));
            saw = true;
        }
    }
    assert!(saw, "FileEditWithSymbols with op=edit must be emitted");
    assert_eq!(
        std::fs::read_to_string(cwd.join("f.rs")).unwrap(),
        "fn new_name() {}\nfn keep() {}\n"
    );
}
