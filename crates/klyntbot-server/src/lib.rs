//! Klyntbot MCP server — library interface.
//!
//! Used by the desktop binary's `mcp serve --stdio` subcommand and by the
//! desktop crate's embedded HTTP server (sharing AppCore).

pub mod bridge;
pub mod handler;

use std::sync::Arc;

use anyhow::Result;
use rmcp::service::ServiceExt;

pub use handler::KlyntbotServerHandler;

/// Serve MCP over stdio until the rmcp service exits or the process receives SIGINT.
///
/// Caller is responsible for initialising `AppCore` and draining its event channels;
/// this helper only owns the transport. Calls `app.shutdown().await` before returning.
pub async fn serve_stdio(app: Arc<app_core::AppCore>, whitelist: Vec<String>) -> Result<()> {
    let handler = KlyntbotServerHandler::new(app.clone(), whitelist);
    tracing::info!("Starting MCP server (stdio)");
    let transport = rmcp::transport::io::stdio();
    let service = handler.serve(transport).await?;

    tokio::select! {
        result = service.waiting() => {
            if let Err(e) = result {
                eprintln!("Server error: {e}");
            }
        }
        _ = tokio::signal::ctrl_c() => {
            tracing::info!("Shutting down...");
        }
    }

    app.shutdown().await;
    Ok(())
}

/// Print the list of MCP tools that the server would expose, matching the post-init
/// fill behaviour of `AppCore::init` (auto-fills from `AiFeatureRegistry` when the
/// user's `exposed_tools` list is empty).
pub async fn print_exposed_tools() -> Result<()> {
    let mut cfg = config::load_with_env_overrides()
        .await
        .map_err(|e| anyhow::anyhow!("config load failed: {e}"))?;

    if cfg.mcp.server.exposed_tools.is_empty() {
        let registry = app_core::init::ai_pipeline::build_feature_registry();
        let mut tools: Vec<String> = registry
            .tool_names()
            .into_iter()
            .map(|s| s.to_string())
            .collect();
        tools.extend(
            config::schema::EXPLICIT_TOOL_ALLOWLIST
                .iter()
                .map(|s| s.to_string()),
        );
        cfg.mcp.server.exposed_tools = tools;
    }

    println!("Exposed MCP tools:");
    for name in &cfg.mcp.server.exposed_tools {
        println!("  - {name}");
    }
    Ok(())
}
