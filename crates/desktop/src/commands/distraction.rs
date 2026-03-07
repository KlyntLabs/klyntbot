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
