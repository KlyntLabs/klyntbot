//! Long-lived task that tails kimi-cli per-session `wire.jsonl` files.
//!
//! Layout:
//!   `<sessions_dir>/<work_dir_hash>/<session_uuid>/wire.jsonl`
//!     plus optional `subagents/<id>/wire.jsonl` (recursed).
//!
//! State is in-memory: per-file byte offsets seeded to current size on
//! startup (no backfill). Mirrors `CodexPoller`.

use crate::adapters::kimi_cli::mapper::{map_event, maybe_emit_session_start, SessionState};
use crate::adapters::kimi_cli::wire_file::{collect_events, parse_line, WireLine};
use crate::adapters::kimi_cli::workdir::WorkdirIndex;
use crate::event::AgentEvent;
use crate::store::IngestEventLogRepo;
use common::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::{mpsc, Mutex};

/// Tail kimi `wire.jsonl` files, one tick per `interval`.
pub struct KimiPoller {
    sessions_dir: PathBuf,
    kimi_json_path: PathBuf,
    interval: std::time::Duration,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    repo: Arc<IngestEventLogRepo>,
    offsets: Arc<Mutex<HashMap<PathBuf, u64>>>,
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    workdir_index: Arc<WorkdirIndex>,
}

impl KimiPoller {
    /// Construct with the user's `~/.kimi/sessions` and `~/.kimi/kimi.json`.
    pub fn new(
        sessions_dir: PathBuf,
        kimi_json_path: PathBuf,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
        repo: Arc<IngestEventLogRepo>,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            sessions_dir,
            kimi_json_path,
            interval,
            event_tx,
            repo,
            offsets: Arc::new(Mutex::new(HashMap::new())),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            workdir_index: Arc::new(WorkdirIndex::new()),
        }
    }

    /// Spawn the polling loop as a detached tokio task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            tracing::info!(
                sessions_dir = %self.sessions_dir.display(),
                "kimi poller starting"
            );
            if let Err(e) = self.workdir_index.refresh(&self.kimi_json_path).await {
                tracing::warn!(error = %e, "kimi workdir_index initial refresh failed");
            }
            if let Err(e) = self.seed_offsets().await {
                tracing::warn!(error = %e, "kimi poller seed_offsets failed");
            }
            let mut interval = tokio::time::interval(self.interval);
            loop {
                interval.tick().await;
                if let Err(e) = self.poll_once().await {
                    tracing::warn!(error = %e, "kimi poll failed");
                }
            }
        })
    }

    async fn seed_offsets(&self) -> Result<()> {
        let files = list_wire_files(&self.sessions_dir).await?;
        let mut guard = self.offsets.lock().await;
        for f in files {
            if let Ok(meta) = tokio::fs::metadata(&f).await {
                guard.insert(f, meta.len());
            }
        }
        Ok(())
    }

    async fn poll_once(&self) -> Result<()> {
        if !self.sessions_dir.exists() {
            return Ok(());
        }
        let files = list_wire_files(&self.sessions_dir).await?;
        for path in files {
            if let Err(e) = self.tail_file(&path).await {
                tracing::warn!(
                    error = %e,
                    file = %path.display(),
                    "kimi tail failed"
                );
            }
        }
        Ok(())
    }

    async fn tail_file(&self, path: &Path) -> Result<()> {
        let file_size = match tokio::fs::metadata(path).await {
            Ok(m) => m.len(),
            Err(_) => return Ok(()),
        };
        let last_offset = {
            let guard = self.offsets.lock().await;
            *guard.get(path).unwrap_or(&0)
        };
        if file_size <= last_offset {
            return Ok(());
        }

        let session_id = match path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
        {
            Some(s) => s.to_string(),
            None => return Ok(()),
        };
        let work_dir_hash = match path
            .parent()
            .and_then(|p| p.parent())
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
        {
            Some(s) => s.to_string(),
            None => return Ok(()),
        };
        let cwd = match self.workdir_index.get(&work_dir_hash).await {
            Some(p) => p,
            None => {
                // One-shot refresh on miss before falling back.
                let _ = self.workdir_index.refresh(&self.kimi_json_path).await;
                self.workdir_index
                    .get(&work_dir_hash)
                    .await
                    .unwrap_or_else(|| PathBuf::from("/"))
            }
        };

        let mut file = tokio::fs::File::open(path).await.map_err(|e| {
            common::KlyntbotError::Storage(format!("kimi open {}: {e}", path.display()))
        })?;
        file.seek(std::io::SeekFrom::Start(last_offset))
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("kimi seek: {e}")))?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut new_offset = last_offset;
        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| common::KlyntbotError::Storage(format!("kimi read: {e}")))?;
            if n == 0 {
                break;
            }
            // Skip trailing partial line if file is being written.
            if !line.ends_with('\n') {
                break;
            }
            new_offset += n as u64;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            self.process_line(trimmed, &session_id, &cwd).await;
        }
        let mut guard = self.offsets.lock().await;
        guard.insert(path.to_path_buf(), new_offset);
        Ok(())
    }

    async fn process_line(&self, raw: &str, session_id: &str, cwd: &Path) {
        let parsed = match parse_line(raw) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "kimi parse_line failed");
                return;
            }
        };
        let record = match parsed {
            WireLine::Metadata(_) => return,
            WireLine::Record(r) => r,
        };
        let collected = collect_events(&record.message);

        // Compute all events under the per-session lock, then release it
        // before doing any awaiting work — never hold a mutex across await.
        let to_dispatch: Vec<crate::event::AgentEventV1> = {
            let mut sessions = self.sessions.lock().await;
            let state = sessions.entry(session_id.to_string()).or_default();
            let mut out = Vec::new();

            if !state.session_start_emitted {
                let ts = jiff::Timestamp::new(record.timestamp.trunc() as i64, 0)
                    .unwrap_or_else(|_| jiff::Timestamp::now());
                let model_hint = state.last_model.clone();
                if let Some(evt) =
                    maybe_emit_session_start(state, session_id, cwd, ts, model_hint)
                {
                    out.push(evt);
                }
            }

            for c in &collected {
                out.extend(map_event(state, c, &record, session_id, cwd));
            }
            out
        };

        for evt in to_dispatch {
            self.dispatch(evt).await;
        }
    }

    async fn dispatch(&self, event: crate::event::AgentEventV1) {
        let envelope = AgentEvent::V1(event);
        if let Err(e) = self.repo.insert(&envelope).await {
            tracing::warn!(error = %e, "kimi repo.insert failed");
        }
        if self.event_tx.send(envelope).is_err() {
            tracing::warn!("kimi event_tx send failed (channel closed)");
        }
    }
}

