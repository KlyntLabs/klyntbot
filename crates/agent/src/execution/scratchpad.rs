//! Reasoning scratchpad for tracking execution traces across cycles.

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

fn plan_step_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^\d+\.\s+(?P<desc>.+?)(?:\s*\[tool:\s*(?P<tool>\w+)\])?\s*$").unwrap()
    })
}

/// A single step in an execution plan generated before the ReAct loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub index: usize,
    pub description: String,
    /// Expected tool name (if parseable from LLM output). None = free-form step.
    pub expected_tool: Option<String>,
    pub completed: bool,
}

/// Structured execution plan generated in iteration 0 for complex tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub steps: Vec<PlanStep>,
    /// The raw plan text from the LLM (kept for context injection).
    pub raw_text: String,
}

impl ExecutionPlan {
    /// Parse numbered steps from LLM plan text.
    pub fn parse(text: &str) -> Self {
        let mut steps = Vec::new();
        for line in text.lines() {
            let trimmed = line.trim();
            if let Some(caps) = plan_step_re().captures(trimmed) {
                let description = caps["desc"].trim().to_string();
                let expected_tool = caps.name("tool").map(|m| m.as_str().to_string());
                steps.push(PlanStep {
                    index: steps.len(),
                    description,
                    expected_tool,
                    completed: false,
                });
            }
        }
        Self {
            steps,
            raw_text: text.to_string(),
        }
    }

    /// Collect step descriptions into a Vec<String>.
    pub fn step_descriptions(&self) -> Vec<String> {
        self.steps.iter().map(|s| s.description.clone()).collect()
    }
}

/// A single reasoning trace capturing one execution cycle's thought process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReasoningTrace {
    pub cycle: u32,
    pub thought: String,
    pub planned_actions: Vec<String>,
    pub actual_action: String,
    pub reflection: Option<String>,
    pub timestamp: DateTime<Utc>,
    /// Which plan step this trace corresponds to (if a plan exists).
    pub plan_step_index: Option<usize>,
}

/// Accumulates reasoning traces across execution cycles for context injection.
#[derive(Debug, Default)]
pub struct Scratchpad {
    traces: Vec<ReasoningTrace>,
    plan: Option<ExecutionPlan>,
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

    pub fn set_plan(&mut self, plan: ExecutionPlan) {
        self.plan = Some(plan);
    }

    pub fn plan(&self) -> Option<&ExecutionPlan> {
        self.plan.as_ref()
    }

    /// Mark the first uncompleted plan step that matches the given tool name.
    /// Returns `(index, description, tool_name)` of the completed step, if any.
    pub fn mark_step_completed(&mut self, tool_name: &str) -> Option<(usize, String, String)> {
        let plan = self.plan.as_mut()?;
        let step = plan
            .steps
            .iter_mut()
            .find(|s| !s.completed && s.expected_tool.as_deref() == Some(tool_name))?;
        step.completed = true;
        Some((step.index, step.description.clone(), tool_name.to_string()))
    }

    /// (completed, total) plan progress. None if no plan.
    pub fn plan_progress(&self) -> Option<(usize, usize)> {
        self.plan.as_ref().map(|p| {
            let done = p.steps.iter().filter(|s| s.completed).count();
            (done, p.steps.len())
        })
    }

