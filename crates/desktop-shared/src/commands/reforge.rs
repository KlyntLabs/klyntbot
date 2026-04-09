//! Response types for the Reforge Cycle (Brain page).

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReforgeStateResponse {
    pub last_run_at: Option<String>,
    pub last_run_stats: Option<serde_json::Value>,
    pub run_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillVersionResponse {
    pub id: String,
    pub skill_name: String,
    pub version: i64,
    pub file_path: String,
    pub diff: Option<String>,
    pub source: String,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillVersionDetailResponse {
    pub id: String,
    pub skill_name: String,
    pub version: i64,
    pub file_path: String,
    pub content: String,
    pub diff: Option<String>,
    pub source: String,
    pub reason: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkillListResponse {
    pub skill_names: Vec<String>,
}
