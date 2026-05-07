//! Execution engine — drives LLM-tool cycles.

pub mod budget;
pub mod cache_policy;
pub mod core;
pub mod execute_loop;
pub mod live_context_refresher;
pub mod loop_detector;
pub mod mid_loop_compressor;
pub mod scratchpad;
pub mod types;

pub use budget::{DepthMode, SafetyCap, SkillBudget};
pub use cache_policy::compression_aware_default;
pub use core::ExecutionCore;
pub use execute_loop::{execute_loop, ExecuteLoopResult};
pub use live_context_refresher::{ContextReassembledUpdate, LiveContextRefresher};
pub use loop_detector::{LoopDetector, LoopStatus};
pub use mid_loop_compressor::MidLoopCompressor;
pub use scratchpad::{ExecutionPlan, PlanStep, ReasoningTrace, Scratchpad};
pub use types::{accumulate_usage, CycleOutcome, ExecutionParams, ToolExecutionResult};
