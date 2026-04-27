use app_core::handlers::autotuner::AutoTunerStatus;
use autotuner::{ChampionSummary, ExperimentSummary};
use desktop_shared::CommandResult;
use desktop_macros::klynt_command;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn autotuner_status() -> AutoTunerStatus {
    state.autotuner_status().await
}

#[klynt_command]
pub async fn autotuner_history(
    limit: Option<u32>,
) -> Vec<ExperimentSummary> {
    state.autotuner_history(limit.unwrap_or(20)).await
}

#[klynt_command]
pub async fn autotuner_revert() -> ChampionSummary {
    state.autotuner_revert().await
}

#[klynt_command]
pub async fn autotuner_pause() -> () {
    state.autotuner_pause().await
}

#[klynt_command]
pub async fn autotuner_resume() -> () {
    state.autotuner_resume().await
}

#[klynt_command]
pub async fn autotuner_set_pace(pace: String) -> () {
    state.autotuner_set_pace(&pace).await
}

#[klynt_command]
pub async fn autotuner_get_toast_count() -> i64 {
    state.autotuner_get_toast_count().await
}

#[klynt_command]
pub async fn autotuner_increment_toast_count() -> i64 {
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
