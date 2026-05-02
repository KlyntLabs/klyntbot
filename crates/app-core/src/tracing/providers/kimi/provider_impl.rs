use async_trait::async_trait;
use coding_ingest::adapters::kimi_cli::workdir::WorkdirIndex;
use common::Result;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::tracing::provider::TracingProvider;
use crate::tracing::providers::kimi::{
    cache::SessionCache, context_loader::load_context, discovery::discover_sessions,
    import::import_from_file, loader::load_session_events, state_loader::load_state,
    stats::aggregate, subagent_loader::list_subagents, summary,
};
use crate::tracing::types::*;
use jiff::Timestamp;

pub struct KimiTracingProvider {
    pub kimi_root: PathBuf,
    pub kimi_json: PathBuf,
    pub imported_root: PathBuf,
    pub workdirs: Arc<WorkdirIndex>,
    cache: SessionCache,
}

impl KimiTracingProvider {
    pub fn new(home_dir: &Path, klyntbot_data: &Path, workdirs: Arc<WorkdirIndex>) -> Self {
        Self {
            kimi_root: home_dir.join(".kimi/sessions"),
            kimi_json: home_dir.join(".kimi/kimi.json"),
            imported_root: klyntbot_data.join("coding_memory/imported_wire"),
            workdirs,
            cache: SessionCache::new(),
        }
    }

    /// Test-only constructor used by integration tests with explicit paths.
    #[doc(hidden)]
    pub fn new_for_test(
        kimi_root: PathBuf,
        kimi_json: PathBuf,
        imported_root: PathBuf,
        workdirs: Arc<WorkdirIndex>,
    ) -> Self {
        Self {
            kimi_root,
            kimi_json,
            imported_root,
            workdirs,
            cache: SessionCache::new(),
        }
    }

    async fn refresh_workdirs(&self) {
        let _ = self.workdirs.refresh(&self.kimi_json).await;
    }

    async fn resolve_session_dir(&self, session_id: &str) -> Result<PathBuf> {
        // Native session: <kimi_root>/<hash>/<session_id>/
        let mut entries = match tokio::fs::read_dir(&self.kimi_root).await {
            Ok(d) => d,
            Err(_) => {
                let imp = self.imported_root.join(session_id);
                if imp.is_dir() {
                    return Ok(imp);
                }
                return Err(common::KlyntbotError::StorageNotFound(format!(
                    "session {session_id}"
                )));
            }
        };
        while let Some(h) = entries
            .next_entry()
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("dir iter: {e}")))?
        {
            if !h.file_type().await.is_ok_and(|t| t.is_dir()) {
                continue;
            }
            let candidate = h.path().join(session_id);
            if candidate.is_dir() {
                return Ok(candidate);
            }
        }
        let imp = self.imported_root.join(session_id);
        if imp.is_dir() {
            return Ok(imp);
        }
        Err(common::KlyntbotError::StorageNotFound(format!(
            "session {session_id}"
        )))
    }
}

