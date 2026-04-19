//! Outbound adapter — sends through the existing `MessageBus::publish_outbound`
//! to reach Telegram / Discord / Slack / Email. One adapter instance per
//! channel name; the adapter consults a shared `last_active` slot and sends
//! only when the active channel matches its own name.
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;

use bus::{MessageBus, OutboundMessage};
use common::{ChannelName, ChatId};

use super::{Channel, NotificationPayload};
use crate::error::{NotificationError, Result};

pub struct OutboundChannel {
    channel_name: String,
    bus: Arc<MessageBus>,
    last_active: Arc<RwLock<Option<(ChannelName, ChatId)>>>,
}

impl OutboundChannel {
    pub fn new(
        channel_name: impl Into<String>,
        bus: Arc<MessageBus>,
        last_active: Arc<RwLock<Option<(ChannelName, ChatId)>>>,
    ) -> Self {
        Self {
            channel_name: channel_name.into(),
            bus,
            last_active,
        }
    }
}

#[async_trait]
impl Channel for OutboundChannel {
    fn name(&self) -> &str {
        &self.channel_name
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        let (ch, chat_id) = {
            let guard = self.last_active.read().await;
            match &*guard {
                Some((c, id)) if c.as_str() == self.channel_name => (c.clone(), id.clone()),
                _ => return Ok(()), // no active chat on this channel → drop silently
            }
        };
        let msg = OutboundMessage::new(
            ch,
            chat_id,
            format!("{}\n\n{}", payload.title, payload.body),
        );
        self.bus
            .publish_outbound(msg)
            .await
            .map_err(|e| NotificationError::Delivery {
                channel: self.channel_name.clone(),
                reason: e.to_string(),
            })
    }
}
