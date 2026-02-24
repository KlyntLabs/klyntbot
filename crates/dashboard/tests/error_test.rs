//! Integration tests for error handling middleware and ApiError type.
//!
//! Covers AC-5.3:
//! - ApiError serializes to {"status": N, "message": "..."}
//! - KlyntbotError variants convert to appropriate HTTP status codes
//! - All error responses have consistent JSON shape

mod common;

// ── ApiError shape ────────────────────────────────────────────────────────────

#[tokio::test]
async fn api_error_json_has_status_and_message_fields() {
    // AC-5.3: ApiError { status, message } serializes as {"status":404,"message":"not found"}
    // Tests the ApiError::into_response() implementation.
    // TODO: implement — construct ApiError, call into_response(), parse body
}

#[tokio::test]
async fn api_error_404_has_correct_http_status() {
    // AC-5.3: An ApiError with status=404 produces HTTP 404 response
    // TODO: implement
}

#[tokio::test]
async fn api_error_422_has_correct_http_status() {
    // AC-5.3: Validation errors return 422 Unprocessable Entity
    // TODO: implement
}

#[tokio::test]
async fn api_error_500_has_correct_http_status() {
    // AC-5.3: Internal errors return 500 Internal Server Error
    // TODO: implement
}

// ── KlyntbotError conversion ──────────────────────────────────────────────────

#[tokio::test]
async fn klyntbot_error_not_found_converts_to_404() {
    // AC-5.3: From<KlyntbotError> impl maps NotFound variant to 404
    // TODO: implement
}

#[tokio::test]
async fn klyntbot_error_storage_converts_to_500() {
    // AC-5.3: Storage errors map to 500 (don't leak internal error details)
    // TODO: implement
}

#[tokio::test]
async fn klyntbot_error_validation_converts_to_422() {
    // AC-5.3: Validation errors (e.g., bad request body) map to 422
    // TODO: implement
}

// ── Unhandled routes ──────────────────────────────────────────────────────────

#[tokio::test]
async fn unknown_api_route_returns_404_json() {
    // AC-5.3: GET /api/totally/unknown returns 404 with JSON body (not HTML)
    // TODO: implement
}

#[tokio::test]
async fn spa_fallback_returns_index_html_for_non_api_routes() {
    // AC-5.3: GET /some/spa/path returns the embedded index.html (SPA fallback)
    // All non-API routes should serve the React app for client-side routing.
    // TODO: implement
}
