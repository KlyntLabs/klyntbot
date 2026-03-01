use serde::{Deserialize, Serialize};

use crate::types::{AreaFilter, Priority, Status};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub priority: Priority,
    pub status: Status,
    pub due_date: Option<String>,
    pub tags: Vec<String>,
    pub project: String,
    pub area: AreaFilter,
    pub objective_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub color: String,
    pub task_count: u32,
    pub completed_count: u32,
    pub objective_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveResponse {
    pub id: String,
    pub title: String,
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
