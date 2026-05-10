//! Per-session aggregate counts.

use common::{KlyntbotError, Result};
use jiff::Timestamp;
use std::path::PathBuf;
use storage::repos::Repos;

use crate::tracing::types::{SessionMetadataInfo, SessionSummary};

#[derive(sqlx::FromRow)]
struct SummaryRow {
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

pub async fn compute(repos: &Repos, session_id: &str) -> Result<SessionSummary> {
    let row: Option<SummaryRow> = sqlx::query_as::<_, SummaryRow>(
        r#"
        SELECT
          s.key                                        AS key,
          s.cwd                                        AS cwd,
          s.repo_id                                    AS repo_id,
          s.metadata                                   AS metadata,
          s.archived_at                                AS archived_at,
          s.created_at                                 AS created_at,
          s.updated_at                                 AS updated_at,
          (SELECT COUNT(*) FROM session_messages m
             WHERE m.session_key = s.key)              AS event_count,
          (SELECT COUNT(DISTINCT m.turn_id) FROM session_messages m
             WHERE m.session_key = s.key
               AND m.turn_id IS NOT NULL)              AS turn_count,
          (SELECT COUNT(*) FROM session_messages m
             WHERE m.session_key = s.key
               AND m.parts LIKE '%"kind":"tool_call"%') AS tool_call_count,
          (SELECT COUNT(*) FROM session_messages m
             WHERE m.session_key = s.key
               AND m.parts LIKE '%"is_error":true%')   AS error_count,
          (SELECT COUNT(*) FROM sessions c
             WHERE c.parent_session_id = s.key)        AS subagent_count
        FROM sessions s
        WHERE s.key = ?
        "#,
    )
    .bind(session_id)
    .fetch_optional(repos.pool())
    .await
    .map_err(|e| KlyntbotError::Storage(format!("klynt summary: {e}")))?;

    let r = row.ok_or_else(|| KlyntbotError::StorageNotFound(format!("session {session_id}")))?;

    let title = serde_json::from_str::<serde_json::Value>(&r.metadata)
        .ok()
        .and_then(|v| v.get("title").and_then(|t| t.as_str().map(String::from)));
    let cwd = r.cwd.map(PathBuf::from);
    let project_basename = cwd
        .as_ref()
        .and_then(|p| p.file_name())
        .map(|n| n.to_string_lossy().to_string());

    Ok(SessionSummary {
        session_id: r.key.clone(),
        provider_id: "klynt".into(),
        source_dir: PathBuf::new(),
        cwd,
        project_basename,
        custom_title: title.clone(),
        started_at: ms(r.created_at),
        last_event_at: ms(r.updated_at),
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
    })
}

fn ms(v: i64) -> Timestamp {
    Timestamp::from_millisecond(v).unwrap_or(Timestamp::UNIX_EPOCH)
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
    async fn missing_returns_not_found() {
        let repos = fresh_repos().await;
        let err = compute(&repos, "missing").await.unwrap_err();
        assert!(matches!(err, KlyntbotError::StorageNotFound(_)));
    }

    #[tokio::test]
    async fn returns_zero_counts_for_empty_session() {
        let repos = fresh_repos().await;
        repos
            .sessions
            .upsert_session_with_mode("coding:1", SessionMode::Coding, &serde_json::json!({}))
            .await
            .unwrap();
        let s = compute(&repos, "coding:1").await.unwrap();
        assert_eq!(s.session_id, "coding:1");
        assert_eq!(s.provider_id, "klynt");
        assert_eq!(s.turn_count, 0);
        assert_eq!(s.tool_call_count, 0);
        assert!(!s.has_wire);
    }
}
