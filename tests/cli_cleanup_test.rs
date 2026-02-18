//! Phase A: CLI Cleanup Verification Tests
//!
//! Verifies that removed CLI subcommands produce clap errors,
//! kept commands still work, and the after_help hint is present.

use assert_cmd::Command;

// ---------------------------------------------------------------------------
// A.1 — Deleted commands return clap error (not panic)
// ---------------------------------------------------------------------------

#[test]
fn cli_todo_subcommand_removed() {
    // Given: the klyntbot binary is built
    // When: user runs `klyntbot todo list`
    // Then: clap returns an error (exit code != 0), does not panic
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("todo").arg("list").assert();
    assert.failure(); // TODO: verify stderr contains "unrecognized subcommand" or similar
}

#[test]
fn cli_project_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("project").arg("list").assert();
    assert.failure();
}

#[test]
fn cli_goal_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("goal").arg("list").assert();
    assert.failure();
}

#[test]
fn cli_plan_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("plan").arg("list").assert();
    assert.failure();
}

#[test]
fn cli_calendar_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("calendar").arg("reconcile").assert();
    assert.failure();
}

#[test]
fn cli_channels_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("channels").arg("list").assert();
    assert.failure();
}

#[test]
fn cli_cron_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("cron").arg("list").assert();
    assert.failure();
}

#[test]
fn cli_config_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("config").arg("show").assert();
    assert.failure();
}

#[test]
fn cli_skills_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("skills").arg("list").assert();
    assert.failure();
}

#[test]
fn cli_usage_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("usage").arg("report").assert();
    assert.failure();
}

#[test]
fn cli_learning_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("learning").arg("status").assert();
    assert.failure();
}

#[test]
fn cli_provider_subcommand_removed() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("provider").arg("status").assert();
    assert.failure();
}

// ---------------------------------------------------------------------------
// A.2 — Kept commands still work
// ---------------------------------------------------------------------------

#[test]
fn cli_chat_subcommand_exists() {
    // Given: the klyntbot binary
    // When: user runs `klyntbot chat --help`
    // Then: exit code 0, output contains "chat"
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("chat").arg("--help").assert();
    assert.success();
}

#[test]
fn cli_serve_subcommand_exists() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("serve").arg("--help").assert();
    assert.success();
}

#[test]
fn cli_init_subcommand_exists() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("init").arg("--help").assert();
    assert.success();
}

#[test]
fn cli_status_subcommand_exists() {
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let assert = cmd.arg("status").arg("--help").assert();
    assert.success();
}

// ---------------------------------------------------------------------------
// A.3 — After-help hint
// ---------------------------------------------------------------------------

#[test]
fn cli_help_shows_chat_hint() {
    // Given: the klyntbot binary
    // When: user runs `klyntbot --help`
    // Then: output contains the chat-first hint text
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let output = cmd.arg("--help").output().unwrap();
    let stdout = String::from_utf8_lossy(&output.stdout);
    // TODO: assert!(stdout.contains("available via natural language in 'klyntbot chat'"));
    let _ = stdout; // placeholder until after_help is added
}

#[test]
fn cli_error_shows_after_help() {
    // Given: the klyntbot binary
    // When: user runs `klyntbot nonexistent`
    // Then: stderr contains after_help hint about chat
    let mut cmd = Command::cargo_bin("klyntbot").unwrap();
    let output = cmd.arg("nonexistent").output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);
    // TODO: assert!(stderr.contains("klyntbot chat"));
    let _ = stderr;
}

// ---------------------------------------------------------------------------
// A.4 — Commands enum structure (compile-time verification)
// ---------------------------------------------------------------------------

// NOTE: These tests verify the Commands enum at the type level.
// They will be written as unit tests inside crates/cli/src/commands.rs
// once the cleanup is done. Skeleton here for tracking.

#[test]
fn commands_enum_parse_chat() {
    // TODO: use clap::Parser to verify Commands::Chat variant parses
    // Commands::try_parse_from(["klyntbot", "chat"]).unwrap();
}

#[test]
fn commands_enum_parse_serve() {
    // TODO: Commands::try_parse_from(["klyntbot", "serve"]).unwrap();
}

#[test]
fn commands_enum_reject_todo() {
    // TODO: Commands::try_parse_from(["klyntbot", "todo", "list"]).unwrap_err();
}

#[test]
fn commands_enum_has_four_variants() {
    // TODO: Compile-time check that only Chat, Serve, Init, Status exist.
    // Can verify by exhaustive match in a helper function.
}
