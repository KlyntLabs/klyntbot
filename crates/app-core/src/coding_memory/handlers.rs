//! Handlers backing the desktop Tauri commands for coding-memory.

use coding_ingest::desktop_lock::is_desktop_alive;
use coding_ingest::store::IngestEventLogRepo;
use common::Result;
use desktop_shared::commands::coding_memory::*;
use sqlx::Row;
use std::path::Path;

pub async fn status(
    repo: &IngestEventLogRepo,
    data_dir: &Path,
) -> Result<CodingMemoryStatusResponse> {
    let lock = data_dir.join("desktop.lock");
    let buffer = data_dir.join("ingest-buffer.jsonl");
    let buffered = std::fs::metadata(&buffer).map(|m| m.len()).unwrap_or(0) as i64;
    let unprocessed = repo.count_unprocessed().await?;
    Ok(CodingMemoryStatusResponse {
        daemon_alive: is_desktop_alive(&lock),
        buffered_event_count: buffered,
        unprocessed_event_count: unprocessed,
        socket_path: data_dir.join("ingest.sock").to_string_lossy().into(),
    })
}

pub async fn cli_health(pool: &sqlx::SqlitePool) -> Result<Vec<CliHealthRow>> {
    let sources = ["claude-code", "codex", "kimi-cli", "opencode"];
    let mut out = Vec::with_capacity(sources.len());
    for src in sources {
        let row = sqlx::query(
            "SELECT COUNT(*) as c, MAX(occurred_at) as last
             FROM ingest_event_log
             WHERE source = ? AND received_at > datetime('now', '-1 day')",
        )
        .bind(src)
        .fetch_one(pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("cli_health: {e}")))?;
        out.push(CliHealthRow {
            cli: src.into(),
            enabled: false, // filled in by caller from config
            last_event_at: row.try_get::<Option<String>, _>("last").unwrap_or(None),
            event_count_24h: row.try_get::<i64, _>("c").unwrap_or(0),
        });
    }
    Ok(out)
}

pub async fn session_replay(
    pool: &sqlx::SqlitePool,
    session_id: Option<String>,
    limit: i64,
    offset: i64,
) -> Result<Vec<SessionReplayEntry>> {
    let rows = match session_id {
        Some(sid) => {
            sqlx::query(
                "SELECT id, source, session_id, kind, occurred_at, payload
                 FROM ingest_event_log WHERE session_id = ?
                 ORDER BY occurred_at ASC LIMIT ? OFFSET ?",
            )
            .bind(sid)
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
        None => {
            sqlx::query(
                "SELECT id, source, session_id, kind, occurred_at, payload
                 FROM ingest_event_log
                 ORDER BY received_at DESC LIMIT ? OFFSET ?",
            )
            .bind(limit)
            .bind(offset)
            .fetch_all(pool)
            .await
        }
    }
    .map_err(|e| common::KlyntbotError::Storage(format!("session_replay: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|r| SessionReplayEntry {
            id: r.get("id"),
            source: r.get("source"),
            session_id: r.get("session_id"),
            kind: r.get("kind"),
            occurred_at: r.get("occurred_at"),
            payload: r.get("payload"),
        })
        .collect())
}

pub async fn session_list(
    pool: &sqlx::SqlitePool,
    args: SessionListArgs,
) -> Result<Vec<SessionSummaryDto>> {
    let mut sql = String::from(
        "SELECT session_id,
                source,
                MIN(cwd) AS cwd,
                MIN(repo_id) AS repo_id,
                MIN(occurred_at) AS started_at,
                MAX(occurred_at) AS last_event_at,
                COUNT(*) AS event_count,
                SUM(CASE WHEN kind LIKE '%Turn%' THEN 1 ELSE 0 END) AS turn_count,
                SUM(CASE WHEN kind = 'toolCall' THEN 1 ELSE 0 END) AS tool_call_count,
                SUM(CASE WHEN kind = 'error' THEN 1 ELSE 0 END) AS error_count
         FROM ingest_event_log
         WHERE occurred_at >= datetime('now', ?1)",
    );
    let since_clause = format!("-{} days", args.since_days);
    let mut binds: Vec<String> = vec![since_clause];
    if let Some(src) = args.source.as_ref() {
        sql.push_str(" AND source = ?2");
        binds.push(provider_id_to_db_slug(src));
    }
    if let Some(repo) = args.repo_id.as_ref() {
        sql.push_str(if binds.len() == 2 { " AND repo_id = ?3" } else { " AND repo_id = ?2" });
        binds.push(repo.clone());
    }
    sql.push_str(" GROUP BY session_id, source ORDER BY last_event_at DESC LIMIT ? OFFSET ?");
    binds.push(args.limit.to_string());
    binds.push(args.offset.to_string());

    let mut q = sqlx::query(&sql);
    for b in &binds {
        q = q.bind(b);
    }
    let rows = q.fetch_all(pool).await
        .map_err(|e| common::KlyntbotError::Storage(format!("session_list: {e}")))?;

    let result = rows
        .into_iter()
        .map(|r| {
            SessionSummaryDto {
                session_id: r.get("session_id"),
                source: r.get("source"),
                cwd: r.try_get("cwd").ok(),
                repo_id: r.try_get("repo_id").ok(),
                started_at: r.get("started_at"),
                last_event_at: r.get("last_event_at"),
                event_count: r.get("event_count"),
                turn_count: r.get("turn_count"),
                tool_call_count: r.get("tool_call_count"),
                error_count: r.get("error_count"),
                total_input_tokens: 0,
                total_output_tokens: 0,
                total_cost_usd: 0.0,
            }
        })
        .collect();
    Ok(result)
}

/// Memory Browser — flat list of active (non-superseded) coding-domain semantic facts.
pub async fn memory_browser(
    pool: &sqlx::SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<Vec<MemoryBrowserRow>> {
    let rows = sqlx::query(
        "SELECT id, subject, predicate, object
         FROM semantic_facts
         WHERE domain IN ('coding', 'preferences', 'procedural', 'work')
           AND valid_until IS NULL
         ORDER BY recorded_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("memory_browser: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|r| MemoryBrowserRow {
            id: r.get("id"),
            subject: r.get("subject"),
            predicate: r.get("predicate"),
            object: r.get("object"),
        })
        .collect())
}

/// Activity Timeline — daily count of ingest_event_log rows over the last `days`.
pub async fn activity_timeline(pool: &sqlx::SqlitePool, days: i64) -> Result<Vec<ActivityBucket>> {
    let rows = sqlx::query(
        "SELECT date(occurred_at) AS d, COUNT(*) AS c
         FROM ingest_event_log
         WHERE occurred_at > datetime('now', ? || ' days')
         GROUP BY d
         ORDER BY d ASC",
    )
    .bind(format!("-{days}"))
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("activity_timeline: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|r| ActivityBucket {
            date: r.try_get::<String, _>("d").unwrap_or_default(),
            count: r.try_get::<i64, _>("c").unwrap_or(0),
        })
        .collect())
}

