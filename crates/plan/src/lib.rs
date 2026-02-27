//! Planning engine for structured multi-step ReAct-style execution.
//!
//! This crate provides types and conversions for agent plans:
//! - **Plan**: Multi-step execution plan with lifecycle (Draft → Approved → Executing → Completed/Failed)
//! - **conversions**: Domain ↔ SQL row helpers for direct PlanRepo usage
//!
//! Plans support:
//! - Session isolation (no cross-session visibility)
//! - Optional goal linkage via `goal_id: Option<Uuid>`
//! - Auto-backtracking with configurable retry limits (default 3 attempts per step)
//! - Step-by-step context windowing (current step + next 3)

pub mod conversions;
pub mod error;
pub mod types;

// Re-export commonly used types
pub use error::PlanError;
pub use types::{BacktrackEntry, Plan, PlanStatus, PlanStep, PlanVisibility, StepStatus};
