//! PlanExecutor — ReAct loop, backtracking, and context windowing for plans.

use plan::Plan;

/// PlanExecutor handles step-by-step plan execution with backtracking.
pub struct PlanExecutor {
    // Fields will be added in Phase 4 (Agent Integration)
    // provider: DynProvider,
    // tool_registry: Arc<RwLock<ToolRegistry>>,
}

impl PlanExecutor {
    /// Create a new PlanExecutor.
    /// Full implementation will be added in Phase 4.
    pub fn new() -> Self {
        Self {}
    }

    /// Build context window: current step + next 3 steps.
    /// Returns formatted context string for LLM.
    pub fn build_step_context(&self, plan: &Plan, current_index: usize) -> String {
        let window_end = (current_index + 4).min(plan.steps.len());

        if current_index >= plan.steps.len() {
            return "Plan completed - no active step".to_string();
        }

        let steps_window = &plan.steps[current_index..window_end];

        let mut ctx = format!("## Active Plan: {}\n", plan.title);
        ctx.push_str(&format!(
            "Progress: step {}/{}\n\n",
            current_index + 1,
            plan.steps.len()
        ));

        for (i, step) in steps_window.iter().enumerate() {
            let marker = if i == 0 {
                ">>> CURRENT"
            } else {
                match i {
                    1 => "    NEXT 1",
                    2 => "    NEXT 2",
                    3 => "    NEXT 3",
                    _ => "    NEXT",
                }
            };
            ctx.push_str(&format!(
                "{}: {}\n  Reasoning: {}\n",
                marker, step.description, step.reasoning
            ));
        }
        ctx
    }
}

impl Default for PlanExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use plan::{PlanStatus, PlanStep, StepStatus};
    use uuid::Uuid;

    /// Helper to create a test Plan with N steps
    fn test_plan_with_steps(step_count: usize, session_key: &str) -> Plan {
        let now = Utc::now();
        Plan {
            id: Uuid::new_v4(),
            session_key: session_key.to_string(),
            goal_id: None,
            title: "Test Plan".to_string(),
            description: "Test".to_string(),
            status: PlanStatus::Approved,
            steps: (0..step_count)
                .map(|i| PlanStep {
                    id: Uuid::new_v4(),
                    index: i,
                    description: format!("Step {}", i),
                    reasoning: format!("Reasoning {}", i),
                    expected_tools: vec![],
                    status: StepStatus::Pending,
                    attempt_count: 0,
                    max_attempts: 3,
                    result: None,
                    started_at: None,
                    completed_at: None,
                })
                .collect(),
            current_step_index: 0,
            iteration_limit: 50,
            backtrack_history: vec![],
            created_at: now,
            updated_at: now,
            completed_at: None,
        }
    }

    #[test]
    fn test_step_context_window_builds_correctly() {
        // Test 17
        // Given: a Plan with 10 steps, current_step_index = 2
        // When: build_step_context() is called
        // Then:
        //   - context includes steps 2, 3, 4, 5 (current + next 3)
        //   - current step is marked ">>> CURRENT"
        //   - next steps are marked "    NEXT 1", "    NEXT 2", "    NEXT 3"
        // Maps to: US-3 (AC-3.2)

        let mut plan = test_plan_with_steps(10, "test-session");
        plan.current_step_index = 2;

        let executor = PlanExecutor::new();
        let context = executor.build_step_context(&plan, 2);

        assert!(context.contains(">>> CURRENT: Step 2"));
        assert!(context.contains("    NEXT 1: Step 3"));
        assert!(context.contains("    NEXT 2: Step 4"));
        assert!(context.contains("    NEXT 3: Step 5"));
        assert!(!context.contains("Step 6")); // Outside window
        assert!(!context.contains("Step 1")); // Before window
    }

    #[test]
    fn test_step_context_window_at_end() {
        // Edge case: current step is near the end
        let mut plan = test_plan_with_steps(5, "test-session");
        plan.current_step_index = 3; // Only 2 steps left (3 and 4)

        let executor = PlanExecutor::new();
        let context = executor.build_step_context(&plan, 3);

        assert!(context.contains(">>> CURRENT: Step 3"));
        assert!(context.contains("    NEXT 1: Step 4"));
        assert!(!context.contains("    NEXT 2")); // No step 5
        assert!(!context.contains("    NEXT 3")); // No step 6
    }

    #[test]
    fn test_step_context_window_at_last_step() {
        // Edge case: current step is the last step
        let mut plan = test_plan_with_steps(3, "test-session");
        plan.current_step_index = 2; // Last step

        let executor = PlanExecutor::new();
        let context = executor.build_step_context(&plan, 2);

        assert!(context.contains(">>> CURRENT: Step 2"));
        assert!(!context.contains("    NEXT 1")); // No next steps
    }

    #[test]
    fn test_step_context_window_completed() {
        // Edge case: plan completed (index beyond steps)
        let plan = test_plan_with_steps(3, "test-session");

        let executor = PlanExecutor::new();
        let context = executor.build_step_context(&plan, 5); // Beyond steps

        assert_eq!(context, "Plan completed - no active step");
    }

    // NOTE: Tests 18-19 (backtracking) will be implemented in Phase 4 (Agent Integration)
    // as they require full agent loop and tool execution context.

    // NOTE: Test 20 (iteration limit switching) belongs in agent_loop.rs tests,
    // not in plan_executor.rs.
}
