//! Cross-session aggregate stats for Klynt.
//!
//! v1: per-project totals only — derived from the same correlated
//! subqueries that discovery uses, grouped by cwd. Tool usage / errors
//! by tool / token series are stubbed out (Klynt does not record per-tool
//! token splits today).

use common::Result;
use std::collections::HashMap;
use std::path::PathBuf;
use storage::repos::Repos;

use crate::tracing::types::{ProjectTotals, StatsBundle};

#[derive(sqlx::FromRow)]
struct StatsRow {
    cwd: Option<String>,
    turn_count: i64,
    tool_call_count: i64,
    error_count: i64,
}

pub async fn aggregate(repos: &Repos) -> Result<StatsBundle> {
    let rows: Vec<StatsRow> = sqlx::query_as::<_, StatsRow>(
        r#"
        SELECT
          s.cwd as cwd,
          (SELECT COUNT(DISTINCT m.turn_id) FROM session_messages m
             WHERE m.session_key = s.key
               AND m.turn_id IS NOT NULL) AS turn_count,
          (SELECT COUNT(*) FROM session_messages m
             WHERE m.session_key = s.key
               AND m.parts LIKE '%"kind":"tool_call"%') AS tool_call_count,
          (SELECT COUNT(*) FROM session_messages m
             WHERE m.session_key = s.key
               AND m.parts LIKE '%"is_error":true%') AS error_count
        FROM sessions s
        WHERE s.mode = 'coding' AND s.parent_session_id IS NULL
        "#,
    )
    .fetch_all(repos.pool())
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("klynt stats: {e}")))?;

    let mut by_project: HashMap<String, ProjectTotals> = HashMap::new();
    for r in rows {
        let cwd_str = r.cwd.unwrap_or_default();
        let basename = PathBuf::from(&cwd_str)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".into());
        let entry = by_project
            .entry(cwd_str.clone())
            .or_insert_with(|| ProjectTotals {
                project_basename: basename,
                cwd: PathBuf::from(&cwd_str),
                session_count: 0,
                turn_count: 0,
                tool_call_count: 0,
                error_count: 0,
                total_input_tokens: 0,
                total_output_tokens: 0,
                cache_read_tokens: 0,
            });
        entry.session_count += 1;
        entry.turn_count += r.turn_count.max(0) as u32;
        entry.tool_call_count += r.tool_call_count.max(0) as u32;
        entry.error_count += r.error_count.max(0) as u32;
    }

    Ok(StatsBundle {
        per_project: by_project.into_values().collect(),
        ..StatsBundle::default()
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
    async fn empty_db_returns_empty_bundle() {
        let repos = fresh_repos().await;
        let s = aggregate(&repos).await.unwrap();
        assert!(s.per_project.is_empty());
    }

    #[tokio::test]
    async fn groups_sessions_by_cwd() {
        let repos = fresh_repos().await;
        repos
            .sessions
            .upsert_session_with_mode("c1", SessionMode::Coding, &serde_json::json!({}))
            .await
            .unwrap();
        repos
            .sessions
            .upsert_session_with_mode("c2", SessionMode::Coding, &serde_json::json!({}))
            .await
            .unwrap();
        sqlx::query("UPDATE sessions SET cwd = ? WHERE key = ?")
            .bind("/proj/foo")
            .bind("c1")
            .execute(repos.pool())
            .await
            .unwrap();
        sqlx::query("UPDATE sessions SET cwd = ? WHERE key = ?")
            .bind("/proj/foo")
            .bind("c2")
            .execute(repos.pool())
            .await
            .unwrap();
        let s = aggregate(&repos).await.unwrap();
        assert_eq!(s.per_project.len(), 1);
        assert_eq!(s.per_project[0].session_count, 2);
        assert_eq!(s.per_project[0].project_basename, "foo");
    }
}
