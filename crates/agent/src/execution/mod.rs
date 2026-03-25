//! Execution engine — drives LLM-tool cycles.

pub mod core;
pub mod live_context_refresher;
pub mod mid_loop_compressor;
pub mod scratchpad;
pub mod types;

pub use core::ExecutionCore;
pub use live_context_refresher::{ContextReassembledUpdate, LiveContextRefresher};
pub use mid_loop_compressor::MidLoopCompressor;
pub use scratchpad::{ExecutionPlan, PlanStep, ReasoningTrace, Scratchpad};
pub use types::{accumulate_usage, CycleOutcome, ExecutionParams, ToolExecutionResult};
