use desktop_shared::commands::{
    EntityMergeParams, EntityNeighborhoodResponse, EntityResponse, EntitySearchParams,
};

use desktop_macros::klynt_command;

#[klynt_command]
pub async fn entity_search(params: EntitySearchParams) -> Vec<EntityResponse> {
    state.entity_search(&params).await
}

#[klynt_command]
pub async fn entity_merge(params: EntityMergeParams) -> EntityResponse {
    state.entity_merge(&params).await
}

#[klynt_command]
pub async fn entity_get_neighborhood(
    entity_id: String,
    depth: Option<u32>,
) -> EntityNeighborhoodResponse {
    state
        .entity_get_neighborhood(&entity_id, depth.unwrap_or(1))
        .await
}
