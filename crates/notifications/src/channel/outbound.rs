//! Outbound channel adapter.
//!
//! Routes a notification as a chat message through the [`MessageBus`]
//! to a specific channel + chat ID.  Useful for delivering alarms to
//! Telegram, Discord, etc.

use std::sync::Arc;

use async_trait::async_trait;
use bus::{MessageBus, OutboundMessage};
use common::{ChannelName, ChatId};

use crate::error::{NotificationError, Result};

use super::{Channel, NotificationPayload};

pub struct OutboundChannel {
    bus: Arc<MessageBus>,
    channel_name: ChannelName,
    chat_id: ChatId,
}

impl OutboundChannel {
    pub fn new(
        bus: Arc<MessageBus>,
        channel_name: impl Into<ChannelName>,
        chat_id: impl Into<ChatId>,
    ) -> Self {
        Self {
            bus,
            channel_name: channel_name.into(),
            chat_id: chat_id.into(),
        }
    }
}

#[async_trait]
impl Channel for OutboundChannel {
    fn name(&self) -> &str {
        self.channel_name.as_str()
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        let text = format!("*{}*\n{}", payload.title, payload.body);
        let msg = OutboundMessage::new(self.channel_name.clone(), self.chat_id.clone(), text);

        self.bus
            .publish_outbound(msg)
            .await
            .map_err(|e| NotificationError::Delivery {
                channel: self.channel_name.as_str().to_string(),
                reason: e.to_string(),
            })
    }
}
