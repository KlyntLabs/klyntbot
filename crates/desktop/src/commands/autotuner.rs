use std::sync::Arc;

use app_core::handlers::autotuner::AutoTunerStatus;
use autotuner::{ChampionSummary, ExperimentSummary};
use desktop_shared::CommandResult;
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
    "autotuner_get_toast_count",
    "autotuner_increment_toast_count",
];

#[tauri::command]
#[specta::specta]
pub async fn autotuner_status(state: State<'_, Arc<AppCore>>) -> CommandResult<AutoTunerStatus> {
    state.autotuner_status().await
}

#[tauri::command]
#[specta::specta]
pub async fn autotuner_history(
    state: State<'_, Arc<AppCore>>,
    limit: Option<u32>,
) -> CommandResult<Vec<ExperimentSummary>> {
    state.autotuner_history(limit.unwrap_or(20)).await
}

#[tauri::command]
#[specta::specta]
pub async fn autotuner_revert(state: State<'_, Arc<AppCore>>) -> CommandResult<ChampionSummary> {
    state.autotuner_revert().await
}

#[tauri::command]
#[specta::specta]
pub async fn autotuner_pause(state: State<'_, Arc<AppCore>>) -> CommandResult<()> {
    state.autotuner_pause().await
}

#[tauri::command]
#[specta::specta]
pub async fn autotuner_resume(state: State<'_, Arc<AppCore>>) -> CommandResult<()> {
    state.autotuner_resume().await
}

#[tauri::command]
#[specta::specta]
pub async fn autotuner_set_pace(
    state: State<'_, Arc<AppCore>>,
    pace: String,
) -> CommandResult<()> {
    state.autotuner_set_pace(&pace).await
}

#[tauri::command]
#[specta::specta]
pub async fn autotuner_get_toast_count(state: State<'_, Arc<AppCore>>) -> CommandResult<i64> {
    state.autotuner_get_toast_count().await
}

#[tauri::command]
#[specta::specta]
pub async fn autotuner_increment_toast_count(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<i64> {
    state.autotuner_increment_toast_count().await
}

// ── Dev server dispatch ──────────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
        "autotuner_get_toast_count" => dev::val(core.autotuner_get_toast_count().await),
        "autotuner_increment_toast_count" => dev::val(core.autotuner_increment_toast_count().await),
        _ => return None,
    })
}
