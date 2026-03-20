use std::sync::Arc;

use app_core::handlers::autotuner::AutoTunerStatus;
use autotuner::{ChampionSummary, ExperimentSummary};
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "autotuner_status",
    "autotuner_history",
    "autotuner_revert",
    "autotuner_pause",
    "autotuner_resume",
    "autotuner_set_pace",
];

#[tauri::command]
pub async fn autotuner_status(state: State<'_, Arc<AppCore>>) -> Result<AutoTunerStatus, ApiError> {
    state.autotuner_status().await
}

#[tauri::command]
pub async fn autotuner_history(
    state: State<'_, Arc<AppCore>>,
    limit: Option<u32>,
) -> Result<Vec<ExperimentSummary>, ApiError> {
    state.autotuner_history(limit.unwrap_or(20)).await
}

#[tauri::command]
pub async fn autotuner_revert(state: State<'_, Arc<AppCore>>) -> Result<ChampionSummary, ApiError> {
    state.autotuner_revert().await
}

#[tauri::command]
pub async fn autotuner_pause(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    state.autotuner_pause().await
}

#[tauri::command]
pub async fn autotuner_resume(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    state.autotuner_resume().await
}

#[tauri::command]
pub async fn autotuner_set_pace(
    state: State<'_, Arc<AppCore>>,
    pace: String,
) -> Result<(), ApiError> {
    state.autotuner_set_pace(&pace).await
}

// ── Dev server dispatch ──────────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "autotuner_status" => dev::val(core.autotuner_status().await),
        "autotuner_history" => {
            let limit: Option<u32> = dev::get(body, "limit");
            dev::val(core.autotuner_history(limit.unwrap_or(20)).await)
        }
        "autotuner_revert" => dev::val(core.autotuner_revert().await),
        "autotuner_pause" => dev::val(core.autotuner_pause().await),
        "autotuner_resume" => dev::val(core.autotuner_resume().await),
        "autotuner_set_pace" => {
            let pace = try_field!(dev::get_str(body, "pace"));
            dev::val(core.autotuner_set_pace(&pace).await)
        }
        _ => return None,
    })
}
