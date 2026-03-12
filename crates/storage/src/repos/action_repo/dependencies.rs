//! Dependency DAG operations for `action_dependencies`.

use crate::error::StorageError;
use crate::rows::action::{ActionDependencyRow, ActionRow};

use super::ActionRepo;

impl ActionRepo {
    /// Add a dependency edge (action_id is blocked by blocker_id).
    pub async fn add_dependency(
        &self,
        action_id: &str,
        blocker_id: &str,
    ) -> Result<(), StorageError> {
        let would_cycle = self.would_create_cycle(action_id, blocker_id).await?;
        if would_cycle {
            return Err(StorageError::Conflict(format!(
                "Adding dependency {action_id} -> {blocker_id} would create a cycle"
            )));
        }

        sqlx::query("INSERT INTO action_dependencies (action_id, blocker_id) VALUES (?1, ?2)")
            .bind(action_id)
            .bind(blocker_id)
            .execute(&self.pool)
            .await
            .map_err(|e| {
                if let sqlx::Error::Database(ref db_err) = e {
                    if db_err.constraint().is_some() {
                        return StorageError::Conflict(format!(
                            "Cannot add dependency: {action_id} -> {blocker_id}"
                        ));
                    }
                }
                StorageError::Sqlx(e)
            })?;

        Ok(())
    }

    /// Remove a dependency edge.
    pub async fn remove_dependency(
        &self,
        action_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM action_dependencies WHERE action_id = ?1 AND blocker_id = ?2")
                .bind(action_id)
                .bind(blocker_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get all blockers for an action.
    pub async fn get_blockers(&self, action_id: &str) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT t.* FROM actions t
            INNER JOIN action_dependencies d ON d.blocker_id = t.id
            WHERE d.action_id = ?1
            "#,
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get incomplete blockers for an action (status != 'done').
    pub async fn incomplete_blockers(
        &self,
        action_id: &str,
    ) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT t.* FROM actions t
            INNER JOIN action_dependencies d ON d.blocker_id = t.id
            WHERE d.action_id = ?1 AND t.status != 'done'
            "#,
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get actions blocked by this action.
    pub async fn get_blocking(&self, blocker_id: &str) -> Result<Vec<ActionRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionRow>(
            r#"
            SELECT t.* FROM actions t
            INNER JOIN action_dependencies d ON d.action_id = t.id
            WHERE d.blocker_id = ?1
            "#,
        )
        .bind(blocker_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get all dependency edges for an action.
    pub async fn get_dependencies(
        &self,
        action_id: &str,
    ) -> Result<Vec<ActionDependencyRow>, StorageError> {
        let rows = sqlx::query_as::<_, ActionDependencyRow>(
            "SELECT * FROM action_dependencies WHERE action_id = ?1",
        )
        .bind(action_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Check if adding (action_id → blocker_id) would create a cycle via recursive CTE.
    async fn would_create_cycle(
        &self,
        action_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE reachable AS (
                SELECT blocker_id AS node FROM action_dependencies WHERE action_id = ?2
                UNION
                SELECT d.blocker_id FROM action_dependencies d
                INNER JOIN reachable r ON d.action_id = r.node
            )
            SELECT EXISTS(SELECT 1 FROM reachable WHERE node = ?1)
            "#,
        )
        .bind(action_id)
        .bind(blocker_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }
}
