//! Execution engines for different agent strategies.
//!
//! Each engine implements a distinct execution pattern:
//! - `direct` — single LLM call, no tool use (Q&A, explanations)
//! - `react` — ReAct+ loop with tool dispatch (TODO: Task 5.2)
//! - `plan` — plan-then-execute with backtracking (TODO: Task 5.3)

pub mod direct;
pub mod plan;
pub mod plan_execute;
pub mod react;

pub use plan_execute::{PlanExecuteEngine, PlanExecuteOutcome, PlanStep};
pub use react::{ReactEngine, ReactResult};
