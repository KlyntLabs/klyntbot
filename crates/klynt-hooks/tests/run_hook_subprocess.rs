use klynt_hooks::engine::command_runner::{run_command, CommandRunResult};
use klynt_hooks::schema::Hook;
use klynt_protocol::HookEventName;
use std::fs;
use tempfile::TempDir;

#[tokio::test]
async fn hook_subprocess_runs_and_returns_stdout() {
    let dir = TempDir::new().unwrap();
    let script = dir.path().join("hello.sh");
    fs::write(
        &script,
        "#!/usr/bin/env bash\nread input\necho \"got=$input\"\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let hook = Hook {
        event: HookEventName::PreToolUse,
        matcher: None,
        command: script.to_string_lossy().into_owned(),
        timeout_ms: Some(5000),
        fail_open: Some(true),
    };
    let input = serde_json::json!({"tool":"bash", "args": {"command": "ls"}}).to_string();
    let res: CommandRunResult = run_command(&hook, &input).await;
    assert_eq!(res.exit_code, Some(0));
    assert!(res.stdout.contains("got="));
}

#[tokio::test]
async fn hook_subprocess_times_out_at_configured_limit() {
    let dir = TempDir::new().unwrap();
    let script = dir.path().join("slow.sh");
    fs::write(&script, "#!/usr/bin/env bash\nsleep 10\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let hook = Hook {
        event: HookEventName::PreToolUse,
        matcher: None,
        command: script.to_string_lossy().into_owned(),
        timeout_ms: Some(100),
        fail_open: Some(true),
    };
    let res = run_command(&hook, "{}").await;
    assert!(res.error.as_deref().unwrap_or("").contains("timed out"));
    assert_eq!(res.exit_code, None);
}
