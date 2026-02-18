//! Unit tests for PlanRepo.

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn upsert_and_get() {
        // Given: empty DB
        // When: upsert plan with title, session_key
        // Then: get(uuid) returns matching plan
    }

    #[tokio::test]
    async fn upsert_updates_existing() {
        // Given: existing plan
        // When: upsert with same ID, different title
        // Then: title updated, created_at unchanged
    }

    #[tokio::test]
    async fn get_active_plan_for_session() {
        // Given: session with Draft plan and Completed plan
        // When: get_active_plan(session_key)
        // Then: returns Draft plan (active = Draft|Approved|Executing)
    }

    #[tokio::test]
    async fn list_by_status() {
        // Given: 2 Executing plans, 1 Draft
        // When: list_by_status(Executing)
        // Then: returns 2 plans
    }

    #[tokio::test]
    async fn add_step() {
        // Given: existing plan
        // When: add_step(plan_id, index=0, description="Run tests")
        // Then: plan_steps row with correct FK and index
    }

    #[tokio::test]
    async fn update_step_status() {
        // Given: plan step in "pending"
        // When: update step to "completed" with result="All tests passed"
        // Then: step has updated status and result
    }

    #[tokio::test]
    async fn delete_cascades_steps() {
        // Given: plan with 3 steps
        // When: delete(plan_id)
        // Then: plan and all steps removed
    }

    #[tokio::test]
    async fn backtrack_history_jsonb_roundtrip() {
        // Given: plan with backtrack_history = [{step: 2, reason: "timeout"}]
        // When: insert and retrieve
        // Then: JSONB roundtrips correctly
    }

    #[tokio::test]
    async fn steps_returned_in_order() {
        // Given: plan with steps at indexes 2, 0, 1 (inserted out of order)
        // When: get_steps(plan_id)
        // Then: returned as [0, 1, 2] (ORDER BY step_index)
    }
}
