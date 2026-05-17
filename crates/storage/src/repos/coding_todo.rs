//! Repository for the coding_todos table.

use crate::error::StorageError;
use sqlx::SqlitePool;

#[derive(Debug, Clone)]
pub struct TodoRepo {
    pool: SqlitePool,
}

impl TodoRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    /// Insert or replace the row for `(thread_id, agent_id)`.
    pub async fn upsert(
        &self,
        thread_id: &str,
        agent_id: &str,
        items_json: &str,
        proposed_in_plan_session: Option<&str>,
    ) -> Result<(), StorageError> {
        let now = jiff::Timestamp::now().to_string();
        sqlx::query(
            r#"
            INSERT INTO coding_todos (thread_id, agent_id, items_json, proposed_in_plan_session, updated_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(thread_id, agent_id) DO UPDATE SET
                items_json = excluded.items_json,
                proposed_in_plan_session = excluded.proposed_in_plan_session,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(thread_id)
        .bind(agent_id)
        .bind(items_json)
        .bind(proposed_in_plan_session)
        .bind(&now)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn get(
        &self,
        thread_id: &str,
        agent_id: &str,
    ) -> Result<Option<TodoRow>, StorageError> {
        let row: Option<(String, String, String, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT thread_id, agent_id, items_json, proposed_in_plan_session, updated_at
              FROM coding_todos
             WHERE thread_id = ? AND agent_id = ?
            "#,
        )
        .bind(thread_id)
        .bind(agent_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(match row {
            Some(t) => Some(TodoRow::from_tuple(t)?),
            None => None,
        })
    }

    pub async fn list_for_thread(&self, thread_id: &str) -> Result<Vec<TodoRow>, StorageError> {
        let rows: Vec<(String, String, String, Option<String>, String)> = sqlx::query_as(
            r#"
            SELECT thread_id, agent_id, items_json, proposed_in_plan_session, updated_at
              FROM coding_todos
             WHERE thread_id = ?
             ORDER BY agent_id
            "#,
        )
        .bind(thread_id)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter().map(TodoRow::from_tuple).collect()
    }

    pub async fn delete_row(&self, thread_id: &str, agent_id: &str) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM coding_todos WHERE thread_id = ? AND agent_id = ?")
            .bind(thread_id)
            .bind(agent_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Quick liveness check — verifies the coding_todos table is accessible.
    pub async fn check_health(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1 FROM coding_todos LIMIT 1")
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Clear the `proposed_in_plan_session` tag for all rows in the thread
    /// whose tag matches `plan_session_id`. Used at ratify time.
    pub async fn clear_plan_session_tag(
        &self,
        thread_id: &str,
        plan_session_id: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE coding_todos SET proposed_in_plan_session = NULL \
             WHERE thread_id = ? AND proposed_in_plan_session = ?",
        )
        .bind(thread_id)
        .bind(plan_session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Delete all rows in the thread tagged with `plan_session_id`.
    /// Used at cancel time — proposed items had no execution history yet.
    pub async fn delete_plan_session(
        &self,
        thread_id: &str,
        plan_session_id: &str,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "DELETE FROM coding_todos \
             WHERE thread_id = ? AND proposed_in_plan_session = ?",
        )
        .bind(thread_id)
        .bind(plan_session_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TodoRow {
    pub thread_id: String,
    pub agent_id: String,
    pub items_json: String,
    pub proposed_in_plan_session: Option<String>,
    pub updated_at: jiff::Timestamp,
}

impl TodoRow {
    fn from_tuple(
        (t, a, items, plan, ts): (String, String, String, Option<String>, String),
    ) -> Result<Self, StorageError> {
        Ok(Self {
            thread_id: t,
            agent_id: a,
            items_json: items,
            proposed_in_plan_session: plan,
            updated_at: ts.parse().map_err(|e| {
                StorageError::serialization(format!("invalid timestamp '{}': {}", ts, e))
            })?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::StoragePool;

    const TEST_MIGRATION: &str = r#"
        CREATE TABLE IF NOT EXISTS coding_todos (
            thread_id TEXT NOT NULL,
            agent_id TEXT NOT NULL,
            items_json TEXT NOT NULL DEFAULT '[]',
            proposed_in_plan_session TEXT,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (thread_id, agent_id)
        );
    "#;

    async fn setup() -> TodoRepo {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        sqlx::query(TEST_MIGRATION)
            .execute(pool.inner())
            .await
            .unwrap();
        TodoRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn upsert_inserts_new_row() {
        let repo = setup().await;
        repo.upsert("t1", "root", "[]", None).await.unwrap();
    }

    #[tokio::test]
    async fn get_returns_none_when_missing() {
        let repo = setup().await;
        let row = repo.get("absent", "root").await.unwrap();
        assert!(row.is_none());
    }

    #[tokio::test]
    async fn upsert_then_get_returns_row() {
        let repo = setup().await;
        repo.upsert("t1", "root", r#"[{"id":"a"}]"#, None)
            .await
            .unwrap();
        let row = repo.get("t1", "root").await.unwrap().unwrap();
        assert_eq!(row.thread_id, "t1");
        assert_eq!(row.agent_id, "root");
        assert_eq!(row.items_json, r#"[{"id":"a"}]"#);
        assert!(row.proposed_in_plan_session.is_none());
    }

    #[tokio::test]
    async fn upsert_replaces_on_conflict() {
        let repo = setup().await;
        repo.upsert("t1", "root", "[]", None).await.unwrap();
        repo.upsert("t1", "root", r#"[{"id":"new"}]"#, Some("p_xyz"))
            .await
            .unwrap();
        let row = repo.get("t1", "root").await.unwrap().unwrap();
        assert_eq!(row.items_json, r#"[{"id":"new"}]"#);
        assert_eq!(row.proposed_in_plan_session.as_deref(), Some("p_xyz"));
    }

    #[tokio::test]
    async fn list_for_thread_returns_all_agents() {
        let repo = setup().await;
        repo.upsert("t1", "root", "[]", None).await.unwrap();
        repo.upsert("t1", "subagent_a", "[]", None).await.unwrap();
        repo.upsert("t1", "subagent_b", "[]", None).await.unwrap();
        repo.upsert("t2", "root", "[]", None).await.unwrap(); // different thread
        let rows = repo.list_for_thread("t1").await.unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].agent_id, "root");
    }

    #[tokio::test]
    async fn delete_row_removes_existing() {
        let repo = setup().await;
        repo.upsert("t1", "root", "[]", None).await.unwrap();
        repo.delete_row("t1", "root").await.unwrap();
        assert!(repo.get("t1", "root").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn clear_plan_session_tag_clears_matching_rows_only() {
        let repo = setup().await;
        repo.upsert("t1", "root", "[]", Some("p_a")).await.unwrap();
        repo.upsert("t1", "sub_x", "[]", Some("p_a")).await.unwrap();
        repo.upsert("t1", "sub_y", "[]", Some("p_b")).await.unwrap();
        repo.clear_plan_session_tag("t1", "p_a").await.unwrap();
        let r1 = repo.get("t1", "root").await.unwrap().unwrap();
        let r2 = repo.get("t1", "sub_x").await.unwrap().unwrap();
        let r3 = repo.get("t1", "sub_y").await.unwrap().unwrap();
        assert!(r1.proposed_in_plan_session.is_none());
        assert!(r2.proposed_in_plan_session.is_none());
        assert_eq!(r3.proposed_in_plan_session.as_deref(), Some("p_b"));
    }

    #[tokio::test]
    async fn delete_plan_session_removes_matching_rows() {
        let repo = setup().await;
        repo.upsert("t1", "root", "[]", Some("p_a")).await.unwrap();
        repo.upsert("t1", "sub", "[]", Some("p_b")).await.unwrap();
        repo.delete_plan_session("t1", "p_a").await.unwrap();
        assert!(repo.get("t1", "root").await.unwrap().is_none());
        assert!(repo.get("t1", "sub").await.unwrap().is_some());
    }
}
