//! Channel trait + concrete adapters for notification fan-out.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::Result;

pub mod os_native;
pub mod outbound;
pub mod tray;

#[derive(Debug, Clone)]
pub struct NotificationPayload {
    pub alarm_id: String,
    pub title: String,
    pub body: String,
    pub priority: Priority,
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
        })
        .await
        .unwrap();
        assert_eq!(m.count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
