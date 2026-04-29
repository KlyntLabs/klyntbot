//! Manage klyntbot's managed entries in `~/.claude/settings.json`.
//!
//! Install semantics: read → merge → atomic write. Entries are identified by
//! a fixed matcher string (`klyntbot-managed`) so we can remove them cleanly
//! on uninstall without touching user-written hooks.

use common::{KlyntbotError, Result};
use jiff::Timestamp;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};

const MATCHER_TAG: &str = "klyntbot-managed";
const HOOK_EVENTS: [&str; 9] = [
    "SessionStart",
    "SessionEnd",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "Stop",
    "SubagentStop",
    "PreCompact",
    "Notification",
];

/// Claude Code settings installer.
pub struct ClaudeCodeInstaller;

impl ClaudeCodeInstaller {
    /// Install klyntbot-managed hook entries. Creates a backup of the original
    /// if the settings file already existed.
    pub fn install(settings_path: &Path, hook_binary: &Path) -> Result<()> {
        let mut doc: Value = read_or_empty(settings_path)?;
        if settings_path.exists() {
            backup(settings_path)?;
        }
        let hooks = doc.get_mut("hooks").and_then(Value::as_object_mut).cloned();
        let mut hooks = hooks.unwrap_or_default();

        for event in HOOK_EVENTS {
            let arr = hooks
                .entry(event.to_string())
                .or_insert_with(|| Value::Array(vec![]))
                .as_array_mut()
                .ok_or_else(|| KlyntbotError::Storage(format!("hooks[{event}] not array")))?;
            arr.retain(|entry| entry.get("matcher").and_then(|m| m.as_str()) != Some(MATCHER_TAG));
            arr.push(json!({
                "matcher": MATCHER_TAG,
                "hooks": [{
                    "type": "command",
                    "command": build_hook_command(hook_binary, "claude-code", event),
                }]
            }));
        }
        doc["hooks"] = Value::Object(hooks);
        atomic_write(settings_path, &doc)
    }

    /// Remove klyntbot-managed entries. Leaves user entries intact.
    pub fn uninstall(settings_path: &Path) -> Result<()> {
        if !settings_path.exists() {
            return Ok(());
        }
        let mut doc: Value = read_or_empty(settings_path)?;
        if let Some(hooks) = doc.get_mut("hooks").and_then(Value::as_object_mut) {
            for event in HOOK_EVENTS {
                if let Some(arr) = hooks.get_mut(event).and_then(Value::as_array_mut) {
                    arr.retain(|entry| {
                        entry.get("matcher").and_then(|m| m.as_str()) != Some(MATCHER_TAG)
                    });
                    // Drop the hook-event key entirely if no matchers remain —
                    // avoids littering settings.json with empty arrays.
                    if arr.is_empty() {
                        hooks.remove(event);
                    }
                }
            }
            // Drop the top-level "hooks" key if every event was emptied.
            if hooks.is_empty() {
                doc.as_object_mut().map(|m| m.remove("hooks"));
            }
        }
        atomic_write(settings_path, &doc)
    }

    /// Run the binary with a synthetic payload to verify exit code 0.
    pub fn diagnose(hook_binary: &Path) -> Result<()> {
        use std::io::Write;
        let mut child = std::process::Command::new(hook_binary)
            .args(["--hook", "claude-code", "SessionStart"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| KlyntbotError::Storage(format!("spawn hook: {e}")))?;
        let body = br#"{"session_id":"diagnose","cwd":"/tmp","source":"diagnose"}"#;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(body)
            .map_err(|e| KlyntbotError::Storage(format!("write stdin: {e}")))?;
        let status = child
            .wait()
            .map_err(|e| KlyntbotError::Storage(format!("wait: {e}")))?;
        if !status.success() {
            return Err(KlyntbotError::Storage(format!(
                "hook exited {}",
                status.code().unwrap_or(-1)
            )));
        }
        Ok(())
    }
}

/// Build the shell command string written into the hook config.
///
/// If `KLYNTBOT_HOME` is set in the desktop app's process environment (e.g.
/// dev runs with `KLYNTBOT_HOME=~/.klyntbot-dev`), bake it into the command
/// via `/usr/bin/env` so the hook writes to the same data dir the desktop
/// reads from — regardless of which shell launches the CLI later.
pub(crate) fn build_hook_command(hook_binary: &Path, source: &str, event: &str) -> String {
    let bin = hook_binary.display();
    match std::env::var("KLYNTBOT_HOME").ok().filter(|v| !v.trim().is_empty()) {
        Some(home) => {
            let home = shell_escape(&home);
            format!("/usr/bin/env KLYNTBOT_HOME={home} {bin} --hook {source} {event}")
        }
        None => format!("{bin} --hook {source} {event}"),
    }
}

fn shell_escape(s: &str) -> String {
    if s.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.' | '~')) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', "'\\''"))
    }
}

fn read_or_empty(path: &Path) -> Result<Value> {
    if !path.exists() {
        return Ok(json!({}));
    }
    let raw = std::fs::read_to_string(path)
        .map_err(|e| KlyntbotError::Storage(format!("read settings: {e}")))?;
    if raw.trim().is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_str(&raw).map_err(|e| KlyntbotError::Storage(format!("parse settings: {e}")))
}

fn backup(path: &Path) -> Result<()> {
    let ts = Timestamp::now().as_millisecond();
    let bak: PathBuf = path.with_extension(format!("json.klyntbot-backup.{ts}"));
    std::fs::copy(path, &bak).map_err(|e| KlyntbotError::Storage(format!("backup: {e}")))?;
    Ok(())
}

fn atomic_write(path: &Path, doc: &Value) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| KlyntbotError::Storage(format!("mkdir: {e}")))?;
    }
    let body = serde_json::to_vec_pretty(doc)
        .map_err(|e| KlyntbotError::Storage(format!("serialize: {e}")))?;
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    std::fs::write(&tmp, &body).map_err(|e| KlyntbotError::Storage(format!("write tmp: {e}")))?;
    std::fs::rename(&tmp, path).map_err(|e| KlyntbotError::Storage(format!("rename: {e}")))?;
    Ok(())
}
