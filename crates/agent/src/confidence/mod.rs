//! Confidence-driven decision engine.
//!
//! Evaluates LLM confidence in understanding user intent before tool execution.
//! When confidence is low, auto-triggers clarification via `ask_user`.

pub mod evaluator;
pub mod log;
pub mod prompt;
pub mod types;

pub use evaluator::ConfidenceEvaluator;
pub use log::DecisionLogger;
pub use types::{AssessmentPhase, ConfidenceAssessment, DecisionAction};
