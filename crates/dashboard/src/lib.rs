//! Klyntbot web dashboard — Axum REST/WebSocket API + embedded React SPA.
//!
//! # Architecture
//!
//! The dashboard crate sits at Layer 4.5 in the workspace dependency graph:
//! it depends on `agent`, `storage`, `config`, `common`, and `scheduling`,
//! and is wired into `cli::serve` (Layer 6).
//!
//! ## Entry point
//!
//! ```rust,ignore
//! let dashboard = DashboardServer::new(config.gateway.clone(), state);
//! dashboard.start(shutdown_signal).await?;
//! ```

pub mod api;
pub mod embed;
pub mod error;
pub mod router;
pub mod state;
pub mod ws;

use std::future::Future;
use std::net::SocketAddr;

use tracing::info;

pub use error::ApiError;
pub use state::AppState;

/// The HTTP server that serves the REST API and embedded SPA.
pub struct DashboardServer {
    gateway: config::schema::GatewayConfig,
    state: AppState,
}

impl DashboardServer {
    pub fn new(gateway: config::schema::GatewayConfig, state: AppState) -> Self {
        Self { gateway, state }
    }

    /// Start the dashboard server and run until `shutdown` resolves.
    pub async fn start(
        self,
        shutdown: impl Future<Output = ()> + Send + 'static,
    ) -> common::Result<()> {
        let addr: SocketAddr = format!("{}:{}", self.gateway.host, self.gateway.port)
            .parse()
            .map_err(|e: std::net::AddrParseError| {
                common::KlyntbotError::Bus(format!("Invalid gateway address: {}", e))
            })?;

        let router = router::build(self.state);

        info!("Dashboard listening on http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(common::KlyntbotError::Io)?;

        axum::serve(listener, router)
            .with_graceful_shutdown(shutdown)
            .await
            .map_err(common::KlyntbotError::Io)?;

        Ok(())
    }
}
