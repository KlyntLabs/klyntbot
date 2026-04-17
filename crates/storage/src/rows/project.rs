//! Row struct for the `projects` table.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `projects` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRow {
    pub id: String,
    pub area_id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    #[sqlx(json)]
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
    pub workflow_id: Option<String>,
    pub instructions: Option<String>,
    pub ai_personality: Option<String>,
    pub user_role: Option<String>,
    pub start_date: Option<String>,
    pub target_end_date: Option<String>,
    pub settings: Option<String>,
}
