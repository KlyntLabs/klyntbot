//! Core types for the intent pipeline: execution modes, complexity signals, intent analysis.

use context_engine::ExecutionStrategy;
use domain::PlanVisibility;
use serde::{Deserialize, Serialize};

/// The three execution modes the pipeline can select.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionMode {
    /// Single LLM call, no tools. Greetings, factual Q&A.
    Direct,
    /// ReAct loop with tools. Single-shot tasks, searches, CRUD.
    Reactive { max_iterations: u32 },
    /// Multi-step plan with structured execution.
    Planned {
        visibility: PlanVisibility,
        max_steps: u8,
    },
}

impl ExecutionMode {
    /// Short lowercase name for logging/strategy records.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Reactive { .. } => "reactive",
            Self::Planned { .. } => "planned",
        }
    }

    /// Maximum iterations/steps for this mode.
    pub fn max_iterations(&self) -> u32 {
        match self {
            Self::Direct => 1,
            Self::Reactive { max_iterations } => *max_iterations,
            Self::Planned { max_steps, .. } => *max_steps as u32,
        }
    }
}

impl std::fmt::Display for ExecutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Direct => write!(f, "Direct"),
            Self::Reactive { max_iterations } => {
                write!(f, "Reactive(max={})", max_iterations)
            }
            Self::Planned { max_steps, .. } => {
                write!(f, "Planned(steps={})", max_steps)
            }
        }
    }
}

/// Bridge: `ExecutionMode` → `ExecutionStrategy` for the `ContextEngine`.
///
/// The `ContextEngine` allocates token budgets based on `ExecutionStrategy`.
/// This conversion lets us keep `ContextEngine` unchanged.
impl From<&ExecutionMode> for ExecutionStrategy {
    fn from(mode: &ExecutionMode) -> Self {
        match mode {
            ExecutionMode::Direct => ExecutionStrategy::DirectResponse,
            ExecutionMode::Reactive { max_iterations } => ExecutionStrategy::ToolAssisted {
                max_iterations: *max_iterations,
            },
            ExecutionMode::Planned { .. } => {
                ExecutionStrategy::AutonomousTask { max_iterations: 50 }
            }
        }
    }
}

/// Risk level for operations that could fail or have side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FailureRisk {
    Low,
    Medium,
    High,
}

/// Structured complexity analysis of a user request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySignals {
    pub estimated_tool_calls: u8,
    pub has_sequential_deps: bool,
    pub failure_risk: FailureRisk,
    pub requires_state_tracking: bool,
    pub requires_retries: bool,
}

impl ComplexitySignals {
    /// Score the overall complexity (0-7).
    ///
    /// Scoring:
    /// - estimated_tool_calls >= 3: +2, >= 2: +1
    /// - has_sequential_deps: +2
    /// - failure_risk >= Medium: +1
    /// - requires_state_tracking: +1
    /// - requires_retries: +1
    pub fn complexity_score(&self) -> u8 {
        let mut score: u8 = 0;
        if self.estimated_tool_calls >= 3 {
            score += 2;
        } else if self.estimated_tool_calls >= 2 {
            score += 1;
        }
        if self.has_sequential_deps {
            score += 2;
        }
        if self.failure_risk >= FailureRisk::Medium {
            score += 1;
        }
        if self.requires_state_tracking {
            score += 1;
        }
        if self.requires_retries {
            score += 1;
        }
        score
    }
}

/// How the intent analysis was produced.
#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisSource {
    Heuristic,
    LlmClassifier,
    MidExecutionEscalation,
}

/// Groups of tools that can be selectively exposed based on intent classification.
///
/// Instead of presenting all tools to the LLM on every request, the intent pipeline
/// narrows the action space to relevant groups. This reduces hallucinated tool calls
/// and improves response quality.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ToolGroup {
    /// No tools needed (greetings, factual Q&A).
    None,
    /// Task/todo/goal/plan management.
    TaskManagement,
    /// File search and web retrieval.
    Search,
    /// Calendar operations.
    Calendar,
    /// Finance operations.
    Finance,
    /// Messaging and user interaction.
    Communication,
    /// Cron jobs and subagent spawning.
    Automation,
    /// All tools — used as fallback or for complex/ambiguous requests.
    Full,
}

impl ToolGroup {
    /// Returns the tool names associated with this group.
    pub fn tool_names(&self) -> &'static [&'static str] {
        match self {
            Self::None => &[],
            Self::TaskManagement => &["todo", "goal", "plan"],
            Self::Search => &[
                "grep",
                "glob",
                "read_file",
                "list_dir",
                "web_search",
                "web_fetch",
                "memory",
            ],
            Self::Calendar => &["calendar", "todo"],
            Self::Finance => &["finance"],
            Self::Communication => &["message", "ask_user"],
            Self::Automation => &["cron", "spawn"],
            Self::Full => &[], // Special: means all tools
        }
    }
}

