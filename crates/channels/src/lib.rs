//! Klyntbot Channels — Chat platform integrations.
//!
//! This crate defines the [`Channel`] trait (the port) and concrete adapters
//! for various messaging platforms. The [`manager`] module orchestrates
//! channel lifecycle and outbound message dispatch.

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

use bus::{MessageBus, OutboundMessage};
use common::{FormResponse, InteractionRequest, Result};

pub mod adapters;
pub mod formatter;
pub mod manager;
pub mod shared;
pub mod utils;
pub mod ws_manager;

// ── Adapters ──────────────────────────────────────────────
#[cfg(feature = "email")]
pub use adapters::EmailChannel;
pub use adapters::{DiscordChannel, SlackChannel, TelegramChannel, TelegramApprovalChannel};

// ── Manager ───────────────────────────────────────────────
pub use manager::ChannelManager;

/// Channel trait — implemented by all chat platform adapters.
#[async_trait]
pub trait Channel: Send + Sync {
    /// Channel name (e.g., "telegram", "discord")
    fn name(&self) -> &str;

    /// Start the channel (long-running task)
    async fn start(&self, bus: Arc<MessageBus>) -> Result<()>;

    /// Stop the channel
    async fn stop(&self) -> Result<()>;

    /// Send a message through this channel
    async fn send(&self, msg: &OutboundMessage) -> Result<()>;

    /// Check if sender is allowed
    fn is_allowed(&self, sender_id: &str) -> bool;

    /// Send a typing indicator (default: no-op)
    async fn send_typing(&self, _chat_id: &str) -> Result<()> {
        Ok(())
    }

    /// Whether this channel supports structured interactions (buttons, menus).
    /// Override in channel implementations that have native UI elements.
    fn supports_interaction(&self) -> bool {
        false
    }

    /// Send a structured interaction request using platform-native UI.
    /// Only called when `supports_interaction()` returns true.
    async fn send_interaction(
        &self,
        _chat_id: &str,
        _request: &InteractionRequest,
    ) -> Result<FormResponse> {
        Err(common::ToolError::ExecutionFailed(
            "Channel does not support structured interactions".into(),
        )
        .into())
    }
}

/// Type alias for dynamic channel
pub type DynChannel = Arc<dyn Channel>;

/// Helper function for allowlist checking
pub fn check_allowlist(allow_from: &[String], sender_id: &str) -> bool {
    // If no allowlist, allow everyone
    if allow_from.is_empty() {
        return true;
    }

    // Check exact match
    if allow_from.contains(&sender_id.to_string()) {
        return true;
    }

    // Check compound IDs (e.g., "123456|username")
    if sender_id.contains('|') {
        for part in sender_id.split('|') {
            if !part.is_empty() && allow_from.contains(&part.to_string()) {
                return true;
            }
        }
    }

    false
}

/// Helper for reconnection loop pattern used by channels.
/// Automatically retries on error with a 5-second delay.
#[tracing::instrument(skip(running, connect))]
pub async fn reconnect_loop<F, Fut>(name: &str, running: &Arc<AtomicBool>, mut connect: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    while running.load(Ordering::SeqCst) {
        if let Err(e) = connect().await {
            error!("{} error: {}", name, e);
            if running.load(Ordering::SeqCst) {
                info!("Reconnecting to {} in 5 seconds...", name);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
