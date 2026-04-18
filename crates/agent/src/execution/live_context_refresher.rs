//! Live context refresher — injects mid-execution context updates from
//! cognitive, productivity, and coaching systems into the ReactiveEngine.

use std::sync::Arc;

use bus::{ContextUpdateQueue, UpdatePriority};
use context_engine::TokenCounter;
use providers::Message;
use serde::Serialize;
use tracing::info;

/// Standard updates reserve 20% of remaining context for the LLM response.
const STANDARD_RESPONSE_RESERVE_PCT: usize = 20;

/// High-priority updates reserve only 10%, allowing more aggressive injection.
const HIGH_PRIORITY_RESPONSE_RESERVE_PCT: usize = 10;

#[derive(Debug, Clone, Serialize)]
pub struct ContextReassembledUpdate {
    pub reason: String,
    pub summary: String,
    pub tokens: usize,
}

pub struct LiveContextRefresher {
    token_counter: Arc<dyn TokenCounter>,
    queue: Arc<ContextUpdateQueue>,
}

impl LiveContextRefresher {
    pub fn new(token_counter: Arc<dyn TokenCounter>, queue: Arc<ContextUpdateQueue>) -> Self {
        Self {
            token_counter,
            queue,
        }
    }

    /// Drain pending updates and inject them into the message vec.
    /// Returns a list of injected updates for event emission.
    pub fn inject_pending(
        &self,
        messages: &mut Vec<Message>,
        context_window: usize,
    ) -> Vec<ContextReassembledUpdate> {
        let mut updates = self.queue.drain();
        if updates.is_empty() {
            return Vec::new();
        }

        // Sort by priority (High first)
        updates.sort_by(|a, b| b.priority.cmp(&a.priority));

        let current_tokens: usize = messages
            .iter()
            .map(|m| context_engine::estimate_message_tokens(&*self.token_counter, m))
            .sum();

        let remaining = context_window.saturating_sub(current_tokens);
        let standard_budget = remaining * (100 - STANDARD_RESPONSE_RESERVE_PCT) / 100;
        let high_budget = remaining * (100 - HIGH_PRIORITY_RESPONSE_RESERVE_PCT) / 100;

        let mut used_tokens = 0;
        let mut injected = Vec::new();

        for update in &updates {
            let reason_str = update.reason.as_str();
            let content = update.content.as_deref().unwrap_or(reason_str);

            let msg = Message::context_update(reason_str, content);
            let tokens = context_engine::estimate_message_tokens(&*self.token_counter, &msg);

            let budget = if update.priority == UpdatePriority::High {
                high_budget
            } else {
                standard_budget
            };

            if used_tokens + tokens > budget {
                tracing::warn!(
                    reason = ?update.reason,
                    tokens,
                    remaining_budget = budget.saturating_sub(used_tokens),
                    "context update dropped — insufficient token budget"
                );
                continue;
            }

            used_tokens += tokens;
            injected.push(ContextReassembledUpdate {
                reason: reason_str.to_string(),
                summary: content.to_string(),
                tokens,
            });
            messages.push(msg);
        }

        if !injected.is_empty() {
            info!(
                count = injected.len(),
                tokens = used_tokens,
                "live context updates injected"
            );
        }

        injected
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_refresher() -> (LiveContextRefresher, Arc<bus::ContextUpdateQueue>) {
        let queue = Arc::new(bus::ContextUpdateQueue::new());
        let refresher = LiveContextRefresher::new(
            Arc::new(context_engine::CharTokenCounter),
            Arc::clone(&queue),
        );
        (refresher, queue)
    }

    fn push_memory_update(queue: &bus::ContextUpdateQueue, content: &str) {
        queue.push(bus::ContextUpdate {
            reason: bus::ContextUpdateReason::MemoryPromoted,
            content: Some(content.to_string()),
            metadata: None,
            priority: bus::UpdatePriority::Normal,
            timestamp: jiff::Timestamp::now(),
        });
    }

    #[test]
    fn empty_queue_is_noop() {
        let (refresher, _queue) = make_refresher();
        let mut messages = vec![
            providers::Message::system("System prompt"),
            providers::Message::user("Hello"),
        ];
        let result = refresher.inject_pending(&mut messages, 128_000);
        assert!(result.is_empty());
        assert_eq!(messages.len(), 2);
    }

    #[test]
    fn injects_pending_update() {
        let (refresher, queue) = make_refresher();
        push_memory_update(&queue, "User likes morning work");

        let mut messages = vec![
            providers::Message::system("System prompt"),
            providers::Message::user("Plan my day"),
            providers::Message::assistant("Let me check..."),
        ];
        let result = refresher.inject_pending(&mut messages, 128_000);
        assert_eq!(result.len(), 1);
        assert_eq!(messages.len(), 4);
        assert!(matches!(
            &messages[3],
            providers::Message::ContextUpdate { content, .. } if content.contains("morning work")
        ));
    }

    #[test]
    fn respects_token_budget() {
        let (refresher, queue) = make_refresher();
        push_memory_update(&queue, &"x".repeat(10000));

        let mut messages = vec![
            providers::Message::system("y".repeat(10000)),
            providers::Message::user("Hello"),
        ];
        let result = refresher.inject_pending(&mut messages, 100);
        assert!(result.is_empty());
    }

    #[test]
    fn priority_ordering_high_first() {
        let (refresher, queue) = make_refresher();
        let now = jiff::Timestamp::now();
        queue.push(bus::ContextUpdate {
            reason: bus::ContextUpdateReason::MemoryPromoted,
            content: Some("Low priority fact".to_string()),
            metadata: None,
            priority: bus::UpdatePriority::Low,
            timestamp: now,
        });
        queue.push(bus::ContextUpdate {
            reason: bus::ContextUpdateReason::FocusSessionEnded,
            content: Some("High priority event".to_string()),
            metadata: None,
            priority: bus::UpdatePriority::High,
            timestamp: now,
        });

        let mut messages = vec![providers::Message::system("sys")];
        let result = refresher.inject_pending(&mut messages, 128_000);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].reason, "focus_session_ended");
        assert_eq!(result[1].reason, "memory_promoted");
    }

    #[test]
    fn drains_queue_after_inject() {
        let (refresher, queue) = make_refresher();
        push_memory_update(&queue, "Fact");
        let mut messages = vec![providers::Message::system("sys")];
        refresher.inject_pending(&mut messages, 128_000);
        let result = refresher.inject_pending(&mut messages, 128_000);
        assert!(result.is_empty());
    }
}
