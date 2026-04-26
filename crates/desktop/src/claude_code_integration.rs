//! Auto-register klyntbot's MCP server with Claude Code on first launch.
//!
//! Runs once per install. If Claude Code's `claude` CLI is on `$PATH` and the
//! MCP entry isn't already registered, we shell out to `claude mcp add` with
//! this binary's absolute path. A zero-byte marker file in `~/.klyntbot/` records
//! that we've offered, so we don't run again — even if the user later removes
//! the registration. Manual re-trigger: delete the marker.
//!
//! Intentionally does NOT patch `~/.claude/settings.json` (Phase-4 hooks):
//! that's a user-editable file and warrants explicit consent. Instead we log a
//! copy-paste snippet for the user to add themselves.

use std::path::{Path, PathBuf};
use std::process::Command;

const MARKER_FILENAME: &str = ".claude-code-integration-offered";
const MCP_ENTRY_NAME: &str = "klyntbot";

/// Run the auto-register flow. Safe to call on every launch — short-circuits
/// quickly when not applicable. Errors are logged, never propagated.
pub fn run_first_launch_check() {
    if let Err(e) = try_run() {
        tracing::debug!("Claude Code auto-register skipped: {e}");
    }
}

fn try_run() -> Result<(), String> {
    let klyntbot_home = klyntbot_home_dir();
    let marker = klyntbot_home.join(MARKER_FILENAME);
    if marker.exists() {
        return Ok(());
    }

    let claude_home = home_dir().join(".claude");
    if !claude_home.exists() {
        // Claude Code not installed for this user; nothing to do.
        write_marker(&marker);
        return Ok(());
    }

    if !claude_cli_available() {
        log_manual_instructions(None);
        write_marker(&marker);
        return Ok(());
    }

    if mcp_already_registered() {
        write_marker(&marker);
        return Ok(());
    }

    let bin = current_binary_path().map_err(|e| format!("current_exe: {e}"))?;
    match register_mcp(&bin) {
        Ok(()) => {
            tracing::info!(
                bin = %bin.display(),
                "Registered klyntbot MCP server with Claude Code (claude mcp add {MCP_ENTRY_NAME})"
            );
            log_manual_instructions(Some(&bin));
        }
        Err(e) => {
            tracing::warn!("Failed to auto-register klyntbot MCP: {e}");
            log_manual_instructions(Some(&bin));
        }
    }

    write_marker(&marker);
    Ok(())
}

fn home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"))
}

fn klyntbot_home_dir() -> PathBuf {
    std::env::var_os("KLYNTBOT_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| home_dir().join(".klyntbot"))
}

fn current_binary_path() -> std::io::Result<PathBuf> {
    std::env::current_exe()
}

fn claude_cli_available() -> bool {
    Command::new("claude")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn mcp_already_registered() -> bool {
    let Ok(output) = Command::new("claude").arg("mcp").arg("list").output() else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    String::from_utf8_lossy(&output.stdout).contains(MCP_ENTRY_NAME)
}

fn register_mcp(bin: &Path) -> Result<(), String> {
    let bin_str = bin.to_string_lossy().to_string();
    let status = Command::new("claude")
        .args([
            "mcp",
            "add",
            MCP_ENTRY_NAME,
            "--",
            &bin_str,
            "mcp",
            "serve",
            "--stdio",
        ])
        .status()
        .map_err(|e| format!("spawn `claude mcp add`: {e}"))?;
    if !status.success() {
        return Err(format!("`claude mcp add` exited with {status}"));
    }
    Ok(())
}

fn write_marker(marker: &Path) {
    if let Some(parent) = marker.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let _ = std::fs::write(marker, b"");
}

fn log_manual_instructions(bin: Option<&Path>) {
    let bin_display = bin
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "/path/to/klyntbot".to_string());
    tracing::info!(
        "To enable Claude Code Phase-4 context injection, add to ~/.claude/settings.json:\n\
         {{\n  \"hooks\": {{\n    \"SessionStart\": [{{ \"hooks\": [{{ \"type\": \"command\", \"command\": \"{bin_display} --hook context --session-start\" }}] }}],\n    \"UserPromptSubmit\": [{{ \"hooks\": [{{ \"type\": \"command\", \"command\": \"{bin_display} --hook context --user-prompt-submit \\\"$CLAUDE_USER_PROMPT\\\"\" }}] }}]\n  }}\n}}"
    );
}
