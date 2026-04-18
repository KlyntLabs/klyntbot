//! Repository for `task_recurrence_templates`.
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::task_recurrence::TaskRecurrenceTemplateRow;

#[derive(Debug, Clone)]
pub struct TaskRecurrenceRepo {
    pool: SqlitePool,
}

impl TaskRecurrenceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, row: &TaskRecurrenceTemplateRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO task_recurrence_templates
             (id, source_task_id, rrule, iana_tz, materialize_ahead,
              next_instance_at_ms, last_instance_at_ms, until_at_ms,
              count_remaining, enabled, created_at_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(id) DO UPDATE SET
                 rrule = EXCLUDED.rrule,
                 iana_tz = EXCLUDED.iana_tz,
                 materialize_ahead = EXCLUDED.materialize_ahead,
                 next_instance_at_ms = EXCLUDED.next_instance_at_ms,
                 last_instance_at_ms = EXCLUDED.last_instance_at_ms,
                 until_at_ms = EXCLUDED.until_at_ms,
                 count_remaining = EXCLUDED.count_remaining,
                 enabled = EXCLUDED.enabled",
        )
        .bind(&row.id)
        .bind(&row.source_task_id)
        .bind(&row.rrule)
        .bind(&row.iana_tz)
        .bind(row.materialize_ahead)
        .bind(row.next_instance_at_ms)
        .bind(row.last_instance_at_ms)
        .bind(row.until_at_ms)
        .bind(row.count_remaining)
        .bind(row.enabled)
        .bind(row.created_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<TaskRecurrenceTemplateRow>, StorageError> {
        Ok(sqlx::query_as::<_, TaskRecurrenceTemplateRow>(
            "SELECT * FROM task_recurrence_templates WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?)
    }

    pub async fn list_enabled(&self) -> Result<Vec<TaskRecurrenceTemplateRow>, StorageError> {
        Ok(sqlx::query_as::<_, TaskRecurrenceTemplateRow>(
            "SELECT * FROM task_recurrence_templates WHERE enabled = 1",
        )
        .fetch_all(&self.pool)
        .await?)
    }
}
