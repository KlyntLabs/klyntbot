use crate::sqlite_types::SqlTs;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSourceRow {
    pub id: String,
    pub project_id: String,
    pub source_type: String,
    pub title: String,
    pub content: Option<String>,
    pub url: Option<String>,
    pub file_path: Option<String>,
    pub embedding_id: Option<String>,
    pub metadata: Option<String>,
    pub tags: String,
    pub created_at: SqlTs,
    pub updated_at: SqlTs,
}
