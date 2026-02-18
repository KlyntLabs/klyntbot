//! Unit tests for GoalRepo.

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn add_and_get() {
        // Given: empty DB
        // When: add goal with title, metrics JSONB, priority
        // Then: get(uuid) returns goal with all fields
    }

    #[tokio::test]
    async fn update_status() {
        // Given: active goal
        // When: update status to "completed"
        // Then: get returns updated status, updated_at changed
    }

    #[tokio::test]
    async fn delete_goal() {
        // Given: existing goal
        // When: delete(uuid)
        // Then: get returns None
        // Also: goal_project_links rows for this goal removed (CASCADE)
    }

    #[tokio::test]
    async fn list_by_status() {
        // Given: 2 active goals, 1 completed
        // When: list(status=Active)
        // Then: returns 2 goals
    }

    #[tokio::test]
    async fn link_and_unlink_project() {
        // Given: goal and project
        // When: link_project(goal_id, project_id)
        // Then: goal_project_links has row
        // When: unlink_project(goal_id, project_id)
        // Then: row removed
    }

    #[tokio::test]
    async fn metrics_jsonb_roundtrip() {
        // Given: goal with complex metrics: {"tasks_completed": 5, "progress": 0.5}
        // When: insert and retrieve
        // Then: JSONB deserializes to identical serde_json::Value
    }
}
