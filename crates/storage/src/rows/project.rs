//! Row struct for the `projects` table.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

/// Row struct for the `projects` table.
#[derive(Debug, Clone, FromRow)]
pub struct ProjectRow {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub color: String,
    pub tags: Vec<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
