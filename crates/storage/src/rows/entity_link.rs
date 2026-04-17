use crate::sqlite_types::SqlTs;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EntityLinkRow {
    pub id: String,
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub link_type: String,
    pub metadata: Option<String>,
    pub created_at: SqlTs,
}
