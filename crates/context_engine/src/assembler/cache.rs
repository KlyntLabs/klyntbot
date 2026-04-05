use std::collections::{HashMap, VecDeque};

use super::types::AssembledContext;

/// During ReAct loops each iteration changes the cache key (growing message history),
/// so 8 entries means 8 full conversation snapshots coexist. 2 entries retain the
/// current + one prior without wasting memory on stale intermediate states.
pub(super) const DEFAULT_CACHE_CAPACITY: usize = 2;

/// Bounded LRU cache for assembled contexts, keyed by SHA-256 of request inputs.
///
/// The cache key captures all relevant state (system prompt, history, message,
/// strategy, tools, context window), so entries naturally expire when inputs change.
/// No explicit invalidation is needed — tool execution appends to history,
/// changing the cache key.
pub(super) struct ContextCache {
    entries: HashMap<String, AssembledContext>,
    /// Insertion order for eviction (oldest first). O(1) eviction, O(n) promotion.
    order: VecDeque<String>,
    capacity: usize,
}

impl ContextCache {
    pub(super) fn new(capacity: usize) -> Self {
        Self {
            entries: HashMap::with_capacity(capacity),
            order: VecDeque::with_capacity(capacity),
            capacity,
        }
    }

    pub(super) fn get(&self, key: &str) -> Option<&AssembledContext> {
        self.entries.get(key)
    }

    pub(super) fn insert(&mut self, key: String, value: AssembledContext) {
        let is_update = self.entries.contains_key(&key);
        if !is_update && self.entries.len() >= self.capacity {
            // Evict oldest
            if let Some(oldest_key) = self.order.pop_front() {
                self.entries.remove(&oldest_key);
            }
        }
        if is_update {
            // Promote to back: remove from old position, push to back.
            self.order.retain(|k| k != &key);
        }
        self.order.push_back(key.clone());
        self.entries.insert(key, value);
    }
}
