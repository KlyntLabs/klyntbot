//! Channel trait + concrete adapters for notification fan-out.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::Result;

pub const CHANNEL_OS_NATIVE: u32 = 1 << 0;
pub const CHANNEL_TRAY: u32 = 1 << 1;
pub const CHANNEL_TELEGRAM: u32 = 1 << 2;
pub const CHANNEL_DISCORD: u32 = 1 << 3;
pub const CHANNEL_EMAIL: u32 = 1 << 4;

/// Convert a channel_mask bitfield into the corresponding channel-name strings.
/// Returns an empty Vec for mask == 0. Order is OS_NATIVE, TRAY, TELEGRAM, DISCORD, EMAIL.
pub fn mask_to_names(mask: u32) -> Vec<String> {
    let mut v = Vec::new();
    if mask & CHANNEL_OS_NATIVE != 0 {
        v.push("os_native".into());
    }
    if mask & CHANNEL_TRAY != 0 {
        v.push("tray".into());
    }
    if mask & CHANNEL_TELEGRAM != 0 {
        v.push("telegram".into());
    }
    if mask & CHANNEL_DISCORD != 0 {
        v.push("discord".into());
    }
    if mask & CHANNEL_EMAIL != 0 {
        v.push("email".into());
    }
    v
}

pub mod discord;
pub mod email;
pub mod os_native;
pub mod outbound;
pub mod telegram;
pub mod tray;

pub use discord::DiscordNotificationChannel;
#[cfg(feature = "email")]
pub use email::EmailNotificationChannel;
pub use telegram::TelegramNotificationChannel;

// TODO(4.8 / follow-up): Wire TelegramNotificationChannel / DiscordNotificationChannel /
// EmailNotificationChannel into NotificationDispatcher's channel registry at app-core
// init time, constructing each from the corresponding config section
// (notifications.telegram, .discord, .email) when present.

#[derive(Debug, Clone)]
pub struct NotificationPayload {
    pub alarm_id: String,
    pub title: String,
    pub body: String,
    pub priority: Priority,
    pub channel_mask: u32,                 // 0 = inherit defaults
    pub priority_override: Option<String>, // "urgent" | None
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority {
    Normal,
    Urgent,
}

#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn deliver(&self, payload: &NotificationPayload) -> Result<()>;
}

#[derive(Clone, Default)]
pub struct ChannelRegistry {
    channels: HashMap<String, Arc<dyn Channel>>,
}

impl ChannelRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, ch: Arc<dyn Channel>) {
        self.channels.insert(ch.name().to_string(), ch);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Channel>> {
        self.channels.get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> {
        self.channels.keys().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockChannel {
        name: String,
        count: std::sync::atomic::AtomicUsize,
    }

    #[async_trait]
    impl Channel for MockChannel {
        fn name(&self) -> &str {
            &self.name
        }
        async fn deliver(&self, _p: &NotificationPayload) -> Result<()> {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn registry_dispatches_by_name() {
        let mut reg = ChannelRegistry::new();
        let m = Arc::new(MockChannel {
            name: "mock".into(),
            count: std::sync::atomic::AtomicUsize::new(0),
        });
        reg.register(m.clone());
        let ch = reg.get("mock").unwrap();
        ch.deliver(&NotificationPayload {
            alarm_id: "x".into(),
            title: "t".into(),
            body: "b".into(),
            priority: Priority::Normal,
            channel_mask: 0,
            priority_override: None,
        })
        .await
        .unwrap();
        assert_eq!(m.count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
