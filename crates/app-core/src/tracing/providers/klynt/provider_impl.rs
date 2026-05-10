//! `KlyntTracingProvider` — implements `TracingProvider` over Klynt's native
//! `sessions` + `session_messages` tables.

use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::{Path, PathBuf};
use storage::repos::Repos;

use crate::tracing::provider::TracingProvider;
use crate::tracing::providers::klynt::{
    context_loader, discovery, loader, state_loader, stats as stats_mod, subagent_loader, summary,
};
use crate::tracing::types::{
    ContextMessage, HeaderChip, Scope, SessionDetail, SessionState, SessionSummary, SessionTab,
    StatsBundle, SubagentSummary,
};

pub struct KlyntTracingProvider {
    repos: Repos,
    data_dir: PathBuf,
}

impl KlyntTracingProvider {
    pub fn new(repos: Repos, data_dir: PathBuf) -> Self {
        Self { repos, data_dir }
    }
}

#[async_trait]
impl TracingProvider for KlyntTracingProvider {
    fn id(&self) -> &'static str {
        "klynt"
    }

    fn display_name(&self) -> &'static str {
        "Klynt"
    }

    fn supported_tabs(&self) -> &'static [SessionTab] {
        &[
            SessionTab::Wire,
            SessionTab::Context,
            SessionTab::State,
            SessionTab::Agents,
        ]
    }

    fn header_layout(&self) -> &'static [HeaderChip] {
        &[
            HeaderChip::Turns,
            HeaderChip::ToolCalls,
            HeaderChip::Errors,
            HeaderChip::Compactions,
            HeaderChip::Agents,
            HeaderChip::Tokens,
            HeaderChip::Model,
        ]
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        discovery::list_sessions(&self.repos).await
    }

    async fn load_session(&self, session_id: &str, scope: Scope) -> Result<SessionDetail> {
        let target_id = match &scope {
            Scope::Main => session_id.to_string(),
            Scope::Subagent { agent_id } => agent_id.clone(),
        };
        let parent = match &scope {
            Scope::Main => None,
            Scope::Subagent { agent_id } => Some(agent_id.clone()),
        };
        let loaded = loader::load_session(&self.repos, &target_id).await?;
        Ok(SessionDetail {
            session_id: session_id.to_string(),
            provider_id: "klynt".into(),
            scope,
            stats: loaded.stats,
            events: loaded
                .events
                .into_iter()
                .map(|mut e| {
                    if e.parent_subagent_id.is_none() {
                        e.parent_subagent_id = parent.clone();
                    }
                    e
                })
                .collect(),
            truncated: loaded.truncated,
            total_event_count: loaded.total_event_count,
        })
    }

    async fn load_context(&self, session_id: &str, scope: Scope) -> Result<Vec<ContextMessage>> {
        let target_id = match scope {
            Scope::Main => session_id.to_string(),
            Scope::Subagent { agent_id } => agent_id,
        };
        context_loader::load_context(&self.repos, &target_id).await
    }

    async fn load_state(&self, session_id: &str) -> Result<SessionState> {
        state_loader::load_state(&self.repos, session_id).await
    }

    async fn list_subagents(&self, session_id: &str) -> Result<Vec<SubagentSummary>> {
        subagent_loader::list_subagents(&self.repos, session_id).await
    }

    async fn import_from_file(&self, _path: &Path) -> Result<String> {
        Err(KlyntbotError::Storage(
            "klynt provider does not support file import".into(),
        ))
    }

    async fn open_dir(&self, _session_id: &str) -> Result<PathBuf> {
        // Surface the data directory; the FE uses this for "Open in Finder".
        Ok(self.data_dir.clone())
    }

    async fn stats(&self) -> Result<StatsBundle> {
        stats_mod::aggregate(&self.repos).await
    }

    async fn session_summary(&self, session_id: &str) -> Result<SessionSummary> {
        summary::compute(&self.repos, session_id).await
    }

    async fn load_subagent_session(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<SessionDetail> {
        self.load_session(
            session_id,
            Scope::Subagent {
                agent_id: agent_id.to_string(),
            },
        )
        .await
    }

    async fn load_subagent_context(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<Vec<ContextMessage>> {
        self.load_context(
            session_id,
            Scope::Subagent {
                agent_id: agent_id.to_string(),
            },
        )
        .await
    }
}
