//! Adapter for `klyntbot-hook git-post-commit` stdin payload.

use crate::adapters::IngestAdapter;
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind};
use jiff::Timestamp;
use serde::Deserialize;
use std::path::PathBuf;
use uuid::Uuid;

/// Adapter for git post-commit events.
pub struct GitPostCommitAdapter;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Payload {
    commit_hash: String,
    parent_hash: Option<String>,
    repo_root: PathBuf,
    changed_files: Vec<PathBuf>,
}

impl GitPostCommitAdapter {
    /// Parse a complete stdin buffer into an `AgentEvent`.
    pub fn parse(raw: &[u8]) -> common::Result<Option<AgentEvent>> {
        let payload: Payload = serde_json::from_slice(raw)
            .map_err(|e| common::KlyntbotError::Storage(format!("git-post-commit parse: {e}")))?;
        let kind = EventKind::GitCommit {
            commit_hash: payload.commit_hash.clone(),
            parent_hash: payload.parent_hash,
            repo_root: payload.repo_root.clone(),
            changed_files: payload.changed_files,
        };
        Ok(Some(AgentEvent::V1(AgentEventV1 {
            id: Uuid::new_v4(),
            source: AgentSource::ClaudeCode,
            session_id: format!("git:{}", payload.commit_hash),
            turn_id: None,
            cwd: payload.repo_root.clone(),
            repo: None,
            occurred_at: Timestamp::now(),
            kind,
        })))
    }
}

impl IngestAdapter for GitPostCommitAdapter {
    fn source_name(&self) -> &'static str {
        "git-post-commit"
    }

    fn parse(&self, _hook_event: &str, raw: &[u8]) -> common::Result<Option<AgentEvent>> {
        Self::parse(raw)
    }
}
