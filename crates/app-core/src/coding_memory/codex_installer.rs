//! Manage `[[hooks]]` block in Codex's `config.toml`.

use common::{KlyntbotError, Result};
use std::path::Path;

const START: &str = "# klyntbot-managed:start";
const END: &str = "# klyntbot-managed:end";

const EVENTS: &[&str] = &["session.start", "user.prompt", "tool.pre", "tool.post", "session.end"];

/// Codex config.toml installer.
pub struct CodexInstaller;

impl CodexInstaller {
    /// Install klyntbot-managed hooks block.
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
                "[[hooks]]\nevent = \"{ev}\"\ncommand = \"{} codex {ev}\"\n\n",
                hook_binary.display()
            ));
        }
        block.push_str(END);
        block.push('\n');
        let body = if user.trim().is_empty() {
            block
        } else {
            format!("{user}\n{block}")
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
            .args(["codex", "session.start"])
            .stdin(std::process::Stdio::piped())
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
        std::fs::create_dir_all(parent)
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
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
