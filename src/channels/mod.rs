//! Chat channel implementations for various platforms.

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::time::sleep;
use tracing::{error, info};

use crate::bus::{MessageBus, OutboundMessage};
use crate::error::Result;

pub mod discord;
pub mod email;
pub mod manager;
pub mod qq;
pub mod slack;
pub mod telegram;
pub mod whatsapp;

pub use discord::DiscordChannel;
pub use email::EmailChannel;
pub use qq::QQChannel;
pub use slack::SlackChannel;
pub use telegram::TelegramChannel;
pub use whatsapp::WhatsAppChannel;

/// Channel trait - implemented by all chat platforms
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
pub async fn reconnect_loop<F, Fut>(
    name: &str,
    running: &Arc<RwLock<bool>>,
    mut connect: F,
) where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<()>>,
{
    while *running.read().await {
        if let Err(e) = connect().await {
            error!("{} error: {}", name, e);
            if *running.read().await {
                info!("Reconnecting to {} in 5 seconds...", name);
                sleep(Duration::from_secs(5)).await;
            }
        }
    }
}
