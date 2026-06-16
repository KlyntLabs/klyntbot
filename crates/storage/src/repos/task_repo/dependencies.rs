//! Task dependency (blocker) operations.

use super::TaskRepo;
use crate::error::StorageError;
use crate::rows::task::TaskRow;

impl TaskRepo {
    /// Add a dependency edge (task_id is blocked by blocker_id) with a dependency type.
    pub async fn add_dependency(
        &self,
        task_id: &str,
        blocker_id: &str,
        dep_type: &str,
    ) -> Result<(), StorageError> {
        let mut tx = self.pool.begin().await?;

        let would_cycle = self
            .would_create_cycle(&mut tx, task_id, blocker_id)
            .await?;
        if would_cycle {
            tx.rollback().await?;
            return Err(StorageError::Conflict(format!(
                "Adding dependency {task_id} -> {blocker_id} would create a cycle"
            )));
        }

        sqlx::query(
            "INSERT INTO task_dependencies (task_id, blocker_id, dep_type) VALUES (?1, ?2, ?3)",
        )
        .bind(task_id)
        .bind(blocker_id)
        .bind(dep_type)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint().is_some() {
                    return StorageError::Conflict(format!(
                        "Cannot add dependency: {task_id} -> {blocker_id}"
                    ));
                }
            }
            StorageError::Sqlx(e)
        })?;

        tx.commit().await?;
        Ok(())
    }

    /// Remove a dependency edge.
    pub async fn remove_dependency(
        &self,
        task_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM task_dependencies WHERE task_id = ?1 AND blocker_id = ?2")
                .bind(task_id)
                .bind(blocker_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get all blockers for a task.
    pub async fn get_blockers(&self, task_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN task_dependencies d ON d.blocker_id = t.id
            WHERE d.task_id = ?1
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get incomplete blockers for a task (completed = false).
    pub async fn incomplete_blockers(&self, task_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN task_dependencies d ON d.blocker_id = t.id
            WHERE d.task_id = ?1 AND t.completed = 0
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get tasks blocked by this task.
    pub async fn get_blocking(&self, blocker_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN task_dependencies d ON d.task_id = t.id
            WHERE d.blocker_id = ?1
            "#,
        )
        .bind(blocker_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn would_create_cycle(
        &self,
        conn: &mut sqlx::SqliteConnection,
        task_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE reachable AS (
                SELECT blocker_id AS node FROM task_dependencies WHERE task_id = ?2
                UNION
                SELECT d.blocker_id FROM task_dependencies d
                INNER JOIN reachable r ON d.task_id = r.node
            )
            SELECT EXISTS(SELECT 1 FROM reachable WHERE node = ?1)
            "#,
        )
        .bind(task_id)
        .bind(blocker_id)
        .fetch_optional(conn)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }
}
