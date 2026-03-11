//! Day planning handler trait.
//!
//! Defined here (Layer 4) for dependency inversion. The implementation
//! lives in the agent crate (Layer 5) and uses LLM calls to generate
//! optimized day plans based on task priorities, energy levels, and calendar.

use async_trait::async_trait;
use common::Result;

use crate::types::{DayPlan, PlanningContext};

/// Handler for generating and adjusting day plans.
///
/// Analyzes available tasks, calendar blocks, energy profiles, and
/// working hours to produce an optimized schedule for the day.
#[async_trait]
pub trait DayPlanningHandler: Send + Sync {
    /// Generate a day plan from the given context.
    ///
    /// The handler should:
    /// - Score and rank available tasks
    /// - Fit tasks into available time slots (respecting calendar blocks)
    /// - Match task energy levels to user energy profile
    /// - Respect WIP limits and focus slot constraints
    /// - Provide reasoning for the plan
    async fn plan_day(&self, context: &PlanningContext) -> Result<DayPlan>;

    /// Adjust an existing plan based on changed circumstances.
    ///
    /// Called when the user completes a task, a meeting runs long,
    /// or priorities change mid-day.
    async fn replan(
        &self,
        context: &PlanningContext,
        current_plan: &DayPlan,
        reason: &str,
    ) -> Result<DayPlan>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn test_day_planning_handler_is_object_safe() {
        fn _check(_: Arc<dyn DayPlanningHandler>) {}
    }
}
