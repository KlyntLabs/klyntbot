use dashmap::DashMap;

/// Per-turn steer queue — allows mid-turn user corrections to be injected
/// as synthetic user messages into the agent's context.
///
/// Each active turn registers a channel; the frontend can push text via
/// `coding_turn_steer` which ends up in the turn's receiver.
pub struct SteerQueue {
    txs: DashMap<String, tokio::sync::mpsc::UnboundedSender<String>>,
}

impl SteerQueue {
    pub fn new() -> Self {
        Self {
            txs: DashMap::new(),
        }
    }

    /// Register a new turn and return its receiver.
    pub fn register_turn(&self, turn_id: &str) -> tokio::sync::mpsc::UnboundedReceiver<String> {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        self.txs.insert(turn_id.to_string(), tx);
        rx
    }

    /// Push a steer message into an active turn.
    pub fn push(&self, turn_id: &str, text: String) -> common::Result<()> {
        if let Some(tx) = self.txs.get(turn_id) {
            tx.send(text)
                .map_err(|_| common::KlyntbotError::StorageNotFound(format!("turn {turn_id}")))?;
            Ok(())
        } else {
            Err(common::KlyntbotError::StorageNotFound(format!(
                "turn {turn_id}"
            )))
        }
    }

    /// Remove a turn's steer channel (called on turn completion).
    pub fn unregister_turn(&self, turn_id: &str) {
        self.txs.remove(turn_id);
    }
}

impl Default for SteerQueue {
    fn default() -> Self {
        Self::new()
    }
}
