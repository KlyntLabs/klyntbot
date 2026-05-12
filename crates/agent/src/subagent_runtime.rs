//! Persistent subagent runtime — backs the `subagents` multi-action tool.
//!
//! State lives in `subagent_instances` + a session in `sessions`. The same
//! `execute_loop` and `SafetyCap` machinery runs both spawn and resume
//! calls; the only difference is whether messages are bootstrapped fresh
//! (spawn) or loaded from the session (resume).

use std::path::PathBuf;
use std::sync::Arc;

use dashmap::DashMap;
use tokio_util::sync::CancellationToken;

use providers::DynProvider;
use storage::repos::{NewSubagentInstance, SubagentInstanceRepo, SessionRepo};
use storage::rows::{SubagentInstanceRow, SubagentStatus};
use common::Result;

/// Default turn cap for spawn/resume calls. Matches Kimi's max_steps_per_turn.
pub const DEFAULT_TURN_CAP: u32 = 500;

/// Live cancel tokens for currently-running subagent instances.
/// Keyed by `agent_id`. Entries are inserted at start of spawn/resume and
/// removed when the call returns (success, error, cap-hit, or cancellation).
#[derive(Debug, Default, Clone)]
pub struct ActiveSubagentRegistry {
    tokens: Arc<DashMap<String, CancellationToken>>,
}

impl ActiveSubagentRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new run. Caller must hold the token alive for the run.
    pub fn register(&self, agent_id: &str, token: CancellationToken) {
        self.tokens.insert(agent_id.to_string(), token);
    }

    /// Trigger cancel for an active run. Returns true if the agent was active.
    pub fn cancel(&self, agent_id: &str) -> bool {
        if let Some((_, token)) = self.tokens.remove(agent_id) {
            token.cancel();
            true
        } else {
            false
        }
    }

    /// Remove (without cancelling) — used at clean run completion.
    pub fn unregister(&self, agent_id: &str) {
        self.tokens.remove(agent_id);
    }

    pub fn is_active(&self, agent_id: &str) -> bool {
        self.tokens.contains_key(agent_id)
    }
}

/// Structured errors returned by the subagent runtime to its tool layer.
/// The tool layer converts these into `is_error: true` payloads.
#[derive(Debug, thiserror::Error, serde::Serialize)]
pub enum SubagentError {
    #[error("subagent {agent_id} hit turn cap at {turns_used}: {partial_summary}")]
    CapHit {
        agent_id: String,
        session_id: String,
        turns_used: u32,
        partial_summary: String,
    },
    #[error("subagent {0} is currently running; cannot resume concurrently")]
    AlreadyRunning(String),
    #[error("subagent {agent_id} is not resumable (status={status})")]
    NotResumable {
        agent_id: String,
        status: &'static str,
    },
    #[error("subagent {0} not found")]
    Unknown(String),
    #[error("subagent runtime error: {0}")]
    Internal(String),
}

impl SubagentError {
    pub fn not_resumable(agent_id: impl Into<String>, status: SubagentStatus) -> Self {
        Self::NotResumable {
            agent_id: agent_id.into(),
            status: status.as_str(),
        }
    }
}

/// Parameters for spawning a new instance.
#[derive(Debug, Clone)]
pub struct SpawnParams {
    pub description: String,
    pub prompt: String,
    pub model: Option<String>,
    pub max_turns: Option<u32>,
    pub workspace_path: PathBuf,
    pub parent_session_id: String,
    pub parent_agent_id: Option<String>,
}

/// Parameters for resuming an existing instance.
#[derive(Debug, Clone)]
pub struct ResumeParams {
    pub agent_id: String,
    pub prompt: String,
}

/// Result of a single spawn / resume call (before tool-layer conversion).
#[derive(Debug, Clone)]
pub struct SubagentRunResult {
    pub agent_id: String,
    pub session_id: String,
    pub status: SubagentStatus,
    pub summary: String,
    pub turns_used: u32,
}

/// Wires the storage repos + provider + active-registry for the subagent runtime.
/// Constructed once by `app-core` and held inside `SubagentManager`.
#[derive(Clone)]
pub struct SubagentRuntime {
    pub repo: SubagentInstanceRepo,
    pub sessions: SessionRepo,
    pub active: ActiveSubagentRegistry,
    pub provider: DynProvider,
    pub workspace: PathBuf,
    pub model: String,
    pub tool_kit: Option<Arc<klynt_core::ToolKitBuilder>>,
    pub hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    pub coding_policies: Option<
        Arc<DashMap<String, Arc<parking_lot::RwLock<approval::CodingApprovalPolicy>>>>,
    >,
    pub job_supervisor: Option<tools_core::DynJobSupervisor>,
    /// Optional lifecycle event broadcaster (injected after construction).
    pub event_tx: Arc<std::sync::Mutex<Option<tokio::sync::broadcast::Sender<crate::subagent_events::SubagentLifecycleEvent>>>>,
}

