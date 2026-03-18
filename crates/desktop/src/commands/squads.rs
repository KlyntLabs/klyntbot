use std::sync::Arc;

use app_core::AppCore;
use desktop_shared::commands::{
    CreateSquadParams, SquadMemberParams, SquadResponse, UpdateSquadParams,
};
use desktop_shared::errors::ApiError;
use tauri::State;

#[tauri::command]
pub async fn list_squads(state: State<'_, Arc<AppCore>>) -> Result<Vec<SquadResponse>, ApiError> {
    state.list_squads().await
}

#[tauri::command]
pub async fn get_squad(
    state: State<'_, Arc<AppCore>>,
    id: String,
) -> Result<SquadResponse, ApiError> {
    state.get_squad(&id).await
}

#[tauri::command]
pub async fn create_squad(
    state: State<'_, Arc<AppCore>>,
    params: CreateSquadParams,
) -> Result<SquadResponse, ApiError> {
    state.create_squad(params).await
}

#[tauri::command]
pub async fn update_squad(
    state: State<'_, Arc<AppCore>>,
    params: UpdateSquadParams,
) -> Result<SquadResponse, ApiError> {
    state.update_squad(params).await
}

#[tauri::command]
pub async fn delete_squad(state: State<'_, Arc<AppCore>>, id: String) -> Result<(), ApiError> {
    state.delete_squad(&id).await
}

#[tauri::command]
pub async fn add_squad_member(
    state: State<'_, Arc<AppCore>>,
    params: SquadMemberParams,
) -> Result<(), ApiError> {
    state.add_squad_member(params).await
}

#[tauri::command]
pub async fn remove_squad_member(
    state: State<'_, Arc<AppCore>>,
    squad_id: String,
    persona_id: String,
) -> Result<(), ApiError> {
    state.remove_squad_member(&squad_id, &persona_id).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "list_squads",
    "get_squad",
    "create_squad",
    "update_squad",
    "delete_squad",
    "add_squad_member",
    "remove_squad_member",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "list_squads" => dev::val(core.list_squads().await),
        "get_squad" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.get_squad(&id).await)
        }
        "create_squad" => dev::val(core.create_squad(try_field!(dev::parse_params(body))).await),
        "update_squad" => dev::val(core.update_squad(try_field!(dev::parse_params(body))).await),
        "delete_squad" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.delete_squad(&id).await)
        }
        "add_squad_member" => dev::val(
            core.add_squad_member(try_field!(dev::parse_params(body)))
                .await,
        ),
        "remove_squad_member" => {
            let squad_id = try_field!(dev::get_str(body, "squadId"));
            let persona_id = try_field!(dev::get_str(body, "personaId"));
            dev::val(core.remove_squad_member(&squad_id, &persona_id).await)
        }
        _ => return None,
    })
}
