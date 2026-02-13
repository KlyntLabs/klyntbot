//! Serve command handler for gateway daemon mode

use agent::AgentLoop;
use anyhow::Result;
use bus::MessageBus;
use channels::ChannelManager;
use heartbeat::HeartbeatService;
use scheduling::CronService;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use tokio::signal;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};

/// Handle serve command
pub async fn handle_serve(port: u16) -> Result<()> {
    info!("Starting klyntbot gateway on port {}", port);

    let config = config::load()?;
    info!("Configuration loaded from: {:?}", config::config_path());

    // Initialize LLM provider
    let provider = providers::create_provider(&config)?;
    info!("Provider ready: {}", provider.name());

    // Initialize message bus
    let bus = Arc::new(MessageBus::new(100));
    info!("Message bus initialized");

    // Initialize cron service BEFORE agent loop
    let cron_store_path = config.workspace_path().join(".klyntbot").join("cron.json");
    let mut cron_service = CronService::new(cron_store_path);
    cron_service.start().await?;
    info!("Cron service started");

    // Create SHARED TodoStore (one instance for everything)
    let todo_path = config.todo_store_path();
    let todo_store = Arc::new(RwLock::new(tools::todo_store::TodoStore::new(todo_path)));

    // Create SHARED NotificationDispatcher
    let notification_dispatcher = Arc::new(agent::NotificationDispatcher::new(
        bus.outbound_sender(),
        config.todo.notifications.clone(),
    ));

    // Set callback BEFORE wrapping in Arc (requires &mut self)
    {
        let todo_store = Arc::clone(&todo_store);
        let dispatcher = Arc::clone(&notification_dispatcher);
        let config_focus = config.todo.focus.clone();
        let rt = tokio::runtime::Handle::current();

        cron_service.set_callback(Arc::new(move |job: &scheduling::CronJob| {
            let todo_store = Arc::clone(&todo_store);
            let dispatcher = Arc::clone(&dispatcher);
            let config_focus = config_focus.clone();
            let job_name = job.name.clone();

            rt.block_on(async move {
                match job_name.as_str() {
                    "todo_focus_check" => {
                        let mut store = todo_store.write().await;
                        let focused = store.focused().await?;
                        for task in &focused {
                            if let Some(deadline) = task.focus_deadline {
                                let remaining = deadline - chrono::Utc::now();
                                let hours_left = remaining.num_hours();
                                // Send reminders at 6h, 3h, 1h thresholds
                                if hours_left <= 1 && hours_left > 0 {
                                    dispatcher
                                        .notify(
                                            "⏰ Focus Deadline: 1h left",
                                            &format!("\"{}\" — deadline approaching!", task.title),
                                        )
                                        .await
                                        .ok();
                                } else if hours_left <= 3 && hours_left > 1 {
                                    dispatcher
                                        .notify(
                                            "⏰ Focus Deadline: 3h left",
                                            &format!("\"{}\" — stay on track", task.title),
                                        )
                                        .await
                                        .ok();
                                } else if hours_left <= 6 && hours_left > 3 {
                                    dispatcher
                                        .notify(
                                            "⏰ Focus Deadline: 6h left",
                                            &format!("\"{}\" — keep going", task.title),
                                        )
                                        .await
                                        .ok();
                                }
                            }
                        }
                        Ok(Some(format!("Checked {} focused tasks", focused.len())))
                    }
                    "todo_daily_digest" => {
                        let mut store = todo_store.write().await;
                        let summary = store.summary().await?;
                        // Use by_status HashMap for counts
                        let todo_count = summary
                            .by_status
                            .get(&tools::todo_types::TodoStatus::Todo)
                            .unwrap_or(&0);
                        let doing_count = summary
                            .by_status
                            .get(&tools::todo_types::TodoStatus::Doing)
                            .unwrap_or(&0);
                        let done_count = summary
                            .by_status
                            .get(&tools::todo_types::TodoStatus::Done)
                            .unwrap_or(&0);
                        let body = format!(
                            "Total: {} | Todo: {} | Doing: {} | Done: {} | Overdue: {}",
                            summary.total,
                            todo_count,
                            doing_count,
                            done_count,
                            summary.overdue.len()
                        );
                        dispatcher.notify("📋 Daily Task Digest", &body).await.ok();
                        Ok(Some("Daily digest sent".to_string()))
                    }
                    "todo_overdue_check" => {
                        let mut store = todo_store.write().await;
                        let expired_ids = store.auto_unfocus_expired().await?;
                        if !expired_ids.is_empty() {
                            let body = format!(
                                "{} task(s) auto-unfocused due to {}h deadline",
                                expired_ids.len(),
                                config_focus.deadline_hours
                            );
                            dispatcher
                                .notify("⏰ Focus Tasks Expired", &body)
                                .await
                                .ok();
                        }
                        Ok(Some("Overdue check complete".to_string()))
                    }
                    _ => Ok(None),
                }
            })
        }));
    }

    // NOW wrap in Arc
    let cron_service = Arc::new(cron_service);

    // Register cron jobs (add_job takes &self, works on Arc)
    cron_service
        .add_job(
            "todo_focus_check",
            scheduling::CronSchedule::Every {
                every_ms: 30 * 60 * 1000,
            },
            "Check focus task deadlines",
            false,
            None,
            None,
            false,
        )
        .await?;

    cron_service
        .add_job(
            "todo_daily_digest",
            scheduling::CronSchedule::Cron {
                expr: "0 9 * * *".to_string(),
                tz: None,
            },
            "Daily task summary",
            false,
            None,
            None,
            false,
        )
        .await?;

    cron_service
        .add_job(
            "todo_overdue_check",
            scheduling::CronSchedule::Every {
                every_ms: 60 * 60 * 1000,
            },
            "Check for overdue focus tasks",
            false,
            None,
            None,
            false,
        )
        .await?;

    info!("Todo cron jobs registered");

    // Initialize agent loop WITH cron service and shared instances
    let agent_loop = Arc::new(Mutex::new(
        AgentLoop::new_with_cron(
            bus.clone(),
            provider,
            config.clone(),
            Some(cron_service.clone()),
            todo_store.clone(),
            Some(notification_dispatcher.last_active_handle()),
        )
        .await?,
    ));
    info!("Agent loop initialized");

    // Initialize channel manager
    let channel_manager = Arc::new(Mutex::new(ChannelManager::new(
        Arc::new(config.clone()),
        bus.clone(),
    )));

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
                let msg = bus::InboundMessage::new("system", "heartbeat", "heartbeat", prompt);
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

    // Grab the agent's shutdown flag before spawning — this lets us signal
    // stop without acquiring the Mutex (which run() holds for its lifetime).
    let agent_shutdown = agent_loop.lock().await.shutdown_flag();

    // Start agent loop in background
    let agent_loop_handle = {
        let agent = agent_loop.clone();
        tokio::spawn(async move {
            if let Err(e) = agent.lock().await.run().await {
                error!("Agent loop error: {}", e);
            }
        })
    };

    // Start channel manager in background
    let channel_manager_handle = {
        let cm = channel_manager.clone();
        tokio::spawn(async move {
            if let Err(e) = cm.lock().await.start_all().await {
                error!("Channel manager error: {}", e);
            }
        })
    };

    println!("\nklyntbot gateway running on port {}", port);
    println!("\nServices:");
    println!("  Agent loop");
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
    println!("\nShutting down gracefully...");
    info!("Shutting down gracefully...");

    // Signal the agent loop to stop via the atomic flag (no Mutex needed).
    agent_shutdown.store(false, Ordering::SeqCst);

    // Stop services that don't hold long-lived Mutex locks.
    cron_service.stop().await;
    heartbeat_service.stop().await;

    // Wait for spawned tasks to finish (they should exit soon now).
    // The agent loop checks its flag every 1s; channel tasks may block
    // on long-polling, so abort them after the timeout.
    let shutdown_timeout = tokio::time::Duration::from_secs(5);
    if tokio::time::timeout(shutdown_timeout, async {
        let _ = tokio::join!(agent_loop_handle, channel_manager_handle);
    })
    .await
    .is_err()
    {
        info!("Shutdown timeout — aborting remaining tasks");
    }

    info!("All services stopped");
    println!("klyntbot stopped");
    Ok(())
}
