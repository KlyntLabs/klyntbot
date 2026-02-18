//! Learning system — records tool outcomes, analyzes performance,
//! and adapts confidence thresholds based on real-world results.
//!
//! ## Privacy
//!
//! Privacy-by-omission: `OutcomeRecord` does NOT store tool arguments
//! or user messages. Only tool name, success, duration, and confidence
//! score are persisted.

pub mod adaptive;
pub mod analyzer;
pub mod outcome_store;
pub mod recorder;
pub mod service;
pub mod strategy_store;
pub mod strategy_tracker;
pub mod tool_confidence;
pub mod types;

pub use outcome_store::OutcomeStore;
pub use recorder::OutcomeRecorder;
pub use service::LearningService;
pub use strategy_store::{StrategyLearningStore, StrategyRecord};
pub use strategy_tracker::{compute_stats, StrategyStats};
pub use tool_confidence::ToolConfidenceMap;
pub use types::*;
