//! Tauri adapters for coding-memory.

use std::sync::Arc;
use tauri::State;

use desktop_shared::commands::coding_memory::*;
use desktop_shared::{errors::ApiError, CommandResult};

use crate::app_core::AppCore;

/// dev_server command coverage.
#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "coding_memory_status",
    "coding_memory_enable_cli",
    "coding_memory_disable_cli",
    "coding_memory_diagnose_cli",
    "coding_memory_session_replay",
    "coding_memory_cli_health",
    "coding_memory_browser",
    "coding_memory_activity",
    "coding_memory_cost",
    "coding_memory_sensitivity",
    "coding_memory_distill_now",
    "coding_memory_recall_index",
    "coding_memory_recall_timeline",
    "coding_memory_recall_fetch",
    "coding_memory_check_dead_ends",
    "coding_memory_recall_facts_as_of",
    "coding_memory_recall_change_history",
    "coding_memory_recall_decision_points",
    "coding_memory_recall_log",
    "coding_memory_session_replay_recall_overlay",
    "coding_memory_mirror_alerts_feed",
    "coding_memory_mirror_alert_action",
    "coding_memory_effectiveness_trends",
    "coding_memory_reforge_cycle_list",
    "coding_memory_reforge_cycle_diff",
    "coding_memory_project_skills_for_repo",
];

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_status(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<CodingMemoryStatusResponse> {
    state.coding_memory_status().await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_cli_health(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<CliHealthRow>> {
    state.coding_memory_cli_health().await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_session_replay(
    state: State<'_, Arc<AppCore>>,
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CommandResult<Vec<SessionReplayEntry>> {
    state
        .coding_memory_session_replay(session_id, limit.unwrap_or(500), offset.unwrap_or(0))
        .await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_enable_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> CommandResult<()> {
    state.coding_memory_enable_cli(cli).await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_disable_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> CommandResult<()> {
    state.coding_memory_disable_cli(cli).await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_diagnose_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> CommandResult<DiagnoseResult> {
    state.coding_memory_diagnose_cli(cli).await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_browser(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CommandResult<Vec<MemoryBrowserRow>> {
    state.coding_memory_browser(limit, offset).await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_activity(
    state: State<'_, Arc<AppCore>>,
    days: Option<i64>,
) -> CommandResult<Vec<ActivityBucket>> {
    state.coding_memory_activity(days).await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_cost(
    state: State<'_, Arc<AppCore>>,
    days: Option<i64>,
) -> CommandResult<CostBreakdown> {
    state.coding_memory_cost(days).await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_sensitivity(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> CommandResult<Vec<SensitivityRow>> {
    state.coding_memory_sensitivity(limit, offset).await
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_distill_now(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
    turn_id: Option<String>,
) -> CommandResult<desktop_shared::specta_helpers::JsonValueWrapper> {
    state.coding_memory_distill_now(session_id, turn_id).await.map(desktop_shared::specta_helpers::JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_recall_index(
    state: State<'_, Arc<AppCore>>,
    args: desktop_shared::specta_helpers::JsonValueWrapper,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_index_handler(svc, args.0)
        .await
        .map_err(|e| e.to_string())
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_recall_timeline(
    state: State<'_, Arc<AppCore>>,
    args: desktop_shared::specta_helpers::JsonValueWrapper,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_timeline_handler(svc, args.0)
        .await
        .map_err(|e| e.to_string())
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_recall_fetch(
    state: State<'_, Arc<AppCore>>,
    args: desktop_shared::specta_helpers::JsonValueWrapper,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_fetch_handler(svc, args.0)
        .await
        .map_err(|e| e.to_string())
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_check_dead_ends(
    state: State<'_, Arc<AppCore>>,
    args: desktop_shared::specta_helpers::JsonValueWrapper,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::check_dead_ends_handler(svc, args.0)
        .await
        .map_err(|e| e.to_string())
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_recall_facts_as_of(
    state: State<'_, Arc<AppCore>>,
    args: desktop_shared::specta_helpers::JsonValueWrapper,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_facts_as_of_handler(svc, args.0)
        .await
        .map_err(|e| e.to_string())
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_recall_change_history(
    state: State<'_, Arc<AppCore>>,
    args: desktop_shared::specta_helpers::JsonValueWrapper,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_change_history_handler(svc, args.0)
        .await
        .map_err(|e| e.to_string())
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_recall_decision_points(
    state: State<'_, Arc<AppCore>>,
    args: desktop_shared::specta_helpers::JsonValueWrapper,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_decision_points_handler(svc, args.0)
        .await
        .map_err(|e| e.to_string())
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_recall_log(
    state: State<'_, Arc<AppCore>>,
    args: RecallLogArgs,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    let rows = app_core::coding_memory::recall::recall_log_handler(svc, args.layer, args.limit, args.offset)
        .await
        .map_err(|e| e.to_string())?;
    serde_json::to_value(rows)
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_session_replay_recall_overlay(
    state: State<'_, Arc<AppCore>>,
    args: SessionRecallOverlayArgs,
) -> Result<desktop_shared::specta_helpers::JsonValueWrapper, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    let rows = app_core::coding_memory::recall::session_recall_overlay_handler(
        svc,
        args.session_id,
        args.limit,
        args.offset,
    )
    .await
    .map_err(|e| e.to_string())?;
    serde_json::to_value(rows)
        .map(desktop_shared::specta_helpers::JsonValueWrapper)
        .map_err(|e| e.to_string())
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_mirror_alerts_feed(
    state: State<'_, Arc<AppCore>>,
    args: MirrorAlertsFeedArgs,
) -> CommandResult<Vec<MirrorAlertRow>> {
    app_core::coding_memory::panels_phase5::mirror_alerts_feed(state.storage_pool.clone(), args)
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_mirror_alert_action(
    state: State<'_, Arc<AppCore>>,
    args: MirrorAlertActionArgs,
) -> CommandResult<()> {
    app_core::coding_memory::panels_phase5::mirror_alert_action(state.storage_pool.clone(), args)
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_effectiveness_trends(
    state: State<'_, Arc<AppCore>>,
    pattern_id: String,
) -> CommandResult<EffectivenessTrendsResponse> {
    app_core::coding_memory::panels_phase5::effectiveness_trends(
        state.storage_pool.clone(),
        pattern_id,
    )
    .await
    .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_reforge_cycle_list(
    state: State<'_, Arc<AppCore>>,
) -> CommandResult<Vec<ReforgeCycleSummary>> {
    app_core::coding_memory::panels_phase5::reforge_cycle_list(state.storage_pool.clone())
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_reforge_cycle_diff(
    state: State<'_, Arc<AppCore>>,
    args: ReforgeCycleDiffArgs,
) -> CommandResult<ReforgeCycleDiffResponse> {
    app_core::coding_memory::panels_phase5::reforge_cycle_diff(state.storage_pool.clone(), args)
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn coding_memory_project_skills_for_repo(
    state: State<'_, Arc<AppCore>>,
    repo_id: String,
) -> CommandResult<Vec<ProjectSkillRow>> {
    app_core::coding_memory::panels_phase5::project_skills_for_repo(
        state.storage_pool.clone(),
        repo_id,
    )
    .await
    .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
}

/// dev_server dispatcher — wired by dev_server/mod.rs.
#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &Arc<AppCore>,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers as dev;
    Some(match cmd {
        "coding_memory_status" => dev::val(core.coding_memory_status().await),
        "coding_memory_cli_health" => dev::val(core.coding_memory_cli_health().await),
        "coding_memory_session_replay" => {
            #[derive(serde::Deserialize)]
            struct A {
                session_id: Option<String>,
                limit: Option<i64>,
                offset: Option<i64>,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                core.coding_memory_session_replay(
                    a.session_id,
                    a.limit.unwrap_or(500),
                    a.offset.unwrap_or(0),
                )
                .await,
            )
        }
        "coding_memory_enable_cli" => {
            #[derive(serde::Deserialize)]
            struct A {
                cli: String,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(core.coding_memory_enable_cli(a.cli).await)
        }
        "coding_memory_disable_cli" => {
            #[derive(serde::Deserialize)]
            struct A {
                cli: String,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(core.coding_memory_disable_cli(a.cli).await)
        }
        "coding_memory_diagnose_cli" => {
            #[derive(serde::Deserialize)]
            struct A {
                cli: String,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(core.coding_memory_diagnose_cli(a.cli).await)
        }
        "coding_memory_browser" => {
            #[derive(serde::Deserialize)]
            struct A {
                limit: Option<i64>,
                offset: Option<i64>,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(core.coding_memory_browser(a.limit, a.offset).await)
        }
        "coding_memory_activity" => {
            #[derive(serde::Deserialize)]
            struct A {
                days: Option<i64>,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(core.coding_memory_activity(a.days).await)
        }
        "coding_memory_cost" => {
            #[derive(serde::Deserialize)]
            struct A {
                days: Option<i64>,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(core.coding_memory_cost(a.days).await)
        }
        "coding_memory_sensitivity" => {
            #[derive(serde::Deserialize)]
            struct A {
                limit: Option<i64>,
                offset: Option<i64>,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(core.coding_memory_sensitivity(a.limit, a.offset).await)
        }
        "coding_memory_distill_now" => {
            #[derive(serde::Deserialize)]
            struct A {
                session_id: String,
                turn_id: Option<String>,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                core.coding_memory_distill_now(a.session_id, a.turn_id)
                    .await,
            )
        }
        "coding_memory_recall_index" => dev::val(
            app_core::coding_memory::recall::recall_index_handler(
                core.recall.as_ref()?,
                body.clone(),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
        ),
        "coding_memory_recall_timeline" => dev::val(
            app_core::coding_memory::recall::recall_timeline_handler(
                core.recall.as_ref()?,
                body.clone(),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
        ),
        "coding_memory_recall_fetch" => dev::val(
            app_core::coding_memory::recall::recall_fetch_handler(
                core.recall.as_ref()?,
                body.clone(),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
        ),
        "coding_memory_check_dead_ends" => dev::val(
            app_core::coding_memory::recall::check_dead_ends_handler(
                core.recall.as_ref()?,
                body.clone(),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
        ),
        "coding_memory_recall_facts_as_of" => dev::val(
            app_core::coding_memory::recall::recall_facts_as_of_handler(
                core.recall.as_ref()?,
                body.clone(),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
        ),
        "coding_memory_recall_change_history" => dev::val(
            app_core::coding_memory::recall::recall_change_history_handler(
                core.recall.as_ref()?,
                body.clone(),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
        ),
        "coding_memory_recall_decision_points" => dev::val(
            app_core::coding_memory::recall::recall_decision_points_handler(
                core.recall.as_ref()?,
                body.clone(),
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
        ),
        "coding_memory_recall_log" => {
            let a: RecallLogArgs = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                app_core::coding_memory::recall::recall_log_handler(
                    core.recall.as_ref()?,
                    a.layer,
                    a.limit,
                    a.offset,
                )
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
            )
        }
        "coding_memory_session_replay_recall_overlay" => {
            let a: SessionRecallOverlayArgs = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                app_core::coding_memory::recall::session_recall_overlay_handler(
                    core.recall.as_ref()?,
                    a.session_id,
                    a.limit,
                    a.offset,
                )
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
            )
        }
        "coding_memory_mirror_alerts_feed" => {
            let a: MirrorAlertsFeedArgs = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                app_core::coding_memory::panels_phase5::mirror_alerts_feed(
                    core.storage_pool.clone(),
                    a,
                )
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
            )
        }
        "coding_memory_mirror_alert_action" => {
            let a: MirrorAlertActionArgs = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                app_core::coding_memory::panels_phase5::mirror_alert_action(
                    core.storage_pool.clone(),
                    a,
                )
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
            )
        }
        "coding_memory_effectiveness_trends" => {
            #[derive(serde::Deserialize)]
            struct A {
                pattern_id: String,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                app_core::coding_memory::panels_phase5::effectiveness_trends(
                    core.storage_pool.clone(),
                    a.pattern_id,
                )
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
            )
        }
        "coding_memory_reforge_cycle_list" => dev::val(
            app_core::coding_memory::panels_phase5::reforge_cycle_list(core.storage_pool.clone())
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
        ),
        "coding_memory_reforge_cycle_diff" => {
            let a: ReforgeCycleDiffArgs = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                app_core::coding_memory::panels_phase5::reforge_cycle_diff(
                    core.storage_pool.clone(),
                    a,
                )
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
            )
        }
        "coding_memory_project_skills_for_repo" => {
            #[derive(serde::Deserialize)]
            struct A {
                repo_id: String,
            }
            let a: A = match dev::parse_params(body) {
                Ok(v) => v,
                Err(e) => return Some(Err(e)),
            };
            dev::val(
                app_core::coding_memory::panels_phase5::project_skills_for_repo(
                    core.storage_pool.clone(),
                    a.repo_id,
                )
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())),
            )
        }
        _ => return None,
    })
}
