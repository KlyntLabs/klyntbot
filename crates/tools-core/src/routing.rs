//! Routing context and dependency-inversion port traits.
//!
//! [`RoutingContext`] carries channel/chat identity and optional interaction
//! channels through every tool execution. Port traits ([`ProgressHandler`],
//! [`InteractionChannel`]) are defined here at Layer 1 so lower-layer crates
//! can consume them without circular deps — implementations live in Layer 5.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};
use tokio_util::sync::CancellationToken;

use crate::events::ToolEvent;
use common::{ChannelName, ChatId, FormResponse, InteractionRequest, Result, SessionKey};

/// Bundle sent from ask_user tool to the CLI.
/// Each request carries its own response channel.
pub struct InteractionBundle {
    pub request: InteractionRequest,
    pub response_tx: oneshot::Sender<FormResponse>,
}

/// Trait for cascading progress updates (KR → Objective).
///
/// Defined in `tools-core` (Layer 1) so both `tools` (OkrTool) and `feature-tasks`
/// (TaskTool) can consume it without circular deps. Implemented in `agent` (Layer 5).
#[async_trait]
pub trait ProgressHandler: Send + Sync {
    /// Recalculate progress for a key result and cascade to its parent objective.
    async fn recalculate_kr_progress(&self, key_result_id: &str) -> Result<()>;
}

/// Trait for channels that support structured interactions (buttons, menus, etc.).
///
/// Defined in `tools-core` (Layer 1) to avoid circular deps with `channels` (Layer 5).
/// Channel implementations that support native UI elements (Telegram inline keyboards,
/// Discord buttons/selects, Slack Block Kit) implement this trait.
#[async_trait]
pub trait InteractionChannel: Send + Sync {
    /// Whether this channel supports structured interactions.
    fn supports_interaction(&self) -> bool;

    /// Send a structured interaction request and wait for the user's response.
    ///
    /// The channel renders questions using platform-native UI (buttons, menus)
    /// and returns the user's selections.
    async fn send_interaction(
        &self,
        chat_id: &str,
        request: &InteractionRequest,
    ) -> Result<FormResponse>;
}

/// Routing context for tools that need channel/chat information.
/// This is passed explicitly to execute() instead of using shared mutable state.
///
/// The optional `interaction_tx` allows the ask_user tool to send interaction
/// requests to the CLI during execution. Each request includes a oneshot channel
/// for the response, enabling blocking behavior within the tool.
#[derive(Clone)]
pub struct RoutingContext {
    pub channel: ChannelName,
    /// Authoritative session mode discriminator. Drives:
    /// - which `SoulContextSource` variant to inject
    /// - which tools the LLM sees (via `ChannelMask` interpreted in mode-aware mode)
    /// - which `ContextSource`s gate themselves on (e.g. CodingRecall)
    pub session_mode: common::SessionMode,
    pub chat_id: ChatId,
    /// Channel for ask_user tool to send interaction bundles.
    /// Only available when running in CLI chat mode (TTY).
    /// Clone-safe: mpsc::Sender is Clone.
    pub interaction_tx: Option<mpsc::Sender<InteractionBundle>>,
    /// When true, the user receives responses directly via the event stream
    /// (CLI or dashboard WebSocket), so the `message` tool should return
    /// content inline rather than publishing to the bus.
    pub is_direct_mode: bool,
    /// Current delegation depth (0 = top-level, incremented per delegation).
    pub delegation_depth: u32,
    /// Channel for tools to emit entity cards (e.g., after creating a task).
    /// The execution layer drains this and emits AgentEvent::EntityCreated.
    pub entity_tx: Option<mpsc::Sender<common::EntityCard>>,
    /// Platform-native interaction channel (Telegram buttons, Discord selects, etc.).
    /// When present, `ask_user` uses this for structured UI instead of text fallback.
    pub interaction_channel: Option<Arc<dyn InteractionChannel>>,
    /// Autotuner champion parameters for the current trial (if any).
    pub champion_params: Option<common::TrialParams>,
    /// Cancellation token for interrupting long-running tool operations.
    pub cancel_token: Option<CancellationToken>,
    /// Per-call streaming event channel. Tools push ToolEvent variants here
    /// (e.g., FileEditWithSymbols, SandboxPolicyApplied) for the relay to
    /// translate into Tauri events. May be None for non-streaming contexts.
    pub event_tx: Option<mpsc::Sender<ToolEvent>>,
    /// Hook engine for firing PreToolUse / PostToolUse / etc. at tool-execute
    /// boundaries. None = hooks disabled (e.g., in unit tests).
    pub hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
    /// Session key for conversation continuity. Used by hook firing.
    pub session_key: Option<SessionKey>,
    /// Message ID of the turn that triggered tool execution (used for rewind anchors).
    pub message_id: Option<String>,
    /// Repo identifier for repo-scoped operations (e.g. Mirror Layer 3 caching).
    pub repo_id: String,
    /// Agent identifier ("root" for top-level, subagent short-id for delegates).
    pub agent_id: String,
    /// Agent profile: "root" | "explore" | "code" | "general".
    pub agent_profile: String,
    /// When true, plan-mode validation gates apply (only `pending` status allowed).
    pub plan_mode_active: bool,
    /// Plan session id when in plan mode; None otherwise.
    pub plan_session_id: Option<String>,
    /// True if a user-facing assistant message was emitted in the current turn.
    pub same_turn_user_msg_emitted: bool,
    /// Phase 2.3a — workspace root for cwd resolution.
    pub workspace_cwd: Option<std::path::PathBuf>,
    /// Phase 2.3a — agent delegation chain (root → … → self).
    pub agent_chain: Vec<String>,
    /// Phase 2.3a — background job supervisor handle.
    pub job_supervisor: Option<crate::DynJobSupervisor>,
}

