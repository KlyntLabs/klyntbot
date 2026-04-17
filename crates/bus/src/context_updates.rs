//! Shared queue for mid-execution context updates.
//!
//! Producers (cognitive background service, focus manager, etc.) push updates;
//! the LiveContextRefresher in the agent crate drains and injects them into
//! the ReactiveEngine at iteration boundaries.

use std::collections::VecDeque;
use std::sync::Mutex;

use jiff::Timestamp;
use serde::Serialize;

const DEDUP_WINDOW_SECS: i64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextUpdateReason {
    MemoryPromoted,
    FocusSessionStarted,
    FocusSessionEnded,
    DistractionDetected,
    BudgetThresholdCrossed,
    NoteStructureChanged,
    CommunityDiscovered,
    CommunityUpdated,
    CommunityWeakened,
    Custom(String),
}

impl ContextUpdateReason {
    pub fn as_str(&self) -> &str {
        match self {
            Self::MemoryPromoted => "memory_promoted",
            Self::FocusSessionStarted => "focus_session_started",
            Self::FocusSessionEnded => "focus_session_ended",
            Self::DistractionDetected => "distraction_detected",
            Self::BudgetThresholdCrossed => "budget_threshold_crossed",
            Self::NoteStructureChanged => "note_structure_changed",
            Self::CommunityDiscovered => "community_discovered",
            Self::CommunityUpdated => "community_updated",
            Self::CommunityWeakened => "community_weakened",
            Self::Custom(s) => s,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone)]
pub struct ContextUpdate {
    pub reason: ContextUpdateReason,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub priority: UpdatePriority,
    pub timestamp: Timestamp,
}

pub struct ContextUpdateQueue {
    inner: Mutex<VecDeque<ContextUpdate>>,
}

impl std::fmt::Debug for ContextUpdateQueue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let len = self.inner.lock().map(|q| q.len()).unwrap_or(0);
        f.debug_struct("ContextUpdateQueue")
            .field("pending", &len)
            .finish()
    }
}

impl ContextUpdateQueue {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(VecDeque::new()),
        }
    }

    /// Maximum pending updates before oldest are dropped.
    const MAX_PENDING: usize = 200;

    /// Push an update with 30-second deduplication by (reason, content).
    pub fn push(&self, update: ContextUpdate) {
        let mut queue = self.inner.lock().unwrap();
        let is_duplicate = queue.iter().any(|existing| {
            existing.reason == update.reason
                && existing.content == update.content
                && (update.timestamp.as_millisecond() - existing.timestamp.as_millisecond())
                    .abs()
                    < DEDUP_WINDOW_SECS * 1000
        });
        if !is_duplicate {
            // Drop oldest entries if queue is too large (prevents unbounded growth).
            // VecDeque::pop_front is O(1) vs Vec::remove(0) which is O(n).
            while queue.len() >= Self::MAX_PENDING {
                queue.pop_front();
            }
            queue.push_back(update);
        }
    }

    /// Drain all pending updates atomically.
    pub fn drain(&self) -> Vec<ContextUpdate> {
        let mut queue = self.inner.lock().unwrap();
        std::mem::take(&mut *queue).into()
    }
}

impl Default for ContextUpdateQueue {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jiff::SignedDuration;

    #[test]
    fn queue_push_and_drain() {
        let queue = ContextUpdateQueue::new();
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::MemoryPromoted,
            content: Some("User likes coffee".to_string()),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: Timestamp::now(),
        });
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::FocusSessionEnded,
            content: None,
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: Timestamp::now(),
        });
        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        assert!(queue.drain().is_empty());
    }

    #[test]
    fn queue_deduplicates_within_window() {
        let queue = ContextUpdateQueue::new();
        let now = Timestamp::now();
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::MemoryPromoted,
            content: Some("Same fact".to_string()),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: now,
        });
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::MemoryPromoted,
            content: Some("Same fact".to_string()),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: now + SignedDuration::from_secs(5),
        });
        let drained = queue.drain();
        assert_eq!(drained.len(), 1);
    }

    #[test]
    fn queue_allows_different_reasons() {
        let queue = ContextUpdateQueue::new();
        let now = Timestamp::now();
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::MemoryPromoted,
            content: Some("Fact A".to_string()),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: now,
        });
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::FocusSessionEnded,
            content: Some("Fact A".to_string()),
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: now,
        });
        assert_eq!(queue.drain().len(), 2);
    }

    #[test]
    fn queue_dedup_none_content_by_reason_only() {
        let queue = ContextUpdateQueue::new();
        let now = Timestamp::now();
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::FocusSessionStarted,
            content: None,
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: now,
        });
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::FocusSessionStarted,
            content: None,
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: now + SignedDuration::from_secs(10),
        });
        assert_eq!(queue.drain().len(), 1);
    }

    #[test]
    fn priority_ordering() {
        assert!(UpdatePriority::High > UpdatePriority::Normal);
        assert!(UpdatePriority::Normal > UpdatePriority::Low);
    }
}
