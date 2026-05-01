use bus::DomainEventBus;
use config::schema::CodingPermissions;
use klynt_core::approval::{Layer1, PendingApprovalsMap};
use common::tool_channel::{Channel, NonUiPolicy};
use klynt_core::tools::{
    apply_patch::{run_for_test as patch_run, ApplyPatchArgs},
    edit::{run_for_test as edit_run, EditArgs},
    write::{run_for_test as write_run, WriteArgs},
};
use klynt_execpolicy::Policy;
use tools_core::events::ToolEvent;
use proptest::prelude::*;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

fn perms() -> CodingPermissions {
    CodingPermissions {
        allow: vec![
            "Write(./**)".into(),
            "Edit(./**)".into(),
            "ApplyPatch(./**)".into(),
        ],
        ..Default::default()
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 16, .. ProptestConfig::default() })]
    #[test]
    fn k5_each_mutation_emits_exactly_one_event(
        op_idx in 0u8..3,
        content in r"[a-z\n]{1,100}",
    ) {
        tokio_test::block_on(async move {
            let dir = tempfile::tempdir().unwrap();
            std::fs::write(dir.path().join("f.txt"), "seed\n").unwrap();
            let l1 = Arc::new(Layer1::compile(&perms()).unwrap());
            let pol = Arc::new(Policy::empty());
            let pri = Arc::new(klynt_core::privacy::PrivacyGuard::from_globs(&[]).unwrap());
            let pen = Arc::new(PendingApprovalsMap::new());
            let bus = Arc::new(DomainEventBus::new(64));
            let (tx, mut rx) = mpsc::channel(64);

            match op_idx {
                0 => { write_run(WriteArgs { path: "f.txt".into(), content: content.clone() },
                        dir.path().to_path_buf(), l1, pol, pri, pen, Some(tx), bus, CancellationToken::new(),
                        Channel::Coding, NonUiPolicy::Allow, None, "k5-test".to_string())
                        .await.ok(); }
                1 => { edit_run(EditArgs { path: "f.txt".into(),
                            old_text: "seed".into(), new_text: content.clone() },
                        dir.path().to_path_buf(), l1, pol, pri, pen, Some(tx), bus, CancellationToken::new(),
                        Channel::Coding, NonUiPolicy::Allow, None, "k5-test".to_string())
                        .await.ok(); }
                2 => {
                    let patch = "--- f.txt\n+++ f.txt\n@@ -1 +1 @@\n-seed\n+changed\n".to_string();
                    patch_run(ApplyPatchArgs { path: "f.txt".into(), patch },
                        dir.path().to_path_buf(), l1, pol, pri, pen, Some(tx), bus, CancellationToken::new(),
                        Channel::Coding, NonUiPolicy::Allow, None, "k5-test".to_string())
                        .await.ok();
                }
                _ => unreachable!(),
            }

            let mut count = 0;
            while let Ok(e) = rx.try_recv() {
                if matches!(e, ToolEvent::FileEditWithSymbols { .. }) { count += 1; }
            }
            prop_assert!(count == 1 || count == 0,
                "expected exactly 1 FileEditWithSymbols (or 0 if op failed), got {count}");
            Ok::<(), TestCaseError>(())
        }).unwrap();
    }
}
