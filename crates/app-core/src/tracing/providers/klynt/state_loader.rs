//! Loads a Klynt session's row → `SessionState`.

use common::{KlyntbotError, Result};
use serde_json::Value;
use storage::repos::Repos;

use crate::tracing::types::SessionState;

#[derive(sqlx::FromRow)]
struct StateRow {
    metadata: String,
    archived_at: Option<i64>,
    cwd: Option<String>,
    repo_id: Option<String>,
    repo_branch: Option<String>,
    tool_profile: Option<String>,
    approval_mode: String,
    total_cost_usd: f64,
    total_tokens: i64,
    compressed_at: Option<i64>,
    compressed_through_idx: Option<i64>,
}

pub async fn load_state(repos: &Repos, session_id: &str) -> Result<SessionState> {
    let row: StateRow = sqlx::query_as::<_, StateRow>(
        r#"
        SELECT metadata, archived_at, cwd, repo_id, repo_branch, tool_profile,
               approval_mode, total_cost_usd, total_tokens,
               compressed_at, compressed_through_idx
        FROM sessions WHERE key = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(repos.pool())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("klynt load_state: {e}")))?
    .ok_or_else(|| KlyntbotError::StorageNotFound(format!("session {session_id}")))?;

    let metadata: Value = serde_json::from_str(&row.metadata).unwrap_or(Value::Null);
    let custom_title = metadata
        .get("title")
        .and_then(|t| t.as_str().map(String::from));

    let raw = serde_json::json!({
        "metadata": metadata,
        "cwd": row.cwd,
        "repo_id": row.repo_id,
        "repo_branch": row.repo_branch,
        "tool_profile": row.tool_profile,
        "approval_mode": row.approval_mode,
        "total_cost_usd": row.total_cost_usd,
        "total_tokens": row.total_tokens,
        "compressed_at": row.compressed_at,
        "compressed_through_idx": row.compressed_through_idx,
    });

    Ok(SessionState {
        custom_title,
        plan_mode: false,
        archived: row.archived_at.is_some(),
        todos: Vec::new(),
        raw,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use common::SessionMode;
    use storage::StoragePool;

    async fn fresh_repos() -> Repos {
        let pool = StoragePool::connect_in_memory().await.expect("memory pool");
        Repos::from_pool(&pool)
    }

    #[tokio::test]
    async fn missing_session_returns_not_found() {
        let repos = fresh_repos().await;
        let err = load_state(&repos, "nope").await.unwrap_err();
        match err {
            KlyntbotError::StorageNotFound(_) => {}
            other => panic!("wrong error: {other:?}"),
        }
    }

    #[tokio::test]
    async fn loads_state_with_metadata_title() {
        let repos = fresh_repos().await;
        repos
            .sessions
            .upsert_session_with_mode(
                "coding:1",
                SessionMode::Coding,
                &serde_json::json!({"title": "Hello session"}),
            )
            .await
            .unwrap();
        let s = load_state(&repos, "coding:1").await.unwrap();
        assert_eq!(s.custom_title.as_deref(), Some("Hello session"));
        assert!(!s.archived);
        assert_eq!(s.raw["approval_mode"], "default");
    }

    #[tokio::test]
    async fn archived_when_archived_at_is_set() {
        let repos = fresh_repos().await;
        repos
            .sessions
            .upsert_session_with_mode("coding:1", SessionMode::Coding, &serde_json::json!({}))
            .await
            .unwrap();
        sqlx::query("UPDATE sessions SET archived_at = ? WHERE key = ?")
            .bind(1_700_000_000_000_i64)
            .bind("coding:1")
            .execute(repos.pool())
            .await
            .unwrap();
        let s = load_state(&repos, "coding:1").await.unwrap();
        assert!(s.archived);
    }
}
