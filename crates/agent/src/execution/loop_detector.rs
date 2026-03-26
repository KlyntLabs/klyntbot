//! Hash-based loop detection for the ReAct execution loop.
//!
//! Identifies when the agent repeats the same set of tool calls across iterations
//! by comparing iteration signatures (hashes of sorted tool-call sets).

use serde_json::Value;
use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};

/// Signature of a single iteration in the ReAct loop.
#[derive(Debug)]
pub struct IterationSignature {
    /// Hash of the (sorted) tool calls made in this iteration.
    pub hash: u64,
    /// Tool names invoked in this iteration (sorted).
    pub tools: Vec<String>,
    /// The iteration number.
    pub iteration: usize,
}

/// Result of checking for loops after recording an iteration.
#[derive(Debug, PartialEq, Eq)]
pub enum LoopStatus {
    /// No loop detected.
    NoLoop,
    /// A repeating pattern was detected at the warning threshold.
    Warning {
        count: u32,
        hash: u64,
        tools_summary: String,
    },
    /// A repeating pattern hit the hard-stop threshold — execution should abort.
    HardStop { count: u32, tools_summary: String },
}

/// Detects oscillation in the ReAct loop by tracking iteration hashes.
///
/// Session-scoped — the internal `DefaultHasher` is not stable across process
/// restarts, which is fine since the detector lives for the duration of a single
/// agent execution.
#[derive(Debug)]
pub struct LoopDetector {
    history: VecDeque<IterationSignature>,
    warned_hashes: HashSet<u64>,
    window_size: usize,
    warn_threshold: u32,
    hard_stop_threshold: u32,
}

impl LoopDetector {
    /// Create a detector with default thresholds (window=20, warn=3, hard_stop=5).
    pub fn new() -> Self {
        Self::with_config(20, 3, 5)
    }

    /// Create a detector with custom thresholds.
    pub fn with_config(window_size: usize, warn_threshold: u32, hard_stop_threshold: u32) -> Self {
        Self {
            history: VecDeque::with_capacity(window_size),
            warned_hashes: HashSet::new(),
            window_size,
            warn_threshold,
            hard_stop_threshold,
        }
    }

    /// Record a completed iteration and check for loops.
    ///
    /// `tool_calls` is the list of `(tool_name, arguments)` pairs invoked during
    /// this iteration.
    pub fn record_iteration(
        &mut self,
        iteration: usize,
        tool_calls: &[(String, Value)],
    ) -> LoopStatus {
        let hash = compute_iteration_hash(tool_calls);

        let mut tools: Vec<String> = tool_calls.iter().map(|(name, _)| name.clone()).collect();
        tools.sort();

        self.history.push_back(IterationSignature {
            hash,
            tools,
            iteration,
        });

        // Trim to window size.
        while self.history.len() > self.window_size {
            self.history.pop_front();
        }

        // Count consecutive identical hashes at the tail.
        let consecutive = self
            .history
            .iter()
            .rev()
            .take_while(|sig| sig.hash == hash)
            .count() as u32;

        let tools_summary = self.tools_summary(hash);

        if consecutive >= self.hard_stop_threshold {
            return LoopStatus::HardStop {
                count: consecutive,
                tools_summary,
            };
        }

        if consecutive >= self.warn_threshold && self.warned_hashes.insert(hash) {
            // First time seeing this hash at warn threshold.
            return LoopStatus::Warning {
                count: consecutive,
                hash,
                tools_summary,
            };
        }
        // Already warned for this hash, or below threshold — no action needed.

        LoopStatus::NoLoop
    }

    /// Build a human-readable summary of the tools for a given hash.
    fn tools_summary(&self, hash: u64) -> String {
        self.history
            .iter()
            .rev()
            .find(|sig| sig.hash == hash)
            .map(|sig| sig.tools.join(", "))
            .unwrap_or_default()
    }
}

impl Default for LoopDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Compute an order-independent hash for a set of tool calls.
///
/// Tool calls are sorted lexicographically by `(name, stable_json(args))` before
/// hashing, so different orderings of the same call set produce the same hash.
pub fn compute_iteration_hash(tool_calls: &[(String, Value)]) -> u64 {
    let mut canonical: Vec<(String, String)> = tool_calls
        .iter()
        .map(|(name, args)| (name.clone(), stable_json(args)))
        .collect();

    canonical.sort();

    let mut hasher = DefaultHasher::new();
    for (name, json) in &canonical {
        name.hash(&mut hasher);
        json.hash(&mut hasher);
    }
    hasher.finish()
}