    /// Iterate over uncompleted plan step descriptions.
    pub fn remaining_step_descriptions(&self) -> Vec<String> {
        self.plan
            .as_ref()
            .map(|p| {
                p.steps
                    .iter()
                    .filter(|s| !s.completed)
                    .map(|s| s.description.clone())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Produce a brief summary string suitable for context injection.
    ///
    /// Caps output to the last 20 traces to prevent unbounded growth
    /// when injected into the context window during long execution runs.
    pub fn summarize(&self) -> String {
        if self.traces.is_empty() {
            return String::new();
        }

        const MAX_SUMMARY_TRACES: usize = 20;
        let traces = self.last_n(MAX_SUMMARY_TRACES);
        let skipped = self.traces.len().saturating_sub(MAX_SUMMARY_TRACES);

        let mut summary = format!("Reasoning trace ({} cycles):\n", self.traces.len());
        if skipped > 0 {
            summary.push_str(&format!("  ({} earlier cycles omitted)\n", skipped));
        }
        for t in traces {
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
            plan_step_index: None,
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
            plan_step_index: None,
        });

        let summary = pad.summarize();
        assert!(!summary.is_empty());
        assert!(summary.contains("2 cycles"));
        assert!(summary.contains("read_file"));
        assert!(summary.contains("edit_file"));
        assert!(summary.contains("Reflection: successful edit"));
    }

    #[test]
    fn plan_step_tracks_completion() {
        let mut step = PlanStep {
            index: 0,
            description: "Fetch tasks".to_string(),
            expected_tool: Some("task".to_string()),
            completed: false,
        };
        assert!(!step.completed);
        step.completed = true;
        assert!(step.completed);
    }

    #[test]
    fn execution_plan_default_is_empty() {
        let plan = ExecutionPlan::default();
        assert!(plan.steps.is_empty());
        assert!(plan.raw_text.is_empty());
    }

    #[test]
    fn scratchpad_plan_lifecycle() {
        let mut pad = Scratchpad::new();
        assert!(pad.plan().is_none());
        assert!(pad.plan_progress().is_none());

        let plan = ExecutionPlan {
            steps: vec![
                PlanStep {
                    index: 0,
                    description: "Search web".to_string(),
                    expected_tool: Some("web_search".to_string()),
                    completed: false,
                },
                PlanStep {
                    index: 1,
                    description: "Summarize results".to_string(),
                    expected_tool: None,
                    completed: false,
                },
                PlanStep {
                    index: 2,
                    description: "Create task".to_string(),
                    expected_tool: Some("task".to_string()),
                    completed: false,
                },
            ],
            raw_text: "1. Search web\n2. Summarize\n3. Create task".to_string(),
        };
        pad.set_plan(plan);

        assert!(pad.plan().is_some());
        assert_eq!(pad.plan_progress(), Some((0, 3)));

        let completed = pad.mark_step_completed("web_search");
        assert_eq!(completed.as_ref().map(|(idx, _, _)| *idx), Some(0));
        assert_eq!(pad.plan_progress(), Some((1, 3)));

        // Same tool again doesn't double-count
        assert!(pad.mark_step_completed("web_search").is_none());
        assert_eq!(pad.plan_progress(), Some((1, 3)));

        let completed = pad.mark_step_completed("task");
        assert_eq!(completed.as_ref().map(|(idx, _, _)| *idx), Some(2));
        assert_eq!(pad.plan_progress(), Some((2, 3)));
    }

    #[test]
    fn reasoning_trace_has_plan_step_index() {
        let trace = ReasoningTrace {
            cycle: 1,
            thought: "test".to_string(),
            planned_actions: vec![],
            actual_action: "test".to_string(),
            reflection: None,
            timestamp: Utc::now(),
            plan_step_index: Some(0),
        };
        assert_eq!(trace.plan_step_index, Some(0));
    }

    #[test]
    fn remaining_step_descriptions_filters_completed() {
        let mut pad = Scratchpad::new();
        let plan = ExecutionPlan {
            steps: vec![
                PlanStep {
                    index: 0,
                    description: "Step A".to_string(),
                    expected_tool: Some("tool_a".to_string()),
                    completed: false,
                },
                PlanStep {
                    index: 1,
                    description: "Step B".to_string(),
                    expected_tool: Some("tool_b".to_string()),
                    completed: false,
                },
            ],
            raw_text: String::new(),
        };
        pad.set_plan(plan);

        pad.mark_step_completed("tool_a");
        let remaining = pad.remaining_step_descriptions();
        assert_eq!(remaining, vec!["Step B"]);
    }

    // ── Plan parsing tests ──────────────────────────────────────────

    #[test]
    fn parse_plan_extracts_steps_with_tools() {
        let text = "Here's my plan:\n1. Search the web [tool: web_search]\n2. Summarize results\n3. Create a task [tool: task]";
        let plan = ExecutionPlan::parse(text);
        assert_eq!(plan.steps.len(), 3);
        assert_eq!(plan.steps[0].description, "Search the web");
        assert_eq!(plan.steps[0].expected_tool.as_deref(), Some("web_search"));
        assert_eq!(plan.steps[1].description, "Summarize results");
        assert!(plan.steps[1].expected_tool.is_none());
        assert_eq!(plan.steps[2].expected_tool.as_deref(), Some("task"));
        assert!(!plan.raw_text.is_empty());
    }

    #[test]
    fn parse_plan_handles_empty_text() {
        let plan = ExecutionPlan::parse("No plan here, just text.");
        assert!(plan.steps.is_empty());
    }

    #[test]
    fn parse_plan_handles_mixed_content() {
        let text = "I'll help you with that.\n\n1. First step [tool: alpha]\nSome explanation.\n2. Second step\n\nMore text.";
        let plan = ExecutionPlan::parse(text);
        assert_eq!(plan.steps.len(), 2);
    }

    #[test]
    fn step_descriptions_collects_all() {
        let plan = ExecutionPlan::parse("1. Do A [tool: a]\n2. Do B\n3. Do C [tool: c]");
        assert_eq!(plan.step_descriptions(), vec!["Do A", "Do B", "Do C"]);
    }
}
