//! Row structs for the `custom_columns` and `custom_column_values` tables.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `custom_columns` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomColumnRow {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub column_type: String,
    pub options_json: Option<String>,
    pub position: i32,
    pub width: Option<i32>,
    pub created_at: SqlTs,
}

/// Row struct for the `custom_column_values` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomColumnValueRow {
    pub task_id: String,
    pub column_id: String,
    pub value_json: String,
}
