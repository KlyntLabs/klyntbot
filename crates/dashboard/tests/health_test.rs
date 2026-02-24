//! Integration tests for GET /api/health.
//!
//! Covers:
//! - AC-5.1: Health endpoint returns 200 with {"status":"ok"}
//! - AC-4.1: Default gateway host is 127.0.0.1 (tested via config fixture)

mod common;

#[tokio::test]
async fn health_returns_200_ok() {
    // AC-5.1: GET /api/health responds with HTTP 200
    // TODO: implement
    // let app = common::build_test_app().await;
    // let response = app.oneshot(Request::get("/api/health").body(Body::empty()).unwrap()).await.unwrap();
    // assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn health_response_body_is_status_ok() {
    // AC-5.1: Response body is JSON {"status":"ok"}
    // TODO: implement
    // Parse body as serde_json::Value, assert body["status"] == "ok"
}

#[tokio::test]
async fn health_response_content_type_is_json() {
    // AC-5.1: Content-Type header is application/json
    // TODO: implement
}

#[tokio::test]
async fn default_gateway_host_is_localhost() {
    // AC-4.1: GatewayConfig default host is "127.0.0.1" (not "0.0.0.0")
    // Protects against accidental exposure on all interfaces.
    // TODO: implement
    // let config = config::GatewayConfig::default();
    // assert_eq!(config.host, "127.0.0.1");
}