/// Recursively produce a canonical JSON string with sorted object keys.
///
/// - Object keys are sorted lexicographically.
/// - Arrays preserve element order.
/// - Primitives are serialised as-is.
pub(crate) fn stable_json(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let sorted: BTreeMap<_, _> = map.iter().collect();
            let entries: Vec<String> = sorted
                .iter()
                .map(|(k, v)| format!("{}:{}", serde_json::to_string(k).unwrap(), stable_json(v)))
                .collect();
            format!("{{{}}}", entries.join(","))
        }
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(stable_json).collect();
            format!("[{}]", items.join(","))
        }
        // Primitives: null, bool, number, string — serde_json::to_string is deterministic.
        _ => serde_json::to_string(value).unwrap(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn calls(pairs: &[(&str, Value)]) -> Vec<(String, Value)> {
        pairs
            .iter()
            .map(|(name, args)| (name.to_string(), args.clone()))
            .collect()
    }

    #[test]
    fn test_hash_consistency() {
        let c = calls(&[("search", json!({"q": "rust"}))]);
        assert_eq!(compute_iteration_hash(&c), compute_iteration_hash(&c));
    }

    #[test]
    fn test_hash_order_independence() {
        let a = calls(&[
            ("search", json!({"q": "rust"})),
            ("read", json!({"path": "/tmp"})),
        ]);
        let b = calls(&[
            ("read", json!({"path": "/tmp"})),
            ("search", json!({"q": "rust"})),
        ]);
        assert_eq!(compute_iteration_hash(&a), compute_iteration_hash(&b));
    }

    #[test]
    fn test_different_args_different_hash() {
        let a = calls(&[("search", json!({"q": "rust"}))]);
        let b = calls(&[("search", json!({"q": "python"}))]);
        assert_ne!(compute_iteration_hash(&a), compute_iteration_hash(&b));
    }

    #[test]
    fn test_no_loop_below_threshold() {
        let mut det = LoopDetector::new();
        let c = calls(&[("search", json!({"q": "rust"}))]);

        assert_eq!(det.record_iteration(1, &c), LoopStatus::NoLoop);
        assert_eq!(det.record_iteration(2, &c), LoopStatus::NoLoop);
    }

    #[test]
    fn test_warning_at_threshold() {
        let mut det = LoopDetector::new();
        let c = calls(&[("search", json!({"q": "rust"}))]);

        det.record_iteration(1, &c);
        det.record_iteration(2, &c);
        let status = det.record_iteration(3, &c);

        match status {
            LoopStatus::Warning { count, .. } => assert_eq!(count, 3),
            other => panic!("expected Warning, got {:?}", other),
        }
    }

    #[test]
    fn test_warning_once_per_hash() {
        let mut det = LoopDetector::new();
        let c = calls(&[("search", json!({"q": "rust"}))]);

        det.record_iteration(1, &c);
        det.record_iteration(2, &c);
        let _ = det.record_iteration(3, &c); // Warning

        // 4th identical should be NoLoop (already warned).
        assert_eq!(det.record_iteration(4, &c), LoopStatus::NoLoop);
    }

    #[test]
    fn test_hard_stop_at_threshold() {
        let mut det = LoopDetector::new();
        let c = calls(&[("search", json!({"q": "rust"}))]);

        det.record_iteration(1, &c);
        det.record_iteration(2, &c);
        det.record_iteration(3, &c); // Warning
        det.record_iteration(4, &c); // NoLoop (warned)

        let status = det.record_iteration(5, &c);
        match status {
            LoopStatus::HardStop { count, .. } => assert_eq!(count, 5),
            other => panic!("expected HardStop, got {:?}", other),
        }
    }

    #[test]
    fn test_different_hash_resets_count() {
        let mut det = LoopDetector::new();
        let a = calls(&[("search", json!({"q": "rust"}))]);
        let b = calls(&[("read", json!({"path": "/tmp"}))]);

        // Build up 2 consecutive a's.
        det.record_iteration(1, &a);
        det.record_iteration(2, &a);
        // Interleave a different call — resets the consecutive count.
        det.record_iteration(3, &b);
        // Only 1 consecutive 'a' after the break.
        assert_eq!(det.record_iteration(4, &a), LoopStatus::NoLoop);
        // 2 consecutive 'a' — still below warn threshold of 3.
        assert_eq!(det.record_iteration(5, &a), LoopStatus::NoLoop);
    }

    #[test]
    fn test_interleaved_hashes_no_false_trigger() {
        let mut det = LoopDetector::new();
        let a = calls(&[("search", json!({"q": "rust"}))]);
        let b = calls(&[("read", json!({"path": "/tmp"}))]);

        // Alternating: a, b, a, b, a — no consecutive streak > 1.
        assert_eq!(det.record_iteration(1, &a), LoopStatus::NoLoop);
        assert_eq!(det.record_iteration(2, &b), LoopStatus::NoLoop);
        assert_eq!(det.record_iteration(3, &a), LoopStatus::NoLoop);
        assert_eq!(det.record_iteration(4, &b), LoopStatus::NoLoop);
        assert_eq!(det.record_iteration(5, &a), LoopStatus::NoLoop);
    }

    #[test]
    fn test_sliding_window_eviction() {
        // Window of 4, warn at 3, hard stop at 5.
        let mut det = LoopDetector::with_config(4, 3, 5);
        let a = calls(&[("search", json!({"q": "rust"}))]);
        let b = calls(&[("read", json!({"path": "/tmp"}))]);

        // Fill window with 'a': 4 entries, all 'a'.
        det.record_iteration(1, &a);
        det.record_iteration(2, &a);
        det.record_iteration(3, &a); // Warning (3 consecutive)
        det.record_iteration(4, &a); // NoLoop (already warned)

        // Now insert 'b' — evicts oldest 'a' since window is 4.
        det.record_iteration(5, &b); // history: [a, a, a, b]

        // Two more 'a' — only 2 consecutive at tail.
        det.record_iteration(6, &a); // history: [a, a, b, a]
        det.record_iteration(7, &a); // history: [a, b, a, a]

        // Only 2 consecutive a's at the tail → NoLoop.
        assert_eq!(det.record_iteration(8, &a), LoopStatus::NoLoop);
        // history: [b, a, a, a] → 3 consecutive a's, but we already warned for
        // this hash, so it returns NoLoop. That validates the warned_hashes guard too.
    }

    #[test]
    fn test_json_key_order_stability() {
        let a = calls(&[("tool", json!({"b": 2, "a": 1}))]);
        let b = calls(&[("tool", json!({"a": 1, "b": 2}))]);
        assert_eq!(compute_iteration_hash(&a), compute_iteration_hash(&b));
    }

    #[test]
    fn test_empty_tool_calls() {
        let mut det = LoopDetector::new();
        let empty: Vec<(String, Value)> = vec![];
        // Should not panic.
        let status = det.record_iteration(1, &empty);
        assert_eq!(status, LoopStatus::NoLoop);
    }
}
