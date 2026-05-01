use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use config::schema::CodingPermissions;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::write::{run_for_test as write_run, WriteArgs};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tools_core::events::ToolEvent;

fn allow_all_perms() -> CodingPermissions {
    CodingPermissions {
        allow: vec![
            "Write(./**)".into(),
            "Edit(./**)".into(),
            "ApplyPatch(./**)".into(),
        ],
        ..Default::default()
    }
}

#[tokio::test]
async fn writes_file_and_emits_event() {
    let dir = tempfile::tempdir().unwrap();
    let layer1 = Arc::new(Layer1::compile(&allow_all_perms()).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, mut rx) = mpsc::channel(32);

    let cwd = dir.path().to_path_buf();
    let res = write_run(
        WriteArgs {
            path: "out.txt".into(),
            content: "hello write".into(),
        },
        cwd.clone(),
        layer1,
        policy,
        privacy,
        pending,
        Some(tx.clone()),
        bus,
        CancellationToken::new(),
        Channel::Coding,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
    )
    .await
    .unwrap();

    assert!(res.contains("wrote"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("out.txt")).unwrap(),
        "hello write"
    );

    drop(tx);
    let mut saw_edit = false;
    while let Some(e) = rx.recv().await {
        if let ToolEvent::FileEditWithSymbols { op, path, .. } = e {
            assert_eq!(op, "write");
            assert!(path.ends_with("out.txt"));
            saw_edit = true;
        }
    }
    assert!(saw_edit, "FileEditWithSymbols must be emitted");
}

#[tokio::test]
async fn outside_cwd_denied_no_write_no_event() {
    let dir = tempfile::tempdir().unwrap();
    let layer1 = Arc::new(Layer1::compile(&allow_all_perms()).unwrap());
    let policy = Arc::new(Policy::empty());
    let privacy = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pending = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = write_run(
        WriteArgs {
            path: "/etc/passwd".into(),
            content: "x".into(),
        },
        dir.path().to_path_buf(),
        layer1,
        policy,
        privacy,
        pending,
        Some(tx),
        bus,
        CancellationToken::new(),
        Channel::Coding,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
    )
    .await;
    assert!(r.is_err());
}
