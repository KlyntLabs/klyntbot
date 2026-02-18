//! Axum HTTP server — routing, CORS, static file serving.
//!
//! Routes:
//!   GET  /                → index.html (or embedded assets)
//!   GET  /assets/*        → static assets
//!   POST /graphql         → GraphQL endpoint (reserved for dev-1/dev-2)
//!   GET  /graphql         → GraphQL playground
//!   GET  /ws/chat         → WebSocket upgrade for chat protocol
//!   GET  /health          → { "status": "ok" }

use std::net::SocketAddr;
use std::sync::Arc;

use axum::body::Body;
use axum::http::{header, Response, StatusCode};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::json;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use bus::MessageBus;

use crate::events::DashboardEventBus;
use crate::ws::chat::{ws_chat_handler, ChatState};

/// Configuration for the dashboard HTTP server.
pub struct DashboardConfig {
    pub port: u16,
    pub bus: Arc<MessageBus>,
    pub event_bus: Arc<DashboardEventBus>,
}

/// The dashboard HTTP server.
pub struct DashboardServer {
    config: DashboardConfig,
}

impl DashboardServer {
    pub fn new(config: DashboardConfig) -> Self {
        Self { config }
    }

    /// Build the axum router (useful for testing without binding a port).
    pub fn build_router(&self) -> Router {
        let chat_state = ChatState {
            bus: self.config.bus.clone(),
            event_bus: self.config.event_bus.clone(),
        };

        // CORS: allow all origins for localhost-only use.
        let cors = CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any);

        Router::new()
            .route("/health", get(health_handler))
            .route("/ws/chat", get(ws_chat_handler))
            .route("/", get(index_handler))
            .route("/assets/{*path}", get(static_asset_handler))
            .with_state(chat_state)
            .layer(cors)
    }

    /// Start the server on `config.port` and run until stopped.
    pub async fn start(self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let router = self.build_router();
        let addr = SocketAddr::from(([0, 0, 0, 0], self.config.port));
        info!("Dashboard server starting on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, router).await?;
        Ok(())
    }

    /// Bind to port 0 (OS-assigned) and return the actual address.
    /// Useful for integration tests.
    pub async fn start_on_random_port(
        self,
    ) -> Result<(SocketAddr, tokio::task::JoinHandle<()>), Box<dyn std::error::Error + Send + Sync>>
    {
        let router = self.build_router();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?;
        info!("Dashboard test server on http://{}", addr);

        let handle = tokio::spawn(async move {
            axum::serve(listener, router).await.ok();
        });

        Ok((addr, handle))
    }
}

async fn health_handler() -> impl IntoResponse {
    Json(json!({ "status": "ok" }))
}

async fn index_handler() -> impl IntoResponse {
    // In debug mode, return a minimal HTML page.
    // In production, this would serve the embedded Svelte app.
    let html = r#"<!DOCTYPE html>
<html>
<head><title>Klyntbot Dashboard</title></head>
<body><div id="app">Loading...</div></body>
</html>"#;

    Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/html")
        .body(Body::from(html))
        .unwrap_or_else(|_| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::empty())
                .unwrap()
        })
}

async fn static_asset_handler() -> impl IntoResponse {
    StatusCode::NOT_FOUND
}
