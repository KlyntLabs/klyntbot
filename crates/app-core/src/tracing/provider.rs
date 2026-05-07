//! TracingProvider trait — implemented per CLI.

use async_trait::async_trait;
use common::Result;
use std::path::{Path, PathBuf};

use super::types::{
    ContextMessage, HeaderChip, Scope, SessionDetail, SessionState, SessionSummary, SessionTab,
    StatsBundle, SubagentSummary,
};

#[async_trait]
pub trait TracingProvider: Send + Sync {
    fn id(&self) -> &'static str;
    fn display_name(&self) -> &'static str;

    /// Tabs this provider supports in the session detail page.
    /// Default returns Kimi's five-tab layout for back-compat.
    fn supported_tabs(&self) -> &'static [SessionTab] {
        &[
            SessionTab::Wire,
            SessionTab::Context,
            SessionTab::State,
            SessionTab::Dual,
            SessionTab::Agents,
        ]
    }

    /// Header chip set for the session detail page, in render order.
    /// Default returns Kimi's chip set.
    fn header_layout(&self) -> &'static [HeaderChip] {
        &[
            HeaderChip::Turns,
            HeaderChip::Steps,
            HeaderChip::ToolCalls,
            HeaderChip::Errors,
            HeaderChip::Compactions,
            HeaderChip::Agents,
            HeaderChip::Duration,
            HeaderChip::Tokens,
            HeaderChip::CacheHitPct,
        ]
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;
    async fn load_session(&self, session_id: &str, scope: Scope) -> Result<SessionDetail>;
    async fn load_context(&self, session_id: &str, scope: Scope) -> Result<Vec<ContextMessage>>;
    async fn load_state(&self, session_id: &str) -> Result<SessionState>;
    async fn list_subagents(&self, session_id: &str) -> Result<Vec<SubagentSummary>>;
    async fn import_from_file(&self, path: &Path) -> Result<String>;
    async fn open_dir(&self, session_id: &str) -> Result<PathBuf>;
    async fn stats(&self) -> Result<StatsBundle>;
    async fn session_summary(&self, session_id: &str) -> Result<SessionSummary>;
    async fn load_subagent_session(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<SessionDetail>;
    async fn load_subagent_context(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<Vec<ContextMessage>>;
}
