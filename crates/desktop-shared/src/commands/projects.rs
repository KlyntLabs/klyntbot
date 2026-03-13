use serde::{Deserialize, Serialize};

// ── Project ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub instructions: Option<serde_json::Value>,
    pub ai_personality: Option<String>,
    pub user_role: Option<String>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub settings: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectCreateParams {
    pub name: String,
    pub area_id: String,
    pub color: Option<String>,
    pub description: Option<String>,
    pub tags: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub instructions: Option<serde_json::Value>,
    pub ai_personality: Option<Option<String>>,
    pub user_role: Option<Option<String>>,
    pub start_date: Option<Option<String>>,
    pub target_end_date: Option<Option<String>>,
    pub settings: Option<serde_json::Value>,
}
