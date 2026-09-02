//! Klyntbot MCP server — library interface.
//!
//! Used by the desktop binary's `mcp serve --stdio` subcommand and by the
//! desktop crate's embedded HTTP server (sharing AppCore).

pub mod bridge;
pub mod handler;

use std::sync::Arc;

use anyhow::Result;
use mcp::server::exposure::{ExposureValidation, RuntimeState};
use rmcp::service::ServiceExt;

pub use handler::KlyntbotServerHandler;

/// Serve MCP over stdio until the rmcp service exits or the process receives SIGINT.
///
/// Caller is responsible for initialising `AppCore` and draining its event channels;
/// this helper only owns the transport. Calls `app.shutdown().await` before returning.
///
/// Returns `Err` when exposure validation is [`RuntimeState::Invalid`] (EXPO-3.9).
pub async fn serve_stdio(app: Arc<app_core::AppCore>) -> Result<()> {
    let exposure = require_exposure(&app)?;
    ensure_stdio_servable(&exposure)?;

    let handler = KlyntbotServerHandler::new(app.clone(), &exposure);
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

/// Boot enough AppCore to obtain the final live `ToolRegistry`, run the shared
/// exposure validator (via post_init), print diagnostic fields, and exit
/// unsuccessfully when Invalid (EXPO-4.1–4.4, EXPO-3.9).
pub async fn print_exposed_tools() -> Result<()> {
    let config = config::load_with_env_overrides()
        .await
        .map_err(|e| anyhow::anyhow!("config load failed: {e}"))?;
    let server_enabled = config.mcp.server.enabled;

    let (app, events) = app_core::AppCore::init_with_sender(
        common::AppMode::Server,
        Some(config),
        None,
        None,
        None,
    )
    .await
    .map_err(|e| anyhow::anyhow!("AppCore init failed: {e}"))?;

    events.spawn_background_drain();

    let exposure = require_exposure(&app)?;
    print_exposure_report(server_enabled, &exposure);

    let invalid = exposure.runtime_state == RuntimeState::Invalid;
    app.shutdown().await;

    if invalid {
        anyhow::bail!("MCP exposure invalid");
    }
    Ok(())
}

fn require_exposure(app: &app_core::AppCore) -> Result<ExposureValidation> {
    app.mcp_exposure()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("mcp exposure status missing after AppCore init"))
}

/// Stdio refuses to serve when configuration is Invalid (EXPO-3.9).
fn ensure_stdio_servable(exposure: &ExposureValidation) -> Result<()> {
    if exposure.runtime_state == RuntimeState::Invalid {
        for entry in &exposure.rejected {
            eprintln!(
                "rejected: {} ({})",
                entry.name,
                entry.reason.as_str()
            );
        }
        anyhow::bail!("MCP exposure Invalid — refusing to serve (stdio)");
    }
    Ok(())
}

/// Print configured/enabled state, runtime state, requested, effective, rejected.
fn print_exposure_report(server_enabled: bool, exposure: &ExposureValidation) {
    println!("MCP exposure diagnostic");
    println!("  server_enabled: {server_enabled}");
    println!("  runtime_state: {}", exposure.runtime_state.as_str());

    print!("  requested:");
    if exposure.requested.is_empty() {
        println!(" (none — auto-default)");
    } else {
        println!();
        for name in &exposure.requested {
            println!("    - {name}");
        }
    }

    println!("  effective:");
    for builtin in &exposure.effective_builtins {
        println!("    - {} (builtin)", builtin.as_str());
    }
    for name in &exposure.effective_registry_tools {
        println!("    - {name}");
    }

    print!("  rejected:");
    if exposure.rejected.is_empty() {
        println!(" (none)");
    } else {
        println!();
        for entry in &exposure.rejected {
            println!(
                "    - {} ({})",
                entry.name,
                entry.reason.as_str()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcp::server::exposure::{BuiltinId, RejectedEntry, RejectionReason};

    fn sample(state: RuntimeState) -> ExposureValidation {
        ExposureValidation {
            runtime_state: state,
            requested: vec!["tasks".into()],
            effective_registry_tools: vec!["tasks".into()],
            effective_builtins: vec![BuiltinId::GetStatus, BuiltinId::Agent],
            rejected: if state == RuntimeState::Invalid {
                vec![RejectedEntry {
                    name: "shell".into(),
                    reason: RejectionReason::Forbidden,
                }]
            } else {
                vec![]
            },
        }
    }

    #[test]
    fn stdio_refuses_invalid_exposure() {
        assert!(ensure_stdio_servable(&sample(RuntimeState::Invalid)).is_err());
        assert!(ensure_stdio_servable(&sample(RuntimeState::Ready)).is_ok());
        assert!(ensure_stdio_servable(&sample(RuntimeState::Disabled)).is_ok());
    }
}
