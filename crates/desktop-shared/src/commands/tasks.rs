use serde::{Deserialize, Serialize};

// ── Task ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskResponse {
    pub id: String,
    pub title: String,
    pub completed: bool,
    pub priority: Option<String>,
    pub status: String,
    pub due_date: Option<String>,
    pub tags: Vec<String>,
    pub project_id: Option<String>,
    pub area_id: String,
    pub objective_id: Option<String>,
    pub description: Option<String>,
    pub parent_id: Option<String>,
    pub subtask_count: u32,
    pub subtask_completed_count: u32,
    pub status_label_id: Option<String>,
    pub status_label: Option<StatusLabelResponse>,
    pub group_id: Option<String>,
    pub task_type: Option<String>,
    pub execution_state: Option<String>,
    pub energy_level: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub estimated_minutes: Option<i32>,
    pub actual_minutes: Option<i32>,
    pub complexity_score: Option<i32>,
    pub total_tracked_secs: Option<i64>,
    pub focused_at: Option<String>,
    pub created_at: Option<String>,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskCreateParams {
    pub title: String,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<String>,
    pub tags: Option<Vec<String>>,
    pub parent_id: Option<String>,
    pub status_label_id: Option<String>,
    pub group_id: Option<String>,
    pub task_type: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub energy_level: Option<String>,
    pub estimated_minutes: Option<i32>,
}

// ── AI Suggestion ───────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SuggestionResponse {
    pub id: String,
    pub suggestion_type: String,
    pub title: String,
    pub description: Option<String>,
    pub confidence: f64,
    pub status: String,
    pub created_at: String,
}

// ── Today Task (tray view) ──────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TodayTaskResponse {
    pub id: String,
    pub title: String,
    pub priority: Option<String>,
    pub status: String,
    pub completed: bool,
    pub is_overdue: bool,
    pub is_due_today: bool,
    pub due_display: Option<String>,
}

// ── Task Update ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdateParams {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub status: Option<String>,
    pub due_date: Option<Option<String>>,
    pub project_id: Option<Option<String>>,
    pub area_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub key_result_id: Option<Option<String>>,
    pub status_label_id: Option<Option<String>>,
    pub position: Option<i32>,
    pub group_id: Option<Option<String>>,
    pub task_type: Option<String>,
    pub acceptance_criteria: Option<Option<String>>,
    pub energy_level: Option<String>,
    pub execution_state: Option<String>,
    pub estimated_minutes: Option<Option<i32>>,
}

// ── Status Workflows ──────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusWorkflowResponse {
    pub id: String,
    pub name: String,
    pub is_template: bool,
    pub is_global_default: bool,
    pub labels: Vec<StatusLabelResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusLabelResponse {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub color: String,
    pub status_group: String,
    pub position: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowCreateParams {
    pub name: String,
    pub is_template: Option<bool>,
    pub source_workflow_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelCreateParams {
    pub workflow_id: String,
    pub name: String,
    pub color: String,
    pub status_group: String,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub status_group: Option<String>,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelReorderParams {
    pub workflow_id: String,
    pub label_ids: Vec<String>,
}

// ── Task Groups ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupResponse {
    pub id: String,
    pub project_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
    pub position: i32,
    pub task_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupCreateParams {
    pub project_id: Option<String>,
    pub name: String,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub color: Option<String>,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskGroupReorderParams {
    pub project_id: Option<String>,
    pub group_ids: Vec<String>,
}

// ── Custom Columns ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomColumnResponse {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub column_type: String,
    pub options: Option<Vec<String>>,
    pub position: i32,
    pub width: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomColumnValueResponse {
    pub task_id: String,
    pub column_id: String,
    pub value: serde_json::Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnCreateParams {
    pub project_id: String,
    pub name: String,
    pub column_type: String,
    pub options: Option<Vec<String>>,
    pub width: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnUpdateParams {
    pub id: String,
    pub name: Option<String>,
    pub options: Option<Option<Vec<String>>>,
    pub width: Option<i32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnReorderParams {
    pub project_id: String,
    pub ids: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ColumnValueSetParams {
    pub task_id: String,
    pub column_id: String,
    pub value: serde_json::Value,
}
