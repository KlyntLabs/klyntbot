//! Manage `[[hooks]]` block in Kimi's `~/.kimi/config.toml`.
//!
//! Kimi-cli's hook system mirrors Claude Code's exactly: identical 9 events
//! (PascalCase) with identical JSON payloads delivered via stdin. The only
//! configuration differences are: TOML format (vs Claude's JSON) and a
//! different config path.
//!
//! We delimit our managed entries with `# klyntbot-managed:start` and
//! `# klyntbot-managed:end` so user-written `[[hooks]]` blocks are preserved
//! across enable/disable cycles.

use common::{KlyntbotError, Result};
use std::path::Path;

const START: &str = "# klyntbot-managed:start";
const END: &str = "# klyntbot-managed:end";

/// Same 9 events as Claude Code — kimi-cli emits identical hook events.
const EVENTS: &[&str] = &[
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

/// Kimi `~/.kimi/config.toml` installer.
pub struct KimiInstaller;

impl KimiInstaller {
    /// Install klyntbot-managed `[[hooks]]` block into `~/.kimi/config.toml`.
    pub fn install(config_path: &Path, hook_binary: &Path) -> Result<()> {
        let existing = if config_path.exists() {
            std::fs::read_to_string(config_path)
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?
        } else {
            String::new()
        };
        if config_path.exists() {
            backup(config_path)?;
        }
        let user = strip_managed(&existing);
        let mut block = String::from(START);
        block.push('\n');
        for ev in EVENTS {
            block.push_str(&format!(
                "[[hooks]]\nevent = \"{ev}\"\ncommand = \"{} --hook kimi-cli {ev}\"\n\n",
                hook_binary.display()
            ));
        }
        block.push_str(END);
        block.push('\n');
        let body = if user.trim().is_empty() {
            block
        } else {
            format!("{}\n{block}", user.trim_end())
        };
        atomic_write(config_path, &body)
    }

    /// Remove klyntbot-managed block. Leaves user config intact.
    pub fn uninstall(config_path: &Path) -> Result<()> {
        if !config_path.exists() {
            return Ok(());
        }
        let body = std::fs::read_to_string(config_path)
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        atomic_write(config_path, &strip_managed(&body))
    }

    /// Run the binary with a synthetic payload to verify exit code 0.
    pub fn diagnose(hook_binary: &Path) -> Result<()> {
        use std::io::Write;
        let mut child = std::process::Command::new(hook_binary)
            .args(["--hook", "kimi-cli", "SessionStart"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| KlyntbotError::Storage(format!("spawn: {e}")))?;
        let body = br#"{"session_id":"diagnose","cwd":"/tmp","source":"diagnose"}"#;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(body)
            .map_err(|e| KlyntbotError::Storage(format!("stdin: {e}")))?;
        drop(child.stdin.take());
        let status = child
            .wait()
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        if !status.success() {
            return Err(KlyntbotError::Storage(format!("hook exit {status}")));
        }
        Ok(())
    }
}

/// Strip our managed block AND any top-level `hooks = [...]` line.
///
/// Kimi's default config seeds `hooks = []` (an inline empty array). TOML
/// treats `hooks = [...]` (inline array) and `[[hooks]]` (array-of-tables)
/// as conflicting definitions of the same key, so we must remove the inline
/// version before appending our `[[hooks]]` blocks. Detection is line-based
/// at the top level (column 0) — we deliberately don't strip indented lines
/// inside other tables.
fn strip_managed(s: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    let mut current_table_is_top = true;
    for line in s.lines() {
        let trimmed = line.trim();
        if trimmed == START {
            in_block = true;
            continue;
        }
        if trimmed == END {
            in_block = false;
            continue;
        }
        if in_block {
            continue;
        }
        // Track table headers — `[foo]` enters a sub-table, `[[foo]]` enters
        // an array-of-tables. We only want to drop `hooks = ...` at the
        // root level (no current table).
        if line.starts_with('[') && trimmed.ends_with(']') {
            current_table_is_top = false;
        }
        if current_table_is_top
            && line.starts_with("hooks")
            && line
                .trim_start_matches("hooks")
                .trim_start()
                .starts_with('=')
        {
            // Drop the inline `hooks = [...]` line entirely.
            continue;
        }
        out.push_str(line);
        out.push('\n');
    }
    out
}

fn atomic_write(p: &Path, body: &str) -> Result<()> {
    let tmp = p.with_extension("tmp");
    if let Some(parent) = p.parent() {
        std::fs::create_dir_all(parent).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    }
    std::fs::write(&tmp, body).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    std::fs::rename(&tmp, p).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}

fn backup(p: &Path) -> Result<()> {
    let bak = p.with_extension(format!("toml.bak.{}", jiff::Timestamp::now().as_second()));
    std::fs::copy(p, &bak).map_err(|e| KlyntbotError::Storage(e.to_string()))?;
    Ok(())
}
