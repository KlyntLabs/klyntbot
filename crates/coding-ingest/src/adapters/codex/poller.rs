//! Long-lived task that tails Codex's session rollout JSONL files.
//!
//! Codex stores rich per-session logs at
//! `~/.codex/sessions/<YYYY>/<MM>/<DD>/rollout-<ts>-<sessionId>.jsonl`. Each
//! line is a typed JSON envelope (`session_meta`, `response_item`, or
//! `event_msg`). Codex itself has no general-purpose hook system, so we
//! ingest by polling its directory tree and tailing new bytes per file.
//!
//! State is kept in memory only (per-file byte offsets) — on restart we
//! re-process from current EOF rather than back-filling. Acceptable for
//! best-effort ingestion.

use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use crate::scope_resolver::resolve_scope;
use crate::store::IngestEventLogRepo;
use common::Result;
use jiff::Timestamp;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncSeekExt, BufReader};
use tokio::sync::{mpsc, Mutex};
use uuid::Uuid;

/// Filesystem-backed poller for Codex session rollout files.
pub struct CodexPoller {
    sessions_dir: PathBuf,
    interval: std::time::Duration,
    event_tx: mpsc::UnboundedSender<AgentEvent>,
    /// Repo for inserting events into `ingest_event_log` — mirrors what the
    /// socket-handler path does for hook-sourced events.
    repo: Arc<IngestEventLogRepo>,
    /// Per-file byte offset (last position consumed).
    offsets: Arc<Mutex<HashMap<PathBuf, u64>>>,
    /// Sticky per-session metadata (cwd, model) extracted from `session_meta`
    /// so subsequent `response_item` lines can attach it.
    session_meta: Arc<Mutex<HashMap<String, SessionMeta>>>,
}

#[derive(Clone, Debug, Default)]
struct SessionMeta {
    session_id: String,
    cwd: PathBuf,
    model: Option<String>,
}

impl CodexPoller {
    /// Create a poller. `sessions_dir` is `~/.codex/sessions` typically.
    pub fn new(
        sessions_dir: PathBuf,
        event_tx: mpsc::UnboundedSender<AgentEvent>,
        repo: Arc<IngestEventLogRepo>,
        interval: std::time::Duration,
    ) -> Self {
        Self {
            sessions_dir,
            interval,
            event_tx,
            repo,
            offsets: Arc::new(Mutex::new(HashMap::new())),
            session_meta: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Spawn the polling loop as a detached tokio task.
    pub fn spawn(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            tracing::info!(sessions_dir = %self.sessions_dir.display(), "codex poller starting");
            if let Err(e) = self.seed_offsets().await {
                tracing::warn!(error = %e, "codex poller seed_offsets failed");
            } else {
                let n = self.offsets.lock().await.len();
                tracing::info!(seeded_files = n, "codex poller seeded");
            }
            let mut interval = tokio::time::interval(self.interval);
            loop {
                interval.tick().await;
                if let Err(e) = self.poll_once().await {
                    tracing::warn!(error = %e, "codex poll failed");
                }
            }
        })
    }

    async fn seed_offsets(&self) -> Result<()> {
        let files = list_jsonl_files(&self.sessions_dir).await?;
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
        let files = list_jsonl_files(&self.sessions_dir).await?;
        for path in files {
            if let Err(e) = self.tail_file(&path).await {
                tracing::warn!(error = %e, file = %path.display(), "codex tail failed");
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
        tracing::info!(file = %path.display(), last_offset, file_size, "codex tailing new bytes");
        let mut file = tokio::fs::File::open(path).await.map_err(|e| {
            common::KlyntbotError::Storage(format!("codex open {}: {e}", path.display()))
        })?;
        file.seek(std::io::SeekFrom::Start(last_offset))
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("codex seek: {e}")))?;
        let mut reader = BufReader::new(file);
        let mut line = String::new();
        let mut new_offset = last_offset;
        loop {
            line.clear();
            let n = reader
                .read_line(&mut line)
                .await
                .map_err(|e| common::KlyntbotError::Storage(format!("codex read: {e}")))?;
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
            match self.parse_line(trimmed).await {
                Ok(events) => {
                    if !events.is_empty() {
                        tracing::info!(count = events.len(), "codex emitting events");
                    }
                    for e in events {
                        let event = AgentEvent::V1(e);
                        if let Err(err) = self.repo.insert(&event).await {
                            tracing::warn!(error = %err, "codex repo.insert failed");
                        }
                        if self.event_tx.send(event).is_err() {
                            tracing::warn!("codex event_tx send failed (channel closed)");
                        }
                    }
                }
                Err(e) => tracing::warn!(error = %e, "codex parse line failed"),
            }
        }
        let mut guard = self.offsets.lock().await;
        guard.insert(path.to_path_buf(), new_offset);
        Ok(())
    }

