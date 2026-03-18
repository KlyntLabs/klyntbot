use std::collections::HashMap;

use desktop_shared::commands::{
    EntityMergeParams, EntityNeighborhoodResponse, EntityRelationshipResponse, EntityResponse,
    EntitySearchParams,
};
use desktop_shared::errors::ApiError;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

impl AppCore {
    #[allow(clippy::unnecessary_map_or)] // is_none_or requires Rust 1.82, MSRV is 1.75
    pub async fn entity_search(
        &self,
        params: &EntitySearchParams,
    ) -> Result<Vec<EntityResponse>, ApiError> {
        let repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let rows = repo
            .find_by_name(&params.query)
            .await
            .map_err(map_cognitive_err)?;

        let limit = params.limit.unwrap_or(20) as usize;
        Ok(rows
            .into_iter()
            .filter(|r| {
                params
                    .entity_type
                    .as_ref()
                    .map_or(true, |t| r.entity_type == *t)
            })
            .take(limit)
            .map(entity_row_to_response)
            .collect())
    }

    pub async fn entity_merge(
        &self,
        params: &EntityMergeParams,
    ) -> Result<EntityResponse, ApiError> {
        let repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        // merge_entities(source_id, target_id) -- source is deleted, target is kept
        let merged = repo
            .merge_entities(&params.merge_id, &params.keep_id)
            .await
            .map_err(map_cognitive_err)?;

        Ok(entity_row_to_response(merged))
    }

    pub async fn entity_get_neighborhood(
        &self,
        entity_id: &str,
        depth: u32,
    ) -> Result<EntityNeighborhoodResponse, ApiError> {
        let repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let neighborhood = repo
            .get_neighborhood(entity_id, depth)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Entity not found"))?;

        // Build neighbor map for lookup
        let neighbor_map: HashMap<&str, &cognitive::repos::EntityRow> = neighborhood
            .neighbors
            .iter()
            .map(|n| (n.id.as_str(), n))
            .collect();

        let neighbors = neighborhood
            .relationships
            .iter()
            .filter_map(|rel| {
                let (neighbor_id, direction) = if rel.source_entity_id == entity_id {
                    (rel.target_entity_id.as_str(), "outgoing")
                } else {
                    (rel.source_entity_id.as_str(), "incoming")
                };
                neighbor_map
                    .get(neighbor_id)
                    .map(|entity| EntityRelationshipResponse {
                        entity: entity_row_to_response((*entity).clone()),
                        relationship_type: rel.relationship_type.clone(),
                        strength: rel.strength,
                        direction: direction.to_string(),
                    })
            })
            .collect();

        Ok(EntityNeighborhoodResponse {
            center: entity_row_to_response(neighborhood.center),
            neighbors,
        })
    }
}

fn entity_row_to_response(row: cognitive::repos::EntityRow) -> EntityResponse {
    EntityResponse {
        id: row.id,
        name: row.name,
        entity_type: row.entity_type,
        description: row.description,
        mention_count: row.mention_count,
        last_seen_at: row.last_seen_at,
    }
}