impl SubagentRuntime {
    /// Generate a stable, short agent_id: `ag` + 8 hex chars.
    fn new_agent_id() -> String {
        let raw = uuid::Uuid::new_v4().simple().to_string();
        format!("ag{}", &raw[..8])
    }

    /// Generate a new subagent session key.
    fn new_session_id() -> String {
        format!("sub-{}", uuid::Uuid::new_v4().simple())
    }

    pub fn set_event_sender(
        &self,
        tx: tokio::sync::broadcast::Sender<crate::subagent_events::SubagentLifecycleEvent>,
    ) {
        let mut lock = self.event_tx.lock().unwrap();
        *lock = Some(tx);
    }

    fn emit(&self, event: crate::subagent_events::SubagentLifecycleEvent) {
        let lock = self.event_tx.lock().unwrap();
        if let Some(ref tx) = *lock {
            let _ = tx.send(event);
        }
    }

    fn make_heartbeat_callback(&self, agent_id: &str) -> std::sync::Arc<dyn Fn() + Send + Sync> {
        let repo = self.repo.clone();
        let aid = agent_id.to_string();
        let iter_counter = std::sync::Arc::new(std::sync::atomic::AtomicU32::new(0));
        let iter_counter_cb = iter_counter.clone();
        let rt = self.clone();
        std::sync::Arc::new(move || {
            let r = repo.clone();
            let a = aid.clone();
            let count = iter_counter_cb.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
            tokio::spawn(async move {
                let _ = r.tick_turn(&a).await;
            });
            rt.emit(crate::subagent_events::SubagentLifecycleEvent::Progress {
                agent_id: aid.clone(),
                iteration: count,
                last_tool: None,
            });
        })
    }

    fn emit_result(&self, agent_id: &str, result: &Result<SubagentRunResult>, duration_ms: u64) {
        match result {
            Ok(res) => {
                self.emit(crate::subagent_events::SubagentLifecycleEvent::Completed {
                    agent_id: agent_id.to_string(),
                    success: res.status == SubagentStatus::Idle,
                    summary: res.summary.clone(),
                    tokens_used: 0,
                    duration_ms,
                });
            }
            Err(e) => {
                self.emit(crate::subagent_events::SubagentLifecycleEvent::Cancelled {
                    agent_id: agent_id.to_string(),
                    reason: e.to_string(),
                    cancelled_at: jiff::Timestamp::now().as_millisecond(),
                });
            }
        }
    }

    /// Spawn a new subagent instance, persist it, run the loop, and return the result.
    pub async fn spawn(&self, p: SpawnParams) -> Result<SubagentRunResult> {
        let agent_id = Self::new_agent_id();
        let session_id = Self::new_session_id();
        let max_turns = p.max_turns.unwrap_or(DEFAULT_TURN_CAP);
        let model = p.model.unwrap_or_else(|| self.model.clone());
        let started_at = jiff::Timestamp::now().as_millisecond();

        // 1. Insert a subagent session row.
        self.sessions.insert_subagent_session(
            &session_id,
            &p.parent_session_id,
            p.workspace_path.to_string_lossy().as_ref(),
        ).await?;

        // 2. Insert the metadata row (status=running).
        self.repo.insert(&NewSubagentInstance {
            agent_id: agent_id.clone(),
            session_id: session_id.clone(),
            parent_agent_id: p.parent_agent_id.clone(),
            description: p.description.clone(),
            model: Some(model.clone()),
            workspace_path: p.workspace_path.to_string_lossy().to_string(),
            turn_cap: max_turns as i64,
        }).await?;

        // 3. Register a cancel token in the active map.
        let token = CancellationToken::new();
        self.active.register(&agent_id, token.clone());

        self.emit(crate::subagent_events::SubagentLifecycleEvent::Spawned {
            agent_id: agent_id.clone(),
            label: p.description.clone(),
            profile: "full".to_string(),
            parent_session_id: p.parent_session_id.clone(),
            spawned_at: started_at,
        });

        // 4. Compose the initial messages list (system + user prompt).
        let system = crate::subagent::build_subagent_prompt(&p.workspace_path, &p.prompt);
        let messages = vec![
            providers::types::Message::system(system),
            providers::types::Message::user(p.prompt.clone()),
        ];

        // 5. Build heartbeat callback that also emits Progress events.
        let on_iter = self.make_heartbeat_callback(&agent_id);

        // 6. Run the loop.
        let started = std::time::Instant::now();
        let loop_res = crate::subagent::run_subagent_loop(
            self.provider.clone(),
            messages,
            p.workspace_path.clone(),
            model,
            self.tool_kit.clone(),
            self.hook_engine.clone(),
            session_id.clone(),
            agent_id.clone(),
            self.coding_policies.clone(),
            self.job_supervisor.clone(),
            token.clone(),
            max_turns,
            Some(on_iter),
        )
        .await;

        // 7. Convert the loop result into a SubagentRunResult and persist.
        let duration_ms = started.elapsed().as_millis() as u64;
        let result = self.finalize_run(&agent_id, &session_id, loop_res).await;
        self.active.unregister(&agent_id);
        self.emit_result(&agent_id, &result, duration_ms);
        result
    }

