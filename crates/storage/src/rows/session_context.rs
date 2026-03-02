//! Row struct for the `session_context` table.

use serde::Serialize;
use sqlx::FromRow;

/// Row struct for the `session_context` table.
#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextRow {
    pub session_key: String,
    pub context_type: String,
    pub entity_kind: Option<String>,
    pub entity_id: Option<String>,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub is_ephemeral: bool,
    pub is_pinned: bool,
    pub created_at: String,
    pub updated_at: String,
}
