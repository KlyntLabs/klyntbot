//! `ClaudeCodeTracingProvider` — wires the per-file submodules into the trait.

use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::{Path, PathBuf};

use crate::tracing::provider::TracingProvider;
use crate::tracing::types::{
    ContextMessage, HeaderChip, Scope, SessionDetail, SessionState, SessionSummary, SessionTab,
    StatsBundle, SubagentSummary,
};

const ID: &str = "claudeCode";
const DISPLAY_NAME: &str = "Claude Code";

const TABS: &[SessionTab] = &[SessionTab::Wire, SessionTab::Agents];

const HEADER: &[HeaderChip] = &[
    HeaderChip::Turns,
    HeaderChip::ToolCalls,
    HeaderChip::Errors,
    HeaderChip::Compactions,
    HeaderChip::Agents,
    HeaderChip::Model,
    HeaderChip::Tokens,
    HeaderChip::CacheHitPct,
    HeaderChip::Duration,
];

pub struct ClaudeCodeTracingProvider {
    pub claude_root: PathBuf,
    pub imported_root: PathBuf,
}

impl ClaudeCodeTracingProvider {
    pub fn new(claude_root: PathBuf, imported_root: PathBuf) -> Self {
        Self {
            claude_root,
            imported_root,
        }
    }
}

fn unsupported<T>(method: &str) -> Result<T> {
    Err(KlyntbotError::NotImplemented(format!(
        "claudeCode tracing provider: {method}"
    )))
}

#[async_trait]
impl TracingProvider for ClaudeCodeTracingProvider {
    fn id(&self) -> &'static str {
        ID
    }
    fn display_name(&self) -> &'static str {
        DISPLAY_NAME
    }

    fn supported_tabs(&self) -> &'static [SessionTab] {
        TABS
    }

    fn header_layout(&self) -> &'static [HeaderChip] {
        HEADER
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        Ok(vec![])
    }
    async fn load_session(&self, _: &str, _: Scope) -> Result<SessionDetail> {
        unsupported("load_session")
    }
    async fn load_context(&self, _: &str, _: Scope) -> Result<Vec<ContextMessage>> {
        unsupported("load_context")
    }
    async fn load_state(&self, _: &str) -> Result<SessionState> {
        unsupported("load_state")
    }
    async fn list_subagents(&self, _: &str) -> Result<Vec<SubagentSummary>> {
        Ok(vec![])
    }
    async fn import_from_file(&self, _: &Path) -> Result<String> {
        unsupported("import_from_file")
    }
    async fn open_dir(&self, _: &str) -> Result<PathBuf> {
        unsupported("open_dir")
    }
    async fn stats(&self) -> Result<StatsBundle> {
        Ok(StatsBundle::default())
    }
    async fn session_summary(&self, _: &str) -> Result<SessionSummary> {
        unsupported("session_summary")
    }
    async fn load_subagent_session(&self, _: &str, _: &str) -> Result<SessionDetail> {
        unsupported("load_subagent_session")
    }
    async fn load_subagent_context(&self, _: &str, _: &str) -> Result<Vec<ContextMessage>> {
        unsupported("load_subagent_context")
    }
}
