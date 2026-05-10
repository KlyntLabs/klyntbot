//! Top-level coding-session discovery for Klynt.

use common::Result;
use jiff::Timestamp;
use std::path::PathBuf;
use storage::repos::Repos;

use crate::tracing::types::{SessionMetadataInfo, SessionSummary};

const PROVIDER_ID: &str = "klynt";

#[derive(sqlx::FromRow)]
struct DiscoveryRow {
    key: String,
    cwd: Option<String>,
    repo_id: Option<String>,
    metadata: String,
    archived_at: Option<i64>,
    created_at: i64,
    updated_at: i64,
    event_count: i64,
    turn_count: i64,
    tool_call_count: i64,
    error_count: i64,
    subagent_count: i64,
}

pub async fn list_sessions(repos: &Repos) -> Result<Vec<SessionSummary>> {
    let pool = repos.pool();
    let rows: Vec<DiscoveryRow> = sqlx::query_as::<_, DiscoveryRow>(
        r#"
        SELECT
          s.key                                        AS key,
          s.cwd                                        AS cwd,
          s.repo_id                                    AS repo_id,
          s.metadata                                   AS metadata,
          s.archived_at                                AS archived_at,
          s.created_at                                 AS created_at,
          s.updated_at                                 AS updated_at,
          (SELECT COUNT(*)
             FROM session_messages m
             WHERE m.session_key = s.key)              AS event_count,
          (SELECT COUNT(DISTINCT m.turn_id)
             FROM session_messages m
             WHERE m.session_key = s.key
               AND m.turn_id IS NOT NULL)              AS turn_count,
          (SELECT COUNT(*)
             FROM session_messages m
             WHERE m.session_key = s.key
               AND m.parts LIKE '%"kind":"tool_call"%') AS tool_call_count,
          (SELECT COUNT(*)
             FROM session_messages m
             WHERE m.session_key = s.key
               AND m.parts LIKE '%"is_error":true%')   AS error_count,
          (SELECT COUNT(*)
             FROM sessions c
             WHERE c.parent_session_id = s.key)        AS subagent_count
        FROM sessions s
        WHERE s.mode = 'coding'
          AND s.parent_session_id IS NULL
        ORDER BY s.updated_at DESC
        LIMIT 500
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("klynt list_sessions: {e}")))?;

    Ok(rows.into_iter().map(row_to_summary).collect())
}

fn row_to_summary(r: DiscoveryRow) -> SessionSummary {
    let title = serde_json::from_str::<serde_json::Value>(&r.metadata)
        .ok()
        .and_then(|v| v.get("title").and_then(|t| t.as_str().map(String::from)));
    let cwd = r.cwd.map(PathBuf::from);
    let project_basename = cwd
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string());
    SessionSummary {
        session_id: r.key.clone(),
        provider_id: PROVIDER_ID.into(),
        source_dir: PathBuf::new(),
        cwd,
        project_basename,
        custom_title: title.clone(),
        started_at: ms_to_timestamp(r.created_at),
        last_event_at: ms_to_timestamp(r.updated_at),
        size_bytes: 0,
        turn_count: r.turn_count.max(0) as u32,
        step_count: 0,
        tool_call_count: r.tool_call_count.max(0) as u32,
        error_count: r.error_count.max(0) as u32,
        subagent_count: r.subagent_count.max(0) as u32,
        has_wire: r.event_count > 0,
        has_context: r.event_count > 0,
        imported: false,
        work_dir_hash: r.repo_id.clone().unwrap_or_default(),
        has_state: true,
        wire_size: 0,
        context_size: 0,
        state_size: 0,
        total_size: 0,
        metadata: Some(SessionMetadataInfo {
            session_id: r.key.clone(),
            title: title.unwrap_or_else(|| r.key.clone()),
            title_generated: false,
            archived: r.archived_at.is_some(),
            archived_at: r.archived_at,
            auto_archive_exempt: false,
            wire_mtime: Some(r.updated_at),
        }),
    }
}

fn ms_to_timestamp(ms: i64) -> Timestamp {
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

    async fn insert_coding_session(repos: &Repos, key: &str) {
        repos
            .sessions
            .upsert_session_with_mode(key, SessionMode::Coding, &serde_json::json!({}))
            .await
            .expect("insert coding session");
    }

    async fn insert_assistant_session(repos: &Repos, key: &str) {
        repos
            .sessions
            .upsert_session_with_mode(key, SessionMode::Assistant, &serde_json::json!({}))
            .await
            .expect("insert assistant session");
    }

    #[tokio::test]
    async fn empty_database_returns_no_sessions() {
        let repos = fresh_repos().await;
        let out = list_sessions(&repos).await.unwrap();
        assert!(out.is_empty());
    }

    #[tokio::test]
    async fn one_coding_session_is_returned() {
        let repos = fresh_repos().await;
        insert_coding_session(&repos, "coding:1").await;
        let out = list_sessions(&repos).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "coding:1");
        assert_eq!(out[0].provider_id, "klynt");
    }

    #[tokio::test]
    async fn assistant_sessions_are_excluded() {
        let repos = fresh_repos().await;
        insert_assistant_session(&repos, "assistant:1").await;
        insert_coding_session(&repos, "coding:1").await;
        let out = list_sessions(&repos).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "coding:1");
    }

    #[tokio::test]
    async fn ordered_by_updated_at_desc() {
        let repos = fresh_repos().await;
        insert_coding_session(&repos, "coding:older").await;
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        insert_coding_session(&repos, "coding:newer").await;
        let out = list_sessions(&repos).await.unwrap();
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].session_id, "coding:newer");
        assert_eq!(out[1].session_id, "coding:older");
    }

    #[tokio::test]
    async fn child_sessions_are_excluded_from_top_level() {
        let repos = fresh_repos().await;
        insert_coding_session(&repos, "coding:parent").await;
        insert_coding_session(&repos, "coding:child").await;
        sqlx::query("UPDATE sessions SET parent_session_id = ? WHERE key = ?")
            .bind("coding:parent")
            .bind("coding:child")
            .execute(repos.pool())
            .await
            .unwrap();
        let out = list_sessions(&repos).await.unwrap();
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].session_id, "coding:parent");
    }
}
