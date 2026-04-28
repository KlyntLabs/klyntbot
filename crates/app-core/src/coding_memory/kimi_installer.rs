//! Manage klyntbot-managed block in Kimi's `hooks.json`.

use common::{KlyntbotError, Result};
use serde_json::{json, Value};
use std::path::Path;

const EVENTS: &[&str] = &[
    "session_start",
    "user_input",
    "thinking_start",
    "thinking_end",
    "tool_pre",
    "tool_post",
    "file_edit",
    "file_read",
    "assistant_msg",
    "compact_triggered",
    "error",
    "agent_pause",
    "session_end",
];

/// Kimi hooks.json installer.
pub struct KimiInstaller;

impl KimiInstaller {
    /// Install klyntbot-managed hooks block.
    pub fn install(config_path: &Path, hook_binary: &Path) -> Result<()> {
        let mut doc: Value = if config_path.exists() {
            serde_json::from_str(
                &std::fs::read_to_string(config_path)
                    .map_err(|e| KlyntbotError::Storage(e.to_string()))?,
            )
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?
        } else {
            json!({})
        };
        let entries: Vec<Value> = EVENTS
            .iter()
            .map(|ev| {
                json!({
                    "event": ev,
                    "command": format!("{} kimi-cli {ev}", hook_binary.display()),
                })
            })
            .collect();
        doc.as_object_mut()
            .unwrap()
            .insert("klyntbot".into(), Value::Array(entries));
        atomic_write(config_path, &serde_json::to_string_pretty(&doc).unwrap())
    }

    /// Remove klyntbot-managed block. Leaves user config intact.
    pub fn uninstall(config_path: &Path) -> Result<()> {
        if !config_path.exists() {
            return Ok(());
        }
        let mut doc: Value = serde_json::from_str(
            &std::fs::read_to_string(config_path)
                .map_err(|e| KlyntbotError::Storage(e.to_string()))?,
        )
        .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        if let Some(o) = doc.as_object_mut() {
            o.remove("klyntbot");
        }
        atomic_write(config_path, &serde_json::to_string_pretty(&doc).unwrap())
    }

    /// Run the binary with a synthetic payload to verify exit code 0.
    pub fn diagnose(hook_binary: &Path) -> Result<()> {
        use std::io::Write;
        let mut child = std::process::Command::new(hook_binary)
            .args(["kimi-cli", "session_start"])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(
                br#"{"event":"session_start","session":"diagnose","cwd":"/tmp","model":"diagnose"}"#,
            )
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        drop(child.stdin.take());
        let s = child
            .wait()
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;
        if !s.success() {
            return Err(KlyntbotError::Storage(format!("hook exit {s}")));
        }
        Ok(())
    }
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
