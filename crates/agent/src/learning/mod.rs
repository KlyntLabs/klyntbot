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
pub mod interaction_recorder;
pub mod pattern_analyzer;
pub mod recorder;
pub mod service;
pub mod tool_tracking;
pub mod types;

pub use interaction_recorder::InteractionRecorder;
pub use pattern_analyzer::PatternAnalyzer;
pub use recorder::OutcomeStore;
pub use service::LearningService;
pub use tool_tracking::{compute_stats, StrategyRecord, StrategyStats, ToolConfidenceMap};
pub use types::*;
