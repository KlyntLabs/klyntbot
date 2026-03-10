//! Repository for the `entity_links` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::EntityLinkRow;

/// Repository for cross-entity link CRUD.
#[derive(Debug, Clone)]
pub struct EntityLinkRepo {
    pool: SqlitePool,
}

impl EntityLinkRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Create a new entity link. Returns the inserted row.
    pub async fn create(
        &self,
        source_kind: &str,
        source_id: &str,
        target_kind: &str,
        target_id: &str,
        link_type: &str,
        metadata: Option<&str>,
    ) -> Result<EntityLinkRow, StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        let row = sqlx::query_as::<_, EntityLinkRow>(
            "INSERT INTO entity_links (id, source_kind, source_id, target_kind, target_id, link_type, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             RETURNING *",
        )
        .bind(&id)
        .bind(source_kind)
        .bind(source_id)
        .bind(target_kind)
        .bind(target_id)
        .bind(link_type)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Delete a link by ID.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM entity_links WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all links where the given entity is either source or target (bidirectional).
    pub async fn list_by_entity(
        &self,
        kind: &str,
        id: &str,
    ) -> Result<Vec<EntityLinkRow>, StorageError> {
        let rows = sqlx::query_as::<_, EntityLinkRow>(
            "SELECT * FROM entity_links WHERE source_kind = ?1 AND source_id = ?2
             UNION ALL
             SELECT * FROM entity_links WHERE target_kind = ?1 AND target_id = ?2
             ORDER BY created_at DESC",
        )
        .bind(kind)
        .bind(id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get all links where a project is either source or target.
    pub async fn get_project_links(
        &self,
        project_id: &str,
    ) -> Result<Vec<EntityLinkRow>, StorageError> {
        self.list_by_entity("project", project_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> (EntityLinkRepo, crate::StoragePool) {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = EntityLinkRepo::new(pool.inner().clone());
        (repo, pool)
    }

    #[tokio::test]
    async fn test_create_and_list() {
        let (repo, _pool) = setup().await;
        let link = repo
            .create("task", "task-1", "note", "note-1", "related", None)
            .await
            .unwrap();
        assert_eq!(link.source_kind, "task");
        assert_eq!(link.link_type, "related");

        let links = repo.list_by_entity("task", "task-1").await.unwrap();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].target_id, "note-1");
    }

    #[tokio::test]
    async fn test_delete() {
        let (repo, _pool) = setup().await;
        let link = repo
            .create("task", "t1", "note", "n1", "related", None)
            .await
            .unwrap();
        let deleted = repo.delete(&link.id).await.unwrap();
        assert!(deleted);
        let links = repo.list_by_entity("task", "t1").await.unwrap();
        assert!(links.is_empty());
    }

    #[tokio::test]
    async fn test_unique_constraint() {
        let (repo, _pool) = setup().await;
        repo.create("task", "t1", "note", "n1", "related", None)
            .await
            .unwrap();
        // Same link again should fail (unique constraint violation)
        let result = repo
            .create("task", "t1", "note", "n1", "related", None)
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_bidirectional_query() {
        let (repo, _pool) = setup().await;
        repo.create("task", "t1", "note", "n1", "related", None)
            .await
            .unwrap();
        // Should find from both directions
        let from_task = repo.list_by_entity("task", "t1").await.unwrap();
        let from_note = repo.list_by_entity("note", "n1").await.unwrap();
        assert_eq!(from_task.len(), 1);
        assert_eq!(from_note.len(), 1);
    }

    #[tokio::test]
    async fn test_get_project_links() {
        let (repo, _pool) = setup().await;
        repo.create("project", "p1", "task", "t1", "related", None)
            .await
            .unwrap();
        repo.create("note", "n1", "project", "p1", "related", None)
            .await
            .unwrap();
        let links = repo.get_project_links("p1").await.unwrap();
        assert_eq!(links.len(), 2);
    }
}
