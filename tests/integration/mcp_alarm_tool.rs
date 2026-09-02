//! MCP smoke test — verifies that `klyntbot mcp tools --list` boots the live
//! ToolRegistry, runs shared exposure validation, and lists effective tools.
//!
//! **Alarm tool gap (deferred):** No `AlarmTool` has been wired into the MCP
//! tool registry yet. Until an AlarmTool is built and registered this test
//! asserts `"cron"` / `"tasks"` (MCP Default registry tools) plus builtins.

/// Resolve the path to the `desktop` binary built by the `desktop` crate.
/// Cargo places integration test binaries in `target/debug/deps/`; the desktop
/// app binary is one level up at `target/debug/desktop`.
fn klyntbot_bin() -> std::path::PathBuf {
    let test_exe = std::env::current_exe().expect("current_exe");
    let deps_dir = test_exe.parent().expect("deps dir");
    let target_dir = deps_dir.parent().expect("target/debug/");
    let bin = target_dir.join("desktop");
    assert!(
        bin.exists(),
        "desktop binary not found at {}: run `cargo build -p desktop` first",
        bin.display()
    );
    bin
}

/// Verify that `klyntbot mcp tools --list` exits successfully and lists the
/// expected tools. Requires the `desktop` crate's `klyntbot` binary to be built.
#[test]
fn mcp_tools_list_exits_successfully() {
    let bin = klyntbot_bin();
    let out = std::process::Command::new(&bin)
        .args(["mcp", "tools", "--list"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    assert!(
        out.status.success(),
        "klyntbot mcp tools --list exited non-zero ({})\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);

    assert!(
        stdout.contains("runtime_state:"),
        "diagnostic missing runtime_state:\n{stdout}"
    );
    assert!(
        stdout.contains("get_status"),
        "\"get_status\" builtin missing from MCP output:\n{stdout}"
    );
    assert!(
        stdout.contains("agent"),
        "\"agent\" builtin missing from MCP default output:\n{stdout}"
    );
    // Core registry tools that must be present under auto-default.
    assert!(
        stdout.contains("tasks"),
        "\"tasks\" tool missing from MCP output:\n{stdout}"
    );
    assert!(
        stdout.contains("cron"),
        "\"cron\" tool missing from MCP output:\n{stdout}"
    );

    // Legacy AiFeature∪allowlist reconstruction must be gone.
    assert!(
        !stdout.contains("AiFeature"),
        "diagnostic must not reconstruct AiFeature allowlist:\n{stdout}"
    );
}
