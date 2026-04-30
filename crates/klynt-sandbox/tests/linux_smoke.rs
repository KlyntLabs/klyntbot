// crates/klynt-sandbox/tests/linux_smoke.rs
#![cfg(target_os = "linux")]
use klynt_sandbox::{LinuxSandboxRunner, SandboxPolicy, SandboxRunner};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn echo_inside_bwrap_landlock() {
    let cwd = tempfile::tempdir().unwrap();
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let runner = match LinuxSandboxRunner::new() {
        Ok(r) => r,
        Err(_) => { eprintln!("sandbox unavailable; skipping"); return; }
    };
    let out = runner.run_command(
        &policy, "/bin/echo", &["hi-from-sandbox"],
        Some(cwd.path()), Duration::from_secs(5),
    ).await.expect("run completes");
    assert!(out.stdout.contains("hi-from-sandbox"));
    assert_eq!(out.exit_code, 0);
}

#[tokio::test]
async fn write_outside_cwd_blocked() {
    let cwd = tempfile::tempdir().unwrap();
    let outside = std::env::temp_dir().join(format!("klynt-l-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&outside).unwrap();
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let runner = LinuxSandboxRunner::new().unwrap();
    let cmd = format!("touch {}/forbidden 2>&1; echo done", outside.display());
    let out = runner.run_command(
        &policy, "/bin/sh", &["-c", &cmd],
        Some(cwd.path()), Duration::from_secs(5),
    ).await.unwrap();
    assert!(!outside.join("forbidden").exists(), "Landlock failed to block outside-cwd write");
    assert!(out.stdout.contains("done"));
}

#[tokio::test]
async fn timeout_kills_child() {
    let cwd = tempfile::tempdir().unwrap();
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let runner = LinuxSandboxRunner::new().unwrap();
    let r = runner.run_command(
        &policy, "/bin/sleep", &["999"],
        Some(cwd.path()), Duration::from_millis(100),
    ).await;
    assert!(matches!(r, Err(klynt_sandbox::SandboxError::ChildExit(124))));
}
