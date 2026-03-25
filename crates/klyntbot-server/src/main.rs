use std::sync::Arc;

use anyhow::Result;
use axum::extract::Request;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use clap::Parser;
use rmcp::service::ServiceExt;
use rmcp::transport::streamable_http_server::session::local::LocalSessionManager;
use rmcp::transport::streamable_http_server::{StreamableHttpServerConfig, StreamableHttpService};
use tokio_util::sync::CancellationToken;

use klyntbot_server::cli::{Cli, Command};
use klyntbot_server::handler::KlyntbotServerHandler;
use klyntbot_server::logging;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Serve {
            stdio: _,
            http,
            port,
            host,
        } => {
            if http {
                logging::configure_http_tracing();
            } else {
                // Default to stdio
                logging::configure_stdio_tracing();
            }

            // Load config
            let config = config::load_with_env_overrides()
                .await
                .map_err(|e| anyhow::anyhow!("config load failed: {e}"))?;

            // Init AppCore in Server mode
            let (app, events) =
                app_core::AppCore::init(common::AppMode::Server, Some(config.clone()))
                    .await
                    .map_err(|e| anyhow::anyhow!("init failed: {e}"))?;
            let app = Arc::new(app);

            // Drain unused EventChannels — both receivers must close before task exits.
            // In Server mode, intervention_tx is dropped (coaching not started) so
            // intervention_rx closes immediately; pipeline_rx closes when domain_event_bus
            // senders are dropped at shutdown.
            tokio::spawn(async move {
                let mut intervention_rx = events.intervention_rx;
                let mut pipeline_rx = events.pipeline_rx;
                let mut intervention_closed = false;
                let mut pipeline_closed = false;
                while !intervention_closed || !pipeline_closed {
                    tokio::select! {
                        msg = intervention_rx.recv(), if !intervention_closed => {
                            if msg.is_none() { intervention_closed = true; }
                        }
                        result = pipeline_rx.recv(), if !pipeline_closed => {
                            if result.is_err() { pipeline_closed = true; }
                        }
                    }
                }
            });

            let whitelist = config.mcp.server.exposed_tools.clone();

            if http {
                let bind_host = host.unwrap_or_else(|| config.mcp.server.host.clone());
                let bind_port = port.unwrap_or(config.mcp.server.port);
                let auth_config = config.mcp.server.auth.clone();

                let ct = CancellationToken::new();
                let mcp_config = StreamableHttpServerConfig {
                    cancellation_token: ct.clone(),
                    ..Default::default()
                };

                let factory_app = app.clone();
                let factory_whitelist = whitelist;
                let mcp_service: StreamableHttpService<KlyntbotServerHandler, LocalSessionManager> =
                    StreamableHttpService::new(
                        move || {
                            Ok(KlyntbotServerHandler::new(
                                factory_app.clone(),
                                factory_whitelist.clone(),
                            ))
                        },
                        Arc::new(LocalSessionManager::default()),
                        mcp_config,
                    );

                let mut router = axum::Router::new().nest_service("/mcp", mcp_service);

                // Wire bearer-token auth middleware if configured.
                if auth_config.enabled {
                    if let Some(ref token) = auth_config.token {
                        let expected = token.expose().clone();
                        router =
                            router.layer(middleware::from_fn(move |req: Request, next: Next| {
                                let expected = expected.clone();
                                async move {
                                    bearer_auth_check(&req, &expected).map_err(|e| *e)?;
                                    Ok::<Response, Response>(next.run(req).await)
                                }
                            }));
                    }
                }

                let bind_addr = format!("{bind_host}:{bind_port}");
                let tcp_listener = tokio::net::TcpListener::bind(&bind_addr)
                    .await
                    .map_err(|e| anyhow::anyhow!("failed to bind {bind_addr}: {e}"))?;
                tracing::info!("MCP HTTP server listening on {bind_addr}");

                tokio::select! {
                    result = axum::serve(tcp_listener, router)
                        .with_graceful_shutdown(async move { ct.cancelled_owned().await }) => {
                        if let Err(e) = result {
                            eprintln!("HTTP server error: {e}");
                        }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("Shutting down...");
                    }
                }

                app.shutdown().await;
            } else {
                let handler = KlyntbotServerHandler::new(app.clone(), whitelist);
                tracing::info!("Starting MCP server (stdio)");
                let transport = rmcp::transport::io::stdio();
                let service = handler.serve(transport).await?;

                tokio::select! {
                    result = service.waiting() => {
                        if let Err(e) = result { eprintln!("Server error: {e}"); }
                    }
                    _ = tokio::signal::ctrl_c() => {
                        tracing::info!("Shutting down...");
                    }
                }

                app.shutdown().await;
            }
        }
        Command::Tools { list, schema } => {
            if list {
                let cfg = config::load_with_env_overrides()
                    .await
                    .map_err(|e| anyhow::anyhow!("config load failed: {e}"))?;
                println!("Exposed MCP tools:");
                for name in &cfg.mcp.server.exposed_tools {
                    println!("  - {name}");
                }
            }
            if let Some(name) = schema {
                eprintln!(
                    "Schema for '{name}' not yet available via CLI. Use MCP tools/list instead."
                );
            }
        }
    }

    Ok(())
}

/// Validate that the request carries a valid `Authorization: Bearer <token>` header.
/// Returns `Err(Response)` with 401 if the token is missing or incorrect.
fn bearer_auth_check(req: &Request, expected_token: &str) -> Result<(), Box<Response>> {
    let auth_header = req
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok());

    match auth_header {
        Some(value) if value.strip_prefix("Bearer ") == Some(expected_token) => Ok(()),
        _ => Err(Box::new(
            (
                axum::http::StatusCode::UNAUTHORIZED,
                "Unauthorized: invalid or missing Bearer token",
            )
                .into_response(),
        )),
    }
}
