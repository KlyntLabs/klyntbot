use desktop_macros::klynt_command;
use desktop_shared::entity_link_types::*;

#[klynt_command]
pub async fn entity_link_create(params: EntityLinkCreateParams) -> EntityLinkResponse {
    let (result, updates) = state.entity_link_create(params).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn entity_link_delete(id: String) -> bool {
    let (result, updates) = state.entity_link_delete(id).await?;
    super::emit_updates_ev(emitter, &updates);
    Ok(result)
}

#[klynt_command]
pub async fn entity_links_for_entity(kind: String, id: String) -> LinkedEntitiesResponse {
    state.entity_links_for_entity(kind, id).await
}
