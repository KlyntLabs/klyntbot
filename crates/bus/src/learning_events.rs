//! Learning event types and broadcast bus for the Learning→Context reactive system.
//!
//! `LearningEventBus` is a thin wrapper around `tokio::sync::broadcast` so that
//! multiple subscribers (AgentLoop, future dashboards) each receive every event
//! independently.

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

/// Events emitted by `LearningService` after analysis runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LearningEvent {
    /// The adaptive confidence threshold changed.
    ThresholdChanged {
        old_threshold: f32,
        new_threshold: f32,
        /// Human-readable reason (e.g., "success_rate_increased").
        reason: String,
    },
    /// A full analysis cycle completed (threshold may or may not have changed).
    AnalysisCompleted {
        total_outcomes: usize,
        suggested_threshold: f32,
    },
}

/// Broadcast bus for `LearningEvent`s.
///
/// Clone or pass an `Arc<LearningEventBus>` to producers and consumers.
/// Call `subscribe()` once per consumer to get an independent receiver.
pub struct LearningEventBus {
    tx: broadcast::Sender<LearningEvent>,
}

impl LearningEventBus {
    /// Create a new bus with the specified channel capacity.
    ///
    /// A capacity of 16 is sufficient for normal operation; increase if many
    /// events are published faster than consumers process them.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx }
    }

    /// Publish an event to all current subscribers.
    ///
    /// No-op (does not error) when there are no active subscribers.
    pub async fn publish(&self, event: LearningEvent) {
        let _ = self.tx.send(event);
    }

    /// Subscribe to the event stream.
    ///
    /// Each subscriber receives its own independent copy of every event published
    /// after the call to `subscribe()`.
    pub fn subscribe(&self) -> broadcast::Receiver<LearningEvent> {
        self.tx.subscribe()
    }
}
