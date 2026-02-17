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
pub mod types;

pub use outcome_store::OutcomeStore;
pub use recorder::OutcomeRecorder;
pub use service::LearningService;
pub use types::*;