// Pricing lives in `common::pricing` — single source of truth across the workspace.
// To add or update a model, edit `crates/common/src/pricing.rs::MODEL_PRICING`.

/// Cost Tracker — rich breakdown per (date × model) with aggregates.
pub async fn cost_breakdown(pool: &sqlx::SqlitePool, days: i64) -> Result<CostBreakdown> {
    use std::collections::HashSet;

    let mut rows: Vec<CostBreakdownRow> = Vec::new();

    // The klynt-cli rollup and the hook-derived rollup are independent — issue both in parallel.
    let days_arg = format!("-{days}");
    let klynt_q = sqlx::query(
        "SELECT date(started_at) AS d,
                COALESCE(SUM(total_cost_usd), 0.0) AS total,
                COALESCE(SUM(total_tokens_in), 0)  AS in_tok,
                COALESCE(SUM(total_tokens_out), 0) AS out_tok,
                COUNT(*) AS req
         FROM klynt_sessions
         WHERE started_at > datetime('now', ? || ' days')
         GROUP BY d",
    )
    .bind(&days_arg)
    .fetch_all(pool);
    let hook_q = sqlx::query(
        "WITH session_models AS (
            SELECT session_id,
                   COALESCE(json_extract(payload, '$.kind.model'), 'unknown') AS model
            FROM ingest_event_log
            WHERE kind = 'sessionStart'
            GROUP BY session_id
         )
         SELECT date(e.occurred_at) AS d,
                COALESCE(sm.model, 'unknown') AS model,
                COUNT(*) AS req,
                COALESCE(SUM(json_extract(e.payload, '$.kind.token_usage.prompt_tokens')), 0)     AS in_tok,
                COALESCE(SUM(json_extract(e.payload, '$.kind.token_usage.completion_tokens')), 0) AS out_tok,
                COALESCE(SUM(json_extract(e.payload, '$.kind.token_usage.cached_tokens')), 0)     AS cache_tok
         FROM ingest_event_log e
         LEFT JOIN session_models sm ON sm.session_id = e.session_id
         WHERE e.kind = 'assistantMsg'
           AND e.occurred_at > datetime('now', ? || ' days')
           AND json_extract(e.payload, '$.kind.token_usage') IS NOT NULL
         GROUP BY d, model",
    )
    .bind(&days_arg)
    .fetch_all(pool);
    let (klynt_rows, hook_rows) = tokio::try_join!(klynt_q, hook_q)
        .map_err(|e| common::KlyntbotError::Storage(format!("cost_breakdown: {e}")))?;

    for r in klynt_rows {
        let total: f64 = r.try_get("total").unwrap_or(0.0);
        rows.push(CostBreakdownRow {
            date: r.try_get("d").unwrap_or_default(),
            model: "klynt-cli".into(),
            source: "klynt-cli".into(),
            requests: r.try_get("req").unwrap_or(0),
            input_tokens: r.try_get("in_tok").unwrap_or(0),
            output_tokens: r.try_get("out_tok").unwrap_or(0),
            cached_tokens: 0,
            input_cost_usd: 0.0,
            output_cost_usd: 0.0,
            total_cost_usd: total,
        });
    }

    for r in hook_rows {
        let model: String = r.try_get("model").unwrap_or_else(|_| "unknown".into());
        let in_tok: i64 = r.try_get("in_tok").unwrap_or(0);
        let out_tok: i64 = r.try_get("out_tok").unwrap_or(0);
        let cache_tok: i64 = r.try_get("cache_tok").unwrap_or(0);
        let req: i64 = r.try_get("req").unwrap_or(0);
        let (in_cost, out_cost) =
            common::pricing::cost_for(&model, in_tok as u64, out_tok as u64).unwrap_or((0.0, 0.0));
        rows.push(CostBreakdownRow {
            date: r.try_get("d").unwrap_or_default(),
            model,
            source: "hooks".into(),
            requests: req,
            input_tokens: in_tok,
            output_tokens: out_tok,
            cached_tokens: cache_tok,
            input_cost_usd: in_cost,
            output_cost_usd: out_cost,
            total_cost_usd: in_cost + out_cost,
        });
    }

    rows.sort_by(|a, b| a.date.cmp(&b.date).then_with(|| a.model.cmp(&b.model)));

    let total_cost_usd = rows.iter().map(|r| r.total_cost_usd).sum();
    let total_input_tokens = rows.iter().map(|r| r.input_tokens).sum();
    let total_output_tokens = rows.iter().map(|r| r.output_tokens).sum();
    let total_cached_tokens = rows.iter().map(|r| r.cached_tokens).sum();
    let total_requests = rows.iter().map(|r| r.requests).sum();
    let model_count = rows
        .iter()
        .map(|r| r.model.as_str())
        .collect::<HashSet<_>>()
        .len() as i64;
    let day_count = rows
        .iter()
        .map(|r| r.date.as_str())
        .collect::<HashSet<_>>()
        .len() as i64;
    let bucket_count = rows.len() as i64;

    Ok(CostBreakdown {
        rows,
        total_cost_usd,
        total_input_tokens,
        total_output_tokens,
        total_cached_tokens,
        total_requests,
        bucket_count,
        model_count,
        day_count,
    })
}

