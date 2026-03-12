//! Task time entry operations.

use chrono::{DateTime, Utc};

use super::TaskRepo;
use crate::error::StorageError;
use crate::rows::task::TaskTimeEntryRow;

impl TaskRepo {
    /// Add a time entry.
    pub async fn add_time_entry(
        &self,
        task_id: &str,
        source: &str,
        started_at: DateTime<Utc>,
        duration_secs: Option<i64>,
        note: Option<&str>,
        energy_level: Option<&str>,
    ) -> Result<TaskTimeEntryRow, StorageError> {
        let ended_at = duration_secs.map(|d| started_at + chrono::Duration::seconds(d));
        let id = uuid::Uuid::new_v4();
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, TaskTimeEntryRow>(
            r#"
            INSERT INTO task_time_entries (id, task_id, source, started_at, ended_at, duration_secs, note, energy_level)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(task_id)
        .bind(source)
        .bind(started_at)
        .bind(ended_at)
        .bind(duration_secs)
        .bind(note)
        .bind(energy_level)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(secs) = duration_secs {
            sqlx::query(
                "UPDATE tasks SET total_tracked_secs = total_tracked_secs + ?2, updated_at = datetime('now') WHERE id = ?1",
            )
            .bind(task_id)
            .bind(secs)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(row)
    }

    /// Close an open time entry.
    pub async fn close_time_entry(
        &self,
        task_id: &str,
        entry_id: uuid::Uuid,
    ) -> Result<TaskTimeEntryRow, StorageError> {
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, TaskTimeEntryRow>(
            r#"
            UPDATE task_time_entries
            SET ended_at = datetime('now'),
                duration_secs = (unixepoch('now') - unixepoch(started_at))
            WHERE id = ?1 AND task_id = ?2 AND ended_at IS NULL
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            StorageError::NotFound(format!("open time entry {entry_id} for task {task_id}"))
        })?;

        if let Some(secs) = row.duration_secs {
            sqlx::query(
                "UPDATE tasks SET total_tracked_secs = total_tracked_secs + ?2, updated_at = datetime('now') WHERE id = ?1",
            )
            .bind(task_id)
            .bind(secs)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(row)
    }

    /// List time entries for a task.
    pub async fn list_time_entries(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskTimeEntryRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskTimeEntryRow>(
            "SELECT * FROM task_time_entries WHERE task_id = ?1 ORDER BY started_at",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
