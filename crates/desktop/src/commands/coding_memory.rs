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
        _ => return None,
    })
}
