//! Time tracking operations for the `action_time_entries` join table.

use chrono::{DateTime, Utc};

use crate::error::StorageError;
use crate::rows::action::ActionTimeEntryRow;

use super::{ActionRepo, TimeEntryWithTask};

impl ActionRepo {
    /// Add a time entry.
    pub async fn add_time_entry(
        &self,
        action_id: &str,
        source: &str,
        started_at: DateTime<Utc>,
        duration_secs: Option<i64>,
        note: Option<&str>,
    ) -> Result<ActionTimeEntryRow, StorageError> {
        let ended_at = duration_secs.map(|d| started_at + chrono::Duration::seconds(d));
        let id = uuid::Uuid::new_v4();
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, ActionTimeEntryRow>(
            r#"
            INSERT INTO action_time_entries (id, action_id, source, started_at, ended_at, duration_secs, note)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(action_id)
        .bind(source)
        .bind(started_at)
        .bind(ended_at)
        .bind(duration_secs)
        .bind(note)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(secs) = duration_secs {
            sqlx::query(
                "UPDATE actions SET total_tracked_secs = total_tracked_secs + ?2, updated_at = datetime('now') WHERE id = ?1",
            )
            .bind(action_id)
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
        action_id: &str,
        entry_id: uuid::Uuid,
    ) -> Result<ActionTimeEntryRow, StorageError> {
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, ActionTimeEntryRow>(
            r#"
            UPDATE action_time_entries
            SET ended_at = datetime('now'),
                duration_secs = (unixepoch('now') - unixepoch(started_at))
            WHERE id = ?1 AND action_id = ?2 AND ended_at IS NULL
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(action_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            StorageError::NotFound(format!("open time entry {entry_id} for action {action_id}"))
        })?;

        if let Some(secs) = row.duration_secs {
            sqlx::query(
                "UPDATE actions SET total_tracked_secs = total_tracked_secs + ?2, updated_at = datetime('now') WHERE id = ?1",
            )
            .bind(action_id)
            .bind(secs)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(row)
    }

    /// List time entries for an action.
    pub async fn list_time_entries(
        &self,
        action_id: &str,
    ) -> Result<Vec<ActionTimeEntryRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionTimeEntryRow>(
            "SELECT * FROM action_time_entries WHERE action_id = ?1 ORDER BY started_at",
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Fetch time entries within a date range (inclusive), oldest first.
    /// Returns entries with their parent action's title for display.
    pub async fn time_entries_in_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<TimeEntryWithTask>, StorageError> {
        // Direct string comparison preserves index usage on started_at.
        let rows = sqlx::query_as::<_, TimeEntryWithTask>(
            r#"
            SELECT te.id, te.action_id, a.title AS action_title,
                   te.started_at, te.ended_at, te.duration_secs, te.note
            FROM action_time_entries te
            JOIN actions a ON a.id = te.action_id
            WHERE te.started_at >= ?1
              AND te.started_at < ?2
            ORDER BY te.started_at ASC
            "#,
        )
        .bind(start_date)
        .bind(format!("{end_date}T23:59:59Z"))
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Fetch tasks relevant to a date range for the timeline:
    /// due on the date, created during the range, or completed during the range.
    pub async fn tasks_for_timeline(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<crate::rows::action::ActionRow>, StorageError> {
        let end_bound = format!("{end_date}T23:59:59Z");
        let rows = sqlx::query_as::<_, crate::rows::action::ActionRow>(
            r#"
            SELECT * FROM actions
            WHERE is_template = 0 AND (
                (due_date >= ?1 AND due_date < ?2)
                OR (created_at >= ?1 AND created_at < ?2)
                OR (completed_at >= ?1 AND completed_at < ?2)
            )
            ORDER BY COALESCE(due_date, created_at) ASC
            "#,
        )
        .bind(start_date)
        .bind(&end_bound)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}
