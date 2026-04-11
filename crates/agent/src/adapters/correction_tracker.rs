//! Tracks user corrections per session via DomainEventBus.
//! Lightweight in-memory buffer — no DB, no persistence.

use std::collections::HashMap;
use std::sync::Arc;

use context_engine::rewriter::CorrectionContext;
use tokio::sync::RwLock;

#[derive(Clone)]
pub struct CorrectionTracker {
    corrections: Arc<RwLock<HashMap<String, CorrectionContext>>>,
}

impl CorrectionTracker {
    pub fn new() -> Self {
        Self {
            corrections: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Get the most recent correction for a session (if any).
    pub async fn latest_for_session(&self, session_key: &str) -> Option<CorrectionContext> {
        self.corrections.read().await.get(session_key).cloned()
    }

    /// Clear correction for a session.
    pub async fn clear(&self, session_key: &str) {
        self.corrections.write().await.remove(session_key);
    }

    /// Record a correction for a session (called when UserCorrectedAI event fires).
    pub async fn record(&self, session_key: String, rejected_topic: String, corrected_to: String) {
        let ctx = CorrectionContext {
            rejected_topic,
            corrected_to,
        };
        self.corrections.write().await.insert(session_key, ctx);
    }

    /// Subscribe to the DomainEventBus and automatically record UserCorrectedAI events.
    ///
    /// Returns a JoinHandle that runs until the bus shuts down or the channel is closed.
    /// The handle should be stored so the task is not dropped prematurely.
    pub fn start_listener(&self, bus: Arc<bus::DomainEventBus>) -> tokio::task::JoinHandle<()> {
        let corrections = self.corrections.clone();
        let mut rx = bus.subscribe();
        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                if let bus::DomainEvent::UserCorrectedAI {
                    original,
                    correction,
                    session_key,
                    ..
                } = event
                {
                    let ctx = CorrectionContext {
                        rejected_topic: original,
                        corrected_to: correction,
                    };
                    let mut map = corrections.write().await;
                    // Evict oldest entries if map grows too large (cap at 100 sessions)
                    if map.len() >= 100 {
                        if let Some(first_key) = map.keys().next().cloned() {
                            map.remove(&first_key);
                        }
                    }
                    map.insert(session_key, ctx);
                }
            }
        })
    }
}

impl Default for CorrectionTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn record_and_retrieve() {
        let tracker = CorrectionTracker::new();
        tracker
            .record(
                "desktop:main".into(),
                "finance".into(),
                "productivity".into(),
            )
            .await;

        let ctx = tracker.latest_for_session("desktop:main").await.unwrap();
        assert_eq!(ctx.rejected_topic, "finance");
        assert_eq!(ctx.corrected_to, "productivity");
    }

    #[tokio::test]
    async fn clear_removes_entry() {
        let tracker = CorrectionTracker::new();
        tracker
            .record("s1".into(), "old".into(), "new".into())
            .await;
        tracker.clear("s1").await;
        assert!(tracker.latest_for_session("s1").await.is_none());
    }

    #[tokio::test]
    async fn listener_records_event() {
        let bus = Arc::new(bus::DomainEventBus::new(16));
        let tracker = CorrectionTracker::new();
        let handle = tracker.start_listener(Arc::clone(&bus));

        // Give the listener task a moment to subscribe before publishing.
        tokio::task::yield_now().await;

        bus.publish(bus::DomainEvent::UserCorrectedAI {
            original: "memory".into(),
            correction: "tasks".into(),
            kind: bus::CorrectionKind::Reaction,
            strength: 1.0,
            session_key: "tg:alice".into(),
            active_skill: None,
        });

        // Allow the spawned task to process the event.
        tokio::task::yield_now().await;
        tokio::task::yield_now().await;

        let ctx = tracker.latest_for_session("tg:alice").await.unwrap();
        assert_eq!(ctx.rejected_topic, "memory");
        assert_eq!(ctx.corrected_to, "tasks");

        handle.abort();
    }
}
