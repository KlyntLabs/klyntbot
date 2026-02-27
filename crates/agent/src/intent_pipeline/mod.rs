//! Intent Pipeline — unified intent analysis and execution routing.
//!
//! Replaces the Orchestrator + EngineDispatch + AgentPipeline with a single
//! pipeline that auto-decides Direct vs Reactive vs Planned execution based
//! on structured complexity analysis.

pub mod analyzer;
pub mod classifier;
pub mod heuristics;
pub mod types;

pub use types::{
    AnalysisSource, ComplexitySignals, ExecutionMode, FailureRisk, IntentAnalysis,
};
