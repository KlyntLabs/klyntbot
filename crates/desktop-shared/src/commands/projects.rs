use serde::{Deserialize, Serialize};

// ── Project ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub color: String,
    pub area_id: String,
    pub task_count: u32,
    pub completed_count: u32,
    pub objective_ids: Option<Vec<String>>,
    pub workflow_id: Option<String>,
    pub description: Option<String>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub instructions: Option<serde_json::Value>,
    pub ai_personality: Option<String>,
    pub user_role: Option<String>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub settings: Option<serde_json::Value>,
}

/// Health metrics for a project (focus quality, insight freshness).
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectHealthMetricsResponse {
    /// Average focus session quality score (0-1), or null if no sessions.
    pub focus_quality: Option<f64>,
    /// Average insight freshness across linked notes (0-1), or null if no linked notes.
    pub insight_freshness: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateParams {
    pub name: String,
    pub area_id: String,
    pub color: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ProjectUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub area_id: Option<String>,
    pub color: Option<String>,
    pub description: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub workflow_id: Option<Option<String>>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub instructions: Option<serde_json::Value>,
    pub ai_personality: Option<Option<String>>,
    pub user_role: Option<Option<String>>,
    pub start_date: Option<Option<String>>,
    pub target_end_date: Option<Option<String>>,
    #[specta(type = Option<crate::specta_helpers::JsonValue>)]
    pub settings: Option<serde_json::Value>,
}
