use std::sync::Arc;

use desktop_shared::commands::{
    EntityMergeParams, EntityNeighborhoodResponse, EntityResponse, EntitySearchParams,
};
use desktop_shared::CommandResult;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn entity_search(
    state: State<'_, Arc<AppCore>>,
    params: EntitySearchParams,
) -> CommandResult<Vec<EntityResponse>> {
    state.entity_search(&params).await
}

#[tauri::command]
#[specta::specta]
pub async fn entity_merge(
    state: State<'_, Arc<AppCore>>,
    params: EntityMergeParams,
) -> CommandResult<EntityResponse> {
    state.entity_merge(&params).await
}

#[tauri::command]
#[specta::specta]
pub async fn entity_get_neighborhood(
    state: State<'_, Arc<AppCore>>,
    entity_id: String,
    depth: Option<u32>,
) -> CommandResult<EntityNeighborhoodResponse> {
    state
        .entity_get_neighborhood(&entity_id, depth.unwrap_or(1))
        .await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] =
    &["entity_search", "entity_merge", "entity_get_neighborhood"];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "entity_search" => dev::val(
            core.entity_search(&try_field!(dev::parse_params(body)))
                .await,
        ),
        "entity_merge" => dev::val(
            core.entity_merge(&try_field!(dev::parse_params(body)))
                .await,
        ),
        "entity_get_neighborhood" => {
            let entity_id = try_field!(dev::get_str(body, "entityId"));
            let depth: u32 = dev::get(body, "depth").unwrap_or(1);
            dev::val(core.entity_get_neighborhood(&entity_id, depth).await)
        }
        _ => return None,
    })
}
