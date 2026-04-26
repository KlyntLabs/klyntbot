//! MCP smoke test — verifies that the merged `klyntbot mcp tools --list`
//! subcommand lists the expected tools.
//!
//! **Alarm tool gap (deferred):** No `AlarmTool` has been wired into the MCP
//! tool registry yet; `"alarm"` does not appear in `default_exposed_tools()`.
//! Until an AlarmTool is built and registered this test instead asserts the
//! presence of `"cron"` (the scheduling-adjacent tool that is exposed) and
//! `"tasks"` (core task tool).  A TODO marks the alarm assertion so it can be
//! uncommented when AlarmTool lands.

/// Resolve the path to the `klyntbot` binary built by the `desktop` crate.
/// Cargo places integration test binaries in `target/debug/deps/`; the desktop
/// app binary is one level up at `target/debug/klyntbot`.
fn klyntbot_bin() -> std::path::PathBuf {
    let test_exe = std::env::current_exe().expect("current_exe");
    let deps_dir = test_exe.parent().expect("deps dir");
    let target_dir = deps_dir.parent().expect("target/debug/");
    let bin = target_dir.join("klyntbot");
    assert!(
        bin.exists(),
        "klyntbot binary not found at {}: run `cargo build -p desktop` first",
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
