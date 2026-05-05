//! Repository for the `project_sources` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::ProjectSourceRow;

/// Repository for project source material CRUD.
#[derive(Debug, Clone)]
pub struct ProjectSourceRepo {
    pool: SqlitePool,
}

impl ProjectSourceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert a new project source. Returns the inserted row.
    pub async fn create(
        &self,
        project_id: &str,
        source_type: &str,
        title: &str,
        content: Option<&str>,
        url: Option<&str>,
        file_path: Option<&str>,
    ) -> Result<ProjectSourceRow, StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        let row = sqlx::query_as::<_, ProjectSourceRow>(
            "INSERT INTO project_sources (id, project_id, source_type, title, content, url, file_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             RETURNING *",
        )
        .bind(&id)
        .bind(project_id)
        .bind(source_type)
        .bind(title)
        .bind(content)
        .bind(url)
        .bind(file_path)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get a project source by ID.
    pub async fn get(&self, id: &str) -> Result<Option<ProjectSourceRow>, StorageError> {
        let row =
            sqlx::query_as::<_, ProjectSourceRow>("SELECT * FROM project_sources WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// List all sources for a project, ordered by creation date descending.
    pub async fn list_by_project(
        &self,
        project_id: &str,
    ) -> Result<Vec<ProjectSourceRow>, StorageError> {
        let rows = sqlx::query_as::<_, ProjectSourceRow>(
            "SELECT * FROM project_sources WHERE project_id = ?1 ORDER BY created_at DESC",
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete a project source by ID.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM project_sources WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update the content of a source.
    pub async fn update_content(&self, id: &str, content: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE project_sources SET content = ?2, updated_at = (unixepoch('now') * 1000) WHERE id = ?1",
        )
        .bind(id)
        .bind(content)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    get_by_ids_impl!("project_sources", ProjectSourceRow);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::{AreaRepo, ProjectRepo};
    use crate::rows::area::AreaRow;
    use crate::rows::project::ProjectRow;

    async fn setup() -> (ProjectSourceRepo, crate::StoragePool) {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();

        // Create prerequisite area + project (FK constraints).
        AreaRepo::new(db.clone())
            .create(&AreaRow {
                id: "a1".into(),
                name: "Work".into(),
                description: None,
                color: "blue".into(),
                icon: None,
                position: 0,
                status: "active".into(),
                created_at: jiff::Timestamp::now().into(),
                updated_at: jiff::Timestamp::now().into(),
            })
            .await
            .unwrap();

        ProjectRepo::new(db.clone())
            .create(&ProjectRow {
                id: "proj-1".into(),
                area_id: "a1".into(),
                name: "Test Project".into(),
                description: None,
                color: "blue".into(),
                tags: vec![],
                status: "active".into(),
                created_at: jiff::Timestamp::now().into(),
                updated_at: jiff::Timestamp::now().into(),
                workflow_id: None,
                instructions: None,
                ai_personality: None,
                user_role: None,
                start_date: None,
                target_end_date: None,
                settings: None,
            })
            .await
            .unwrap();

        let repo = ProjectSourceRepo::new(db);
        (repo, pool)
    }

    #[tokio::test]
    async fn test_create_and_list() {
        let (repo, _pool) = setup().await;
        let source = repo
            .create(
                "proj-1",
                "link",
                "React Docs",
                Some("content"),
                Some("https://react.dev"),
                None,
            )
            .await
            .unwrap();
        assert_eq!(source.project_id, "proj-1");
        assert_eq!(source.source_type, "link");

        let sources = repo.list_by_project("proj-1").await.unwrap();
        assert_eq!(sources.len(), 1);
    }

    #[tokio::test]
    async fn test_get() {
        let (repo, _pool) = setup().await;
        let source = repo
            .create("proj-1", "snippet", "Auth flow", Some("code"), None, None)
            .await
            .unwrap();
        let fetched = repo.get(&source.id).await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().title, "Auth flow");
    }

    #[tokio::test]
    async fn test_delete() {
        let (repo, _pool) = setup().await;
        let source = repo
            .create("proj-1", "snippet", "Auth flow", Some("code"), None, None)
            .await
            .unwrap();
        let deleted = repo.delete(&source.id).await.unwrap();
        assert!(deleted);
        let sources = repo.list_by_project("proj-1").await.unwrap();
        assert!(sources.is_empty());
    }

    #[tokio::test]
    async fn test_update_content() {
        let (repo, _pool) = setup().await;
        let source = repo
            .create("proj-1", "snippet", "Auth flow", Some("old"), None, None)
            .await
            .unwrap();
        let updated = repo
            .update_content(&source.id, "new content")
            .await
            .unwrap();
        assert!(updated);
        let fetched = repo.get(&source.id).await.unwrap().unwrap();
        assert_eq!(fetched.content.as_deref(), Some("new content"));
    }
}
