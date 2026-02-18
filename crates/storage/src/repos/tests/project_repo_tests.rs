//! Unit tests for ProjectRepo.

#[cfg(test)]
mod tests {
    // TODO: use super::super::fixtures::*;

    #[tokio::test]
    async fn add_and_get() {
        // Given: empty DB
        // When: add project "Klyntbot v2" with tags ["rust", "ai"]
        // Then: get(id) returns project with matching fields
    }

    #[tokio::test]
    async fn update_status_and_color() {
        // Given: active project
        // When: update status to "paused", color to "#e74c3c"
        // Then: get returns updated fields
    }

    #[tokio::test]
    async fn delete_project() {
        // Given: existing project
        // When: delete(id)
        // Then: get returns None
        // Note: todos with this project_id should have project_id set to NULL
    }

    #[tokio::test]
    async fn list_filters_by_status() {
        // Given: 2 active, 1 archived project
        // When: list(status="active")
        // Then: returns 2 projects
    }

    #[tokio::test]
    async fn list_filters_by_tag() {
        // Given: projects with different tags
        // When: list(tags=["rust"])
        // Then: returns only projects with "rust" tag (GIN @>)
    }

    #[tokio::test]
    async fn all_returns_everything() {
        // Given: 5 projects in mixed statuses
        // When: all()
        // Then: returns all 5
    }
}
