use serde::{Deserialize, Serialize};

// ── Objective / Key Result ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveResponse {
    pub id: String,
    pub title: String,
    pub status: String,
    pub progress: f64,
    pub project_id: String,
    pub key_results: Option<Vec<KeyResultResponse>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultResponse {
    pub id: String,
    pub title: String,
    pub progress: f64,
    pub current: f64,
    pub target: f64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveCreateParams {
    pub title: String,
    pub project_id: String,
    pub description: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultCreateParams {
    pub objective_id: String,
    pub title: String,
    pub target_value: Option<f64>,
    pub unit: Option<String>,
    pub tracking_mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub status: Option<String>,
    pub due_date: Option<Option<String>>,
}
