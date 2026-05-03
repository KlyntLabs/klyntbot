//! Workspace repository — code-editor workspace lifecycle (Cursor/Codex-style).
//!
//! A `workspace` is a registered folder on disk. The `id` is a UUID string that
//! flows into Phase 2 surfaces via `sessions.repo_id`, `coding_approval_history.repo_id`,
//! and `GuardCtx.repo_id`. `project_id` is an optional link to a Klyntbot project
//! row (organizational); null for unlinked folders.

use sqlx::{FromRow, SqlitePool};

use crate::error::{OptionExt, StorageError};

#[derive(Debug, Clone, FromRow)]
pub struct WorkspaceRow {
    pub id: String,
    pub name: String,
    pub path: String,
    pub connected: i64,
    pub kind: String,
    pub parent_id: Option<String>,
    pub project_id: Option<String>,
    pub settings: String,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone)]
pub struct NewWorkspace<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub path: &'a str,
    pub kind: &'a str,
    pub parent_id: Option<&'a str>,
    pub project_id: Option<&'a str>,
    pub settings_json: &'a str,
}

#[derive(Debug, Clone)]
pub struct WorkspaceRepo {
    pool: SqlitePool,
}

impl WorkspaceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_all(&self) -> Result<Vec<WorkspaceRow>, StorageError> {
        let rows = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT id, name, path, connected, kind, parent_id, project_id, settings, \
                    created_at, updated_at \
             FROM workspaces \
             ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn get(&self, id: &str) -> Result<WorkspaceRow, StorageError> {
        sqlx::query_as::<_, WorkspaceRow>(
            "SELECT id, name, path, connected, kind, parent_id, project_id, settings, \
                    created_at, updated_at \
             FROM workspaces WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("workspace '{id}'"))
    }

    pub async fn get_by_path(&self, path: &str) -> Result<Option<WorkspaceRow>, StorageError> {
        let row = sqlx::query_as::<_, WorkspaceRow>(
            "SELECT id, name, path, connected, kind, parent_id, project_id, settings, \
                    created_at, updated_at \
             FROM workspaces WHERE path = ?1",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn insert(&self, w: NewWorkspace<'_>) -> Result<WorkspaceRow, StorageError> {
        sqlx::query(
            "INSERT INTO workspaces (id, name, path, connected, kind, parent_id, project_id, settings) \
             VALUES (?1, ?2, ?3, 1, ?4, ?5, ?6, ?7)",
        )
        .bind(w.id)
        .bind(w.name)
        .bind(w.path)
        .bind(w.kind)
        .bind(w.parent_id)
        .bind(w.project_id)
        .bind(w.settings_json)
        .execute(&self.pool)
        .await?;
        self.get(w.id).await
    }

    pub async fn set_connected(&self, id: &str, connected: bool) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE workspaces SET connected = ?1, updated_at = (unixepoch('now') * 1000) \
             WHERE id = ?2",
        )
        .bind(if connected { 1_i64 } else { 0 })
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn update_settings(&self, id: &str, settings_json: &str) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE workspaces SET settings = ?1, updated_at = (unixepoch('now') * 1000) \
             WHERE id = ?2",
        )
        .bind(settings_json)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove(&self, id: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM workspaces WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::StoragePool;

    #[tokio::test]
    async fn workspace_round_trip() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = WorkspaceRepo::new(pool.inner().clone());
        let row = repo
            .insert(NewWorkspace {
                id: "ws-1",
                name: "demo",
                path: "/tmp/demo",
                kind: "main",
                parent_id: None,
                project_id: None,
                settings_json: "{}",
            })
            .await
            .unwrap();
        assert_eq!(row.id, "ws-1");
        assert_eq!(row.connected, 1);
        let fetched = repo.get("ws-1").await.unwrap();
        assert_eq!(fetched.path, "/tmp/demo");
        repo.set_connected("ws-1", false).await.unwrap();
        let after = repo.get("ws-1").await.unwrap();
        assert_eq!(after.connected, 0);
        let all = repo.list_all().await.unwrap();
        assert_eq!(all.len(), 1);
        repo.remove("ws-1").await.unwrap();
        assert!(repo.list_all().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn unique_path_constraint() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = WorkspaceRepo::new(pool.inner().clone());
        repo.insert(NewWorkspace {
            id: "a",
            name: "a",
            path: "/p",
            kind: "main",
            parent_id: None,
            project_id: None,
            settings_json: "{}",
        })
        .await
        .unwrap();
        let err = repo
            .insert(NewWorkspace {
                id: "b",
                name: "b",
                path: "/p",
                kind: "main",
                parent_id: None,
                project_id: None,
                settings_json: "{}",
            })
            .await;
        assert!(err.is_err(), "duplicate path should fail");
    }
}
