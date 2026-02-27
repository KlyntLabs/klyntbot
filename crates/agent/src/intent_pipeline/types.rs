//! Core types for the intent pipeline: execution modes, complexity signals, intent analysis.

use context_engine::ExecutionStrategy;
use plan::PlanVisibility;
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

/// Result of intent analysis — the selected execution mode with supporting data.
#[derive(Debug, Clone)]
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub signals: ComplexitySignals,
    pub confidence: f32,
    pub source: AnalysisSource,
    pub reasoning: String,
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
        }
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
    }

    #[test]
    fn failure_risk_ordering() {
        assert!(FailureRisk::Low < FailureRisk::Medium);
        assert!(FailureRisk::Medium < FailureRisk::High);
    }
}
