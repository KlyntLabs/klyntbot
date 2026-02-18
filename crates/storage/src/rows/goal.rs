//! Row structs for `goals` and `goal_project_links` tables.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

/// Row struct for the `goals` table.
#[derive(Debug, Clone, FromRow)]
pub struct GoalRow {
    pub id: uuid::Uuid,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: i16,
    pub target_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metrics: serde_json::Value,
    pub metadata: serde_json::Value,
}

/// Row struct for the `goal_project_links` many-to-many table.
#[derive(Debug, Clone, FromRow)]
pub struct GoalProjectLinkRow {
    pub goal_id: uuid::Uuid,
    pub project_id: String,
}
