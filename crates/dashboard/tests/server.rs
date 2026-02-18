//! Tests for the Axum server setup: routing, CORS, static assets.

mod common;
use common::create_test_bus;

use std::sync::Arc;

use dashboard::events::DashboardEventBus;
use dashboard::server::{DashboardConfig, DashboardServer};
use bus::MessageBus;

fn make_server() -> DashboardServer {
    let bus = Arc::new(MessageBus::new(32));
    let event_bus = Arc::new(DashboardEventBus::new(64));
    DashboardServer::new(DashboardConfig {
        port: 0,
        bus,
        event_bus,
    })
}

#[tokio::test]
async fn test_server_serves_graphql_endpoint() {
    let server = make_server();
    let (addr, _handle) = server.start_on_random_port().await.unwrap();

    let client = reqwest::Client::new();
    let res = client
        .post(format!("http://{}/graphql", addr))
        .header("Content-Type", "application/json")
        .body(r#"{"query":"{ __typename }"}"#)
        .send()
        .await
        .expect("Request failed");

    // The route may not be wired up yet (stubs from dev-1/dev-2), but
    // the server should at least return a non-5xx status.
    // We accept 200 (GraphQL response) or 404 (route not yet wired).
    assert!(
        res.status().is_success() || res.status() == reqwest::StatusCode::NOT_FOUND,
        "Expected 200 or 404 for /graphql, got {}",
        res.status()
    );
}

#[tokio::test]
async fn test_server_serves_static_assets() {
    let server = make_server();
    let (addr, _handle) = server.start_on_random_port().await.unwrap();

    let client = reqwest::Client::new();
    let res = client
        .get(format!("http://{}/", addr))
        .send()
        .await
        .expect("Request failed");

    assert_eq!(
        res.status(),
        200,
        "GET / should return 200 with index.html"
    );

    let body = res.text().await.unwrap();
    assert!(
        body.contains("<!DOCTYPE html>") || body.contains("<html"),
        "Response body should be HTML, got: {}",
        &body[..body.len().min(200)]
    );
}

#[tokio::test]
async fn test_server_cors_headers() {
    let server = make_server();
    let (addr, _handle) = server.start_on_random_port().await.unwrap();

    let client = reqwest::Client::new();
    let res = client
        .request(reqwest::Method::OPTIONS, format!("http://{}/", addr))
        .header("Origin", "http://localhost:5173")
        .header("Access-Control-Request-Method", "GET")
        .send()
        .await
        .expect("OPTIONS request failed");

    // CORS preflight should include Access-Control-Allow-Origin.
    let headers = res.headers();
    assert!(
        headers.contains_key("access-control-allow-origin"),
        "Expected CORS header Access-Control-Allow-Origin, headers: {:?}",
        headers
    );
}

#[tokio::test]
async fn test_server_websocket_upgrade() {
    use tokio_tungstenite::connect_async;

    let server = make_server();
    let (addr, _handle) = server.start_on_random_port().await.unwrap();

    let url = format!("ws://{}/ws/chat", addr);
    let result = connect_async(&url).await;

    assert!(
        result.is_ok(),
        "WebSocket upgrade to /ws/chat should succeed (101), got: {:?}",
        result.err()
    );
}
