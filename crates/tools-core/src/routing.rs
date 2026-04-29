//! Routing context and dependency-inversion port traits.
//!
//! [`RoutingContext`] carries channel/chat identity and optional interaction
//! channels through every tool execution. Port traits ([`ProgressHandler`],
//! [`InteractionChannel`]) are defined here at Layer 1 so lower-layer crates
//! can consume them without circular deps — implementations live in Layer 5.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot};

use common::{ChannelName, ChatId, FormResponse, InteractionRequest, Result};

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
}

impl RoutingContext {
    /// Non-interactive mode (bus-driven channels, non-TTY).
    pub fn new(channel: ChannelName, chat_id: ChatId) -> Self {
        Self {
            channel,
            chat_id,
            interaction_tx: None,
            is_direct_mode: false,
            delegation_depth: 0,
            entity_tx: None,
            interaction_channel: None,
            champion_params: None,
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
            chat_id,
            interaction_tx: Some(interaction_tx),
            is_direct_mode: true,
            delegation_depth: 0,
            entity_tx: None,
            interaction_channel: None,
            champion_params: None,
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

}
