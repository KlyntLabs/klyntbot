//! Repository for the `actions` table and its join tables
//! (`action_attachments`, `action_time_entries`, `action_dependencies`).
//!
//! Split into submodules by concern — mirrors the `task_repo/` layout:
//!   - `core`         — CRUD operations
//!   - `filter`       — list/search
//!   - `focus`        — focus-slot management
//!   - `dependencies` — dependency DAG
//!   - `attachments`  — attachment join table
//!   - `time_entries` — time tracking
//!   - `hierarchy`    — parent/child tree traversal
//!   - `aggregation`  — counts, summaries, LLM context

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

mod aggregation;
mod attachments;
mod core;
mod dependencies;
mod filter;
mod focus;
mod hierarchy;
#[cfg(test)]
mod tests;
mod time_entries;

/// Filter criteria for listing actions.
#[derive(Debug, Default, Clone)]
pub struct ActionFilter {
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub key_result_id: Option<String>,
    pub unassigned: bool,
    pub root_only: bool,
    pub priority_min: Option<i16>,
    pub due_after: Option<DateTime<Utc>>,
    pub due_before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub templates_only: bool,
    pub status_group: Option<String>,
    pub group_id: Option<String>,
}

/// Aggregate counts by status.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionSummary {
    pub todo: i64,
    pub doing: i64,
    pub done: i64,
    pub total: i64,
}

/// Patch struct for partial updates.
#[derive(Debug, Default, Clone)]
pub struct ActionPatch {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub calendar_event_uid: Option<Option<String>>,
    pub next_instance_date: Option<Option<DateTime<Utc>>>,
    pub last_reminded_at: Option<Option<DateTime<Utc>>>,
    pub estimated_minutes: Option<Option<i32>>,
    pub recurrence_rule: Option<Option<String>>,
    pub area_id: Option<String>,
    pub project_id: Option<Option<String>>,
    pub key_result_id: Option<Option<String>>,
    pub status_label_id: Option<Option<String>>,
    pub position: Option<i32>,
    pub group_id: Option<Option<String>>,
}

/// A time entry joined with the parent action's title, for timeline display.
#[derive(Debug, Clone, sqlx::FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimeEntryWithTask {
    pub id: uuid::Uuid,
    pub action_id: String,
    pub action_title: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
    pub duration_secs: Option<i64>,
    pub note: Option<String>,
}

/// Repository for action CRUD, hierarchy, focus, dependencies, attachments, and time tracking.
#[derive(Debug, Clone)]
pub struct ActionRepo {
    pub(super) pool: SqlitePool,
}

impl ActionRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}
