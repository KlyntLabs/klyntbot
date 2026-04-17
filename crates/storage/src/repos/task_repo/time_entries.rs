//! Task time entry operations.

use super::TaskRepo;
use crate::error::StorageError;
use crate::rows::task::TaskTimeEntryRow;
use crate::sqlite_types::SqlTs;

impl TaskRepo {
    /// Add a time entry.
    pub async fn add_time_entry(
        &self,
        task_id: &str,
        source: &str,
        started_at: jiff::Timestamp,
        duration_secs: Option<i64>,
        note: Option<&str>,
        energy_level: Option<&str>,
    ) -> Result<TaskTimeEntryRow, StorageError> {
        let ended_at: Option<SqlTs> =
            duration_secs.map(|d| (started_at + jiff::SignedDuration::from_secs(d)).into());
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
        .bind(SqlTs::from(started_at))
        .bind(ended_at)
        .bind(duration_secs)
        .bind(note)
        .bind(energy_level)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(secs) = duration_secs {
            sqlx::query(
                "UPDATE tasks SET total_tracked_secs = total_tracked_secs + ?2, updated_at = (unixepoch('now') * 1000) WHERE id = ?1",
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
        let now_ms = jiff::Timestamp::now().as_millisecond();

        let row = sqlx::query_as::<_, TaskTimeEntryRow>(
            r#"
            UPDATE task_time_entries
            SET ended_at = ?3,
                duration_secs = (?3 - started_at) / 1000
            WHERE id = ?1 AND task_id = ?2 AND ended_at IS NULL
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(task_id)
        .bind(now_ms)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            StorageError::NotFound(format!("open time entry {entry_id} for task {task_id}"))
        })?;

        if let Some(secs) = row.duration_secs {
            sqlx::query(
                "UPDATE tasks SET total_tracked_secs = total_tracked_secs + ?2, updated_at = (unixepoch('now') * 1000) WHERE id = ?1",
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

    /// Fetch time entries within a date range (inclusive), oldest first.
    /// Returns entries with their parent task's title for display.
    /// Dates are YYYY-MM-DD strings; the range is [start 00:00, end+1 00:00).
    pub async fn time_entries_in_range(
        &self,
        start_date: &str,
        end_date: &str,
    ) -> Result<Vec<super::TimeEntryWithTask>, StorageError> {
        // Convert date strings to epoch ms bounds.
        let start_ms = date_str_to_ms(start_date, false);
        let end_ms = date_str_to_ms(end_date, true); // next-day exclusive
        let rows = sqlx::query_as::<_, super::TimeEntryWithTask>(
            r#"
            SELECT te.id, te.task_id, t.title AS task_title,
                   te.started_at, te.ended_at, te.duration_secs, te.note
            FROM task_time_entries te
            JOIN tasks t ON t.id = te.task_id
            WHERE te.started_at >= ?1
              AND te.started_at < ?2
            ORDER BY te.started_at ASC
            "#,
        )
        .bind(start_ms)
        .bind(end_ms)
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
    ) -> Result<Vec<crate::rows::task::TaskRow>, StorageError> {
        let start_ms = date_str_to_ms(start_date, false);
        let end_ms = date_str_to_ms(end_date, true);
        let rows = sqlx::query_as::<_, crate::rows::task::TaskRow>(
            r#"
            SELECT * FROM tasks
            WHERE is_template = 0 AND (
                (due_date >= ?1 AND due_date < ?2)
                OR (created_at >= ?1 AND created_at < ?2)
                OR (completed_at >= ?1 AND completed_at < ?2)
                OR (scheduled_start < ?2 AND scheduled_end > ?1)
            )
            ORDER BY COALESCE(scheduled_start, due_date, created_at) ASC
            "#,
        )
        .bind(start_ms)
        .bind(end_ms)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }
}

/// Convert a `YYYY-MM-DD` date string to epoch milliseconds.
/// If `next_day` is true, returns the start of the following day (exclusive upper bound).
fn date_str_to_ms(date: &str, next_day: bool) -> i64 {
    let d: jiff::civil::Date = date.parse().unwrap_or_else(|_| {
        jiff::Timestamp::now()
            .to_zoned(jiff::tz::TimeZone::UTC)
            .date()
    });
    let d = if next_day {
        d.checked_add(jiff::Span::new().days(1)).unwrap_or(d)
    } else {
        d
    };
    d.at(0, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .expect("UTC never fails for civil datetime")
        .timestamp()
        .as_millisecond()
}
