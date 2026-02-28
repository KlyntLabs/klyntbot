//! Domain crate — goal and plan types, errors, and conversions.

pub mod goal;
pub mod plan;

// Re-export key types at crate root for convenience
pub use goal::{Goal, GoalError, GoalProgress, GoalStatus};
pub use plan::{
    BacktrackEntry, Plan, PlanError, PlanStatus, PlanStep, PlanVisibility, StepStatus,
    DEFAULT_MAX_STEP_ATTEMPTS,
};
