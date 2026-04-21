//! macOS implementation of `FocusBridge` using the `shortcuts` CLI.

use async_trait::async_trait;
use common::{KlyntbotError, Result};

use crate::manager::FocusBridge;
use crate::FocusMode;

// ── Free helpers ──────────────────────────────────────────────────────────────

/// Run a named shortcut via `shortcuts run <name>`.
///
/// Non-zero exit code is converted to an `Io` error containing whatever the
/// process printed to stderr.
pub async fn run_shortcut(name: &str) -> Result<()> {
    let output = tokio::process::Command::new("shortcuts")
        .args(["run", name])
        .output()
        .await
        .map_err(|e| KlyntbotError::Io(e.to_string()))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Err(KlyntbotError::Io(format!(
        "shortcuts run '{name}' exited with {}: {stderr}",
        output.status
    )))
}

/// Check whether a shortcut with the given name is installed in the user's
/// Shortcuts library by running `shortcuts list`.
///
/// Returns `Ok(false)` on non-zero exit or empty output — best-effort, not an
/// error condition.
pub async fn is_shortcut_installed(name: &str) -> Result<bool> {
    let output = match tokio::process::Command::new("shortcuts")
        .arg("list")
        .output()
        .await
    {
        Ok(o) => o,
        Err(_) => return Ok(false),
    };

    if !output.status.success() {
        return Ok(false);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    Ok(stdout.lines().any(|l| l.trim() == name))
}

/// Write `bytes` to a temp file named `<name>.shortcut`, then open it in
/// Shortcuts.app so the user is prompted to add it to their library.
///
/// This function does **not** await Shortcuts.app completing — opening the
/// file is the UI gesture; completion is detected separately via
/// `is_shortcut_installed`.
pub async fn install_bundled_shortcut(name: &str, bytes: &[u8]) -> Result<()> {
    let path = std::env::temp_dir().join(format!("{name}.shortcut"));

    tokio::fs::write(&path, bytes)
        .await
        .map_err(|e| KlyntbotError::Io(e.to_string()))?;

    tokio::process::Command::new("open")
        .args(["-a", "Shortcuts", path.to_str().unwrap_or_default()])
        .spawn()
        .map_err(|e| KlyntbotError::Io(e.to_string()))?;

    Ok(())
}

// ── MacosFocusBridge ──────────────────────────────────────────────────────────

/// macOS `FocusBridge` that delegates DND toggling to the bundled Apple
/// Shortcuts via the `shortcuts` CLI.
pub struct MacosFocusBridge;

#[async_trait]
impl FocusBridge for MacosFocusBridge {
    async fn turn_on(&self, _mode: FocusMode) -> Result<()> {
        run_shortcut("Klyntbot Focus On").await
    }

    async fn turn_off(&self, _mode: FocusMode) -> Result<()> {
        run_shortcut("Klyntbot Focus Off").await
    }

    async fn is_ready(&self) -> Result<bool> {
        Ok(is_shortcut_installed("Klyntbot Focus On").await?
            && is_shortcut_installed("Klyntbot Focus Off").await?)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// Verify that querying a shortcut name that cannot exist returns Ok(false)
    /// rather than panicking or returning an error. This exercises the
    /// `shortcuts list` path without touching any real DND state.
    #[tokio::test]
    async fn is_shortcut_installed_returns_false_for_nonexistent() {
        let result = is_shortcut_installed("__klyntbot_test_nonexistent_xyz__").await;
        assert!(result.is_ok(), "should not error: {result:?}");
        assert!(!result.unwrap(), "nonexistent shortcut should not be found");
    }
}
