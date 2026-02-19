//! Serve command handler for gateway daemon mode

use agent::AgentLoop;
use anyhow::Result;
use bus::MessageBus;
use channels::ChannelManager;
use heartbeat::HeartbeatService;
use scheduling::CronService;
use std::sync::atomic::Ordering;
use std::sync::Arc;
use storage::{Repos, StoragePool};
use tokio::signal;
use tokio::sync::{Mutex, RwLock};
use tracing::{error, info};

/// Handle serve command
pub async fn handle_serve(port: u16) -> Result<()> {
    info!("Starting klyntbot gateway on port {}", port);

    let config = config::load_with_env_overrides().await?;
    info!("Configuration loaded from: {:?}", config::config_path());

    // Connect to database and create repos
    let database_url = config.database_url.as_deref().ok_or_else(|| {
        anyhow::anyhow!(
            "Database not configured. Run `klyntbot init` to set up PostgreSQL.\n\
             Or set KLYNTBOT_DATABASE_URL in your environment."
        )
    })?;
    let storage_pool = StoragePool::connect(database_url).await?;
    let repos = Repos::from_pool(&storage_pool);
    info!("Database connected and migrations applied");

    // Initialize LLM provider
    let provider = providers::create_provider(&config)?;
    info!("Provider ready: {}", provider.name());

    // Initialize message bus
    let bus = Arc::new(MessageBus::new(100));
    info!("Message bus initialized");

    // Initialize cron service (SQL-backed via from_repo)
    let mut cron_service = CronService::from_repo(repos.cron);
    cron_service.start().await?;
    info!("Cron service started");

    // Create SHARED GoalStore (SQL-backed via from_repo)
    let goal_store = Arc::new(RwLock::new(goal::GoalStore::from_repo(repos.goals)));

    // Create SHARED PlanStore (SQL-backed via from_repo)
    let plan_store = Arc::new(RwLock::new(plan::PlanStore::from_repo(repos.plans)));

    // Create SHARED NotificationDispatcher
    let notification_dispatcher = Arc::new(agent::NotificationDispatcher::new(
        bus.outbound_sender(),
        config.todo.notifications.clone(),
    ));

    // Set callback BEFORE wrapping in Arc (requires &mut self)
    {
        let todo_repo = repos.todos.clone();
        let dispatcher = Arc::clone(&notification_dispatcher);
        let config_focus = config.todo.focus.clone();
        let bus_for_cron = bus.clone();
        let rt = tokio::runtime::Handle::current();

        cron_service.set_callback(Arc::new(move |job: &scheduling::CronJob| {
            let todo_repo = todo_repo.clone();
            let dispatcher = Arc::clone(&dispatcher);
            let config_focus = config_focus.clone();
            let bus = Arc::clone(&bus_for_cron);
            let job_name = job.name.clone();

            rt.block_on(async move {
                match job_name.as_str() {
                    "todo_focus_check" => {
                        let focused = todo_repo
                            .list_focused()
                            .await
                            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
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
                        let summary = todo_repo
                            .summary()
                            .await
                            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                        let overdue = todo_repo
                            .overdue()
                            .await
                            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                        let body = format!(
                            "Total: {} | Todo: {} | Doing: {} | Done: {} | Overdue: {}",
                            summary.total,
                            summary.todo,
                            summary.doing,
                            summary.done,
                            overdue.len()
                        );
                        dispatcher.notify("📋 Daily Task Digest", &body).await.ok();
                        Ok(Some("Daily digest sent".to_string()))
                    }
                    "todo_overdue_check" => {
                        // Auto-unfocus tasks with expired focus deadlines
                        let focused = todo_repo
                            .list_focused()
                            .await
                            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
                        let now = chrono::Utc::now();
                        let mut expired_count = 0u32;
                        for task in &focused {
                            if task.focus_deadline.map(|d| d < now).unwrap_or(false) {
                                let _ = todo_repo.unfocus(&task.id).await;
                                expired_count += 1;
                            }
                        }
                        if expired_count > 0 {
                            let body = format!(
                                "{} task(s) auto-unfocused due to {}h deadline",
                                expired_count, config_focus.deadline_hours
                            );
                            dispatcher
                                .notify("⏰ Focus Tasks Expired", &body)
                                .await
                                .ok();
                        }
                        Ok(Some("Overdue check complete".to_string()))
                    }
                    "__klyntbot_weekly_report" => {
                        // Trigger agent turn with weekly-report skill via bus
                        let msg = bus::InboundMessage::new(
                            "system",
                            "cron",
                            "weekly_report",
                            "Generate weekly progress report using the weekly-report skill"
                                .to_string(),
                        );
                        bus.publish_inbound(msg).await.map_err(|e| {
                            common::KlyntbotError::Bus(format!(
                                "Failed to publish weekly report message: {}",
                                e
                            ))
                        })?;
                        Ok(Some("Weekly report triggered".to_string()))
                    }
                    "__klyntbot_calendar_sync" => {
                        // Trigger calendar sync via bus message
                        let msg = bus::InboundMessage::new(
                            "system",
                            "cron",
                            "calendar_sync",
                            "Sync calendar events with configured providers".to_string(),
                        );
                        bus.publish_inbound(msg).await.map_err(|e| {
                            common::KlyntbotError::Bus(format!(
                                "Failed to publish calendar sync message: {}",
                                e
                            ))
                        })?;
                        Ok(Some("Calendar sync triggered".to_string()))
                    }
                    "__klyntbot_daily_planning" => {
                        // Trigger daily planning skill via bus message
                        let msg = bus::InboundMessage::new(
                            "system",
                            "cron",
                            "daily_planning",
                            "/daily-planning".to_string(),
                        );
                        bus.publish_inbound(msg).await.map_err(|e| {
                            common::KlyntbotError::Bus(format!(
                                "Failed to publish daily planning message: {}",
                                e
                            ))
                        })?;
                        Ok(Some("Daily planning triggered".to_string()))
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

    cron_service
        .add_job(
            "__klyntbot_weekly_report",
            scheduling::CronSchedule::Cron {
                expr: "0 18 * * 0".to_string(), // Sunday at 18:00
                tz: None,
            },
            "Generate weekly progress report",
            false,
            None,
            None,
            false,
        )
        .await?;

    // Register calendar sync cron job if any provider is enabled
    if config.calendar.is_any_enabled() {
        let sync_interval_secs = config.calendar.min_sync_interval_secs();
        cron_service
            .add_job(
                "__klyntbot_calendar_sync",
                scheduling::CronSchedule::Every {
                    every_ms: sync_interval_secs * 1000,
                },
                "Sync calendar events with configured providers",
                false,
                None,
                None,
                false,
            )
            .await?;
        info!(
            "Calendar sync cron registered (interval: {}s)",
            sync_interval_secs
        );
    }

    // Register daily planning cron job if enabled
    if config.todo.daily_planning.enabled {
        let planning_time = &config.todo.daily_planning.planning_time;

        // Parse HH:MM format to cron expression (minute hour * * *)
        let parts: Vec<&str> = planning_time.split(':').collect();
        if parts.len() == 2 {
            if let (Ok(hour), Ok(minute)) = (parts[0].parse::<u8>(), parts[1].parse::<u8>()) {
                if hour < 24 && minute < 60 {
                    let cron_expr = format!("{} {} * * *", minute, hour);
                    cron_service
                        .add_job(
                            "__klyntbot_daily_planning",
                            scheduling::CronSchedule::Cron {
                                expr: cron_expr.clone(),
                                tz: None,
                            },
                            "Generate daily planning notification",
                            false,
                            None,
                            None,
                            false,
                        )
                        .await?;
                    info!(
                        "Daily planning cron registered (time: {}, cron: {})",
                        planning_time, cron_expr
                    );
                } else {
                    anyhow::bail!(
                        "Invalid planning_time '{}': hour must be 0-23, minute 0-59",
                        planning_time
                    );
                }
            } else {
                anyhow::bail!(
                    "Invalid planning_time '{}': must be numeric HH:MM (e.g., '08:00', not '8:00am')",
                    planning_time
                );
            }
        } else {
            anyhow::bail!(
                "Invalid planning_time '{}': must be in HH:MM format (e.g., '08:00')",
                planning_time
            );
        }
    }

    info!("Todo cron jobs registered");

    // Initialize agent loop WITH cron service and shared instances
    let agent_loop = Arc::new(Mutex::new(
        AgentLoop::new_with_cron(
            bus.clone(),
            provider,
            config.clone(),
            Some(cron_service.clone()),
            repos.todos,
            Some(repos.embeddings),
            Some(goal_store.clone()),
            Some(plan_store.clone()),
            Some(notification_dispatcher.last_active_handle()),
            repos.outcomes,
            repos.learning_state,
            repos.memory_notes,
            Some(repos.strategies),
        )
        .await?,
    ));
    info!("Agent loop initialized");

    // Initialize channel manager
    let channel_manager = Arc::new(Mutex::new(ChannelManager::new(
        Arc::new(config.clone()),
        bus.clone(),
    )?));

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
