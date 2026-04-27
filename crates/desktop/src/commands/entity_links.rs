use desktop_shared::entity_link_types::*;
use desktop_shared::CommandResult;
use std::sync::Arc;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
#[specta::specta]
pub async fn entity_link_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: EntityLinkCreateParams,
) -> CommandResult<EntityLinkResponse> {
    let (result, updates) = state.entity_link_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn entity_link_delete(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    id: String,
) -> CommandResult<bool> {
    let (result, updates) = state.entity_link_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[tauri::command]
#[specta::specta]
pub async fn entity_links_for_entity(
    state: State<'_, Arc<AppCore>>,
    kind: String,
    id: String,
) -> CommandResult<LinkedEntitiesResponse> {
    state.entity_links_for_entity(kind, id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "entity_link_create",
    "entity_link_delete",
    "entity_links_for_entity",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "entity_link_create" => dev::val_rh(
            core.entity_link_create(try_field!(dev::parse_params(body)))
                .await,
        ),
        "entity_link_delete" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val_rh(core.entity_link_delete(id).await)
        }
        "entity_links_for_entity" => {
            let kind = try_field!(dev::get_str(body, "kind"));
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.entity_links_for_entity(kind, id).await)
        }
        _ => return None,
    })
}
