//! Integration tests for /api/cron endpoints.
//!
//! Covers AC-11.x: List cron jobs, create, toggle enable/disable, delete.

mod common;

#[tokio::test]
async fn get_cron_empty_returns_empty_array() {
    // AC-11.1: GET /api/cron on fresh DB returns []
    // TODO: implement
}

#[tokio::test]
async fn get_cron_returns_cron_job_rows() {
    // AC-11.1: GET /api/cron returns Vec<CronJob>
    // TODO: implement
}

#[tokio::test]
async fn post_cron_creates_job_returns_201() {
    // AC-11.2: POST /api/cron with schedule + payload → 201
    // TODO: implement
}

#[tokio::test]
async fn post_cron_invalid_schedule_returns_422() {
    // AC-11.2: Invalid cron expression → 422
    // TODO: implement
}

#[tokio::test]
async fn patch_cron_toggle_enables_job() {
    // AC-11.3: PATCH /api/cron/{id}/toggle { "enabled": true } → 200
    // Verify job.enabled is now true
    // TODO: implement
}

#[tokio::test]
async fn patch_cron_toggle_disables_job() {
    // AC-11.3: PATCH /api/cron/{id}/toggle { "enabled": false } → 200
    // Verify job.enabled is now false
    // TODO: implement
}

#[tokio::test]
async fn delete_cron_job_returns_204() {
    // AC-11.4: DELETE /api/cron/{id} → 204 No Content
    // TODO: implement
}

#[tokio::test]
async fn delete_cron_job_not_found_returns_404() {
    // AC-11.4: DELETE nonexistent cron job → 404
    // TODO: implement
}

#[tokio::test]
async fn cron_response_fields_are_camel_case() {
    // AC-1.7: CronJob response fields use camelCase (nextRunAtMs, lastRunAtMs, deleteAfterRun)
    // TODO: implement
}