    /// Resume an existing idle/stopped_turn instance.
    pub async fn resume(&self, p: ResumeParams) -> Result<SubagentRunResult> {
        let row = self
            .repo
            .get(&p.agent_id)
            .await?
            .ok_or_else(|| common::KlyntbotError::StorageNotFound(format!("subagent {}", p.agent_id)))?;
        let status = row.status_enum();
        match status {
            SubagentStatus::Running => {
                return Err(common::KlyntbotError::Storage(format!(
                    "subagent {} is currently running; cannot resume concurrently",
                    p.agent_id
                )));
            }
            SubagentStatus::Idle | SubagentStatus::StoppedTurn => {}
            _ => {
                return Err(common::KlyntbotError::Storage(format!(
                    "subagent {} is not resumable (status={})",
                    p.agent_id,
                    status.as_str()
                )));
            }
        }

        // Reset per-call counters and flip status to running.
        self.repo.reset_turns_for_resume(&p.agent_id).await?;
        self.repo
            .update_status(&p.agent_id, SubagentStatus::Running)
            .await?;

        // Load conversation history from the session's messages and append the new prompt.
        let history: Vec<storage::rows::session::SessionMessageRow> = self.sessions.load_messages(&row.session_id).await?;
        let mut messages: Vec<providers::types::Message> = history
            .into_iter()
            .filter_map(crate::subagent::row_to_message)
            .collect();
        messages.push(providers::types::Message::user(p.prompt.clone()));

        let token = CancellationToken::new();
        self.active.register(&p.agent_id, token.clone());

        let on_iter = self.make_heartbeat_callback(&p.agent_id);

        let model = row.model.clone().unwrap_or_else(|| self.model.clone());
        let started = std::time::Instant::now();
        let loop_res = crate::subagent::run_subagent_loop(
            self.provider.clone(),
            messages,
            std::path::PathBuf::from(&row.workspace_path),
            model,
            self.tool_kit.clone(),
            self.hook_engine.clone(),
            row.session_id.clone(),
            p.agent_id.clone(),
            self.coding_policies.clone(),
            self.job_supervisor.clone(),
            token,
            row.turn_cap as u32,
            Some(on_iter),
        )
        .await;

        let duration_ms = started.elapsed().as_millis() as u64;
        let result = self
            .finalize_run(&p.agent_id, &row.session_id, loop_res)
            .await;
        self.active.unregister(&p.agent_id);
        self.emit_result(&p.agent_id, &result, duration_ms);
        result
    }

    /// Cancel an active run (if any) and flip status to `killed`.
    /// If the instance isn't running, just flips the DB row.
    pub async fn kill(&self, agent_id: &str) -> Result<SubagentRunResult> {
        let row = self
            .repo
            .get(agent_id)
            .await?
            .ok_or_else(|| {
                common::KlyntbotError::StorageNotFound(format!("subagent {agent_id}"))
            })?;
        self.active.cancel(agent_id);
        // Allow kill regardless of state, as long as it isn't already terminal.
        let status_now = row.status_enum();
        if status_now.is_terminal() {
            // Already terminal — no-op, return current state.
            return Ok(SubagentRunResult {
                agent_id: agent_id.to_string(),
                session_id: row.session_id,
                status: status_now,
                summary: row.partial_summary.unwrap_or_default(),
                turns_used: row.turns_used as u32,
            });
        }
        self.repo.update_status(agent_id, SubagentStatus::Killed).await?;
        Ok(SubagentRunResult {
            agent_id: agent_id.to_string(),
            session_id: row.session_id,
            status: SubagentStatus::Killed,
            summary: row.partial_summary.unwrap_or_default(),
            turns_used: row.turns_used as u32,
        })
    }

