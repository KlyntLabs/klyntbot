//! Execution engine — drives LLM-tool cycles.

pub mod core;
pub mod direct;
pub mod dispatch;
pub mod plan_execute;
pub mod react_plus;
pub mod scratchpad;
pub mod types;

pub use core::ExecutionCore;
pub use direct::{DirectEngine, DirectOutcome};
pub use dispatch::{DispatchResult, EngineDispatch};
pub use plan_execute::{PlanExecuteEngine, PlanExecuteOutcome};
pub use react_plus::{ReactOutcome, ReactPlusEngine, ReflectionMode};
pub use scratchpad::{ReasoningTrace, Scratchpad};
pub use types::{accumulate_usage, CycleOutcome, ExecutionParams, ToolExecutionResult};