/// Result of intent analysis — the selected execution mode with supporting data.
#[derive(Debug, Clone)]
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub signals: ComplexitySignals,
    pub confidence: f32,
    pub source: AnalysisSource,
    pub reasoning: String,
    /// Which tool groups are relevant for this intent.
    /// Empty or contains `ToolGroup::Full` means all tools are available.
    pub tool_groups: Vec<ToolGroup>,
}

impl IntentAnalysis {
    /// Fallback analysis when classification fails entirely.
    pub fn fallback() -> Self {
        Self {
            mode: ExecutionMode::Reactive { max_iterations: 10 },
            signals: ComplexitySignals {
                estimated_tool_calls: 1,
                has_sequential_deps: false,
                failure_risk: FailureRisk::Low,
                requires_state_tracking: false,
                requires_retries: false,
            },
            confidence: 0.5,
            source: AnalysisSource::Heuristic,
            reasoning: "Fallback — classification unavailable".to_string(),
            tool_groups: vec![ToolGroup::Full],
        }
    }

    /// Collect the set of allowed tool names from all tool groups.
    /// Returns `None` if all tools should be available (Full group present).
    pub fn allowed_tool_names(&self) -> Option<std::collections::HashSet<&'static str>> {
        if self.tool_groups.is_empty() || self.tool_groups.contains(&ToolGroup::Full) {
            return None; // All tools allowed
        }

        let mut allowed: std::collections::HashSet<&str> = self
            .tool_groups
            .iter()
            .flat_map(|g| g.tool_names().iter().copied())
            .collect();

        // ask_user is always available (clarification is always useful)
        allowed.insert("ask_user");

        Some(allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_complexity_is_direct() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 0,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(signals.complexity_score(), 0);
    }

    #[test]
    fn high_complexity_triggers_planned() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 4,
            has_sequential_deps: true,
            failure_risk: FailureRisk::High,
            requires_state_tracking: true,
            requires_retries: true,
        };
        assert!(signals.complexity_score() >= 3);
        // 2 (tool_calls>=3) + 2 (seq_deps) + 1 (high risk) + 1 (state) + 1 (retries) = 7
        assert_eq!(signals.complexity_score(), 7);
    }

    #[test]
    fn sequential_deps_alone_score_2() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 1,
            has_sequential_deps: true,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(signals.complexity_score(), 2);
    }

    #[test]
    fn two_tool_calls_scores_1() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 2,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(signals.complexity_score(), 1);
    }

    #[test]
    fn medium_risk_adds_1() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 0,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Medium,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(signals.complexity_score(), 1);
    }

    #[test]
    fn fallback_analysis_is_reactive() {
        let analysis = IntentAnalysis::fallback();
        assert!(matches!(
            analysis.mode,
            ExecutionMode::Reactive { max_iterations: 10 }
        ));
        assert_eq!(analysis.confidence, 0.5);
        assert!(analysis.tool_groups.contains(&ToolGroup::Full));
    }

    #[test]
    fn failure_risk_ordering() {
        assert!(FailureRisk::Low < FailureRisk::Medium);
        assert!(FailureRisk::Medium < FailureRisk::High);
    }

    #[test]
    fn tool_group_none_has_no_tools() {
        assert!(ToolGroup::None.tool_names().is_empty());
    }

    #[test]
    fn tool_group_task_management_has_expected_tools() {
        let names = ToolGroup::TaskManagement.tool_names();
        assert!(names.contains(&"todo"));
        assert!(names.contains(&"goal"));
        assert!(names.contains(&"plan"));
    }

    #[test]
    fn allowed_tool_names_with_full_returns_none() {
        let analysis = IntentAnalysis::fallback();
        assert!(analysis.allowed_tool_names().is_none());
    }

    #[test]
    fn allowed_tool_names_with_groups_returns_set() {
        let analysis = IntentAnalysis {
            tool_groups: vec![ToolGroup::TaskManagement, ToolGroup::Search],
            ..IntentAnalysis::fallback()
        };
        let allowed = analysis.allowed_tool_names().unwrap();
        assert!(allowed.contains("todo"));
        assert!(allowed.contains("grep"));
        assert!(allowed.contains("ask_user")); // always included
        assert!(!allowed.contains("finance"));
    }

    #[test]
    fn allowed_tool_names_with_none_group_only_has_ask_user() {
        let analysis = IntentAnalysis {
            tool_groups: vec![ToolGroup::None],
            ..IntentAnalysis::fallback()
        };
        let allowed = analysis.allowed_tool_names().unwrap();
        assert_eq!(allowed.len(), 1);
        assert!(allowed.contains("ask_user"));
    }
}
