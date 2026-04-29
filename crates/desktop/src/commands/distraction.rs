//! Distraction overlay IPC commands — thin Tauri delegates to `AppCore`.

use std::sync::Arc;

use desktop_macros::{klynt_command, klynt_raw_command};
use desktop_shared::commands::LearnedRuleResponse;
use desktop_shared::events::InterventionPayload;
use desktop_shared::CommandResult;
use tauri::State;

use crate::app_core::{self, AppCore};
use crate::focus_timer::FocusTimer;

#[klynt_command]
pub async fn distraction_dismiss(app_name: String) -> () {
    state.distraction_dismiss(app_name).await
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn distraction_allow_temp(
    state: State<'_, Arc<AppCore>>,
    timer: State<'_, Arc<FocusTimer>>,
    pattern: String,
) -> CommandResult<()> {
    // Pause the focus timer so the user doesn't lose focus time during their break
    use crate::focus_timer::SessionCommand;
    timer.send_command(SessionCommand::Pause).await;
    state.distraction_allow_temp(pattern).await
}

#[klynt_command]
pub async fn distraction_allow_session(
    app_name: String,
    window_title: Option<String>,
    classification: String,
) -> () {
    state
        .distraction_allow_session(app_name, window_title, classification)
        .await
}

#[klynt_command]
pub async fn distraction_learned_rules() -> Vec<LearnedRuleResponse> {
    state.distraction_learned_rules().await
}

#[klynt_command]
pub async fn distraction_delete_rule(id: i64) -> () {
    state.distraction_delete_rule(id).await
}

/// Pulled by the overlay's React layer on mount to recover the latest
/// alert if the push event was emitted before the listener was wired.
/// Returns `None` once the overlay has acked via `clear_pending`.
#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn distraction_get_pending_intervention() -> CommandResult<Option<InterventionPayload>> {
    Ok(app_core::take_pending_intervention())
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn distraction_clear_pending_intervention() -> CommandResult<()> {
    app_core::clear_pending_intervention();
    Ok(())
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
        "distraction_dismiss" => {
            let app_name = try_field!(dev::get_str(body, "app_name"));
            dev::val(core.distraction_dismiss(app_name).await)
        }
        "distraction_allow_temp" => {
            let pattern = try_field!(dev::get_str(body, "pattern"));
            dev::val(core.distraction_allow_temp(pattern).await)
        }
        "distraction_allow_session" => {
            let app_name = try_field!(dev::get_str(body, "app_name"));
            let classification = try_field!(dev::get_str(body, "classification"));
            dev::val(
                core.distraction_allow_session(
                    app_name,
                    dev::get(body, "window_title"),
                    classification,
                )
                .await,
            )
        }
        "distraction_learned_rules" => dev::val(core.distraction_learned_rules().await),
        "distraction_delete_rule" => {
            let id: i64 = try_field!(dev::require(body, "id"));
            dev::val(core.distraction_delete_rule(id).await)
        }
        "distraction_get_pending_intervention" => {
            let _ = core;
            dev::val(app_core::take_pending_intervention())
        }
        "distraction_clear_pending_intervention" => {
            let _ = core;
            app_core::clear_pending_intervention();
            dev::val(())
        }
        _ => return None,
    })
}
