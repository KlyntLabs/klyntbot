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
