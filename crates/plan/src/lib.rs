//! Planning engine for structured multi-step ReAct-style execution.
//!
//! This crate provides types and storage for agent plans:
//! - **Plan**: Multi-step execution plan with lifecycle (Draft → Approved → Executing → Completed/Failed)
//! - **PlanStore**: Append-only JSONL persistence (mirrors GoalStore pattern)
//!
//! Plans support:
//! - Session isolation (no cross-session visibility)
//! - Optional goal linkage via `goal_id: Option<Uuid>`
//! - Auto-backtracking with configurable retry limits (default 3 attempts per step)
//! - Step-by-step context windowing (current step + next 3)

pub mod store;
pub mod types;

// Re-export commonly used types
pub use store::PlanStore;
pub use types::{BacktrackEntry, Plan, PlanStatus, PlanStep, StepStatus};
