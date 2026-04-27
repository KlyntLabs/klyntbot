use app_core::AppCore;
use desktop_macros::klynt_command;
use desktop_shared::commands::{
    CreateSquadParams, SquadMemberParams, SquadResponse, UpdateSquadParams,
};
use desktop_shared::CommandResult;

#[klynt_command]
pub async fn list_squads() -> Vec<SquadResponse> {
    state.list_squads().await
}

#[klynt_command]
pub async fn get_squad(id: String) -> SquadResponse {
    state.get_squad(&id).await
}

#[klynt_command]
pub async fn create_squad(params: CreateSquadParams) -> SquadResponse {
    state.create_squad(params).await
}

#[klynt_command]
pub async fn update_squad(params: UpdateSquadParams) -> SquadResponse {
    state.update_squad(params).await
}

#[klynt_command]
pub async fn delete_squad(id: String) -> () {
    state.delete_squad(&id).await
}

#[klynt_command]
pub async fn add_squad_member(params: SquadMemberParams) -> () {
    state.add_squad_member(params).await
}

#[klynt_command]
pub async fn remove_squad_member(squad_id: String, persona_id: String) -> () {
    state.remove_squad_member(&squad_id, &persona_id).await
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
