//! Repository for the `session_context` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::session_context::SessionContextRow;

/// Parameters for upserting a session context.
pub struct SessionContextParams<'a> {
    pub session_key: &'a str,
    pub context_type: &'a str,
    pub entity_kind: Option<&'a str>,
    pub entity_id: Option<&'a str>,
    pub area_id: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub is_ephemeral: bool,
}

/// Repository for session context persistence and querying.
#[derive(Debug, Clone)]
pub struct SessionContextRepo {
    pool: SqlitePool,
}

impl SessionContextRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Upsert a session context — insert or update on conflict.
    pub async fn upsert(
        &self,
        params: SessionContextParams<'_>,
    ) -> Result<SessionContextRow, StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let row = sqlx::query_as::<_, SessionContextRow>(
            "INSERT INTO session_context
                 (session_key, context_type, entity_kind, entity_id, area_id, project_id, is_ephemeral, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT (session_key) DO UPDATE SET
                 context_type = excluded.context_type,
                 entity_kind  = excluded.entity_kind,
                 entity_id    = excluded.entity_id,
                 area_id      = excluded.area_id,
                 project_id   = excluded.project_id,
                 is_ephemeral = excluded.is_ephemeral,
                 updated_at   = excluded.updated_at
             RETURNING *",
        )
        .bind(params.session_key)
        .bind(params.context_type)
        .bind(params.entity_kind)
        .bind(params.entity_id)
        .bind(params.area_id)
        .bind(params.project_id)
        .bind(params.is_ephemeral)
        .bind(now)
        .bind(now)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Get session context by session key.
    pub async fn get(&self, session_key: &str) -> Result<Option<SessionContextRow>, StorageError> {
        let row = sqlx::query_as::<_, SessionContextRow>(
            "SELECT * FROM session_context WHERE session_key = ?1",
        )
        .bind(session_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Update mutable fields on an existing session context.
    pub async fn update(
        &self,
        session_key: &str,
        context_type: Option<&str>,
        entity_kind: Option<&str>,
        entity_id: Option<&str>,
        area_id: Option<&str>,
        project_id: Option<&str>,
    ) -> Result<Option<SessionContextRow>, StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let row = sqlx::query_as::<_, SessionContextRow>(
            "UPDATE session_context SET
                 context_type = COALESCE(?2, context_type),
                 entity_kind  = COALESCE(?3, entity_kind),
                 entity_id    = COALESCE(?4, entity_id),
                 area_id      = COALESCE(?5, area_id),
                 project_id   = COALESCE(?6, project_id),
                 updated_at   = ?7
             WHERE session_key = ?1
             RETURNING *",
        )
        .bind(session_key)
        .bind(context_type)
        .bind(entity_kind)
        .bind(entity_id)
        .bind(area_id)
        .bind(project_id)
        .bind(now)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Delete a session context.
    pub async fn delete(&self, session_key: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM session_context WHERE session_key = ?1")
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List session contexts belonging to a specific area.
    pub async fn list_by_area(
        &self,
        area_id: &str,
    ) -> Result<Vec<SessionContextRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionContextRow>(
            "SELECT * FROM session_context WHERE area_id = ?1 ORDER BY updated_at DESC",
        )
        .bind(area_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List visible session contexts: non-ephemeral, or pinned ephemeral.
    pub async fn list_visible(&self) -> Result<Vec<SessionContextRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionContextRow>(
            "SELECT * FROM session_context
             WHERE is_ephemeral = 0 OR is_pinned = 1
             ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete ephemeral (non-pinned) session contexts older than `days`.
    pub async fn cleanup_old_ephemeral(&self, days: i64) -> Result<u64, StorageError> {
        let cutoff = jiff::Timestamp::now() - jiff::SignedDuration::from_hours((days) * 24);
        let result = sqlx::query(
            "DELETE FROM session_context
             WHERE is_ephemeral = 1 AND is_pinned = 0 AND updated_at < ?1",
        )
        .bind(cutoff.as_millisecond())
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Pin a session context (makes ephemeral sessions visible in the thread list).
    pub async fn pin(&self, session_key: &str) -> Result<bool, StorageError> {
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let result = sqlx::query(
            "UPDATE session_context SET is_pinned = 1, updated_at = ?2 WHERE session_key = ?1",
        )
        .bind(session_key)
        .bind(now)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    async fn test_repo() -> (SessionContextRepo, crate::repos::SessionRepo) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        (
            SessionContextRepo::new(db.clone()),
            crate::repos::SessionRepo::new(db),
        )
    }

    /// Helper: create a session so foreign key constraint is satisfied.
    async fn create_session(session_repo: &crate::repos::SessionRepo, key: &str) {
        session_repo
            .upsert_session(key, &serde_json::json!({}), None)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn session_context_upsert_and_get() {
        let (repo, sessions) = test_repo().await;
        create_session(&sessions, "test:ctx1").await;

        let row = repo
            .upsert(SessionContextParams {
                session_key: "test:ctx1",
                context_type: "project",
                entity_kind: Some("project"),
                entity_id: Some("p-1"),
                area_id: Some("a-1"),
                project_id: None,
                is_ephemeral: false,
            })
            .await
            .unwrap();
        assert_eq!(row.session_key, "test:ctx1");
        assert_eq!(row.context_type, "project");
        assert_eq!(row.entity_kind.as_deref(), Some("project"));
        assert_eq!(row.entity_id.as_deref(), Some("p-1"));
        assert!(!row.is_ephemeral);
        assert!(!row.is_pinned);

        let fetched = repo.get("test:ctx1").await.unwrap().unwrap();
        assert_eq!(fetched.session_key, "test:ctx1");
        assert_eq!(fetched.context_type, "project");
    }

    #[tokio::test]
    async fn session_context_upsert_updates_on_conflict() {
        let (repo, sessions) = test_repo().await;
        create_session(&sessions, "test:ctx2").await;

        repo.upsert(SessionContextParams {
            session_key: "test:ctx2",
            context_type: "general",
            entity_kind: None,
            entity_id: None,
            area_id: None,
            project_id: None,
            is_ephemeral: false,
        })
        .await
        .unwrap();
        let updated = repo
            .upsert(SessionContextParams {
                session_key: "test:ctx2",
                context_type: "task",
                entity_kind: Some("task"),
                entity_id: Some("t-1"),
                area_id: None,
                project_id: Some("p-1"),
                is_ephemeral: false,
            })
            .await
            .unwrap();
        assert_eq!(updated.context_type, "task");
        assert_eq!(updated.entity_kind.as_deref(), Some("task"));
        assert_eq!(updated.project_id.as_deref(), Some("p-1"));
    }

    #[tokio::test]
    async fn session_context_update() {
        let (repo, sessions) = test_repo().await;
        create_session(&sessions, "test:ctx3").await;

        repo.upsert(SessionContextParams {
            session_key: "test:ctx3",
            context_type: "general",
            entity_kind: None,
            entity_id: None,
            area_id: None,
            project_id: None,
            is_ephemeral: false,
        })
        .await
        .unwrap();
        let updated = repo
            .update(
                "test:ctx3",
                Some("project"),
                Some("project"),
                Some("p-1"),
                None,
                None,
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(updated.context_type, "project");
        assert_eq!(updated.entity_kind.as_deref(), Some("project"));
    }

    #[tokio::test]
    async fn session_context_delete() {
        let (repo, sessions) = test_repo().await;
        create_session(&sessions, "test:ctx4").await;

        repo.upsert(SessionContextParams {
            session_key: "test:ctx4",
            context_type: "general",
            entity_kind: None,
            entity_id: None,
            area_id: None,
            project_id: None,
            is_ephemeral: false,
        })
        .await
        .unwrap();
        assert!(repo.delete("test:ctx4").await.unwrap());
        assert!(repo.get("test:ctx4").await.unwrap().is_none());
        assert!(!repo.delete("test:ctx4").await.unwrap());
    }

    #[tokio::test]
    async fn session_context_list_by_area() {
        let (repo, sessions) = test_repo().await;
        create_session(&sessions, "test:a1").await;
        create_session(&sessions, "test:a2").await;
        create_session(&sessions, "test:b1").await;

        repo.upsert(SessionContextParams {
            session_key: "test:a1",
            context_type: "project",
            entity_kind: Some("project"),
            entity_id: Some("p-1"),
            area_id: Some("area-1"),
            project_id: None,
            is_ephemeral: false,
        })
        .await
        .unwrap();
        repo.upsert(SessionContextParams {
            session_key: "test:a2",
            context_type: "project",
            entity_kind: Some("project"),
            entity_id: Some("p-2"),
            area_id: Some("area-1"),
            project_id: None,
            is_ephemeral: false,
        })
        .await
        .unwrap();
        repo.upsert(SessionContextParams {
            session_key: "test:b1",
            context_type: "project",
            entity_kind: Some("project"),
            entity_id: Some("p-3"),
            area_id: Some("area-2"),
            project_id: None,
            is_ephemeral: false,
        })
        .await
        .unwrap();

        let area1 = repo.list_by_area("area-1").await.unwrap();
        assert_eq!(area1.len(), 2);

        let area2 = repo.list_by_area("area-2").await.unwrap();
        assert_eq!(area2.len(), 1);
    }

    #[tokio::test]
    async fn session_context_list_visible() {
        let (repo, sessions) = test_repo().await;
        create_session(&sessions, "test:v1").await;
        create_session(&sessions, "test:v2").await;
        create_session(&sessions, "test:v3").await;

        // Non-ephemeral → visible
        repo.upsert(SessionContextParams {
            session_key: "test:v1",
            context_type: "general",
            entity_kind: None,
            entity_id: None,
            area_id: None,
            project_id: None,
            is_ephemeral: false,
        })
        .await
        .unwrap();
        // Ephemeral, not pinned → hidden
        repo.upsert(SessionContextParams {
            session_key: "test:v2",
            context_type: "task",
            entity_kind: Some("task"),
            entity_id: Some("t-1"),
            area_id: None,
            project_id: None,
            is_ephemeral: true,
        })
        .await
        .unwrap();
        // Ephemeral, pinned → visible
        repo.upsert(SessionContextParams {
            session_key: "test:v3",
            context_type: "task",
            entity_kind: Some("task"),
            entity_id: Some("t-2"),
            area_id: None,
            project_id: None,
            is_ephemeral: true,
        })
        .await
        .unwrap();
        repo.pin("test:v3").await.unwrap();

        let visible = repo.list_visible().await.unwrap();
        assert_eq!(visible.len(), 2);
        let keys: Vec<&str> = visible.iter().map(|r| r.session_key.as_str()).collect();
        assert!(keys.contains(&"test:v1"));
        assert!(keys.contains(&"test:v3"));
    }

    #[tokio::test]
    async fn session_context_cleanup_ephemeral() {
        let (repo, sessions) = test_repo().await;
        create_session(&sessions, "test:e1").await;
        create_session(&sessions, "test:e2").await;

        // Create two ephemeral contexts
        repo.upsert(SessionContextParams {
            session_key: "test:e1",
            context_type: "general",
            entity_kind: None,
            entity_id: None,
            area_id: None,
            project_id: None,
            is_ephemeral: true,
        })
        .await
        .unwrap();
        repo.upsert(SessionContextParams {
            session_key: "test:e2",
            context_type: "general",
            entity_kind: None,
            entity_id: None,
            area_id: None,
            project_id: None,
            is_ephemeral: true,
        })
        .await
        .unwrap();

        // Make e1 old by manually setting updated_at (0 = epoch, far in the past)
        sqlx::query(
            "UPDATE session_context SET updated_at = 0 WHERE session_key = ?1",
        )
        .bind("test:e1")
        .execute(&repo.pool)
        .await
        .unwrap();

        // Cleanup should only delete old ones
        let deleted = repo.cleanup_old_ephemeral(1).await.unwrap();
        assert_eq!(deleted, 1);

        // e1 deleted, e2 still exists
        assert!(repo.get("test:e1").await.unwrap().is_none());
        assert!(repo.get("test:e2").await.unwrap().is_some());
    }

    #[tokio::test]
    async fn session_context_pin() {
        let (repo, sessions) = test_repo().await;
        create_session(&sessions, "test:pin1").await;

        repo.upsert(SessionContextParams {
            session_key: "test:pin1",
            context_type: "task",
            entity_kind: Some("task"),
            entity_id: Some("t-1"),
            area_id: None,
            project_id: None,
            is_ephemeral: true,
        })
        .await
        .unwrap();
        let row = repo.get("test:pin1").await.unwrap().unwrap();
        assert!(!row.is_pinned);

        assert!(repo.pin("test:pin1").await.unwrap());
        let row = repo.get("test:pin1").await.unwrap().unwrap();
        assert!(row.is_pinned);

        // Pinning non-existent returns false
        assert!(!repo.pin("test:nonexistent").await.unwrap());
    }
}
