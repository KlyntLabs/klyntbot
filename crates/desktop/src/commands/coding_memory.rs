//! Tauri adapters for coding-memory.

use std::sync::Arc;
use tauri::State;

use desktop_shared::commands::coding_memory::*;
use desktop_shared::errors::ApiError;

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
];

#[tauri::command]
pub async fn coding_memory_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<CodingMemoryStatusResponse, ApiError> {
    state.coding_memory_status().await
}

#[tauri::command]
pub async fn coding_memory_cli_health(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<CliHealthRow>, ApiError> {
    state.coding_memory_cli_health().await
}

#[tauri::command]
pub async fn coding_memory_session_replay(
    state: State<'_, Arc<AppCore>>,
    session_id: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<SessionReplayEntry>, ApiError> {
    state
        .coding_memory_session_replay(session_id, limit.unwrap_or(500), offset.unwrap_or(0))
        .await
}

#[tauri::command]
pub async fn coding_memory_enable_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> Result<(), ApiError> {
    state.coding_memory_enable_cli(cli).await
}

#[tauri::command]
pub async fn coding_memory_disable_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> Result<(), ApiError> {
    state.coding_memory_disable_cli(cli).await
}

#[tauri::command]
pub async fn coding_memory_diagnose_cli(
    state: State<'_, Arc<AppCore>>,
    cli: String,
) -> Result<DiagnoseResult, ApiError> {
    state.coding_memory_diagnose_cli(cli).await
}

#[tauri::command]
pub async fn coding_memory_browser(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<MemoryBrowserRow>, ApiError> {
    state.coding_memory_browser(limit, offset).await
}

#[tauri::command]
pub async fn coding_memory_activity(
    state: State<'_, Arc<AppCore>>,
    days: Option<i64>,
) -> Result<Vec<ActivityBucket>, ApiError> {
    state.coding_memory_activity(days).await
}

#[tauri::command]
pub async fn coding_memory_cost(
    state: State<'_, Arc<AppCore>>,
    days: Option<i64>,
) -> Result<CostBreakdown, ApiError> {
    state.coding_memory_cost(days).await
}

#[tauri::command]
pub async fn coding_memory_sensitivity(
    state: State<'_, Arc<AppCore>>,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Result<Vec<SensitivityRow>, ApiError> {
    state.coding_memory_sensitivity(limit, offset).await
}

#[tauri::command]
pub async fn coding_memory_distill_now(
    state: State<'_, Arc<AppCore>>,
    session_id: String,
    turn_id: Option<String>,
) -> Result<serde_json::Value, ApiError> {
    state.coding_memory_distill_now(session_id, turn_id).await
}

#[tauri::command]
pub async fn coding_memory_recall_index(
    state: State<'_, Arc<AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_index_handler(svc, args)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_timeline(
    state: State<'_, Arc<AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_timeline_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_fetch(
    state: State<'_, Arc<AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_fetch_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_check_dead_ends(
    state: State<'_, Arc<AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::check_dead_ends_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_facts_as_of(
    state: State<'_, Arc<AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_facts_as_of_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_change_history(
    state: State<'_, Arc<AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_change_history_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_decision_points(
    state: State<'_, Arc<AppCore>>,
    args: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_decision_points_handler(svc, args)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_recall_log(
    state: State<'_, Arc<AppCore>>,
    args: RecallLogArgs,
) -> Result<Vec<coding_memory::RecallInvocationRow>, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::recall_log_handler(svc, args.layer, args.limit, args.offset)
        .await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn coding_memory_session_replay_recall_overlay(
    state: State<'_, Arc<AppCore>>,
    args: SessionRecallOverlayArgs,
) -> Result<Vec<coding_memory::RecallInvocationRow>, String> {
    let svc = state.recall.as_ref().ok_or("recall service unavailable")?;
    app_core::coding_memory::recall::session_recall_overlay_handler(svc, args.session_id, args.limit, args.offset)
        .await.map_err(|e| e.to_string())
}

/// dev_server dispatcher — wired by dev_server/mod.rs.
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &Arc<AppCore>,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
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
        "coding_memory_recall_index" => dev::val(app_core::coding_memory::recall::recall_index_handler(core.recall.as_ref()?, body.clone()).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))),
        "coding_memory_recall_timeline" => dev::val(app_core::coding_memory::recall::recall_timeline_handler(core.recall.as_ref()?, body.clone()).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))),
        "coding_memory_recall_fetch" => dev::val(app_core::coding_memory::recall::recall_fetch_handler(core.recall.as_ref()?, body.clone()).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))),
        "coding_memory_check_dead_ends" => dev::val(app_core::coding_memory::recall::check_dead_ends_handler(core.recall.as_ref()?, body.clone()).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))),
        "coding_memory_recall_facts_as_of" => dev::val(app_core::coding_memory::recall::recall_facts_as_of_handler(core.recall.as_ref()?, body.clone()).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))),
        "coding_memory_recall_change_history" => dev::val(app_core::coding_memory::recall::recall_change_history_handler(core.recall.as_ref()?, body.clone()).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))),
        "coding_memory_recall_decision_points" => dev::val(app_core::coding_memory::recall::recall_decision_points_handler(core.recall.as_ref()?, body.clone()).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))),
        "coding_memory_recall_log" => {
            let a: RecallLogArgs = match dev::parse_params(body) { Ok(v) => v, Err(e) => return Some(Err(e)) };
            dev::val(app_core::coding_memory::recall::recall_log_handler(core.recall.as_ref()?, a.layer, a.limit, a.offset).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())))
        }
        "coding_memory_session_replay_recall_overlay" => {
            let a: SessionRecallOverlayArgs = match dev::parse_params(body) { Ok(v) => v, Err(e) => return Some(Err(e)) };
            dev::val(app_core::coding_memory::recall::session_recall_overlay_handler(core.recall.as_ref()?, a.session_id, a.limit, a.offset).await.map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string())))
        }
        _ => return None,
    })
}