    async fn parse_line(&self, line: &str) -> Result<Vec<AgentEventV1>> {
        let env: Envelope = serde_json::from_str(line)
            .map_err(|e| common::KlyntbotError::Storage(format!("codex envelope: {e}")))?;
        let occurred_at = env
            .timestamp
            .as_deref()
            .and_then(|s| s.parse::<Timestamp>().ok())
            .unwrap_or_else(Timestamp::now);
        match env.envelope_type.as_str() {
            "session_meta" => {
                let payload: SessionMetaPayload = match serde_json::from_value(env.payload.clone()) {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, payload = ?env.payload, "codex session_meta parse failed");
                        return Ok(vec![]);
                    }
                };
                let meta = SessionMeta {
                    session_id: payload.id.clone(),
                    cwd: PathBuf::from(payload.cwd.unwrap_or_else(|| "/".into())),
                    model: payload.model_provider,
                };
                self.session_meta
                    .lock()
                    .await
                    .insert(meta.session_id.clone(), meta.clone());
                let repo = resolve_scope(&meta.cwd);
                Ok(vec![AgentEventV1 {
                    id: Uuid::new_v4(),
                    source: AgentSource::Codex,
                    session_id: meta.session_id,
                    turn_id: None,
                    cwd: meta.cwd,
                    repo,
                    occurred_at,
                    kind: EventKind::SessionStart {
                        model: meta.model,
                        source_reason: payload.source.unwrap_or_else(|| "codex".into()),
                    },
                }])
            }
            "response_item" => self.parse_response_item(env.payload, occurred_at).await,
            // event_msg variants are mostly redundant with response_item — skip
            // for now; we can add specific handling later if needed.
            _ => Ok(vec![]),
        }
    }

    async fn parse_response_item(
        &self,
        payload: serde_json::Value,
        occurred_at: Timestamp,
    ) -> Result<Vec<AgentEventV1>> {
        let item: ResponseItemPayload = match serde_json::from_value(payload.clone()) {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(error = %e, payload = ?payload, "codex response_item parse failed");
                return Ok(vec![]);
            }
        };
        if item.item_type != "message" {
            return Ok(vec![]);
        }
        // We can't recover session_id from a `response_item` line directly —
        // codex emits it only in the session_meta header. We pick the most
        // recently seen session as a fallback for now (codex sessions are
        // serial within a single rollout file, so this is correct).
        let (session_id, cwd) = {
            let guard = self.session_meta.lock().await;
            // HashMap iteration order is unspecified, but in practice only one
            // session is active per rollout file at a time, so any value works.
            guard
                .values()
                .next()
                .map(|m| (m.session_id.clone(), m.cwd.clone()))
                .unwrap_or_else(|| ("unknown".into(), PathBuf::from("/")))
        };
        let repo = resolve_scope(&cwd);
        let text = item
            .content
            .into_iter()
            .filter_map(|c| match c.content_type.as_str() {
                "input_text" | "output_text" => Some(c.text),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("");
        if text.is_empty() {
            return Ok(vec![]);
        }
        let kind = match item.role.as_str() {
            "user" => EventKind::UserPrompt {
                text,
                attachments: vec![],
            },
            "assistant" => EventKind::AssistantMsg {
                text,
                truncated: false,
                token_usage: None,
            },
            // Skip developer/system messages — those are codex's internal scaffolding.
            _ => return Ok(vec![]),
        };
        Ok(vec![AgentEventV1 {
            id: Uuid::new_v4(),
            source: AgentSource::Codex,
            session_id,
            turn_id: None,
            cwd,
            repo,
            occurred_at,
            kind,
        }])
    }
}

#[derive(Debug, Deserialize)]
struct Envelope {
    #[serde(default)]
    timestamp: Option<String>,
    #[serde(rename = "type")]
    envelope_type: String,
    #[serde(default)]
    payload: serde_json::Value,
}

#[derive(Debug, Deserialize)]
struct SessionMetaPayload {
    id: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    model_provider: Option<String>,
    #[serde(default)]
    source: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResponseItemPayload {
    #[serde(rename = "type")]
    item_type: String,
    #[serde(default)]
    role: String,
    #[serde(default)]
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: String,
}

/// Walk `<sessions_dir>/<YYYY>/<MM>/<DD>/*.jsonl` and return every match.
async fn list_jsonl_files(root: &Path) -> Result<Vec<PathBuf>> {
    if !root.exists() {
        return Ok(vec![]);
    }
    let mut out = Vec::new();
    walk(root, &mut out, 0).await?;
    Ok(out)
}

async fn walk(dir: &Path, out: &mut Vec<PathBuf>, depth: usize) -> Result<()> {
    // Codex layout: sessions/YYYY/MM/DD/*.jsonl — depth 4. Cap to avoid
    // run-away recursion on user-modified trees.
    if depth > 4 {
        return Ok(());
    }
    let mut rd = match tokio::fs::read_dir(dir).await {
        Ok(rd) => rd,
        Err(_) => return Ok(()),
    };
    while let Some(entry) = rd
        .next_entry()
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("codex readdir: {e}")))?
    {
        let path = entry.path();
        let ty = entry.file_type().await.map_err(|e| {
            common::KlyntbotError::Storage(format!("codex file_type: {e}"))
        })?;
        if ty.is_dir() {
            Box::pin(walk(&path, out, depth + 1)).await?;
        } else if ty.is_file() && path.extension().and_then(|s| s.to_str()) == Some("jsonl") {
            out.push(path);
        }
    }
    Ok(())
}
