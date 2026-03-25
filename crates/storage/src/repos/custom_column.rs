//! Repository for `custom_columns` and `custom_column_values` tables.

use crate::error::{OptionExt, StorageError};
use crate::rows::custom_column::{CustomColumnRow, CustomColumnValueRow};

/// Repository for custom column CRUD operations.
#[derive(Debug, Clone)]
pub struct CustomColumnRepo {
    pool: sqlx::SqlitePool,
}

impl CustomColumnRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    // ── Column definitions ───────────────────────────────────

    /// List all columns for a project, ordered by position.
    pub async fn list_columns(
        &self,
        project_id: &str,
    ) -> Result<Vec<CustomColumnRow>, StorageError> {
        let rows = sqlx::query_as::<_, CustomColumnRow>(
            "SELECT * FROM custom_columns WHERE project_id = ?1 ORDER BY position",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Fetch a single column by ID.
    pub async fn get_column(&self, id: &str) -> Result<Option<CustomColumnRow>, StorageError> {
        let row =
            sqlx::query_as::<_, CustomColumnRow>("SELECT * FROM custom_columns WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// Create a new custom column. Returns the inserted row.
    pub async fn create_column(
        &self,
        row: &CustomColumnRow,
    ) -> Result<CustomColumnRow, StorageError> {
        let inserted = sqlx::query_as::<_, CustomColumnRow>(
            r#"
            INSERT INTO custom_columns (id, project_id, name, column_type, options_json, position, width)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.project_id)
        .bind(&row.name)
        .bind(&row.column_type)
        .bind(&row.options_json)
        .bind(row.position)
        .bind(row.width)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Partially update a column. Uses COALESCE for optional fields.
    pub async fn update_column(
        &self,
        id: &str,
        name: Option<&str>,
        options_json: Option<Option<&str>>,
        width: Option<i32>,
    ) -> Result<CustomColumnRow, StorageError> {
        // Handle the double-option for options_json:
        // None => don't change, Some(None) => set to NULL, Some(Some(v)) => set to v
        let (should_update_options, options_value) = match options_json {
            None => (false, None),
            Some(v) => (true, v),
        };

        let row = if should_update_options {
            sqlx::query_as::<_, CustomColumnRow>(
                r#"
                UPDATE custom_columns
                SET name         = COALESCE(?2, name),
                    options_json = ?3,
                    width        = COALESCE(?4, width)
                WHERE id = ?1
                RETURNING *
                "#,
            )
            .bind(id)
            .bind(name)
            .bind(options_value)
            .bind(width)
            .fetch_optional(&self.pool)
            .await?
        } else {
            sqlx::query_as::<_, CustomColumnRow>(
                r#"
                UPDATE custom_columns
                SET name  = COALESCE(?2, name),
                    width = COALESCE(?3, width)
                WHERE id = ?1
                RETURNING *
                "#,
            )
            .bind(id)
            .bind(name)
            .bind(width)
            .fetch_optional(&self.pool)
            .await?
        };

        row.ok_or_not_found(&format!("custom_column {id}"))
    }

    /// Delete a column by ID. Returns true if deleted.
    pub async fn delete_column(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM custom_columns WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Reorder columns within a project by setting positions from the given ID order.
    pub async fn reorder_columns(
        &self,
        project_id: &str,
        ids: &[String],
    ) -> Result<(), StorageError> {
        for (i, col_id) in ids.iter().enumerate() {
            sqlx::query(
                "UPDATE custom_columns SET position = ?1 WHERE id = ?2 AND project_id = ?3",
            )
            .bind(i as i32)
            .bind(col_id)
            .bind(project_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    // ── Column values ────────────────────────────────────────

    /// Get all custom column values for a task.
    pub async fn get_values(
        &self,
        task_id: &str,
    ) -> Result<Vec<CustomColumnValueRow>, StorageError> {
        let rows = sqlx::query_as::<_, CustomColumnValueRow>(
            "SELECT * FROM custom_column_values WHERE task_id = ?1",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Bulk-load custom column values for multiple tasks.
    pub async fn get_values_bulk(
        &self,
        task_ids: &[String],
    ) -> Result<Vec<CustomColumnValueRow>, StorageError> {
        if task_ids.is_empty() {
            return Ok(Vec::new());
        }
        // Build a query with IN clause using positional params
        let placeholders: Vec<String> = (1..=task_ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT * FROM custom_column_values WHERE task_id IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query_as::<_, CustomColumnValueRow>(&sql);
        for id in task_ids {
            query = query.bind(id);
        }
        let rows = query.fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Set (upsert) a custom column value for a task.
    pub async fn set_value(
        &self,
        task_id: &str,
        column_id: &str,
        value_json: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            r#"
            INSERT INTO custom_column_values (task_id, column_id, value_json)
            VALUES (?1, ?2, ?3)
            ON CONFLICT (task_id, column_id) DO UPDATE SET value_json = excluded.value_json
            "#,
        )
        .bind(task_id)
        .bind(column_id)
        .bind(value_json)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete a custom column value for a task. Returns true if deleted.
    pub async fn delete_value(&self, task_id: &str, column_id: &str) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM custom_column_values WHERE task_id = ?1 AND column_id = ?2")
                .bind(task_id)
                .bind(column_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (CustomColumnRepo, sqlx::SqlitePool) {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        // The tasks table lives in the feature-tasks migration, not core migrations.
        // We need a minimal stub so custom_column_values FK to tasks(id) resolves.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                area_id TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'todo',
                position INTEGER NOT NULL DEFAULT 0
            )",
        )
        .execute(&db)
        .await
        .unwrap();
        // Create test area and project
        sqlx::query("INSERT INTO areas (id, name, color, status) VALUES ('test-area', 'Test', '#000', 'active')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO projects (id, area_id, name, color) VALUES ('test-proj', 'test-area', 'Test Project', '#fff')")
            .execute(&db)
            .await
            .unwrap();
        (CustomColumnRepo::new(db.clone()), db)
    }

    #[tokio::test]
    async fn test_create_and_list_columns() {
        let (repo, _) = setup().await;

        let row = CustomColumnRow {
            id: "col_1".into(),
            project_id: "test-proj".into(),
            name: "Priority Score".into(),
            column_type: "number".into(),
            options_json: None,
            position: 0,
            width: Some(120),
            created_at: chrono::Utc::now(),
        };
        let created = repo.create_column(&row).await.unwrap();
        assert_eq!(created.id, "col_1");
        assert_eq!(created.name, "Priority Score");

        let row2 = CustomColumnRow {
            id: "col_2".into(),
            project_id: "test-proj".into(),
            name: "Status".into(),
            column_type: "dropdown".into(),
            options_json: Some(r#"["Low","Medium","High"]"#.into()),
            position: 1,
            width: None,
            created_at: chrono::Utc::now(),
        };
        repo.create_column(&row2).await.unwrap();

        let cols = repo.list_columns("test-proj").await.unwrap();
        assert_eq!(cols.len(), 2);
        assert_eq!(cols[0].name, "Priority Score");
        assert_eq!(cols[1].name, "Status");
    }

    #[tokio::test]
    async fn test_get_column() {
        let (repo, _) = setup().await;

        let row = CustomColumnRow {
            id: "col_get".into(),
            project_id: "test-proj".into(),
            name: "Test".into(),
            column_type: "text".into(),
            options_json: None,
            position: 0,
            width: None,
            created_at: chrono::Utc::now(),
        };
        repo.create_column(&row).await.unwrap();

        let fetched = repo.get_column("col_get").await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().name, "Test");

        let missing = repo.get_column("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_update_column() {
        let (repo, _) = setup().await;

        let row = CustomColumnRow {
            id: "col_upd".into(),
            project_id: "test-proj".into(),
            name: "Old Name".into(),
            column_type: "text".into(),
            options_json: None,
            position: 0,
            width: Some(100),
            created_at: chrono::Utc::now(),
        };
        repo.create_column(&row).await.unwrap();

        let updated = repo
            .update_column("col_upd", Some("New Name"), None, Some(200))
            .await
            .unwrap();
        assert_eq!(updated.name, "New Name");
        assert_eq!(updated.width, Some(200));

        // Update options_json to null
        let updated2 = repo
            .update_column("col_upd", None, Some(Some(r#"["A","B"]"#)), None)
            .await
            .unwrap();
        assert_eq!(updated2.options_json, Some(r#"["A","B"]"#.to_string()));

        // Set options_json back to null
        let updated3 = repo
            .update_column("col_upd", None, Some(None), None)
            .await
            .unwrap();
        assert!(updated3.options_json.is_none());
    }

    #[tokio::test]
    async fn test_delete_column() {
        let (repo, _) = setup().await;

        let row = CustomColumnRow {
            id: "col_del".into(),
            project_id: "test-proj".into(),
            name: "To Delete".into(),
            column_type: "text".into(),
            options_json: None,
            position: 0,
            width: None,
            created_at: chrono::Utc::now(),
        };
        repo.create_column(&row).await.unwrap();

        let deleted = repo.delete_column("col_del").await.unwrap();
        assert!(deleted);

        let deleted_again = repo.delete_column("col_del").await.unwrap();
        assert!(!deleted_again);
    }

    #[tokio::test]
    async fn test_reorder_columns() {
        let (repo, _) = setup().await;

        for (i, name) in ["A", "B", "C"].iter().enumerate() {
            let row = CustomColumnRow {
                id: format!("col_r{i}"),
                project_id: "test-proj".into(),
                name: name.to_string(),
                column_type: "text".into(),
                options_json: None,
                position: i as i32,
                width: None,
                created_at: chrono::Utc::now(),
            };
            repo.create_column(&row).await.unwrap();
        }

        // Reverse order
        repo.reorder_columns(
            "test-proj",
            &["col_r2".into(), "col_r1".into(), "col_r0".into()],
        )
        .await
        .unwrap();

        let cols = repo.list_columns("test-proj").await.unwrap();
        assert_eq!(cols[0].id, "col_r2");
        assert_eq!(cols[1].id, "col_r1");
        assert_eq!(cols[2].id, "col_r0");
    }

    #[tokio::test]
    async fn test_values_crud() {
        let (repo, db) = setup().await;

        // Create a column
        let col = CustomColumnRow {
            id: "col_val".into(),
            project_id: "test-proj".into(),
            name: "Score".into(),
            column_type: "number".into(),
            options_json: None,
            position: 0,
            width: None,
            created_at: chrono::Utc::now(),
        };
        repo.create_column(&col).await.unwrap();

        // Create a task
        sqlx::query(
            "INSERT INTO tasks (id, title, area_id, status, position) VALUES ('task-1', 'Test Task', 'test-area', 'todo', 0)",
        )
        .execute(&db)
        .await
        .unwrap();

        // Set value
        repo.set_value("task-1", "col_val", "42").await.unwrap();

        let values = repo.get_values("task-1").await.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].value_json, "42");

        // Upsert value
        repo.set_value("task-1", "col_val", "99").await.unwrap();
        let values = repo.get_values("task-1").await.unwrap();
        assert_eq!(values.len(), 1);
        assert_eq!(values[0].value_json, "99");

        // Delete value
        let deleted = repo.delete_value("task-1", "col_val").await.unwrap();
        assert!(deleted);

        let values = repo.get_values("task-1").await.unwrap();
        assert!(values.is_empty());
    }

    #[tokio::test]
    async fn test_values_bulk() {
        let (repo, db) = setup().await;

        let col = CustomColumnRow {
            id: "col_bulk".into(),
            project_id: "test-proj".into(),
            name: "Tag".into(),
            column_type: "text".into(),
            options_json: None,
            position: 0,
            width: None,
            created_at: chrono::Utc::now(),
        };
        repo.create_column(&col).await.unwrap();

        for i in 1..=3 {
            sqlx::query(
                "INSERT INTO tasks (id, title, area_id, status, position) VALUES (?1, ?2, 'test-area', 'todo', 0)",
            )
            .bind(format!("bulk-{i}"))
            .bind(format!("Task {i}"))
            .execute(&db)
            .await
            .unwrap();

            repo.set_value(&format!("bulk-{i}"), "col_bulk", &format!("\"val-{i}\""))
                .await
                .unwrap();
        }

        let values = repo
            .get_values_bulk(&["bulk-1".into(), "bulk-2".into(), "bulk-3".into()])
            .await
            .unwrap();
        assert_eq!(values.len(), 3);

        // Empty input returns empty
        let empty = repo.get_values_bulk(&[]).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_delete_column_cascades_values() {
        let (repo, db) = setup().await;

        let col = CustomColumnRow {
            id: "col_cascade".into(),
            project_id: "test-proj".into(),
            name: "Cascading".into(),
            column_type: "text".into(),
            options_json: None,
            position: 0,
            width: None,
            created_at: chrono::Utc::now(),
        };
        repo.create_column(&col).await.unwrap();

        sqlx::query(
            "INSERT INTO tasks (id, title, area_id, status, position) VALUES ('cascade-task', 'Cascade', 'test-area', 'todo', 0)",
        )
        .execute(&db)
        .await
        .unwrap();

        repo.set_value("cascade-task", "col_cascade", "\"hello\"")
            .await
            .unwrap();

        // Delete the column — values should cascade
        repo.delete_column("col_cascade").await.unwrap();

        let values = repo.get_values("cascade-task").await.unwrap();
        assert!(values.is_empty());
    }
}
