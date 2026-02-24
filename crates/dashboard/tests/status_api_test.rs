//! Integration tests for GET /api/status.
//!
//! Covers AC-10.x: Version, model name, uptime, storage stats.

mod common;

#[tokio::test]
async fn get_status_returns_200() {
    // AC-10.1: GET /api/status responds with HTTP 200
    // TODO: implement
}

#[tokio::test]
async fn get_status_includes_version_field() {
    // AC-10.1: Response includes a "version" string (e.g., "0.3.0")
    // TODO: implement
}

#[tokio::test]
async fn get_status_includes_model_name() {
    // AC-10.1, UX §8: Response includes "model" field — shown in StatusBar
    // TODO: implement
}

#[tokio::test]
async fn get_status_includes_uptime_seconds() {
    // AC-10.1, UX §8: Response includes "uptimeSeconds" (camelCase) — shown in StatusBar
    // Uptime must be >= 0
    // TODO: implement
}

#[tokio::test]
async fn get_status_includes_storage_stats() {
    // AC-10.1: Response includes storage stats (e.g., task count, session count)
    // TODO: implement
}

#[tokio::test]
async fn status_response_fields_are_camel_case() {
    // AC-10.1: All response fields use camelCase (uptimeSeconds, modelName, etc.)
    // TODO: implement
}
