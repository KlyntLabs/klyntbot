use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityLinkResponse {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub link_type: String,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub metadata: Option<serde_json::Value>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityLinkCreateParams {
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub link_type: Option<String>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct EntityLinksForEntityParams {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct LinkedEntitiesResponse {
    pub tasks: Vec<ActionSummaryResponse>,
    pub notes: Vec<NoteSummaryResponse>,
    pub conversations: Vec<SessionSummaryResponse>,
    pub sources: Vec<ProjectSourceResponse>,
    pub objectives: Vec<ObjectiveSummaryResponse>,
    pub key_results: Vec<KeyResultSummaryResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ActionSummaryResponse {
    pub id: String,
    pub title: String,
    pub status: String,
    pub priority: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct NoteSummaryResponse {
    pub id: String,
    pub title: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryResponse {
    pub key: String,
    pub title: Option<String>,
    pub conversation_type: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveSummaryResponse {
    pub id: String,
    pub title: String,
    pub progress: f64,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultSummaryResponse {
    pub id: String,
    pub title: String,
    pub progress: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceResponse {
    pub id: String,
    pub project_id: String,
    pub source_type: String,
    pub title: String,
    pub content: Option<String>,
    pub url: Option<String>,
    pub file_path: Option<String>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub metadata: Option<serde_json::Value>,
    pub tags: Vec<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceCreateParams {
    pub project_id: String,
    pub source_type: String,
    pub title: String,
    pub content: Option<String>,
    pub url: Option<String>,
    pub file_path: Option<String>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub content: Option<Option<String>>,
    pub url: Option<Option<String>>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub metadata: Option<serde_json::Value>,
    pub tags: Option<Vec<String>>,
}
