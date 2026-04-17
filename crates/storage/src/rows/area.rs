//! Row struct for the `areas` table.

use crate::sqlite_types::SqlTs;
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AreaRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub icon: Option<String>,
    pub position: i32,
    pub status: String,
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
}
