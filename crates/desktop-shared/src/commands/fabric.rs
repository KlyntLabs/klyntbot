use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricNote {
    pub id: String,
    pub title: String,
    pub notebook_id: Option<String>,
    pub tags: Vec<String>,
    pub body_preview: String,
    pub tree_section_count: u32,
    pub entity_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricLink {
    pub source_id: String,
    pub target_id: String,
    pub link_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricCommunity {
    pub id: String,
    pub name: String,
    pub color: String,
    pub stability: f64,
    pub member_count: u32,
    pub member_note_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricGraphBase {
    pub notes: Vec<FabricNote>,
    pub links: Vec<FabricLink>,
    pub communities: Vec<FabricCommunity>,
    pub suggested_preset: Option<String>,
    pub last_activity_timestamp: String,
    pub live_pulse_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricExpandParams {
    pub layer: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricEntity {
    pub id: String,
    pub name: String,
    pub entity_type: String,
    pub mention_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricEntityEdge {
    pub entity_id: String,
    pub note_id: String,
    pub weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricEntitiesResponse {
    pub entities: Vec<FabricEntity>,
    pub edges: Vec<FabricEntityEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricTreeNode {
    pub id: String,
    pub parent_id: Option<String>,
    pub node_type: String,
    pub title: Option<String>,
    pub content_preview: String,
    pub level: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricTreeNodesResponse {
    pub note_id: String,
    pub nodes: Vec<FabricTreeNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricMember {
    pub note_id: String,
    pub tree_node_id: String,
    pub membership_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricCommunityDetail {
    pub community_id: String,
    pub representative_paths: Vec<String>,
    pub top_entities: Vec<String>,
    pub stability_history: Vec<f64>,
    pub members: Vec<FabricMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase", tag = "type", content = "data")]
pub enum FabricExpandResponse {
    Entities(FabricEntitiesResponse),
    Tree(Vec<FabricTreeNodesResponse>),
    CommunityDetail(Vec<FabricCommunityDetail>),
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricActionParams {
    pub action: String,
    #[specta(type = crate::specta_helpers::JsonValue)]
    pub payload: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricActionResponse {
    pub success: bool,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct FabricGraphEvent {
    #[serde(rename = "type")]
    pub event_type: String,
    pub node_type: String,
    pub id: String,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub data: Option<serde_json::Value>,
    pub animation_hint: String,
    pub intensity: f64,
}

impl FabricGraphEvent {
    /// Map a fabric-relevant `DomainEvent` to a `FabricGraphEvent`, if applicable.
    pub fn from_domain_event(event: &bus::DomainEvent) -> Option<Self> {
        match event {
            bus::DomainEvent::NoteContentChanged { note_id, .. } => Some(Self {
                event_type: "node_updated".to_string(),
                node_type: "note".to_string(),
                id: note_id.clone(),
                data: None,
                animation_hint: "pulse".to_string(),
                intensity: 0.3,
            }),
            _ => None,
        }
    }
}
