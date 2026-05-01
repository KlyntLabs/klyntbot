#![cfg(target_os = "linux")]
use klynt_sandbox::{LinuxSandboxRunner, SandboxPolicy, SandboxRunner};
use std::path::PathBuf;
use std::time::Duration;

#[tokio::test]
async fn linux_bash_echo_inside_bwrap() {
    let cwd = tempfile::tempdir().unwrap();
    let runner = match LinuxSandboxRunner::new() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("sandbox unavailable: {e}; skipping");
            return;
        }
    };
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let out = runner
        .run_command(
            &policy,
            "/bin/bash",
            &["-c", "echo hello-from-linux-sandbox"],
            Some(cwd.path()),
            Duration::from_secs(5),
        )
        .await
        .expect("sandbox exec ok");
    assert!(out.stdout.contains("hello-from-linux-sandbox"));
    assert_eq!(out.exit_code, 0);
}

#[tokio::test]
async fn linux_bash_blocked_outside_cwd_write() {
    let cwd = tempfile::tempdir().unwrap();
    let runner = match LinuxSandboxRunner::new() {
        Ok(r) => r,
        Err(_) => return,
    };
    let outside = std::env::temp_dir().join(format!("klynt-l-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&outside).unwrap();
    let cmd = format!(
        "touch {}/forbidden 2>/dev/null; echo done",
        outside.display()
    );
    let policy = SandboxPolicy::cwd_writes_only(cwd.path().to_path_buf());
    let out = runner
        .run_command(
            &policy,
            "/bin/sh",
            &["-c", &cmd],
            Some(cwd.path()),
            Duration::from_secs(5),
        )
        .await
        .unwrap();
    assert!(!outside.join("forbidden").exists());
    assert!(out.stdout.contains("done"));
}
