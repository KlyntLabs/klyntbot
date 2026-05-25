use desktop_macros::klynt_command;
use desktop_shared::commands::fabric::{
    FabricActionParams, FabricActionResponse, FabricExpandParams, FabricExpandResponse,
    FabricGraphBase,
};

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
