use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityResponse {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
    pub mention_count: i64,
    pub last_seen_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntitySearchParams {
    pub query: String,
    pub entity_type: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityMergeParams {
    pub keep_id: String,
    pub merge_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityNeighborhoodResponse {
    pub center: EntityResponse,
    pub neighbors: Vec<EntityRelationshipResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EntityRelationshipResponse {
    pub entity: EntityResponse,
    pub relationship_type: String,
    pub strength: f64,
    pub direction: String,
}
