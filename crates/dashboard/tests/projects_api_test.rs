//! Integration tests for /api/projects endpoints.
//!
//! Covers AC-7.x: Full CRUD for projects, with-stats query.

mod common;

#[tokio::test]
async fn get_projects_empty_returns_empty_array() {
    // AC-7.1: GET /api/projects on fresh DB returns []
    // TODO: implement
}

#[tokio::test]
async fn post_project_creates_project_returns_201() {
    // AC-7.2: POST /api/projects with required fields → 201 with created ProjectRow
    // TODO: implement
}

#[tokio::test]
async fn get_project_by_id_returns_project_row() {
    // AC-7.3: GET /api/projects/{id} returns full ProjectRow
    // TODO: implement
}

#[tokio::test]
async fn get_project_by_id_with_stats() {
    // AC-7.3: GET /api/projects/{id}?withStats=true returns ProjectWithStats
    // (includes task counts and other aggregate stats)
    // TODO: implement
}

#[tokio::test]
async fn get_project_by_id_not_found_returns_404() {
    // AC-7.3: GET /api/projects/nonexistent → 404
    // TODO: implement
}

#[tokio::test]
async fn patch_project_updates_fields() {
    // AC-7.4: PATCH /api/projects/{id} with changed fields → 200
    // Verify partial update doesn't clobber other fields.
    // TODO: implement
}

#[tokio::test]
async fn delete_project_returns_204() {
    // AC-7.5: DELETE /api/projects/{id} → 204 No Content
    // TODO: implement
}

#[tokio::test]
async fn get_projects_filter_by_status() {
    // AC-7.1: GET /api/projects?status=active filters results
    // TODO: implement
}

#[tokio::test]
async fn get_projects_filter_by_tags() {
    // AC-7.1: GET /api/projects?tags=work,personal filters results
    // TODO: implement
}

#[tokio::test]
async fn project_response_fields_are_camel_case() {
    // AC-1.3: All project response fields are camelCase
    // Verify: createdAt, updatedAt, completedAt
    // TODO: implement
}
