//! macOS Do Not Disturb — read state via `defaults read`, toggle via
//! `shortcuts run "Toggle Do Not Disturb"`.
//!
//! Both functions are no-ops on non-macOS platforms.

/// Check whether macOS Focus / Do Not Disturb is currently active.
///
/// Reads `com.apple.controlcenter` defaults. Returns `false` on non-macOS
/// or if the read fails (conservative: assume DND is off).
#[cfg(target_os = "macos")]
pub fn is_dnd_active() -> bool {
    use std::process::Command;

    let output = Command::new("defaults")
        .args([
            "read",
            "com.apple.controlcenter",
            "NSStatusItem Visible FocusModes",
        ])
        .output();

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            stdout.trim() == "1"
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "macos"))]
pub fn is_dnd_active() -> bool {
    false
}

/// Toggle macOS Do Not Disturb via a Shortcuts automation.
///
/// Requires a user-created Shortcut named "Toggle Do Not Disturb".
/// Returns `Err` if the shortcut is not found or execution fails.
#[cfg(target_os = "macos")]
pub fn toggle_dnd() -> Result<(), String> {
    use std::process::Command;

    let output = Command::new("shortcuts")
        .args(["run", "Toggle Do Not Disturb"])
        .output()
        .map_err(|e| format!("Failed to run shortcuts command: {e}"))?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(format!(
            "Shortcut 'Toggle Do Not Disturb' failed (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ))
    }
}

#[cfg(not(target_os = "macos"))]
pub fn toggle_dnd() -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_dnd_active_returns_bool() {
        // Should not panic on any platform
        let _active = is_dnd_active();
    }

    #[test]
    fn toggle_dnd_returns_result() {
        // On CI / non-macOS this is a no-op Ok(())
        // On macOS without the shortcut, it returns Err — which is fine
        let _result = toggle_dnd();
    }
}
