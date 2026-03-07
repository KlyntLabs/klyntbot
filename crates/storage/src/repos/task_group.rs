//! Repository for the `task_groups` table.

use crate::error::{OptionExt, StorageError};
use crate::rows::task_group::TaskGroupRow;

/// Repository for task group CRUD operations.
#[derive(Debug, Clone)]
pub struct TaskGroupRepo {
    pool: sqlx::SqlitePool,
}

impl TaskGroupRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    /// List groups for a project (or all groups if `project_id` is None), ordered by position.
    pub async fn list(&self, project_id: Option<&str>) -> Result<Vec<TaskGroupRow>, StorageError> {
        match project_id {
            Some(pid) => {
                let rows = sqlx::query_as::<_, TaskGroupRow>(
                    "SELECT * FROM task_groups WHERE project_id = ?1 ORDER BY position",
                )
                .bind(pid)
                .fetch_all(&self.pool)
                .await?;
                Ok(rows)
            }
            None => {
                let rows = sqlx::query_as::<_, TaskGroupRow>(
                    "SELECT * FROM task_groups ORDER BY position",
                )
                .fetch_all(&self.pool)
                .await?;
                Ok(rows)
            }
        }
    }

    /// Fetch a single group by ID.
    pub async fn get(&self, id: &str) -> Result<Option<TaskGroupRow>, StorageError> {
        let row = sqlx::query_as::<_, TaskGroupRow>("SELECT * FROM task_groups WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Create a new task group. Returns the inserted row.
    pub async fn create(
        &self,
        project_id: Option<&str>,
        name: &str,
        color: Option<&str>,
        position: i32,
    ) -> Result<TaskGroupRow, StorageError> {
        let id = format!("grp_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let row = sqlx::query_as::<_, TaskGroupRow>(
            r#"
            INSERT INTO task_groups (id, project_id, name, color, position)
            VALUES (?1, ?2, ?3, ?4, ?5)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(project_id)
        .bind(name)
        .bind(color)
        .bind(position)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Partially update a task group. Only non-None fields are overwritten.
    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        color: Option<&str>,
        position: Option<i32>,
    ) -> Result<TaskGroupRow, StorageError> {
        let row = sqlx::query_as::<_, TaskGroupRow>(
            r#"
            UPDATE task_groups
            SET name     = COALESCE(?2, name),
                color    = COALESCE(?3, color),
                position = COALESCE(?4, position)
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(color)
        .bind(position)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("task_group {id}"))?;
        Ok(row)
    }

    /// Delete a task group by ID. Returns true if deleted.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM task_groups WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Reorder groups within a project by setting positions from the given ID order.
    pub async fn reorder(
        &self,
        project_id: Option<&str>,
        group_ids: &[String],
    ) -> Result<(), StorageError> {
        for (i, group_id) in group_ids.iter().enumerate() {
            match project_id {
                Some(pid) => {
                    sqlx::query(
                        "UPDATE task_groups SET position = ?1 WHERE id = ?2 AND project_id = ?3",
                    )
                    .bind(i as i32)
                    .bind(group_id)
                    .bind(pid)
                    .execute(&self.pool)
                    .await?;
                }
                None => {
                    sqlx::query(
                        "UPDATE task_groups SET position = ?1 WHERE id = ?2 AND project_id IS NULL",
                    )
                    .bind(i as i32)
                    .bind(group_id)
                    .execute(&self.pool)
                    .await?;
                }
            }
        }
        Ok(())
    }

    /// Count tasks in a group.
    pub async fn count_tasks(&self, group_id: &str) -> Result<u32, StorageError> {
        let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM actions WHERE group_id = ?1")
            .bind(group_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(count.0 as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> TaskGroupRepo {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        // Create a test project first (needs area)
        let db = pool.inner();
        sqlx::query("INSERT INTO areas (id, name, color, status) VALUES ('test-area', 'Test', '#000', 'active')")
            .execute(db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO projects (id, area_id, name, color) VALUES ('test-proj', 'test-area', 'Test Project', '#fff')")
            .execute(db)
            .await
            .unwrap();
        TaskGroupRepo::new(db.clone())
    }

    #[tokio::test]
    async fn test_create_and_list_groups() {
        let repo = setup().await;

        let g1 = repo
            .create(Some("test-proj"), "Sprint 1", Some("#ff0000"), 0)
            .await
            .unwrap();
        assert!(g1.id.starts_with("grp_"));
        assert_eq!(g1.name, "Sprint 1");
        assert_eq!(g1.project_id, Some("test-proj".to_string()));

        let g2 = repo
            .create(Some("test-proj"), "Sprint 2", None, 1)
            .await
            .unwrap();
        assert_eq!(g2.position, 1);

        let groups = repo.list(Some("test-proj")).await.unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].name, "Sprint 1");
        assert_eq!(groups[1].name, "Sprint 2");

        // List all
        let all = repo.list(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_update_group() {
        let repo = setup().await;

        let g = repo
            .create(Some("test-proj"), "Old Name", Some("#aaa"), 0)
            .await
            .unwrap();

        let updated = repo
            .update(&g.id, Some("New Name"), Some("#bbb"), None)
            .await
            .unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.color, Some("#bbb".to_string()));
        assert_eq!(updated.position, 0); // unchanged
    }

    #[tokio::test]
    async fn test_delete_group() {
        let repo = setup().await;

        let g = repo
            .create(Some("test-proj"), "To Delete", None, 0)
            .await
            .unwrap();

        let deleted = repo.delete(&g.id).await.unwrap();
        assert!(deleted);

        let fetched = repo.get(&g.id).await.unwrap();
        assert!(fetched.is_none());

        // Delete non-existent returns false
        let deleted_again = repo.delete(&g.id).await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_reorder_groups() {
        let repo = setup().await;

        let g1 = repo
            .create(Some("test-proj"), "First", None, 0)
            .await
            .unwrap();
        let g2 = repo
            .create(Some("test-proj"), "Second", None, 1)
            .await
            .unwrap();
        let g3 = repo
            .create(Some("test-proj"), "Third", None, 2)
            .await
            .unwrap();

        // Reverse the order
        repo.reorder(
            Some("test-proj"),
            &[g3.id.clone(), g2.id.clone(), g1.id.clone()],
        )
        .await
        .unwrap();

        let groups = repo.list(Some("test-proj")).await.unwrap();
        assert_eq!(groups[0].id, g3.id);
        assert_eq!(groups[1].id, g2.id);
        assert_eq!(groups[2].id, g1.id);
    }
}
