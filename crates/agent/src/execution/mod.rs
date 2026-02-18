//! Execution engine — drives LLM-tool cycles.

pub mod core;
pub mod scratchpad;
pub mod types;

pub use core::ExecutionCore;
pub use scratchpad::{ReasoningTrace, Scratchpad};
pub use types::{CycleOutcome, ExecutionParams, ToolExecutionResult};
