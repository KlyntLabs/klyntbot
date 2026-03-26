//! Repository for the `areas` table.

use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::rows::area::AreaRow;

/// Repository for area CRUD and reordering.
#[derive(Debug, Clone)]
pub struct AreaRepo {
    pool: SqlitePool,
}

impl AreaRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new area. Returns the inserted row.
    pub async fn create(&self, row: &AreaRow) -> Result<AreaRow, StorageError> {
        let inserted = sqlx::query_as::<_, AreaRow>(
            r#"
            INSERT INTO areas (id, name, description, color, icon, position, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(&row.description)
        .bind(&row.color)
        .bind(&row.icon)
        .bind(row.position)
        .bind(&row.status)
        .bind(row.created_at)
        .bind(row.updated_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Get an area by id. Returns `None` if not found.
    pub async fn get(&self, id: &str) -> Result<Option<AreaRow>, StorageError> {
        let row = sqlx::query_as::<_, AreaRow>("SELECT * FROM areas WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Get an area by id, returning `StorageError::NotFound` if missing.
    pub async fn get_or_err(&self, id: &str) -> Result<AreaRow, StorageError> {
        self.get(id).await?.ok_or_not_found(&format!("area {id}"))
    }

    /// List areas, optionally filtered by status. Ordered by position.
    pub async fn list(&self, status: Option<&str>) -> Result<Vec<AreaRow>, StorageError> {
        let rows = if let Some(s) = status {
            sqlx::query_as::<_, AreaRow>(
                "SELECT * FROM areas WHERE status = ?1 ORDER BY position, created_at",
            )
            .bind(s)
            .fetch_all(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, AreaRow>("SELECT * FROM areas ORDER BY position, created_at")
                .fetch_all(&self.pool)
                .await?
        };
        Ok(rows)
    }

    /// Update mutable fields on an area.
    pub async fn update(
        &self,
        id: &str,
        name: Option<&str>,
        description: Option<Option<&str>>,
        color: Option<&str>,
        icon: Option<Option<&str>>,
        status: Option<&str>,
    ) -> Result<AreaRow, StorageError> {
        let row = sqlx::query_as::<_, AreaRow>(
            r#"
            UPDATE areas SET
                name        = COALESCE(?2, name),
                description = CASE WHEN ?3 THEN ?4 ELSE description END,
                color       = COALESCE(?5, color),
                icon        = CASE WHEN ?6 THEN ?7 ELSE icon END,
                status      = COALESCE(?8, status),
                updated_at  = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(description.is_some())
        .bind(description.unwrap_or_default())
        .bind(color)
        .bind(icon.is_some())
        .bind(icon.unwrap_or_default())
        .bind(status)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("area {id}"))?;
        Ok(row)
    }

    /// Delete an area.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM areas WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update the position of an area.
    pub async fn reorder(&self, id: &str, position: i32) -> Result<AreaRow, StorageError> {
        let row = sqlx::query_as::<_, AreaRow>(
            r#"
            UPDATE areas SET position = ?2, updated_at = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(position)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("area {id}"))?;
        Ok(row)
    }

    /// Count projects in an area.
    pub async fn count_projects(&self, area_id: &str) -> Result<i64, StorageError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM projects WHERE area_id = ?1")
            .bind(area_id)
            .fetch_one(&self.pool)
            .await?;
        Ok(row.0)
    }

    /// Count tasks (non-template) in an area.
    pub async fn count_actions(&self, area_id: &str) -> Result<i64, StorageError> {
        let row: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM tasks WHERE area_id = ?1 AND is_template = FALSE")
                .bind(area_id)
                .fetch_one(&self.pool)
                .await?;
        Ok(row.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (AreaRepo, crate::StoragePool) {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        // The tasks table lives in the feature-tasks migration, not core migrations.
        // We need a minimal stub so the FK cascade chain (areas → projects →
        // custom_columns → custom_column_values → tasks) can be resolved.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                area_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'todo',
                position INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(pool.inner())
        .await
        .unwrap();
        let repo = AreaRepo::new(pool.inner().clone());
        (repo, pool)
    }

    fn sample_area(id: &str, name: &str) -> AreaRow {
        AreaRow {
            id: id.to_string(),
            name: name.to_string(),
            description: None,
            color: "blue".to_string(),
            icon: None,
            position: 0,
            status: "active".to_string(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_create_and_get_area() {
        let (repo, _pool) = setup().await;
        let area = sample_area("a1", "Work");
        let created = repo.create(&area).await.unwrap();
        assert_eq!(created.name, "Work");

        let fetched = repo.get("a1").await.unwrap().unwrap();
        assert_eq!(fetched.name, "Work");
    }

    #[tokio::test]
    async fn test_list_areas_filters_by_status() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_area("a1", "Work")).await.unwrap();
        let mut archived = sample_area("a2", "Old");
        archived.status = "archived".to_string();
        repo.create(&archived).await.unwrap();

        let active = repo.list(Some("active")).await.unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].name, "Work");

        let all = repo.list(None).await.unwrap();
        assert_eq!(all.len(), 2);
    }

    #[tokio::test]
    async fn test_update_area() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_area("a1", "Work")).await.unwrap();
        let updated = repo
            .update("a1", Some("Career"), None, Some("green"), None, None)
            .await
            .unwrap();
        assert_eq!(updated.name, "Career");
        assert_eq!(updated.color, "green");
    }

    #[tokio::test]
    async fn test_delete_area() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_area("a1", "Work")).await.unwrap();
        assert!(repo.delete("a1").await.unwrap());
        assert!(repo.get("a1").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_reorder() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_area("a1", "Work")).await.unwrap();
        let updated = repo.reorder("a1", 5).await.unwrap();
        assert_eq!(updated.position, 5);
    }
}
