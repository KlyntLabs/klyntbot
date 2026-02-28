//! Shared typing indicator manager for channels.
//!
//! Manages the lifecycle of typing indicator tasks per chat/channel.
//! Each channel provides its own send function; this module handles
//! spawning, storing, and stopping the background tasks.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::sleep;
/// Manages typing indicator background tasks keyed by chat/channel ID.
///
/// Usage:
/// ```ignore
/// let typing = TypingManager::new();
/// typing.start("chat_123", Duration::from_secs(4), || async {
///     client.post(".../sendChatAction").send().await.ok();
/// }).await;
/// // ... later ...
/// typing.stop("chat_123").await;
/// ```
#[derive(Clone)]
pub struct TypingManager {
    tasks: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
}

impl TypingManager {
    pub fn new() -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Start a typing indicator for the given chat ID.
    /// Stops any existing typing task for the same chat first.
    ///
    /// `interval` is the repeat duration (e.g., 4s for Telegram, 8s for Discord).
    /// `send_fn` is called repeatedly to actually send the typing API request.
    pub async fn start<F, Fut>(&self, chat_id: &str, interval: Duration, send_fn: F)
    where
        F: Fn() -> Fut + Send + 'static,
        Fut: std::future::Future<Output = ()> + Send,
    {
        self.stop(chat_id).await;

        let task = tokio::spawn(async move {
            loop {
                send_fn().await;
                sleep(interval).await;
            }
        });

        self.tasks
            .write()
            .await
            .insert(chat_id.to_string(), task);
    }

    /// Stop the typing indicator for the given chat ID.
    pub async fn stop(&self, chat_id: &str) {
        if let Some(task) = self.tasks.write().await.remove(chat_id) {
            task.abort();
        }
    }
}

impl Default for TypingManager {
    fn default() -> Self {
        Self::new()
    }
}
