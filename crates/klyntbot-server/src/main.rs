use std::sync::Arc;

use anyhow::Result;
use clap::Parser;
use rmcp::service::ServiceExt;

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
            let handler = KlyntbotServerHandler::new(app.clone(), whitelist);

            if http {
                let bind_host = host.unwrap_or_else(|| config.mcp.server.host.clone());
                let bind_port = port.unwrap_or(config.mcp.server.port);
                tracing::info!("MCP HTTP server listening on {bind_host}:{bind_port}");
                // TODO: Wire rmcp streamable HTTP server (Phase 4)
                eprintln!("HTTP transport not yet implemented. Use --stdio.");
                std::process::exit(1);
            } else {
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
            // TODO: Implement tool listing (Phase 2)
            if list {
                eprintln!("Tool listing not yet implemented.");
            }
            if let Some(name) = schema {
                eprintln!("Schema for '{name}' not yet implemented.");
            }
        }
    }

    Ok(())
}