#[async_trait]
impl TracingProvider for KimiTracingProvider {
    fn id(&self) -> &'static str {
        "kimi"
    }
    fn display_name(&self) -> &'static str {
        "Kimi"
    }

    async fn list_sessions(&self) -> Result<Vec<SessionSummary>> {
        self.refresh_workdirs().await;
        let discovered = discover_sessions(&self.kimi_root, &self.imported_root).await?;
        let entries = self.workdirs.entries().await;
        let hash_to_cwd: std::collections::HashMap<String, PathBuf> = entries.into_iter().collect();

        let mut out = Vec::with_capacity(discovered.len());
        for d in discovered {
            let wire = d.source_dir.join("wire.jsonl");
            let mtime = match tokio::fs::metadata(&wire).await {
                Ok(m) => m.modified().unwrap_or(std::time::SystemTime::UNIX_EPOCH),
                Err(_) => std::time::SystemTime::UNIX_EPOCH,
            };
            if let Some(cached) = self.cache.get(&wire, mtime).await {
                out.push(cached);
                continue;
            }
            let loaded = load_session_events(&wire, "kimi", None, 0).await?;
            let subs = list_subagents(&d.source_dir).await.unwrap_or_default();
            let context_path = d.source_dir.join("context.jsonl");
            let has_context = tokio::fs::try_exists(&context_path).await.unwrap_or(false);
            let state_path = d.source_dir.join("state.json");
            let state = load_state(&state_path).await.unwrap_or_default();
            let custom_title = state.custom_title.clone().or_else(|| {
                loaded
                    .events
                    .iter()
                    .find(|e| e.category == SemanticCategory::TurnBegin)
                    .and_then(|e| extract_user_text(&e.payload))
            });
            let cwd = d.hash.as_ref().and_then(|h| hash_to_cwd.get(h).cloned());
            let project_basename = cwd
                .as_ref()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string());
            let context_size = tokio::fs::metadata(&context_path)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
            let size_bytes = tokio::fs::metadata(&wire)
                .await
                .map(|m| m.len())
                .unwrap_or(0)
                + context_size;

            let summary = SessionSummary {
                session_id: d.session_id.clone(),
                provider_id: "kimi".into(),
                source_dir: d.source_dir.clone(),
                cwd,
                project_basename,
                custom_title,
                started_at: loaded
                    .events
                    .first()
                    .map(|e| e.occurred_at)
                    .unwrap_or(Timestamp::UNIX_EPOCH),
                last_event_at: loaded
                    .events
                    .last()
                    .map(|e| e.occurred_at)
                    .unwrap_or(Timestamp::UNIX_EPOCH),
                size_bytes,
                turn_count: loaded.stats.turn_count,
                step_count: loaded.stats.step_count,
                tool_call_count: loaded.stats.tool_call_count,
                error_count: loaded.stats.error_count,
                subagent_count: subs.len() as u32,
                has_wire: true,
                has_context,
                imported: d.imported,

                work_dir_hash: d.hash.clone().unwrap_or_default(),
                has_state: tokio::fs::try_exists(&state_path).await.unwrap_or(false),
                wire_size: tokio::fs::metadata(&wire)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0),
                context_size,
                state_size: tokio::fs::metadata(&state_path)
                    .await
                    .map(|m| m.len())
                    .unwrap_or(0),
                total_size: size_bytes,
                metadata: None,
            };
            self.cache.put(wire, mtime, summary.clone()).await;
            out.push(summary);
        }
        out.sort_by_key(|b| std::cmp::Reverse(b.last_event_at));
        Ok(out)
    }

    async fn load_session(&self, session_id: &str, scope: Scope) -> Result<SessionDetail> {
        let dir = self.resolve_session_dir(session_id).await?;
        let (wire_path, parent) = match &scope {
            Scope::Main => (dir.join("wire.jsonl"), None),
            Scope::Subagent { agent_id } => (
                dir.join("subagents").join(agent_id).join("wire.jsonl"),
                Some(agent_id.clone()),
            ),
        };
        let agent_count = list_subagents(&dir).await.unwrap_or_default().len() as u32;
        let loaded = load_session_events(&wire_path, "kimi", parent, agent_count).await?;
        Ok(SessionDetail {
            session_id: session_id.to_string(),
            provider_id: "kimi".into(),
            scope,
            stats: loaded.stats,
            events: loaded.events,
            truncated: loaded.truncated,
            total_event_count: loaded.total_event_count,
        })
    }

    async fn load_context(&self, session_id: &str, scope: Scope) -> Result<Vec<ContextMessage>> {
        let dir = self.resolve_session_dir(session_id).await?;
        let path = match scope {
            Scope::Main => dir.join("context.jsonl"),
            Scope::Subagent { agent_id } => {
                dir.join("subagents").join(&agent_id).join("context.jsonl")
            }
        };
        load_context(&path).await
    }

    async fn load_state(&self, session_id: &str) -> Result<SessionState> {
        let dir = self.resolve_session_dir(session_id).await?;
        load_state(&dir.join("state.json")).await
    }

    async fn list_subagents(&self, session_id: &str) -> Result<Vec<SubagentSummary>> {
        let dir = self.resolve_session_dir(session_id).await?;
        list_subagents(&dir).await
    }

    async fn import_from_file(&self, path: &Path) -> Result<String> {
        import_from_file(&self.imported_root, path).await
    }

    async fn open_dir(&self, session_id: &str) -> Result<PathBuf> {
        self.resolve_session_dir(session_id).await
    }

    async fn stats(&self) -> Result<StatsBundle> {
        let sessions = self.list_sessions().await?;
        aggregate(&sessions).await
    }

    async fn session_summary(&self, session_id: &str) -> Result<SessionSummary> {
        let dir = self.resolve_session_dir(session_id).await?;
        summary::compute(&dir).await
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

fn extract_user_text(payload: &serde_json::Value) -> Option<String> {
    if let Some(s) = payload.get("user_input").and_then(|v| v.as_str()) {
        return Some(s.to_string());
    }
    payload
        .get("user_input")
        .and_then(|v| v.as_array())
        .and_then(|arr| {
            arr.iter().find_map(|p| {
                if p.get("type")?.as_str()? == "text" {
                    Some(p.get("text")?.as_str()?.to_string())
                } else {
                    None
                }
            })
        })
}
