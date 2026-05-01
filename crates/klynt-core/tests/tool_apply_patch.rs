use bus::DomainEventBus;
use common::tool_channel::{Channel, NonUiPolicy};
use config::schema::CodingPermissions;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use klynt_core::privacy::PrivacyGuard;
use klynt_core::tools::apply_patch::{run_for_test as patch_run, ApplyPatchArgs};
use klynt_execpolicy::Policy;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn applies_unified_diff() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "line1\nline2\nline3\n").unwrap();
    let patch = "--- f.txt\n+++ f.txt\n@@ -1,3 +1,3 @@\n line1\n-line2\n+LINE2\n line3\n";
    let perms = CodingPermissions {
        allow: vec!["ApplyPatch(./**)".into()],
        ..Default::default()
    };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    patch_run(
        ApplyPatchArgs {
            path: "f.txt".into(),
            patch: patch.into(),
        },
        dir.path().to_path_buf(),
        l1,
        pol,
        pri,
        pen,
        Some(tx),
        bus,
        CancellationToken::new(),
        Channel::Coding,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
        None,
    )
    .await
    .unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("f.txt")).unwrap(),
        "line1\nLINE2\nline3\n"
    );
}

#[tokio::test]
async fn rejects_malformed_patch() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("f.txt"), "abc\n").unwrap();
    let perms = CodingPermissions {
        allow: vec!["ApplyPatch(./**)".into()],
        ..Default::default()
    };
    let l1 = Arc::new(Layer1::compile(&perms).unwrap());
    let pol = Arc::new(Policy::empty());
    let pri = Arc::new(PrivacyGuard::from_globs(&[]).unwrap());
    let pen = Arc::new(PendingApprovalsMap::new());
    let bus = Arc::new(DomainEventBus::new(64));
    let (tx, _rx) = mpsc::channel(32);
    let r = patch_run(
        ApplyPatchArgs {
            path: "f.txt".into(),
            patch: "not a patch".into(),
        },
        dir.path().to_path_buf(),
        l1,
        pol,
        pri,
        pen,
        Some(tx),
        bus,
        CancellationToken::new(),
        Channel::Coding,
        NonUiPolicy::Allow,
        None,
        "".to_string(),
        None,
    )
    .await;
    assert!(r.is_err());
}
