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

    // Drain unused EventChannels so senders do not block during shutdown.
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

    fn sample(state: RuntimeState, with_agent: bool) -> ExposureValidation {
        let mut builtins = vec![BuiltinId::GetStatus];
        if with_agent {
            builtins.push(BuiltinId::Agent);
        }
        ExposureValidation {
            runtime_state: state,
            requested: vec!["tasks".into()],
            effective_registry_tools: vec!["tasks".into()],
            effective_builtins: builtins,
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
        let invalid = sample(RuntimeState::Invalid, false);
        assert!(ensure_stdio_servable(&invalid).is_err());

        let ready = sample(RuntimeState::Ready, true);
        assert!(ensure_stdio_servable(&ready).is_ok());

        let disabled = sample(RuntimeState::Disabled, true);
        assert!(ensure_stdio_servable(&disabled).is_ok());
    }

    #[test]
    fn agent_follows_effective_builtins_not_registry_whitelist() {
        let with_agent = sample(RuntimeState::Ready, true);
        assert!(with_agent.effective_builtins.contains(&BuiltinId::Agent));
        assert!(!with_agent
            .effective_registry_tools
            .iter()
            .any(|n| n == "agent"));

        let without = sample(RuntimeState::Ready, false);
        assert!(!without.effective_builtins.contains(&BuiltinId::Agent));
        assert!(without.effective_builtins.contains(&BuiltinId::GetStatus));
    }

    #[test]
    fn rejection_and_runtime_tokens_match_spec() {
        assert_eq!(RejectionReason::Unknown.as_str(), "unknown");
        assert_eq!(RejectionReason::Forbidden.as_str(), "forbidden");
        assert_eq!(RuntimeState::Ready.as_str(), "ready");
        assert_eq!(RuntimeState::Disabled.as_str(), "disabled");
        assert_eq!(RuntimeState::Invalid.as_str(), "invalid");
    }
}
