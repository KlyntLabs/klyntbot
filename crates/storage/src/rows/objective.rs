//! Row struct for the `objectives` table.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectiveRow {
    pub id: String,
    pub project_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub priority: Option<i16>,
    pub due_date: Option<SqlTs>,
    pub progress: f64,
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
    pub completed_at: Option<SqlTs>,
}