    pub async fn list(
        &self,
        parent_agent_id: Option<&str>,
        status: Option<SubagentStatus>,
    ) -> Result<Vec<SubagentInstanceRow>> {
        let rows = if let Some(s) = status {
            self.repo.list_by_status(s).await?
        } else {
            self.repo.list_by_parent(parent_agent_id).await?
        };
        // If both filters were provided, intersect.
        if status.is_some() && parent_agent_id.is_some() {
            let parent = parent_agent_id.unwrap();
            Ok(rows.into_iter().filter(|r| r.parent_agent_id.as_deref() == Some(parent)).collect())
        } else {
            Ok(rows)
        }
    }

    /// Persist final state and return the structured result.
    /// On cap-hit, sets partial_summary and returns SubagentError::CapHit
    /// (via Result::Err).
    async fn finalize_run(
        &self,
        agent_id: &str,
        session_id: &str,
        loop_res: Result<crate::execution::execute_loop::ExecuteLoopResult>,
    ) -> Result<SubagentRunResult> {
        let res = loop_res?;
        if res.safety_cap_hit {
            let partial = derive_partial_summary(&res.content);
            self.repo.set_partial_summary(agent_id, &partial).await?;
            self.repo
                .update_status(agent_id, SubagentStatus::StoppedTurn)
                .await?;
            let payload = serde_json::to_string(&SubagentError::CapHit {
                agent_id: agent_id.to_string(),
                session_id: session_id.to_string(),
                turns_used: res.turns,
                partial_summary: partial,
            })
            .unwrap_or_default();
            return Err(common::ToolError::ExecutionFailed(payload).into());
        }
        self.repo.update_status(agent_id, SubagentStatus::Idle).await?;
        Ok(SubagentRunResult {
            agent_id: agent_id.to_string(),
            session_id: session_id.to_string(),
            status: SubagentStatus::Idle,
            summary: res.content,
            turns_used: res.turns,
        })
    }
}

/// Pull a short partial summary from the final loop content. If the loop never
/// produced assistant text (e.g. cap hit mid tool call), return a synthetic
/// "Stopped" string. No LLM call.
pub(crate) fn derive_partial_summary(content: &str) -> String {
    let trimmed = content.trim();
    if trimmed.is_empty() {
        return "Stopped before producing a summary.".to_string();
    }
    common::helpers::truncate_chars(trimmed, 2000, "…")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_cancel_returns_true_only_when_active() {
        let r = ActiveSubagentRegistry::new();
        let token = CancellationToken::new();
        r.register("ag1", token.clone());
        assert!(r.is_active("ag1"));
        assert!(r.cancel("ag1"));
        assert!(token.is_cancelled());
        assert!(!r.is_active("ag1"));
        assert!(!r.cancel("ag1"), "second cancel is a no-op");
    }

    #[test]
    fn unregister_does_not_cancel() {
        let r = ActiveSubagentRegistry::new();
        let token = CancellationToken::new();
        r.register("ag1", token.clone());
        r.unregister("ag1");
        assert!(!token.is_cancelled());
        assert!(!r.is_active("ag1"));
    }

    #[test]
    fn error_status_strings_are_static() {
        let e = SubagentError::not_resumable("ag1", SubagentStatus::Killed);
        match e {
            SubagentError::NotResumable { status, .. } => assert_eq!(status, "killed"),
            _ => panic!("wrong variant"),
        }
    }

    #[test]
    fn generated_ids_have_expected_shape() {
        let aid = SubagentRuntime::new_agent_id();
        assert!(aid.starts_with("ag"));
        assert_eq!(aid.len(), 10);
        let sid = SubagentRuntime::new_session_id();
        assert!(sid.starts_with("sub-"));
    }

    #[test]
    fn empty_content_yields_fallback() {
        assert_eq!(
            derive_partial_summary(""),
            "Stopped before producing a summary."
        );
    }

    #[test]
    fn short_content_passes_through() {
        assert_eq!(derive_partial_summary("hi"), "hi");
    }

    #[test]
    fn long_content_is_truncated() {
        let s = "x".repeat(3000);
        let out = derive_partial_summary(&s);
        // truncate_chars limits to 2000 chars + suffix
        assert_eq!(out.chars().count(), 2001);
        assert!(out.ends_with('…'));
    }
}
