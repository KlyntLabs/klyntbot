//! Repository for `subagent_instances`.
//!
//! All lifecycle transitions go through this repo. The state machine is
//! enforced in `update_status` — illegal transitions return an error.

use sqlx::SqlitePool;

use common::Result;

use crate::rows::{SubagentInstanceRow, SubagentStatus};

#[derive(Debug, Clone)]
pub struct SubagentInstanceRepo {
    pool: SqlitePool,
}

#[derive(Debug, Clone)]
pub struct NewSubagentInstance {
    pub agent_id: String,
    pub session_id: String,
    pub parent_agent_id: Option<String>,
    pub description: String,
    pub model: Option<String>,
    pub workspace_path: String,
    pub turn_cap: i64,
}

impl SubagentInstanceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn insert(&self, new: &NewSubagentInstance) -> Result<SubagentInstanceRow> {
        sqlx::query_as::<_, SubagentInstanceRow>(
            r#"
            INSERT INTO subagent_instances
                (agent_id, session_id, parent_agent_id, description, status,
                 model, workspace_path, turn_cap)
            VALUES (?1, ?2, ?3, ?4, 'running', ?5, ?6, ?7)
            RETURNING *
            "#,
        )
        .bind(&new.agent_id)
        .bind(&new.session_id)
        .bind(&new.parent_agent_id)
        .bind(&new.description)
        .bind(&new.model)
        .bind(&new.workspace_path)
        .bind(new.turn_cap)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent insert: {e}")))
    }

    pub async fn get(&self, agent_id: &str) -> Result<Option<SubagentInstanceRow>> {
        sqlx::query_as::<_, SubagentInstanceRow>(
            "SELECT * FROM subagent_instances WHERE agent_id = ?1",
        )
        .bind(agent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent get: {e}")))
    }

    pub async fn get_by_session(&self, session_id: &str) -> Result<Option<SubagentInstanceRow>> {
        sqlx::query_as::<_, SubagentInstanceRow>(
            "SELECT * FROM subagent_instances WHERE session_id = ?1",
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent get_by_session: {e}")))
    }

    pub async fn list_by_parent(
        &self,
        parent_agent_id: Option<&str>,
    ) -> Result<Vec<SubagentInstanceRow>> {
        match parent_agent_id {
            Some(p) => sqlx::query_as::<_, SubagentInstanceRow>(
                "SELECT * FROM subagent_instances WHERE parent_agent_id = ?1 ORDER BY created_at DESC",
            )
            .bind(p)
            .fetch_all(&self.pool)
            .await,
            None => sqlx::query_as::<_, SubagentInstanceRow>(
                "SELECT * FROM subagent_instances WHERE parent_agent_id IS NULL ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await,
        }
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent list_by_parent: {e}")))
    }

    pub async fn list_by_status(&self, status: SubagentStatus) -> Result<Vec<SubagentInstanceRow>> {
        sqlx::query_as::<_, SubagentInstanceRow>(
            "SELECT * FROM subagent_instances WHERE status = ?1 ORDER BY updated_at DESC",
        )
        .bind(status.as_str())
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent list_by_status: {e}")))
    }

    pub async fn list_all(&self) -> Result<Vec<SubagentInstanceRow>> {
        sqlx::query_as::<_, SubagentInstanceRow>(
            "SELECT * FROM subagent_instances ORDER BY updated_at DESC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent list_all: {e}")))
    }

    /// Allowed transitions:
    /// - From terminal states (failed/killed/completed): forbidden.
    /// - From `running`: any non-running state.
    /// - From `idle` or `stopped_turn`: to `running` (on resume) or any terminal.
    ///
    /// Returns `Err(KlyntbotError::Storage)` if the transition is rejected.
    pub async fn update_status(&self, agent_id: &str, next: SubagentStatus) -> Result<()> {
        let current = self
            .get(agent_id)
            .await?
            .ok_or_else(|| common::KlyntbotError::StorageNotFound(format!("subagent {agent_id}")))?
            .status_enum();
        if current.is_terminal() {
            return Err(common::KlyntbotError::Storage(format!(
                "subagent {agent_id}: cannot transition out of terminal state {}",
                current.as_str()
            )));
        }
        sqlx::query(
            "UPDATE subagent_instances SET status = ?1, updated_at = (unixepoch('now') * 1000) WHERE agent_id = ?2",
        )
        .bind(next.as_str())
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent update_status: {e}")))?;
        Ok(())
    }

    /// Increment `turns_used` and `turns_used_total` by 1, refresh `updated_at`.
    /// Called once per iteration boundary in `execute_loop`.
    pub async fn tick_turn(&self, agent_id: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE subagent_instances
            SET turns_used = turns_used + 1,
                turns_used_total = turns_used_total + 1,
                updated_at = (unixepoch('now') * 1000)
            WHERE agent_id = ?1
            "#,
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent tick_turn: {e}")))?;
        Ok(())
    }

    /// Reset `turns_used` to 0 (called when starting a resume call). `turns_used_total` is untouched.
    pub async fn reset_turns_for_resume(&self, agent_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE subagent_instances SET turns_used = 0, updated_at = (unixepoch('now') * 1000) WHERE agent_id = ?1",
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent reset_turns_for_resume: {e}")))?;
        Ok(())
    }

    /// Store the last assistant text (or fallback) when a cap-hit occurs.
    pub async fn set_partial_summary(&self, agent_id: &str, summary: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE subagent_instances
            SET partial_summary = ?1,
                last_cap_hit_at = (unixepoch('now') * 1000),
                updated_at = (unixepoch('now') * 1000)
            WHERE agent_id = ?2
            "#,
        )
        .bind(summary)
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            common::KlyntbotError::Storage(format!("subagent set_partial_summary: {e}"))
        })?;
        Ok(())
    }

    /// Refresh `updated_at` without changing any other field (cheap heartbeat ping).
    pub async fn heartbeat(&self, agent_id: &str) -> Result<()> {
        sqlx::query(
            "UPDATE subagent_instances SET updated_at = (unixepoch('now') * 1000) WHERE agent_id = ?1",
        )
        .bind(agent_id)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent heartbeat: {e}")))?;
        Ok(())
    }

    /// Flip `running` rows whose `updated_at` is older than `threshold_ms` to
    /// `failed`. Run once at app startup (before any new subagent starts).
    /// Returns the number of rows swept.
    pub async fn sweep_zombies(&self, threshold_ms: i64) -> Result<u64> {
        let res = sqlx::query(
            r#"
            UPDATE subagent_instances
            SET status = 'failed',
                partial_summary = COALESCE(partial_summary, 'Process crashed before completion'),
                updated_at = (unixepoch('now') * 1000)
            WHERE status = 'running'
              AND updated_at < (unixepoch('now') * 1000) - ?1
            "#,
        )
        .bind(threshold_ms)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("subagent sweep_zombies: {e}")))?;
        Ok(res.rows_affected())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pool::StoragePool;

    async fn pool() -> StoragePool {
        StoragePool::connect_in_memory().await.unwrap()
    }

    async fn insert_parent_session(pool: &SqlitePool, key: &str) {
        sqlx::query("INSERT INTO sessions (key, mode) VALUES (?1, 'subagent')")
            .bind(key)
            .execute(pool)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn insert_and_get_roundtrip() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        let repo = SubagentInstanceRepo::new(inner.clone());

        let row = repo
            .insert(&NewSubagentInstance {
                agent_id: "ag1".to_string(),
                session_id: "sess-1".to_string(),
                parent_agent_id: None,
                description: "search for X".to_string(),
                model: None,
                workspace_path: "/tmp/ws".to_string(),
                turn_cap: 500,
            })
            .await
            .unwrap();

        assert_eq!(row.agent_id, "ag1");
        assert_eq!(row.status, "running");
        assert_eq!(row.turn_cap, 500);
        assert_eq!(row.turns_used, 0);
        assert_eq!(row.turns_used_total, 0);

        let fetched = repo.get("ag1").await.unwrap().unwrap();
        assert_eq!(fetched.agent_id, "ag1");
    }

    #[tokio::test]
    async fn list_by_parent_filters_correctly() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-a").await;
        insert_parent_session(&inner, "sess-b").await;
        insert_parent_session(&inner, "sess-c").await;
        let repo = SubagentInstanceRepo::new(inner);

        repo.insert(&NewSubagentInstance {
            agent_id: "ag-parent".to_string(),
            session_id: "sess-a".to_string(),
            parent_agent_id: None,
            description: "parent".to_string(),
            model: None,
            workspace_path: "/tmp/ws".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.insert(&NewSubagentInstance {
            agent_id: "ag-child-1".to_string(),
            session_id: "sess-b".to_string(),
            parent_agent_id: Some("ag-parent".to_string()),
            description: "child 1".to_string(),
            model: None,
            workspace_path: "/tmp/ws".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.insert(&NewSubagentInstance {
            agent_id: "ag-child-2".to_string(),
            session_id: "sess-c".to_string(),
            parent_agent_id: Some("ag-parent".to_string()),
            description: "child 2".to_string(),
            model: None,
            workspace_path: "/tmp/ws".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        let top_level = repo.list_by_parent(None).await.unwrap();
        assert_eq!(top_level.len(), 1);
        assert_eq!(top_level[0].agent_id, "ag-parent");

        let children = repo.list_by_parent(Some("ag-parent")).await.unwrap();
        assert_eq!(children.len(), 2);
    }

    #[tokio::test]
    async fn transitions_running_to_stopped_turn() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        let repo = SubagentInstanceRepo::new(inner);

        repo.insert(&NewSubagentInstance {
            agent_id: "ag1".to_string(),
            session_id: "sess-1".to_string(),
            parent_agent_id: None,
            description: "x".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.update_status("ag1", SubagentStatus::StoppedTurn)
            .await
            .unwrap();
        let row = repo.get("ag1").await.unwrap().unwrap();
        assert_eq!(row.status, "stopped_turn");
    }

    #[tokio::test]
    async fn rejects_transition_from_terminal_state() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        let repo = SubagentInstanceRepo::new(inner);

        repo.insert(&NewSubagentInstance {
            agent_id: "ag1".to_string(),
            session_id: "sess-1".to_string(),
            parent_agent_id: None,
            description: "x".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.update_status("ag1", SubagentStatus::Killed)
            .await
            .unwrap();
        let err = repo.update_status("ag1", SubagentStatus::Running).await;
        assert!(
            err.is_err(),
            "must reject transition out of terminal Killed"
        );
    }

    #[tokio::test]
    async fn increments_counters_independently() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        let repo = SubagentInstanceRepo::new(inner);

        repo.insert(&NewSubagentInstance {
            agent_id: "ag1".to_string(),
            session_id: "sess-1".to_string(),
            parent_agent_id: None,
            description: "x".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        repo.tick_turn("ag1").await.unwrap();
        repo.tick_turn("ag1").await.unwrap();
        let row = repo.get("ag1").await.unwrap().unwrap();
        assert_eq!(row.turns_used, 2);
        assert_eq!(row.turns_used_total, 2);

        repo.reset_turns_for_resume("ag1").await.unwrap();
        let row2 = repo.get("ag1").await.unwrap().unwrap();
        assert_eq!(row2.turns_used, 0);
        assert_eq!(row2.turns_used_total, 2, "total accumulates across resumes");
    }

    #[tokio::test]
    async fn zombie_sweep_marks_stale_running_as_failed() {
        let p = pool().await;
        let inner = p.inner().clone();
        insert_parent_session(&inner, "sess-1").await;
        insert_parent_session(&inner, "sess-2").await;
        let repo = SubagentInstanceRepo::new(inner.clone());

        // ag-stale: status=running, updated_at 10 min ago
        repo.insert(&NewSubagentInstance {
            agent_id: "ag-stale".to_string(),
            session_id: "sess-1".to_string(),
            parent_agent_id: None,
            description: "stale".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();
        sqlx::query("UPDATE subagent_instances SET updated_at = (unixepoch('now') * 1000) - 600000 WHERE agent_id = 'ag-stale'")
            .execute(&inner)
            .await
            .unwrap();

        // ag-fresh: status=running, updated_at just now
        repo.insert(&NewSubagentInstance {
            agent_id: "ag-fresh".to_string(),
            session_id: "sess-2".to_string(),
            parent_agent_id: None,
            description: "fresh".to_string(),
            model: None,
            workspace_path: "/tmp".to_string(),
            turn_cap: 500,
        })
        .await
        .unwrap();

        let swept = repo.sweep_zombies(300_000).await.unwrap();
        assert_eq!(swept, 1);

        let stale = repo.get("ag-stale").await.unwrap().unwrap();
        assert_eq!(stale.status, "failed");
        assert_eq!(
            stale.partial_summary.as_deref(),
            Some("Process crashed before completion"),
        );

        let fresh = repo.get("ag-fresh").await.unwrap().unwrap();
        assert_eq!(fresh.status, "running");
    }
}
