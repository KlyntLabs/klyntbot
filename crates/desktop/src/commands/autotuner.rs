use app_core::handlers::autotuner::AutoTunerStatus;
use autotuner::{ChampionSummary, ExperimentSummary};
use desktop_macros::klynt_command;

#[klynt_command]
pub async fn autotuner_status() -> AutoTunerStatus {
    state.autotuner_status().await
}

#[klynt_command]
pub async fn autotuner_history(limit: Option<u32>) -> Vec<ExperimentSummary> {
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
