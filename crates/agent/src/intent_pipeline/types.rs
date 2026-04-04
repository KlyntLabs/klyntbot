//! Core types for the intent pipeline: complexity levels, signals, intent analysis.

use context_engine::ExecutionStrategy;
use serde::{Deserialize, Serialize};

/// Complexity assessment from heuristic analysis.
/// The execute loop uses this to tune its behavior but does NOT
/// pre-select a mode — the model self-selects via tool_use.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplexityLevel {
    /// Simple query, likely no tools needed.
    Simple,
    /// Moderate complexity, likely 1-3 tool calls.
    Moderate,
    /// High complexity, likely 4+ tool calls or sequential dependencies.
    Complex,
}

impl ComplexityLevel {
    /// Short lowercase name for logging/strategy records.
    pub fn short_name(&self) -> &'static str {
        match self {
            Self::Simple => "simple",
            Self::Moderate => "moderate",
            Self::Complex => "complex",
        }
    }
}

impl std::fmt::Display for ComplexityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Simple => write!(f, "Simple"),
            Self::Moderate => write!(f, "Moderate"),
            Self::Complex => write!(f, "Complex"),
        }
    }
}

/// Bridge: `ComplexityLevel` → `ExecutionStrategy` for the `ContextEngine`.
///
/// The `ContextEngine` allocates token budgets based on `ExecutionStrategy`.
/// Simple maps to DirectResponse; Moderate/Complex to ToolAssisted with
/// a default iteration budget derived from the complexity signals.
impl From<&ComplexityLevel> for ExecutionStrategy {
    fn from(level: &ComplexityLevel) -> Self {
        match level {
            ComplexityLevel::Simple => ExecutionStrategy::DirectResponse,
            ComplexityLevel::Moderate => ExecutionStrategy::ToolAssisted { max_iterations: 15 },
            ComplexityLevel::Complex => ExecutionStrategy::ToolAssisted { max_iterations: 30 },
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
    Embedding,
    MidExecutionEscalation,
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
    pub safety_timeout_secs: u64,
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
            safety_timeout_secs: 300,
        }
    }
}

/// Result of intent analysis — the assessed complexity with supporting data.
#[derive(Debug, Clone)]
pub struct IntentAnalysis {
    pub complexity: ComplexityLevel,
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
            complexity: ComplexityLevel::Moderate,
            signals,
            confidence: 0.5,
            source: AnalysisSource::Heuristic,
            reasoning: "Fallback — classification unavailable".to_string(),
            needs_orchestration: false,
        }
    }

    /// Compute the complexity level from a complexity score.
    pub fn complexity_from_score(score: u8) -> ComplexityLevel {
        match score {
            0..=1 => ComplexityLevel::Simple,
            2..=3 => ComplexityLevel::Moderate,
            _ => ComplexityLevel::Complex,
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
    fn fallback_analysis_is_moderate() {
        let analysis = IntentAnalysis::fallback();
        assert_eq!(analysis.complexity, ComplexityLevel::Moderate);
        assert_eq!(analysis.confidence, 0.5);
    }

    #[test]
    fn complexity_from_score_mapping() {
        assert_eq!(
            IntentAnalysis::complexity_from_score(0),
            ComplexityLevel::Simple
        );
        assert_eq!(
            IntentAnalysis::complexity_from_score(1),
            ComplexityLevel::Simple
        );
        assert_eq!(
            IntentAnalysis::complexity_from_score(2),
            ComplexityLevel::Moderate
        );
        assert_eq!(
            IntentAnalysis::complexity_from_score(3),
            ComplexityLevel::Moderate
        );
        assert_eq!(
            IntentAnalysis::complexity_from_score(4),
            ComplexityLevel::Complex
        );
        assert_eq!(
            IntentAnalysis::complexity_from_score(7),
            ComplexityLevel::Complex
        );
    }

    #[test]
    fn failure_risk_ordering() {
        assert!(FailureRisk::Low < FailureRisk::Medium);
        assert!(FailureRisk::Medium < FailureRisk::High);
    }
}
