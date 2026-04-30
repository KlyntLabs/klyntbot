use bus::DomainEventBus;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::edit::{run_for_test as edit_run, EditArgs};
use klynt_execpolicy::Policy;
use config::schema::CodingPermissions;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn edits_unique_match() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "alpha\nbeta\ngamma\n").unwrap();
    let perms = CodingPermissions {
        allow: vec!["Edit(./**)".into()], ..Default::default()
    };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    edit_run(
        EditArgs { path: "f.txt".into(), old_text: "beta".into(), new_text: "BETA".into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await.unwrap();
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "alpha\nBETA\ngamma\n");
}

#[tokio::test]
async fn rejects_multiple_matches() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "x\nx\n").unwrap();
    let perms = CodingPermissions { allow: vec!["Edit(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = edit_run(
        EditArgs { path: "f.txt".into(), old_text: "x".into(), new_text: "Y".into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await;
    assert!(r.is_err());
    assert_eq!(std::fs::read_to_string(dir.path().join("f.txt")).unwrap(), "x\nx\n");
}

#[tokio::test]
async fn rejects_missing_old_text() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "abc\n").unwrap();
    let perms = CodingPermissions { allow: vec!["Edit(./**)".into()], ..Default::default() };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = edit_run(
        EditArgs { path: "f.txt".into(), old_text: "missing".into(), new_text: "x".into() },
        dir.path().to_path_buf(), l1, pol, pri, pen, tx, bus, CancellationToken::new(),
    ).await;
    assert!(r.is_err());
}
