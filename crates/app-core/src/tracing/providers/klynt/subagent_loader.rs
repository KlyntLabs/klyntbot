//! Lists / loads subagent (child) sessions linked via `parent_session_id`.

use common::Result;
use jiff::Timestamp;
use storage::repos::Repos;

use crate::tracing::types::SubagentSummary;

#[derive(sqlx::FromRow)]
struct SubagentRow {
    key: String,
    metadata: String,
    created_at: i64,
    updated_at: i64,
    archived_at: Option<i64>,
    event_count: i64,
}

pub async fn list_subagents(repos: &Repos, parent_id: &str) -> Result<Vec<SubagentSummary>> {
    let rows: Vec<SubagentRow> = sqlx::query_as::<_, SubagentRow>(
        r#"
        SELECT s.key as key,
               s.metadata as metadata,
               s.created_at as created_at,
               s.updated_at as updated_at,
               s.archived_at as archived_at,
               (SELECT COUNT(*) FROM session_messages m WHERE m.session_key = s.key) as event_count
        FROM sessions s
        WHERE s.parent_session_id = ?
        ORDER BY s.created_at ASC
        "#,
    )
    .bind(parent_id)
    .fetch_all(repos.pool())
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("klynt list_subagents: {e}")))?;

    Ok(rows.into_iter().map(row_to_summary).collect())
}

fn row_to_summary(r: SubagentRow) -> SubagentSummary {
    let metadata: serde_json::Value = serde_json::from_str(&r.metadata).unwrap_or_default();
    let subagent_type = metadata
        .get("subagent_type")
        .and_then(|v| v.as_str())
        .unwrap_or("klynt-subagent")
        .to_string();
    let description = metadata
        .get("description")
        .and_then(|v| v.as_str().map(String::from));
    let status = if r.archived_at.is_some() {
        "completed"
    } else {
        "idle"
    }
    .to_string();
    SubagentSummary {
        agent_id: r.key,
        subagent_type,
        status,
        description,
        created_at: ms_to_ts(r.created_at),
        updated_at: ms_to_ts(r.updated_at),
        event_count: r.event_count.max(0) as u32,
    }
}

fn ms_to_ts(ms: i64) -> Timestamp {
    Timestamp::from_millisecond(ms).unwrap_or(Timestamp::UNIX_EPOCH)
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

    async fn insert(repos: &Repos, key: &str, parent: Option<&str>, meta: serde_json::Value) {
        repos
            .sessions
            .upsert_session_with_mode(key, SessionMode::Coding, &meta)
            .await
            .unwrap();
        if let Some(p) = parent {
            sqlx::query("UPDATE sessions SET parent_session_id = ? WHERE key = ?")
                .bind(p)
                .bind(key)
                .execute(repos.pool())
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn no_children_returns_empty() {
        let repos = fresh_repos().await;
        insert(&repos, "p", None, serde_json::json!({})).await;
        let out = list_subagents(&repos, "p").await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn two_children_returned_in_creation_order() {
        let repos = fresh_repos().await;
        insert(&repos, "p", None, serde_json::json!({})).await;
        insert(
            &repos,
            "c1",
            Some("p"),
            serde_json::json!({"subagent_type": "code-reviewer", "description": "review"}),
        )
        .await;
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        insert(
            &repos,
            "c2",
            Some("p"),
            serde_json::json!({"subagent_type": "explorer"}),
        )
        .await;
        let out = list_subagents(&repos, "p").await.unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].agent_id, "c1");
        assert_eq!(out[0].subagent_type, "code-reviewer");
        assert_eq!(out[0].description.as_deref(), Some("review"));
        assert_eq!(out[1].agent_id, "c2");
        assert_eq!(out[1].subagent_type, "explorer");
    }

    #[tokio::test]
    async fn missing_subagent_type_falls_back_to_default() {
        let repos = fresh_repos().await;
        insert(&repos, "p", None, serde_json::json!({})).await;
        insert(&repos, "c1", Some("p"), serde_json::json!({})).await;
        let out = list_subagents(&repos, "p").await.unwrap();
        assert_eq!(out[0].subagent_type, "klynt-subagent");
    }
}
