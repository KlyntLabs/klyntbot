use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::edit::{run_for_test as edit_run, EditArgs};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn edits_unique_match() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "alpha\nbeta\ngamma\n").unwrap();
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    edit_run(
        EditArgs {
            path: "f.txt".into(),
            old_text: "beta".into(),
            new_text: "BETA".into(),
        },
        dir.path().to_path_buf(),
        pol,
        pri,
        Some(tx),
        bus,
        CancellationToken::new(),
        Channel::Desktop,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
        None,
        false,
        5,
        86400,
        "".to_string(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
        "alpha\nBETA\ngamma\n"
    );
}

#[tokio::test]
async fn rejects_multiple_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "x\nx\n").unwrap();
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = edit_run(
        EditArgs {
            path: "f.txt".into(),
            old_text: "x".into(),
            new_text: "Y".into(),
        },
        dir.path().to_path_buf(),
        pol,
        pri,
        Some(tx),
        bus,
        CancellationToken::new(),
        Channel::Desktop,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
        None,
        false,
        5,
        86400,
        "".to_string(),
        None,
    )
    .await;
    assert!(r.is_err());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
        "x\nx\n"
    );
}

#[tokio::test]
async fn rejects_missing_old_text() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "abc\n").unwrap();
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = edit_run(
        EditArgs {
            path: "f.txt".into(),
            old_text: "missing".into(),
            new_text: "x".into(),
        },
        dir.path().to_path_buf(),
        pol,
        pri,
        Some(tx),
        bus,
        CancellationToken::new(),
        Channel::Desktop,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
        None,
        false,
        5,
        86400,
        "".to_string(),
        None,
    )
    .await;
    assert!(r.is_err());
}
