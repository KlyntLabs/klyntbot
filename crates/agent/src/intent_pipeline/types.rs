//! Core types for the intent pipeline: execution modes, complexity signals, intent analysis.

use context_engine::ExecutionStrategy;
use serde::{Deserialize, Serialize};

/// The two execution modes the pipeline can select.
#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionMode {
    /// Single LLM call, no tools. Greetings, factual Q&A.
    Direct,
    /// ReAct loop with tools. Single-shot tasks, searches, CRUD.
    Reactive { max_iterations: u32 },
}

impl ExecutionMode {
    /// Short lowercase name for logging/strategy records.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::Reactive { .. } => "reactive",
        }
    }

    /// Maximum iterations/steps for this mode.
    pub fn max_iterations(&self) -> u32 {
        match self {
            Self::Direct => 1,
            Self::Reactive { max_iterations } => *max_iterations,
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
    /// Whether the message contains hypothetical framing ("what if", "suppose", etc.)
    pub has_hypothetical: bool,
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

    /// Compute a dynamic iteration budget for the reactive engine.
    ///
    /// Formula: `min(max(estimated_tool_calls * 3, 10) + 5, 30)`
    /// - `* 3`: headroom per tool (call + reflection + planning)
    /// - Floor of 10: even simple requests get enough room
    /// - `+ 5` buffer: synthesis and unexpected detours
    /// - Ceiling of 30: safety net against bad estimates
    pub fn iteration_budget(&self) -> u32 {
        let base = (self.estimated_tool_calls as u32 * 3).max(10);
        (base + 5).min(30)
    }
}

/// How the intent analysis was produced.
#[derive(Debug, Clone, PartialEq)]
pub enum AnalysisSource {
    Heuristic,
    LlmClassifier,
    MidExecutionEscalation,
    /// Shadow mode stopped the cascade before Layer 3 (LLM).
    ShadowDeferred,
}

/// Configuration for the execution pipeline (used by `AgentRuntime`).
pub struct PipelineConfig {
    /// Model name for execution.
    pub execution_model: String,
    /// System prompt to prepend.
    pub system_prompt: String,
    /// Context window size in tokens.
    pub context_window: usize,
    /// Maximum response tokens for validation.
    pub max_response_tokens: usize,
    /// Default channel name (overridden per-request by RoutingContext).
    pub channel: String,
    /// Provider name for cost tracking.
    pub provider_name: String,
    /// Max graph depth for scenario reasoning (from Config.scenario.max_graph_depth).
    pub scenario_max_graph_depth: u32,
    /// Maximum wall-clock time for a single pipeline execution (seconds).
    /// 0 means no timeout. Default: 300.
    pub pipeline_timeout_secs: u64,
}

impl Default for PipelineConfig {
    fn default() -> Self {
        Self {
            execution_model: "claude-sonnet-4-20250514".to_string(),
            system_prompt: "You are a helpful assistant.".to_string(),
            context_window: 128_000,
            max_response_tokens: 4096,
            channel: "unknown".to_string(),
            provider_name: "unknown".to_string(),
            scenario_max_graph_depth: 2,
            pipeline_timeout_secs: 300,
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
    /// Whether this message requires orchestration across multiple agents.
    pub needs_orchestration: bool,
}

impl IntentAnalysis {
    /// Fallback analysis when classification fails entirely.
    pub fn fallback() -> Self {
        let signals = ComplexitySignals {
            estimated_tool_calls: 1,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
            has_hypothetical: false,
        };
        Self {
            mode: ExecutionMode::Reactive {
                max_iterations: signals.iteration_budget(),
            },
            signals,
            confidence: 0.5,
            source: AnalysisSource::Heuristic,
            reasoning: "Fallback — classification unavailable".to_string(),
            needs_orchestration: false,
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
            has_hypothetical: false,
        };
        assert_eq!(signals.complexity_score(), 0);
    }

    #[test]
    fn high_complexity_scores_7() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 4,
            has_sequential_deps: true,
            failure_risk: FailureRisk::High,
            requires_state_tracking: true,
            requires_retries: true,
            has_hypothetical: false,
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
            has_hypothetical: false,
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
            has_hypothetical: false,
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
            has_hypothetical: false,
        };
        assert_eq!(signals.complexity_score(), 1);
    }

    #[test]
    fn fallback_analysis_is_reactive() {
        let analysis = IntentAnalysis::fallback();
        assert!(matches!(
            analysis.mode,
            ExecutionMode::Reactive { max_iterations: 15 }
        ));
        assert_eq!(analysis.confidence, 0.5);
    }

    #[test]
    fn failure_risk_ordering() {
        assert!(FailureRisk::Low < FailureRisk::Medium);
        assert!(FailureRisk::Medium < FailureRisk::High);
    }
}
