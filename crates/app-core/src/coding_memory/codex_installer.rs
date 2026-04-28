//! Codex installer — **no hooks installed**.
//!
//! Codex CLI does not have a general-purpose hook system. Real codex only
//! supports a single `[notify]` block (one-shot fire when the agent finishes)
//! and would reject any other `hooks` keys with a TOML deserialisation error.
//!
//! Instead, codex writes rich session rollout JSONL files at
//! `~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<ts>-<sessionId>.jsonl`. The
//! ingestion daemon polls those files via [`crate::coding_memory::adapters::codex::poller`].
//!
//! `install` and `uninstall` here only remove any leftover
//! `# klyntbot-managed:start ... :end` block from `~/.codex/config.toml` —
//! such a block would have been written by an older buggy version of this
//! code and would cause codex to refuse to start. Diagnose pings the desktop
//! binary directly to confirm it can be invoked as a hook (since the codex
//! adapter still parses for legacy reasons, even though no hooks are wired).

use common::{KlyntbotError, Result};
use std::path::Path;

const START: &str = "# klyntbot-managed:start";
const END: &str = "# klyntbot-managed:end";

/// Codex installer — strips any legacy hook block but writes nothing new.
pub struct CodexInstaller;

impl CodexInstaller {
    /// Strip any leftover klyntbot-managed `[[hooks]]` block from
    /// `~/.codex/config.toml`. Codex hook installation is intentionally
    /// disabled; codex ingestion is driven by the JSONL session-log poller.
    pub fn install(config_path: &Path, _hook_binary: &Path) -> Result<()> {
        if !config_path.exists() {
            return Ok(());
        }
        let body = std::fs::read_to_string(config_path)
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        let cleaned = strip_managed(&body);
        if cleaned != body {
            backup(config_path)?;
            atomic_write(config_path, &cleaned)?;
        }
        Ok(())
    }

    /// Same as `install` — codex never has a managed block to remove beyond
    /// what `strip_managed` finds.
    pub fn uninstall(config_path: &Path) -> Result<()> {
        Self::install(config_path, Path::new(""))
    }

    /// Verify the desktop binary is invokable. Codex doesn't actually run our
    /// hooks (we don't install any), but if the user clicks "Diagnose" we
    /// still want to confirm the binary works.
    pub fn diagnose(hook_binary: &Path) -> Result<()> {
        use std::io::Write;
        let mut child = std::process::Command::new(hook_binary)
            .args(["--hook", "codex", "SessionStart"])
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| KlyntbotError::Storage(format!("spawn: {e}")))?;
        let body = br#"{"session_id":"diagnose","cwd":"/tmp","model":"diagnose"}"#;
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

fn strip_managed(s: &str) -> String {
    let mut out = String::new();
    let mut in_block = false;
    for line in s.lines() {
        if line.trim() == START {
            in_block = true;
            continue;
        }
        if line.trim() == END {
            in_block = false;
            continue;
        }
        if !in_block {
            out.push_str(line);
            out.push('\n');
        }
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
