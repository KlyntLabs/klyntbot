use serde::{Deserialize, Serialize};

// ── Area ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AreaResponse {
    pub id: String,
    pub name: String,
    pub color: String,
    pub icon: Option<String>,
    pub project_count: i64,
    pub task_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AreaCreateParams {
    pub name: String,
    pub color: Option<String>,
    pub icon: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AreaUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub icon: Option<Option<String>>,
}