/// Walk `<sessions_dir>` for files named `wire.jsonl`, including under
/// `subagents/<id>/`. Bounded depth to avoid runaway recursion.
async fn list_wire_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    walk(root, &mut out, 0).await?;
    Ok(out)
}

async fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    // Layout depth: sessions/<hash>/<uuid>/wire.jsonl is depth 3; subagents
    // adds two more levels (subagents/<id>/wire.jsonl). Cap at 6.
    if depth > 6 {
        return Ok(());
    }
    let mut rd = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };
    while let Some(entry) = rd
        .next_entry()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("kimi readdir: {e}")))?
    {
        let path = entry.path();
        let ty = entry.file_type().await.map_err(|e| {
            common::KlyntbotError::Storage(format!("kimi file_type: {e}"))
        })?;
        if ty.is_dir() {
            Box::pin(walk(&path, out, depth + 1)).await?;
        } else if ty.is_file()
            && path.file_name().and_then(|s| s.to_str()) == Some("wire.jsonl")
        {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn list_wire_files_finds_nested() {
        let dir = tempfile::tempdir().unwrap();
        let nested = dir.path().join("hash1/uuid1");
        tokio::fs::create_dir_all(&nested).await.unwrap();
        tokio::fs::write(nested.join("wire.jsonl"), "x").await.unwrap();
        let sub = nested.join("subagents/sa1");
        tokio::fs::create_dir_all(&sub).await.unwrap();
        tokio::fs::write(sub.join("wire.jsonl"), "y").await.unwrap();
        let found = list_wire_files(dir.path()).await.unwrap();
        assert_eq!(found.len(), 2);
    }

    #[tokio::test]
    async fn list_wire_files_missing_dir_is_ok() {
        let res = list_wire_files(Path::new("/no/such/path/12345")).await.unwrap();
        assert!(res.is_empty());
    }
}
