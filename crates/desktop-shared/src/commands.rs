use serde::{Deserialize, Serialize};

use crate::types::{Area, Priority, Status};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub priority: Priority,
    pub status: Status,
    pub due_date: String,
    pub tags: Vec<String>,
    pub project: String,
    pub area: Area,
    pub objective_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectResponse {
    pub id: String,
    pub name: String,
    pub color: String,
    pub task_count: u32,
    pub completed_count: u32,
    pub objective_ids: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectiveResponse {
    pub id: String,
    pub title: String,
    pub progress: u32,
    pub project_id: String,
    pub key_results: Option<Vec<KeyResultResponse>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeyResultResponse {
    pub id: String,
    pub title: String,
    pub progress: u32,
    pub current: f64,
    pub target: f64,
    pub unit: String,
}
