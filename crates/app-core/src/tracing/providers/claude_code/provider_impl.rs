//! `ClaudeCodeTracingProvider` — wires submodules into the trait.

use async_trait::async_trait;
use common::{KlyntbotError, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use super::{cache::SummaryCache, discovery, import, loader, stats, subagent_loader, summary};
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
    cache: Arc<SummaryCache>,
}

impl ClaudeCodeTracingProvider {
    pub fn new(claude_root: PathBuf, imported_root: PathBuf) -> Self {
        Self {
            claude_root,
            imported_root,
            cache: Arc::new(SummaryCache::new()),
        }
    }

    async fn discovered(&self) -> Result<Vec<discovery::DiscoveredSession>> {
        discovery::discover_sessions(&self.claude_root, &self.imported_root).await
    }

    async fn find_session(&self, session_id: &str) -> Result<discovery::DiscoveredSession> {
        let all = self.discovered().await?;
        all.into_iter()
            .find(|d| d.session_id == session_id)
            .ok_or_else(|| {
                KlyntbotError::StorageNotFound(format!("claudeCode session: {session_id}"))
            })
    }
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
        let discovered = self.discovered().await?;
        let mut out = Vec::with_capacity(discovered.len());
        for d in discovered {
            let mtime = tokio::fs::metadata(&d.jsonl_path)
                .await
                .ok()
                .and_then(|m| m.modified().ok());
            if let Some(mt) = mtime {
                if let Some(s) = self.cache.get(&d.jsonl_path, mt) {
                    out.push(s);
                    continue;
                }
            }
            let s = summary::build_summary(&d).await?;
            if let Some(mt) = mtime {
                self.cache.put(d.jsonl_path.clone(), mt, s.clone());
            }
            out.push(s);
        }
        out.sort_by(|a, b| b.last_event_at.cmp(&a.last_event_at));
        Ok(out)
    }

    async fn load_session(&self, session_id: &str, scope: Scope) -> Result<SessionDetail> {
        match scope {
            Scope::Main => {
                let d = self.find_session(session_id).await?;
                let loaded = loader::load_session(&d.jsonl_path).await?;
                Ok(SessionDetail {
                    session_id: session_id.to_string(),
                    provider_id: ID.to_string(),
                    scope: Scope::Main,
                    stats: loaded.stats,
                    events: loaded.events,
                    truncated: loaded.truncated,
                    total_event_count: loaded.total_event_count,
                })
            }
            Scope::Subagent { agent_id } => {
                let d = self.find_session(session_id).await?;
                subagent_loader::load_subagent_session(&d.source_dir, session_id, &agent_id).await
            }
        }
    }

    async fn load_context(&self, _: &str, _: Scope) -> Result<Vec<ContextMessage>> {
        Err(KlyntbotError::NotImplemented(
            "claudeCode does not declare Context tab".to_string(),
        ))
    }

    async fn load_state(&self, _: &str) -> Result<SessionState> {
        Err(KlyntbotError::NotImplemented(
            "claudeCode does not declare State tab".to_string(),
        ))
    }

    async fn list_subagents(&self, session_id: &str) -> Result<Vec<SubagentSummary>> {
        let d = self.find_session(session_id).await?;
        subagent_loader::list_subagents(&d.source_dir, session_id).await
    }

    async fn import_from_file(&self, path: &Path) -> Result<String> {
        import::import_from_file(&self.imported_root, path).await
    }

    async fn open_dir(&self, session_id: &str) -> Result<PathBuf> {
        let d = self.find_session(session_id).await?;
        Ok(d.source_dir)
    }

    async fn stats(&self) -> Result<StatsBundle> {
        let discovered = self.discovered().await?;
        stats::aggregate(&discovered).await
    }

    async fn session_summary(&self, session_id: &str) -> Result<SessionSummary> {
        let d = self.find_session(session_id).await?;
        summary::build_summary(&d).await
    }

    async fn load_subagent_session(
        &self,
        session_id: &str,
        agent_id: &str,
    ) -> Result<SessionDetail> {
        let d = self.find_session(session_id).await?;
        subagent_loader::load_subagent_session(&d.source_dir, session_id, agent_id).await
    }

    async fn load_subagent_context(&self, _: &str, _: &str) -> Result<Vec<ContextMessage>> {
        Err(KlyntbotError::NotImplemented(
            "claudeCode does not declare Context tab for subagents".to_string(),
        ))
    }
}
