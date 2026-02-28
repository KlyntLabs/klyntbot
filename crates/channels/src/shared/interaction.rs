//! Shared interaction tracking for channels with native UI (buttons, menus).
//!
//! Eliminates the duplicated `PendingCallback` enum and wait logic
//! that was copy-pasted across telegram.rs, discord.rs, and slack.rs.

use std::sync::Arc;
use std::time::Duration;

use dashmap::DashMap;
use tokio::sync::oneshot;

use common::{ChannelError, Result};

/// State for a pending interaction callback.
pub enum PendingCallback {
    /// Waiting for a single button press (single_select, yes_no).
    Single(oneshot::Sender<String>),
    /// Waiting for free text reply from user.
    FreeText(oneshot::Sender<String>),
}

/// Manages pending interaction callbacks keyed by `"{chat_id}:{question_id}"`.
///
/// Thread-safe (DashMap) — safe to use from both the inbound event handler
/// and the outbound `send_interaction` call concurrently.
#[derive(Clone)]
pub struct InteractionTracker {
    pending: Arc<DashMap<String, PendingCallback>>,
}

impl InteractionTracker {
    pub fn new() -> Self {
        Self {
            pending: Arc::new(DashMap::new()),
        }
    }

    /// Register a callback and wait for the user to press a button.
    /// Times out after 5 minutes.
    pub async fn wait_for_callback(&self, chat_id: &str, question_id: &str) -> Result<String> {
        self.wait_for(chat_id, question_id, PendingCallback::Single)
            .await
    }

    /// Register a free-text callback and wait for the user's text reply.
    /// Times out after 5 minutes.
    pub async fn wait_for_free_text(&self, chat_id: &str, question_id: &str) -> Result<String> {
        self.wait_for(chat_id, question_id, PendingCallback::FreeText)
            .await
    }

    /// Shared implementation for wait_for_callback and wait_for_free_text.
    async fn wait_for(
        &self,
        chat_id: &str,
        question_id: &str,
        variant: fn(oneshot::Sender<String>) -> PendingCallback,
    ) -> Result<String> {
        let (tx, rx) = oneshot::channel();
        let key = format!("{}:{}", chat_id, question_id);
        self.pending.insert(key.clone(), variant(tx));

        match tokio::time::timeout(Duration::from_secs(300), rx).await {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => {
                self.pending.remove(&key);
                Err(ChannelError::SendFailed("Interaction cancelled".into()).into())
            }
            Err(_) => {
                self.pending.remove(&key);
                Err(ChannelError::SendFailed("Interaction timed out (5 min)".into()).into())
            }
        }
    }

    /// Try to resolve a pending single-callback by key.
    /// Returns `true` if the callback was found and resolved.
    pub fn resolve_single(&self, key: &str, value: String) -> bool {
        if let Some((_, PendingCallback::Single(tx))) = self.pending.remove(key) {
            let _ = tx.send(value);
            return true;
        }
        false
    }

    /// Try to resolve a pending free-text callback by key.
    /// Returns `true` if the callback was found and resolved.
    pub fn resolve_free_text(&self, key: &str, value: String) -> bool {
        if let Some((_, PendingCallback::FreeText(tx))) = self.pending.remove(key) {
            let _ = tx.send(value);
            return true;
        }
        false
    }

    /// Check if there's a pending free-text callback for any question in this chat.
    /// Returns the key if found. Used by inbound message handlers to intercept
    /// ordinary messages as interaction responses.
    pub fn find_free_text_key(&self, chat_id: &str) -> Option<String> {
        let prefix = format!("{}:", chat_id);
        for entry in self.pending.iter() {
            if entry.key().starts_with(&prefix)
                && matches!(entry.value(), PendingCallback::FreeText(_))
            {
                return Some(entry.key().clone());
            }
        }
        None
    }
}

impl Default for InteractionTracker {
    fn default() -> Self {
        Self::new()
    }
}
