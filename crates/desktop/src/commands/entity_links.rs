use desktop_macros::klynt_command;
use desktop_shared::entity_link_types::*;
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn entity_link_create(
    app: tauri::AppHandle,
    params: EntityLinkCreateParams,
) -> EntityLinkResponse {
    let (result, updates) = state.entity_link_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn entity_link_delete(app: tauri::AppHandle, id: String) -> bool {
    let (result, updates) = state.entity_link_delete(id).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn entity_links_for_entity(kind: String, id: String) -> LinkedEntitiesResponse {
    state.entity_links_for_entity(kind, id).await
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
