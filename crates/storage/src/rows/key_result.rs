//! Row struct for the `key_results` table.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyResultRow {
    pub id: String,
    pub objective_id: String,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub tracking_mode: String,
    pub target_value: Option<f64>,
    pub current_value: f64,
    pub unit: Option<String>,
    pub progress: f64,
    pub due_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
