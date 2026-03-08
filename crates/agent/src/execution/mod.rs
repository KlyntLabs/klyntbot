//! Execution engine — drives LLM-tool cycles.

pub mod core;
pub mod scratchpad;
pub mod types;

pub use core::ExecutionCore;
pub use scratchpad::{ExecutionPlan, PlanStep, ReasoningTrace, Scratchpad};
pub use types::{accumulate_usage, CycleOutcome, ExecutionParams, ToolExecutionResult};
