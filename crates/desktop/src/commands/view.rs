use std::sync::Arc;

use desktop_shared::commands::view::{ActiveViewResponse, SetActiveViewParams};
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn view_set_active(
    state: State<'_, Arc<AppCore>>,
    params: SetActiveViewParams,
) -> Result<(), ApiError> {
    state.view_set_active(params).await
}

#[tauri::command]
pub async fn view_clear_active(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    state.view_clear_active().await
}

#[tauri::command]
pub async fn view_get_active(
    state: State<'_, Arc<AppCore>>,
) -> Result<ActiveViewResponse, ApiError> {
    state.view_get_active().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] =
    &["view_set_active", "view_clear_active", "view_get_active"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "view_set_active" => dev::val(
            core.view_set_active(try_field!(dev::parse_params(body)))
                .await,
        ),
        "view_clear_active" => dev::val(core.view_clear_active().await),
        "view_get_active" => dev::val(core.view_get_active().await),
        _ => return None,
    })
}
