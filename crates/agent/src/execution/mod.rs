//! Execution engine — drives LLM-tool cycles.

pub mod core;
pub mod types;

pub use core::ExecutionCore;
pub use types::{CycleOutcome, ExecutionParams, ToolExecutionResult};
