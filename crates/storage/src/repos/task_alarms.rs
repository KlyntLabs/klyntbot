//! Repository for `task_alarms`.
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::task_alarm::TaskAlarmRow;

#[derive(Debug, Clone)]
pub struct TaskAlarmsRepo {
    pool: SqlitePool,
}

impl TaskAlarmsRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, row: &TaskAlarmRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO task_alarms
             (id, task_id, rule_type, offset_secs, day_offset, time_of_day, iana_tz,
              absolute_fire_at_ms, channel_mask, priority_override, misfire_policy,
              grace_window_secs, created_at_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(&row.rule_type)
        .bind(row.offset_secs)
        .bind(row.day_offset)
        .bind(&row.time_of_day)
        .bind(&row.iana_tz)
        .bind(row.absolute_fire_at_ms)
        .bind(row.channel_mask)
        .bind(&row.priority_override)
        .bind(&row.misfire_policy)
        .bind(row.grace_window_secs)
        .bind(row.created_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_by_task(&self, task_id: &str) -> Result<Vec<TaskAlarmRow>, StorageError> {
        Ok(sqlx::query_as::<_, TaskAlarmRow>(
            "SELECT * FROM task_alarms WHERE task_id = ?1 ORDER BY created_at_ms ASC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?)
    }

    pub async fn delete_by_task(&self, task_id: &str) -> Result<u64, StorageError> {
        Ok(sqlx::query("DELETE FROM task_alarms WHERE task_id = ?1")
            .bind(task_id)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }

    pub async fn delete_by_id(&self, id: &str) -> Result<u64, StorageError> {
        Ok(sqlx::query("DELETE FROM task_alarms WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?
            .rows_affected())
    }
}
