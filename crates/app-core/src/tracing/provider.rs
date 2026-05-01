//! TracingProvider trait — implemented per CLI.

use async_trait::async_trait;
use common::Result;
use std::path::{Path, PathBuf};

use super::types::{
    ContextMessage, Scope, SessionDetail, SessionState, SessionSummary, StatsBundle,
    SubagentSummary,
};

/// Per-CLI agent-tracing provider. v1 implements only Kimi.
#[async_trait]
pub trait TracingProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;
    async fn load_session(&self, session_id: &str, scope: Scope) -> Result<SessionDetail>;
    async fn load_context(&self, session_id: &str, scope: Scope) -> Result<Vec<ContextMessage>>;
    async fn load_state(&self, session_id: &str) -> Result<SessionState>;
    async fn list_subagents(&self, session_id: &str) -> Result<Vec<SubagentSummary>>;
    async fn import_from_file(&self, path: &Path) -> Result<String>;
    async fn open_dir(&self, session_id: &str) -> Result<PathBuf>;
    async fn stats(&self) -> Result<StatsBundle>;
}
