//! Intent Pipeline — unified intent analysis and execution routing.
//!
//! Replaces the Orchestrator + EngineDispatch + AgentPipeline with a single
//! pipeline that auto-decides Direct vs Reactive vs Planned execution based
//! on structured complexity analysis.

pub mod analysis;
pub mod engines;
pub mod types;

pub use types::{
    AnalysisSource, ComplexityLevel, ComplexitySignals, FailureRisk, IntentAnalysis, PipelineConfig,
};
