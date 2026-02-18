//! Reasoning scratchpad for tracking execution traces across cycles.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single reasoning trace capturing one execution cycle's thought process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub cycle: u32,
    pub thought: String,
    pub planned_actions: Vec<String>,
    pub actual_action: String,
    pub reflection: Option<String>,
    pub timestamp: DateTime<Utc>,
}

/// Accumulates reasoning traces across execution cycles for context injection.
#[derive(Debug, Default)]
pub struct Scratchpad {
    traces: Vec<ReasoningTrace>,
}

impl Scratchpad {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a reasoning trace.
    pub fn add(&mut self, trace: ReasoningTrace) {
        self.traces.push(trace);
    }

    /// All traces in order.
    pub fn traces(&self) -> &[ReasoningTrace] {
        &self.traces
    }

    /// Return the last `n` traces (or all if fewer than `n`).
    pub fn last_n(&self, n: usize) -> &[ReasoningTrace] {
        let start = self.traces.len().saturating_sub(n);
        &self.traces[start..]
    }

    pub fn len(&self) -> usize {
        self.traces.len()
    }

    pub fn is_empty(&self) -> bool {
        self.traces.is_empty()
    }

    /// Produce a brief summary string suitable for context injection.
    pub fn summarize(&self) -> String {
        if self.traces.is_empty() {
            return String::new();
        }

        let mut summary = format!("Reasoning trace ({} cycles):\n", self.traces.len());
        for t in &self.traces {
            summary.push_str(&format!(
                "- Cycle {}: {} → {}\n",
                t.cycle, t.thought, t.actual_action
            ));
            if let Some(ref r) = t.reflection {
                summary.push_str(&format!("  Reflection: {}\n", r));
            }
        }
        summary
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_trace(cycle: u32, thought: &str, action: &str) -> ReasoningTrace {
        ReasoningTrace {
            cycle,
            thought: thought.to_string(),
            planned_actions: vec![action.to_string()],
            actual_action: action.to_string(),
            reflection: None,
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_add_and_retrieve() {
        let mut pad = Scratchpad::new();
        pad.add(make_trace(1, "think A", "do A"));
        pad.add(make_trace(2, "think B", "do B"));
        pad.add(make_trace(3, "think C", "do C"));

        assert_eq!(pad.len(), 3);
        assert!(!pad.is_empty());

        let last2 = pad.last_n(2);
        assert_eq!(last2.len(), 2);
        assert_eq!(last2[0].cycle, 2);
        assert_eq!(last2[1].cycle, 3);

        // last_n with n > len returns all
        assert_eq!(pad.last_n(10).len(), 3);
    }

    #[test]
    fn test_summarize_empty() {
        let pad = Scratchpad::new();
        assert!(pad.summarize().is_empty());
    }

    #[test]
    fn test_summarize_with_traces() {
        let mut pad = Scratchpad::new();
        pad.add(make_trace(1, "analyze input", "read_file"));
        pad.add(ReasoningTrace {
            cycle: 2,
            thought: "need to fix".to_string(),
            planned_actions: vec!["edit_file".to_string()],
            actual_action: "edit_file".to_string(),
            reflection: Some("successful edit".to_string()),
            timestamp: Utc::now(),
        });

        let summary = pad.summarize();
        assert!(!summary.is_empty());
        assert!(summary.contains("2 cycles"));
        assert!(summary.contains("read_file"));
        assert!(summary.contains("edit_file"));
        assert!(summary.contains("Reflection: successful edit"));
    }
}
