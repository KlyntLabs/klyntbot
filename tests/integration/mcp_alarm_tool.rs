//! MCP smoke test — verifies that the klyntbot-mcp binary lists the expected
//! tools via `tools --list`.
//!
//! **Alarm tool gap (deferred):** No `AlarmTool` has been wired into the MCP
//! tool registry yet; `"alarm"` does not appear in `default_exposed_tools()`.
//! Until an AlarmTool is built and registered this test instead asserts the
//! presence of `"cron"` (the scheduling-adjacent tool that is exposed) and
//! `"tasks"` (core task tool).  A TODO marks the alarm assertion so it can be
//! uncommented when AlarmTool lands.

/// Resolve the path to the klyntbot-mcp binary.  The binary lives in the
/// workspace target directory under `debug/` (or `release/` for release
/// builds).  We locate it relative to the integration test binary itself,
/// which Cargo places in `target/debug/deps/`.
fn klyntbot_mcp_bin() -> std::path::PathBuf {
    // The integration test binary is at target/debug/deps/<name>-<hash>.
    // Walk up two levels to reach target/debug/, then look for klyntbot-mcp.
    let test_exe = std::env::current_exe().expect("current_exe");
    let deps_dir = test_exe.parent().expect("deps dir");
    // deps/ → debug/
    let target_dir = deps_dir.parent().expect("target/debug/");
    let bin = target_dir.join("klyntbot-mcp");
    assert!(
        bin.exists(),
        "klyntbot-mcp binary not found at {}: run `cargo build -p klyntbot-mcp` first",
        bin.display()
    );
    bin
}

/// Verify that `klyntbot-mcp tools --list` exits successfully and lists the
/// expected tools.  This requires the `klyntbot-mcp` binary to be built
/// (`cargo build -p klyntbot-mcp`).
#[test]
fn mcp_tools_list_exits_successfully() {
    let bin = klyntbot_mcp_bin();
    let out = std::process::Command::new(&bin)
        .args(["tools", "--list"])
        .output()
        .unwrap_or_else(|e| panic!("failed to run {}: {e}", bin.display()));

    assert!(
        out.status.success(),
        "klyntbot-mcp tools --list exited non-zero ({})\nstderr: {}",
        out.status,
        String::from_utf8_lossy(&out.stderr),
    );

    let stdout = String::from_utf8_lossy(&out.stdout);

    // Core tools that must always be present.
    assert!(
        stdout.contains("tasks"),
        "\"tasks\" tool missing from MCP output:\n{stdout}"
    );
    assert!(
        stdout.contains("cron"),
        "\"cron\" tool missing from MCP output:\n{stdout}"
    );

    // TODO(alarm-tool): uncomment once AlarmTool is built and registered in
    // `default_exposed_tools()` in `crates/config/src/schema/mcp.rs`:
    //
    // assert!(
    //     stdout.contains("alarm"),
    //     "\"alarm\" tool missing from MCP output:\n{stdout}"
    // );
}