/// Sensitivity Inspector — facts where `metadata.sensitivity` is set, ordered most recent first.
pub async fn sensitivity_inspector(
    pool: &sqlx::SqlitePool,
    limit: i64,
    offset: i64,
) -> Result<Vec<SensitivityRow>> {
    let rows = sqlx::query(
        "SELECT id, subject, predicate, object, metadata
         FROM semantic_facts
         WHERE valid_until IS NULL AND metadata IS NOT NULL
         ORDER BY recorded_at DESC
         LIMIT ? OFFSET ?",
    )
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await
    .map_err(|e| common::KlyntbotError::Storage(format!("sensitivity_inspector: {e}")))?;
    Ok(rows
        .into_iter()
        .map(|r| {
            let meta: Option<String> = r.try_get("metadata").ok();
            let sensitivity = meta
                .as_deref()
                .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                .and_then(|v| {
                    v.get("sensitivity")
                        .and_then(|s| s.as_str().map(str::to_string))
                })
                .unwrap_or_else(|| "public".into());
            SensitivityRow {
                id: r.get("id"),
                subject: r.get("subject"),
                predicate: r.get("predicate"),
                object: r.get("object"),
                sensitivity,
            }
        })
        .collect())
}


/// Translate camelCase ProviderId values from the UI to kebab-case DB slugs.
/// `codex` is identity; the other three require translation.
fn provider_id_to_db_slug(id: &str) -> String {
    match id {
        "claudeCode" => "claude-code".into(),
        "kimiCli" => "kimi-cli".into(),
        "openCode" => "opencode".into(),
        _ => id.into(),
    }
}
