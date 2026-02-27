//! Intent Pipeline — unified intent analysis and execution routing.
//!
//! Replaces the Orchestrator + EngineDispatch + AgentPipeline with a single
//! pipeline that auto-decides Direct vs Reactive vs Planned execution based
//! on structured complexity analysis.

pub mod analyzer;
pub mod classifier;
pub mod engines;
pub mod escalation;
pub mod heuristics;
pub mod pipeline;
pub mod router;
pub mod types;

pub use pipeline::IntentPipeline;
pub use types::{AnalysisSource, ComplexitySignals, ExecutionMode, FailureRisk, IntentAnalysis};

/// Format strategy summaries into a human-readable context for the LLM classifier.
pub(crate) fn format_strategy_context(summaries: &[storage::StrategySummaryRow]) -> String {
    let mut ctx = String::from("Historical strategy performance (last 30 days):\n");
    for s in summaries {
        let accuracy = if s.sample_count > 0 {
            s.correct_count as f32 / s.sample_count as f32 * 100.0
        } else {
            0.0
        };
        use std::fmt::Write;
        let _ = writeln!(
            ctx,
            "- {}: {:.0}% accuracy ({} samples), avg {:.1} escalations",
            s.predicted_strategy, accuracy, s.sample_count, s.avg_escalations
        );
    }
    ctx.push_str("Prefer strategies with higher historical accuracy when confidence is similar.");
    ctx
}
