#![cfg(target_os = "macos")]
use klynt_sandbox::{MacOsSeatbeltRunner, SandboxPolicy, SandboxRunner};
use std::path::PathBuf;

#[tokio::test]
async fn echo_hi_runs_inside_seatbelt() {
    let policy = SandboxPolicy::cwd_writes_only(PathBuf::from("/tmp"));
    let runner = MacOsSeatbeltRunner::new();
    let out = runner
        .run_command(
            &policy,
            "/bin/echo",
            &["hi"],
            None,
            std::time::Duration::from_secs(5),
        )
        .await
        .expect("seatbelt run failed");
    assert!(out.stdout.contains("hi"));
    assert_eq!(out.exit_code, 0);
}

#[tokio::test]
async fn write_outside_cwd_blocked() {
    std::fs::create_dir_all("/private/tmp/klynt-seatbelt-test").ok();
    std::fs::create_dir_all("/private/tmp/klynt-forbidden-elsewhere").ok();
    let policy = SandboxPolicy::cwd_writes_only(PathBuf::from("/private/tmp/klynt-seatbelt-test"));
    let runner = MacOsSeatbeltRunner::new();
    let out = runner
        .run_command(
            &policy,
            "/bin/bash",
            &[
                "-c",
                "touch /private/tmp/klynt-forbidden-elsewhere/x 2>&1; echo done",
            ],
            None,
            std::time::Duration::from_secs(5),
        )
        .await
        .expect("run completes");
    assert!(out.stdout.contains("done"));
    assert!(out.stdout.contains("Operation not permitted") || out.stdout.contains("denied"));
}
