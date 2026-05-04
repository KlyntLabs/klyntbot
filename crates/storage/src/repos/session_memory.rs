//! Repository for the `session_memory` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::SessionMemoryRow;

/// Repository for per-session conversation scratchpad storage.
#[derive(Debug, Clone)]
pub struct SessionMemoryRepo {
    pool: SqlitePool,
}

impl SessionMemoryRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Fetch the memory row for a session, or `None` if it does not exist yet.
    pub async fn get(&self, session_key: &str) -> Result<Option<SessionMemoryRow>, StorageError> {
        let row = sqlx::query_as::<_, SessionMemoryRow>(
            "SELECT session_key, content, turn_count, updated_at
             FROM session_memory
             WHERE session_key = ?1",
        )
        .bind(session_key)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Insert or update the memory for a session.
    pub async fn upsert(
        &self,
        session_key: &str,
        content: &str,
        turn_count: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO session_memory (session_key, content, turn_count, updated_at)
             VALUES (?1, ?2, ?3, (unixepoch('now') * 1000))
             ON CONFLICT(session_key) DO UPDATE SET
                 content    = excluded.content,
                 turn_count = excluded.turn_count,
                 updated_at = excluded.updated_at",
        )
        .bind(session_key)
        .bind(content)
        .bind(turn_count)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete the memory entry for a session (no-op if it does not exist).
    pub async fn delete(&self, session_key: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM session_memory WHERE session_key = ?1")
            .bind(session_key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// List session memories updated since a given timestamp.
    pub async fn list_since(&self, since: &str) -> Result<Vec<SessionMemoryRow>, StorageError> {
        let rows = sqlx::query_as::<_, SessionMemoryRow>(
            "SELECT session_key, content, turn_count, updated_at
             FROM session_memory
             WHERE updated_at > ?1
             ORDER BY updated_at DESC",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    delete_older_than_impl!("session_memory", "updated_at");
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> SessionMemoryRepo {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let repo = SessionMemoryRepo::new(pool.inner().clone());
        // Create a session for FK constraint
        sqlx::query("INSERT INTO sessions (key) VALUES ('test-session')")
            .execute(pool.inner())
            .await
            .unwrap();
        repo
    }

    #[tokio::test]
    async fn upsert_and_get() {
        let repo = setup().await;
        repo.upsert("test-session", "Current task: auth refactor", 5)
            .await
            .unwrap();
        let row = repo.get("test-session").await.unwrap().unwrap();
        assert_eq!(row.content, "Current task: auth refactor");
        assert_eq!(row.turn_count, 5);
    }

    #[tokio::test]
    async fn upsert_updates_existing() {
        let repo = setup().await;
        repo.upsert("test-session", "v1", 1).await.unwrap();
        repo.upsert("test-session", "v2", 3).await.unwrap();
        let row = repo.get("test-session").await.unwrap().unwrap();
        assert_eq!(row.content, "v2");
        assert_eq!(row.turn_count, 3);
    }

    #[tokio::test]
    async fn get_nonexistent_returns_none() {
        let repo = setup().await;
        assert!(repo.get("nonexistent").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_removes_entry() {
        let repo = setup().await;
        repo.upsert("test-session", "content", 1).await.unwrap();
        repo.delete("test-session").await.unwrap();
        assert!(repo.get("test-session").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn delete_older_than_removes_stale_entries() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();

        // Create sessions for FK constraint
        sqlx::query("INSERT INTO sessions (key) VALUES ('old-session')")
            .execute(&inner)
            .await
            .unwrap();
        sqlx::query("INSERT INTO sessions (key) VALUES ('recent-session')")
            .execute(&inner)
            .await
            .unwrap();

        // Insert an old session memory directly with an old updated_at
        sqlx::query(
            "INSERT INTO session_memory (session_key, content, turn_count, updated_at) \
             VALUES ('old-session', 'old content', 1, 0)",
        )
        .execute(&inner)
        .await
        .unwrap();

        // Insert a recent session memory (should not be deleted)
        let repo = SessionMemoryRepo::new(inner);
        repo.upsert("recent-session", "recent content", 3)
            .await
            .unwrap();

        let deleted = repo.delete_older_than(90, jiff::Timestamp::now()).await.unwrap();
        assert_eq!(deleted, 1);

        assert!(repo.get("old-session").await.unwrap().is_none());
        assert!(repo.get("recent-session").await.unwrap().is_some());
    }
}
