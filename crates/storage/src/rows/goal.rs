//! Row structs for `goals` and `goal_project_links` tables.

use chrono::{DateTime, Utc};
use sqlx::FromRow;

/// Row struct for the `goals` table.
///
/// Note: The underlying table still has dead `metrics` and `metadata` JSON
/// columns from the original schema. We avoid `SELECT *` and use an explicit
/// column list (`GOAL_COLS`) so that `FromRow` never sees those columns.
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
    pub plans_completed: i32,
    pub plans_failed: i32,
    pub avg_duration_ms: Option<i64>,
    pub last_plan_at: Option<DateTime<Utc>>,
}

/// Row struct for the `goal_project_links` many-to-many table.
#[derive(Debug, Clone, FromRow)]
pub struct GoalProjectLinkRow {
    pub goal_id: uuid::Uuid,
    pub project_id: String,
}