impl RoutingContext {
    pub fn with_session_mode(mut self, mode: common::SessionMode) -> Self {
        self.session_mode = mode;
        self
    }

    /// Non-interactive mode (bus-driven channels, non-TTY).
    pub fn new(channel: ChannelName, chat_id: ChatId) -> Self {
        Self {
            channel,
            session_mode: common::SessionMode::Assistant,
            chat_id,
            interaction_tx: None,
            is_direct_mode: false,
            delegation_depth: 0,
            entity_tx: None,
            interaction_channel: None,
            champion_params: None,
            cancel_token: None,
            event_tx: None,
            hook_engine: None,
            session_key: None,
            message_id: None,
            repo_id: String::new(),
            agent_id: "root".into(),
            agent_profile: "root".into(),
            plan_mode_active: false,
            plan_session_id: None,
            same_turn_user_msg_emitted: false,
            workspace_cwd: None,
            agent_chain: vec!["root".into()],
            job_supervisor: None,
        }
    }

    /// Interactive direct mode (CLI/dashboard) with user interaction support.
    pub fn with_interaction(
        channel: ChannelName,
        chat_id: ChatId,
        interaction_tx: mpsc::Sender<InteractionBundle>,
    ) -> Self {
        Self {
            channel,
            session_mode: common::SessionMode::Assistant,
            chat_id,
            interaction_tx: Some(interaction_tx),
            is_direct_mode: true,
            delegation_depth: 0,
            entity_tx: None,
            interaction_channel: None,
            champion_params: None,
            cancel_token: None,
            event_tx: None,
            hook_engine: None,
            session_key: None,
            message_id: None,
            repo_id: String::new(),
            agent_id: "root".into(),
            agent_profile: "root".into(),
            plan_mode_active: false,
            plan_session_id: None,
            same_turn_user_msg_emitted: false,
            workspace_cwd: None,
            agent_chain: vec!["root".into()],
            job_supervisor: None,
        }
    }
}

impl bus::InjectorContext for RoutingContext {
    fn thread_id(&self) -> &str {
        self.chat_id.as_str()
    }
    fn agent_id(&self) -> &str {
        &self.agent_id
    }
    fn plan_mode_active(&self) -> bool {
        self.plan_mode_active
    }
    fn plan_session_id(&self) -> Option<&str> {
        self.plan_session_id.as_deref()
    }
    fn agent_chain(&self) -> &[String] {
        &self.agent_chain
    }
}

// ============================================================================
// Tool context projection (ADR-0002)
//
// `RoutingContext` is the wide transport struct passed at the untyped
// `Tool::execute` boundary. A tool declares the narrow slice it actually needs
// via `ToolExecute::Ctx<'a>`; the `#[derive(Tool)]` bridge projects the full
// context into that view. See docs/architecture/deep-dive-02-tool-context-projection.md.
// ============================================================================

/// Builds a narrow context view from the full [`RoutingContext`].
///
/// Implemented by each rung of the view ladder (`()`, `FullCtx`, and — as tools
/// are narrowed — `HookCtx`, `IoCtx`). The projection borrows from the context;
/// it never clones or takes ownership.
pub trait FromRoutingContext<'a> {
    fn project(rc: &'a RoutingContext) -> Self;
}

/// The "needs nothing" rung. A tool with `type Ctx<'a> = ()` provably reads no
/// routing context at all.
impl<'a> FromRoutingContext<'a> for () {
    fn project(_rc: &'a RoutingContext) -> Self {}
}

/// Escape-hatch rung: the full context, for the few genuinely-wide tools
/// (e.g. `bash`) and as the transitional view during migration. `Deref` means
/// tool bodies access fields exactly as before (`ctx.hook_engine`, …).
#[derive(Clone, Copy)]
pub struct FullCtx<'a>(pub &'a RoutingContext);

impl<'a> FromRoutingContext<'a> for FullCtx<'a> {
    fn project(rc: &'a RoutingContext) -> Self {
        FullCtx(rc)
    }
}

impl std::ops::Deref for FullCtx<'_> {
    type Target = RoutingContext;
    fn deref(&self) -> &RoutingContext {
        self.0
    }
}
