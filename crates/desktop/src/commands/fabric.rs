use std::sync::Arc;

use desktop_shared::commands::fabric::{
    FabricActionParams, FabricActionResponse, FabricExpandParams, FabricExpandResponse,
    FabricGraphBase,
};
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn fabric_graph_base(
    state: State<'_, Arc<AppCore>>,
) -> Result<FabricGraphBase, ApiError> {
    state.fabric_graph_base().await
}

#[tauri::command]
#[specta::specta]
pub async fn fabric_graph_expand(
    state: State<'_, Arc<AppCore>>,
    params: FabricExpandParams,
) -> Result<FabricExpandResponse, ApiError> {
    state.fabric_graph_expand(params).await
}

#[tauri::command]
#[specta::specta]
pub async fn fabric_graph_action(
    state: State<'_, Arc<AppCore>>,
    params: FabricActionParams,
) -> Result<FabricActionResponse, ApiError> {
    state.fabric_graph_action(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "fabric_graph_base",
    "fabric_graph_expand",
    "fabric_graph_action",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "fabric_graph_base" => dev::val(core.fabric_graph_base().await),
        "fabric_graph_expand" => dev::val(
            core.fabric_graph_expand(try_field!(dev::parse_params(body)))
                .await,
        ),
        "fabric_graph_action" => dev::val(
            core.fabric_graph_action(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
