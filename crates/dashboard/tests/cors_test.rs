//! Integration tests for CORS middleware.
//!
//! Covers AC-5.2: CORS headers present on all /api/* responses,
//! preflight OPTIONS request handled.

mod common;

#[tokio::test]
async fn cors_headers_present_on_api_response() {
    // AC-5.2: GET /api/health includes Access-Control-Allow-Origin header
    // Required for browser fetch from Vite dev server (different port) to work.
    // TODO: implement
}

#[tokio::test]
async fn cors_preflight_returns_204() {
    // AC-5.2: OPTIONS /api/tasks returns 204 with CORS preflight headers
    // Allows browser to proceed with actual POST/PATCH/DELETE requests
    // TODO: implement
}

#[tokio::test]
async fn cors_preflight_includes_allow_methods() {
    // AC-5.2: OPTIONS response includes Access-Control-Allow-Methods with GET, POST, PATCH, DELETE
    // TODO: implement
}

#[tokio::test]
async fn cors_preflight_includes_allow_headers() {
    // AC-5.2: OPTIONS response includes Access-Control-Allow-Headers: content-type, etc.
    // TODO: implement
}

#[tokio::test]
async fn cors_headers_present_on_ws_upgrade() {
    // AC-5.2: The /ws endpoint also responds with CORS headers during upgrade
    // TODO: implement
}
