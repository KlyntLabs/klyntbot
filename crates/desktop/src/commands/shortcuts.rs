use app_core::AppCore;
use config::ShortcutsConfig;
use desktop_shared::{errors::ApiError, CommandResult};
use tauri::AppHandle;

use crate::shortcuts::register_shortcuts;
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn shortcuts_get() -> ShortcutsConfig {
    let cfg = state.config.read().await;
    Ok(cfg.shortcuts.clone())
}

#[klynt_command]
pub async fn shortcuts_update(app: AppHandle, launcher: String, tray: String) -> ShortcutsConfig {
    let shortcuts = ShortcutsConfig { launcher, tray };

    // Snapshot current config for rollback.
    let old_shortcuts = state.config.read().await.shortcuts.clone();

    // Register with the OS (validates + registers in two phases internally).
    if let Err(e) = register_shortcuts(&app, &shortcuts) {
        // Rollback — restore previous shortcuts.
        let _ = register_shortcuts(&app, &old_shortcuts);
        return Err(ApiError::new("SHORTCUT_REGISTRATION_FAILED", e));
    }

    // Persist to config.json (only after OS registration succeeds).
    let mut cfg = state.config.write().await;
    cfg.shortcuts = shortcuts.clone();
    config::save(&cfg)
        .await
        .map_err(|e| ApiError::new("CONFIG_SAVE_FAILED", e.to_string()))?;

    Ok(shortcuts)
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};

    Some(match cmd {
        "shortcuts_get" => {
            let cfg = core.config.read().await;
            dev::val(Ok::<_, ApiError>(cfg.shortcuts.clone()))
        }
        "shortcuts_update" => {
            // Dev mode: validate strings + persist config, but skip OS registration.
            let shortcuts = ShortcutsConfig {
                launcher: try_field!(dev::get_str(body, "launcher")),
                tray: try_field!(dev::get_str(body, "tray")),
            };
            // Validate shortcut strings parse (no OS registration in dev mode).
            for (name, value) in [("launcher", &shortcuts.launcher), ("tray", &shortcuts.tray)] {
                if let Err(e) = value.parse::<tauri_plugin_global_shortcut::Shortcut>() {
                    return Some(Err(ApiError::new(
                        "VALIDATION",
                        format!("invalid shortcut for {name}: '{value}' — {e}"),
                    )));
                }
            }
            let mut cfg = core.config.write().await;
            cfg.shortcuts = shortcuts.clone();
            match config::save(&cfg).await {
                Ok(()) => dev::val(Ok::<_, ApiError>(shortcuts)),
                Err(e) => Err(ApiError::new("CONFIG_SAVE_FAILED", e.to_string())),
            }
        }
        _ => return None,
    })
}
