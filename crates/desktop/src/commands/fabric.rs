use desktop_macros::klynt_command;
use desktop_shared::commands::fabric::{
    FabricActionParams, FabricActionResponse, FabricExpandParams, FabricExpandResponse,
    FabricGraphBase,
};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn fabric_graph_base() -> FabricGraphBase {
    state.fabric_graph_base().await
}

#[klynt_command]
pub async fn fabric_graph_expand(params: FabricExpandParams) -> FabricExpandResponse {
    state.fabric_graph_expand(params).await
}

#[klynt_command]
pub async fn fabric_graph_action(params: FabricActionParams) -> FabricActionResponse {
    state.fabric_graph_action(params).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
