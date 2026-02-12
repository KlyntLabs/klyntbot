//! Serve command handler for gateway daemon mode

use anyhow::Result;
use crate::{AgentLoop, ChannelManager, CronService, HeartbeatService, MessageBus};
use std::sync::Arc;
use tokio::signal;
use tracing::{error, info};

/// Handle serve command
pub async fn handle_serve(port: u16) -> Result<()> {
    info!("Starting klyntbot gateway on port {}", port);

    let config = crate::config::load()?;
    info!("Configuration loaded from: {:?}", crate::config::config_path());

    // Initialize LLM provider
    let provider = crate::providers::create_provider(&config)?;
    info!("Provider ready: {}", provider.name());

    // Initialize message bus
    let bus = Arc::new(MessageBus::new(100));
    info!("Message bus initialized");

    // Initialize cron service BEFORE agent loop
    let cron_store_path = config.workspace_path().join(".klyntbot").join("cron.json");
    let cron_service = Arc::new(CronService::new(cron_store_path));
    cron_service.start().await?;
    info!("Cron service started");

    // Initialize agent loop WITH cron service
    let agent_loop = Arc::new(
        AgentLoop::new_with_cron(
            bus.clone(),
            provider,
            config.clone(),
            Some(cron_service.clone()),
        )
        .await?,
    );
    info!("Agent loop initialized");

    // Initialize channel manager
    let channel_manager = Arc::new(ChannelManager::new(Arc::new(config.clone()), bus.clone()));

    // Initialize heartbeat service with agent loop callback
    let workspace_path = config.workspace_path();
    let mut heartbeat_service = HeartbeatService::new(
        workspace_path,
        1800, // 30 minutes
        true,
    );

    // Wire heartbeat to publish messages through the bus
    {
        let bus_for_heartbeat = bus.clone();
        let rt = tokio::runtime::Handle::current();
        heartbeat_service.set_callback(Arc::new(move |prompt: &str| {
            let bus = bus_for_heartbeat.clone();
            let prompt = prompt.to_string();
            rt.block_on(async {
                let msg =
                    crate::bus::InboundMessage::new("system", "heartbeat", "heartbeat", prompt);
                bus.publish_inbound(msg)
                    .await
                    .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
                Ok("Heartbeat message published".to_string())
            })
        }));
    }

    let heartbeat_service = Arc::new(heartbeat_service);
    heartbeat_service.start().await;
    info!("Heartbeat service started");

    // Start agent loop in background
    let agent_loop_handle = {
        let agent = agent_loop.clone();
        tokio::spawn(async move {
            if let Err(e) = agent.run().await {
                error!("Agent loop error: {}", e);
            }
        })
    };

    // Start channel manager in background
    let channel_manager_handle = {
        let cm = channel_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = cm.start_all().await {
                error!("Channel manager error: {}", e);
            }
        })
    };

    println!("\nklyntbot gateway running on port {}", port);
    println!("\nActive services:");
    println!("  Agent loop (processing messages)");
    println!("  Cron scheduler");
    println!("  Heartbeat monitor");
    println!("\nChannels:");
    for (name, enabled) in [
        ("Telegram", config.channels.telegram.enabled),
        ("Discord", config.channels.discord.enabled),
        ("WhatsApp", config.channels.whatsapp.enabled),
        ("Slack", config.channels.slack.enabled),
        ("QQ", config.channels.qq.enabled),
        ("Email", config.channels.email.enabled),
    ] {
        if enabled {
            println!("  + {}", name);
        }
    }
    println!("\nPress Ctrl+C to stop");

    // Wait for shutdown signal
    signal::ctrl_c().await?;
    info!("Shutting down gracefully...");

    // Stop all services gracefully
    agent_loop.stop().await;
    channel_manager.stop_all().await?;
    cron_service.stop().await;
    heartbeat_service.stop().await;

    // Wait for tasks to finish (with timeout)
    let shutdown_timeout = tokio::time::Duration::from_secs(5);
    let _ = tokio::time::timeout(shutdown_timeout, async {
        let _ = tokio::join!(agent_loop_handle, channel_manager_handle);
    })
    .await;

    info!("All services stopped");
    println!("\nklyntbot stopped");
    Ok(())
}
