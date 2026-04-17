//! Row structs for the `status_workflows` and `status_labels` tables.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `status_workflows` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusWorkflowRow {
    pub id: String,
    pub name: String,
    pub is_template: bool,
    pub is_global_default: bool,
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
}

/// Row struct for the `status_labels` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusLabelRow {
    pub id: String,
    pub workflow_id: String,
    pub name: String,
    pub color: String,
    pub status_group: String,
    pub position: i32,
    pub created_at: SqlTs,
}
