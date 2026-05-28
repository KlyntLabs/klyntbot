//! Tray notification channel adapter.
//!
//! Publishes a [`DomainEvent::TrayNotificationRequested`] onto the
//! [`DomainEventBus`].  The desktop crate listens for this event and
//! updates the system-tray menu accordingly.

use std::sync::Arc;

use async_trait::async_trait;
use bus::{DomainEventBus, NotificationEvent};

use crate::error::Result;

use super::{Channel, NotificationPayload};

pub struct TrayChannel {
    bus: Arc<DomainEventBus>,
}

impl TrayChannel {
    pub fn new(bus: Arc<DomainEventBus>) -> Self {
        Self { bus }
    }
}

#[async_trait]
impl Channel for TrayChannel {
    fn name(&self) -> &str {
        "tray"
    }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        self.bus
            .publish_notification(NotificationEvent::TrayNotificationRequested {
                title: payload.title.clone(),
                body: payload.body.clone(),
                alarm_id: Some(payload.alarm_id.clone()),
            });
        Ok(())
    }
}
