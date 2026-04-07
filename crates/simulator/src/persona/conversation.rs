//! Tracks conversation turns for multi-turn simulation.

use std::collections::VecDeque;

use providers::types::Message;

/// Accumulates (user_message, agent_response) pairs for multi-turn context.
pub struct ConversationTracker {
    turns: VecDeque<(String, String)>,
    max_depth: usize,
}

impl ConversationTracker {
    pub fn new(max_depth: usize) -> Self {
        Self {
            turns: VecDeque::new(),
            max_depth,
        }
    }

    /// Record a completed turn. Trims oldest turns if over max_depth.
    pub fn record(&mut self, user_msg: &str, agent_response: &str) {
        self.turns
            .push_back((user_msg.to_string(), agent_response.to_string()));
        while self.turns.len() > self.max_depth {
            self.turns.pop_front();
        }
    }

    /// Convert accumulated turns into a message history for the AgentRuntime.
    /// Returns alternating User / Assistant messages.
    pub fn history_messages(&self) -> Vec<Message> {
        let mut messages = Vec::with_capacity(self.turns.len() * 2);
        for (user, assistant) in &self.turns {
            messages.push(Message::user(user));
            messages.push(Message::assistant(assistant));
        }
        messages
    }

    /// The agent's most recent response, if any.
    pub fn last_response(&self) -> Option<&str> {
        self.turns.back().map(|(_, resp)| resp.as_str())
    }

    pub fn len(&self) -> usize {
        self.turns.len()
    }

    pub fn is_empty(&self) -> bool {
        self.turns.is_empty()
    }

    /// Compute semantic drift between the last two agent responses.
    /// Returns None if fewer than 2 turns exist.
    /// Uses cosine distance (1 - similarity) — lower = more coherent.
    pub fn semantic_drift(&self, engine: &tools::EmbeddingEngine) -> Option<f64> {
        if self.turns.len() < 2 {
            return None;
        }
        let len = self.turns.len();
        let prev_response = &self.turns[len - 2].1;
        let curr_response = &self.turns[len - 1].1;

        let prev_emb = engine.embed(prev_response).ok()?;
        let curr_emb = engine.embed(curr_response).ok()?;
        let similarity = common::helpers::cosine_similarity(&prev_emb, &curr_emb);
        Some(1.0 - similarity)
    }

    /// Return the current turn depth.
    pub fn depth(&self) -> usize {
        self.turns.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_turns_up_to_max_depth() {
        let mut tracker = ConversationTracker::new(2);
        tracker.record("msg1", "resp1");
        tracker.record("msg2", "resp2");
        tracker.record("msg3", "resp3");

        assert_eq!(tracker.len(), 2);
        assert_eq!(tracker.last_response(), Some("resp3"));

        let history = tracker.history_messages();
        assert_eq!(history.len(), 4); // 2 turns * 2 messages each
    }

    #[test]
    fn empty_tracker_returns_empty_history() {
        let tracker = ConversationTracker::new(5);
        assert!(tracker.is_empty());
        assert!(tracker.history_messages().is_empty());
        assert_eq!(tracker.last_response(), None);
    }
}
