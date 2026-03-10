//! Distraction overlay IPC commands — thin Tauri delegates to `AppCore`.

use std::sync::Arc;

use desktop_shared::commands::LearnedRuleResponse;
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command(rename_all = "snake_case")]
pub async fn distraction_dismiss(
    state: State<'_, Arc<AppCore>>,
    app_name: String,
) -> Result<(), ApiError> {
    state.distraction_dismiss(app_name).await
}

#[tauri::command]
pub async fn distraction_allow_temp(
    state: State<'_, Arc<AppCore>>,
    pattern: String,
) -> Result<(), ApiError> {
    state.distraction_allow_temp(pattern).await
}

#[tauri::command(rename_all = "snake_case")]
pub async fn distraction_allow_session(
    state: State<'_, Arc<AppCore>>,
    app_name: String,
    window_title: Option<String>,
    classification: String,
) -> Result<(), ApiError> {
    state
        .distraction_allow_session(app_name, window_title, classification)
        .await
}

#[tauri::command]
pub async fn distraction_learned_rules(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<LearnedRuleResponse>, ApiError> {
    state.distraction_learned_rules().await
}

#[tauri::command]
pub async fn distraction_delete_rule(
    state: State<'_, Arc<AppCore>>,
    id: i64,
) -> Result<(), ApiError> {
    state.distraction_delete_rule(id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "distraction_dismiss",
    "distraction_allow_temp",
    "distraction_allow_session",
    "distraction_learned_rules",
    "distraction_delete_rule",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
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
        _ => return None,
    })
}
