//! Notification dispatcher for todo reminders and updates.

use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::warn;

use bus::OutboundMessage;
use common::utils::notify::{NotificationSender, OsNotificationSender};
use common::{ChannelName, ChatId, Result};
use config::schema::TodoNotificationConfig;

/// Dispatches notifications to configured targets (OS native + chat channels)
pub struct NotificationDispatcher {
    outbound_tx: mpsc::Sender<OutboundMessage>,
    config: TodoNotificationConfig,
    last_active_channel: Arc<RwLock<Option<(ChannelName, ChatId)>>>,
    sender: Arc<dyn NotificationSender>,
}

impl NotificationDispatcher {
    /// Create a new notification dispatcher with the default OS notification sender.
    pub fn new(outbound_tx: mpsc::Sender<OutboundMessage>, config: TodoNotificationConfig) -> Self {
        Self {
            outbound_tx,
            config,
            last_active_channel: Arc::new(RwLock::new(None)),
            sender: Arc::new(OsNotificationSender),
        }
    }

    /// Create a dispatcher with a custom notification sender (e.g. Tauri native).
    pub fn with_sender(
        outbound_tx: mpsc::Sender<OutboundMessage>,
        config: TodoNotificationConfig,
        sender: Arc<dyn NotificationSender>,
    ) -> Self {
        Self {
            outbound_tx,
            config,
            last_active_channel: Arc::new(RwLock::new(None)),
            sender,
        }
    }

    /// Get a handle to the last active channel tracker
    pub fn last_active_handle(&self) -> Arc<RwLock<Option<(ChannelName, ChatId)>>> {
        Arc::clone(&self.last_active_channel)
    }

    /// Send notification to all configured targets
    pub async fn notify(&self, title: &str, body: &str) -> Result<()> {
        for target in &self.config.targets {
            match target.as_str() {
                "os_native" => {
                    // Don't fail entire notification if OS native fails
                    if let Err(e) = self.sender.send(title, body).await {
                        warn!("OS notification failed: {}", e);
                    }
                }
                channel_name => {
                    // Send to channel (use last active channel if matches)
                    if let Some((ref ch, ref chat_id)) = *self.last_active_channel.read().await {
                        if ch.as_str() == channel_name {
                            let msg = OutboundMessage::new(
                                ch.clone(),
                                chat_id.clone(),
                                format!("{}\n\n{}", title, body),
                            );
                            if let Err(e) = self.outbound_tx.send(msg).await {
                                warn!("Notification send to {} failed: {}", channel_name, e);
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
}
