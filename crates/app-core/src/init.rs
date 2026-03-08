use std::sync::Arc;

use agent::{AgentLoop, PersonaManager};
use bus::{DomainEventBus, MessageBus};
use channels::ChannelManager;
use cognitive::situation::UserSituation;
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use feature_notes::repo::NoteRepo;
use feature_productivity::auto_focus::AutoFocusSession;
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::tracker::categorizer::Categorizer;
use feature_productivity::{DailyAggregator, FocusManager, NudgeService, ProductivityEngine};
use scheduling::CronService;
use storage::{Repos, StoragePool, VectorStore};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

use crate::state::AppCore;

/// Bundle of receiver channels that callers wire to their transport (Tauri, SSE, etc.).
pub struct EventChannels {
    pub intervention_rx: mpsc::Receiver<feature_coaching::router::DeliveredIntervention>,
    pub domain_event_bus: Arc<DomainEventBus>,
    pub pipeline_rx: mpsc::UnboundedReceiver<cognitive::PipelineEvent>,
    pub auto_focus_rx: Option<mpsc::Receiver<AutoFocusSession>>,
    pub nudge_rx: Option<mpsc::Receiver<feature_productivity::types::NudgeRecord>>,
    pub dashboard_tick_rx:
        Option<tokio::sync::broadcast::Receiver<feature_productivity::ActivityTick>>,
    pub dashboard_poll_interval_secs: u64,
}

impl AppCore {
    /// Initialize the full agent stack.
    ///
    /// Mirrors the initialization order from `serve.rs`:
    /// config → storage → bus → provider → cron → persona → agent → channels
    ///
    /// Returns `(AppCore, EventChannels)`. The caller wires `EventChannels`
    /// receivers to their transport layer (Tauri events, SSE, etc.).
    pub async fn init(
        config_override: Option<config::Config>,
    ) -> Result<(Self, EventChannels), String> {
        // 1. Load config
        let mut config = match config_override {
            Some(c) => c,
            None => config::load_with_env_overrides()
                .await
                .map_err(|e| format!("config load failed: {e}"))?,
        };
        info!(path = ?config::config_path(), "configuration loaded");

        // 2. Connect storage
        let data_dir = config.data_dir_path();
        std::fs::create_dir_all(&data_dir)
            .map_err(|e| format!("failed to create data dir: {e}"))?;

        let storage_pool = StoragePool::connect(&data_dir)
            .await
            .map_err(|e| format!("storage connect failed: {e}"))?;
        let repos = Repos::from_pool(&storage_pool);
        let vector_store = VectorStore::connect(&data_dir).await.ok();
        // Create ANN indexes in the background (requires 256+ rows to train).
        if let Some(vs) = &vector_store {
            let vs_bg = vs.clone();
            tokio::spawn(async move {
                if let Err(e) = vs_bg.ensure_indexes(256).await {
                    warn!("ANN index creation failed (non-fatal): {e}");
                }
            });
        }
        info!("storage connected");

        // Run notes feature migrations and create repo.
        let notes_pool = storage_pool.inner().clone();
        StoragePool::run_feature_migrations(
            &notes_pool,
            &feature_notes::NotesFeature::migrations_static(),
        )
        .await
        .map_err(|e| format!("notes migration failed: {e}"))?;
        let note_repo = NoteRepo::new(notes_pool);

        // 3. Create LLM provider (graceful — falls back to noop for setup wizard)
        let (provider, resolved_model) = match providers::create_provider(&config) {
            Ok((p, m)) => {
                info!(provider = %p.name(), "provider ready");
                (p, m)
            }
            Err(e) => {
                warn!("No LLM provider configured ({e}), using noop — setup wizard will handle configuration");
                let noop: providers::DynProvider = Arc::new(providers::NoopProvider);
                (noop, config.agents.defaults.model.clone())
            }
        };
        config.agents.defaults.model = resolved_model;

        // 4. Message bus
        let bus = Arc::new(MessageBus::new(100));

        // 5. Cron service — set callbacks BEFORE wrapping in Arc
        let mut cron_service = CronService::new(repos.cron.clone());
        cron_service
            .start()
            .await
            .map_err(|e| format!("cron start failed: {e}"))?;

        let notification_dispatcher = Arc::new(agent::NotificationDispatcher::new(
            bus.outbound_sender(),
            config.todo.notifications.clone(),
        ));

        register_cron_callbacks(
            &mut cron_service,
            &repos,
            &notification_dispatcher,
            &config,
            &bus,
        );

        let cron_service = Arc::new(cron_service);
        ensure_cron_jobs(&cron_service, &config)
            .await
            .map_err(|e| format!("cron job registration failed: {e}"))?;
        info!("cron service started");

        // 6. Load personas
        let personas_dir = data_dir.join("personas");
        let mut persona_manager = PersonaManager::load(&personas_dir).await;
        persona_manager.resolve_scopes(&repos).await;
        let persona_manager = Arc::new(RwLock::new(persona_manager));
        info!("persona manager loaded");

        // 7. DomainEventBus for cross-feature communication (cognitive + coaching)
        let domain_event_bus = Arc::new(DomainEventBus::new(256));

        // 7b. Create cognitive provider for LLM-backed handlers
        let cognitive_provider = providers::create_cognitive_provider(&config).ok().flatten();
        if cognitive_provider.is_some() {
            info!("cognitive provider created — using LLM handlers");
        } else {
            info!("no cognitive provider — using heuristic handlers");
        }

        // 8. Build AgentLoop
        let (pipeline_tx, pipeline_rx) =
            tokio::sync::mpsc::unbounded_channel::<cognitive::PipelineEvent>();
        let mut builder = AgentLoop::builder(bus.clone(), provider, config.clone())
            .with_pool(storage_pool.inner().clone())
            .with_cron_service(cron_service.clone())
            .with_notification_handle(notification_dispatcher.last_active_handle())
            .with_domain_bus(Arc::clone(&domain_event_bus))
            .with_cognitive_provider(cognitive_provider.clone())
            .with_pipeline_tx(pipeline_tx);

        if let Some(vs) = vector_store {
            builder = builder.with_vector_store(vs);
        }

        let mut agent_loop_raw = builder
            .build()
            .await
            .map_err(|e| format!("agent build failed: {e}"))?;
        let inbound_rx = agent_loop_raw
            .take_inbound_rx()
            .expect("inbound receiver already taken");
        let agent = Arc::new(agent_loop_raw);
        info!("agent loop initialized");

        // 9. Channel manager
        let channel_manager = Arc::new(Mutex::new(
            ChannelManager::new(Arc::new(config.clone()), bus.clone())
                .map_err(|e| format!("channel manager init failed: {e}"))?,
        ));

        let shutdown_token = CancellationToken::new();

        // Initialize productivity feature (optional — requires enabled config).
        let dashboard_poll_interval_secs = config.productivity.tracking.poll_interval_secs;
        let (
            productivity_repos,
            focus_manager,
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
            auto_focus_rx,
            nudge_rx,
            dashboard_tick_rx,
        ) = if config.productivity.enabled {
            let pool = storage_pool.inner().clone();
            // Run feature migrations before creating repos.
            if let Err(e) = StoragePool::run_feature_migrations(
                &pool,
                &feature_productivity::ProductivityFeature::migrations_static(),
            )
            .await
            {
                error!("productivity migration failed — feature disabled: {e}");
                (None, None, None, None, None, None, None, None, None)
            } else {
                let prod_repos = ProductivityRepos::new(pool);
                let prod_config = &config.productivity;
                let mgr = Arc::new(FocusManager::new(
                    prod_repos.clone(),
                    prod_config.focus.clone(),
                ));

                let interceptor = Arc::new(Mutex::new(
                    feature_productivity::distraction::DistractionInterceptor::new(
                        prod_config.focus.clone(),
                        prod_repos.learned_rules.clone(),
                    ),
                ));

                // Daily aggregator for live summaries.
                let agg = Arc::new(DailyAggregator::new(prod_repos.clone()));

                // Build and start the productivity engine (tracker + all subscribers).
                let categories = prod_repos.categories.list_all().await.unwrap_or_default();
                let categorizer = Categorizer::new(categories);
                let mut engine = ProductivityEngine::new_with_bus(
                    prod_config.clone(),
                    prod_repos.clone(),
                    categorizer,
                    Some(Arc::clone(&domain_event_bus)),
                );

                // Take auto-focus receiver — caller wires to transport.
                let auto_focus_rx = engine.take_auto_focus_rx();

                // Subscribe to dashboard ticks — caller wires to DashboardEmitter.
                let dashboard_tick_rx = Some(engine.subscribe());

                engine.start();
                let engine = Arc::new(Mutex::new(engine));

                // Nudge service — break reminders + burnout alerts.
                let (nudge_tx, nudge_rx) =
                    mpsc::channel::<feature_productivity::types::NudgeRecord>(32);
                let mut nudge_svc = NudgeService::new(
                    prod_repos.clone(),
                    config.productivity.nudges.clone(),
                    config.productivity.focus.clone(),
                    nudge_tx,
                );
                nudge_svc.start();
                let nudge_svc = Arc::new(Mutex::new(nudge_svc));

                (
                    Some(prod_repos),
                    Some(mgr),
                    Some(engine),
                    Some(agg),
                    Some(nudge_svc),
                    Some(interceptor),
                    auto_focus_rx,
                    Some(nudge_rx),
                    dashboard_tick_rx,
                )
            }
        } else {
            (None, None, None, None, None, None, None, None, None)
        };

        // Initialize coaching engine state.
        let signal_accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
        let pattern_detector = Arc::new(Mutex::new(PatternDetector::new()));
        let intervention_router = Arc::new(Mutex::new(InterventionRouter::new(Default::default())));
        let coaching_repo = storage::CoachingStrategyRepo::new(storage_pool.inner().clone());
        let mut tracker = FeedbackTracker::new().with_repo(coaching_repo);
        tracker.load_from_db().await;
        let feedback_tracker = Arc::new(Mutex::new(tracker));
        let user_situation = Arc::new(Mutex::new(UserSituation {
            energy_level: 0.7,
            focus_state: 0.5,
            coaching_receptivity: 0.7,
            ..Default::default()
        }));

        // Start CoachingService — processes domain events through coaching pipeline.
        let coaching_reasoner: Arc<dyn feature_coaching::CoachingReasonerHandler> =
            if let Some(ref cp) = cognitive_provider {
                let params = providers::cognitive_chat_params(&config, 1024);
                Arc::new(agent::cognitive_handlers::LlmCoachingReasonerHandler::new(
                    cp.clone(),
                    params,
                ))
            } else {
                Arc::new(agent::cognitive_handlers::HeuristicCoachingReasonerHandler)
            };

        let coaching_cancel = shutdown_token.child_token();
        let (intervention_tx, intervention_rx) =
            mpsc::channel::<feature_coaching::router::DeliveredIntervention>(64);
        let coaching_service = feature_coaching::CoachingService::start(
            domain_event_bus.subscribe(),
            signal_accumulator.clone(),
            pattern_detector.clone(),
            intervention_router.clone(),
            feedback_tracker.clone(),
            user_situation.clone(),
            coaching_reasoner,
            intervention_tx,
            coaching_cancel,
        );
        info!("coaching service started");

        let core = AppCore {
            repos,
            agent: Arc::clone(&agent),
            bus: bus.clone(),
            persona_manager,
            config: RwLock::new(config),
            channel_manager: channel_manager.clone(),
            cron_service: cron_service.clone(),
            shutdown_token: shutdown_token.clone(),
            active_streams: Arc::new(dashmap::DashMap::new()),
            pending_interactions: Arc::new(dashmap::DashMap::new()),
            note_repo,
            productivity_repos,
            focus_manager,
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
            domain_event_bus: Some(Arc::clone(&domain_event_bus)),
            signal_accumulator: Some(signal_accumulator),
            pattern_detector: Some(pattern_detector),
            intervention_router: Some(intervention_router),
            feedback_tracker: Some(feedback_tracker),
            user_situation: Some(user_situation),
            coaching_service: Some(Arc::new(Mutex::new(coaching_service))),
            has_cognitive_provider: cognitive_provider.is_some(),
        };

        // Spawn background services (agent loop + channel manager).
        spawn_background(inbound_rx, channel_manager, &agent, &shutdown_token);

        // Spawn daily analytics retention cleanup.
        {
            let repos_bg = core.repos.clone();
            let token = shutdown_token.clone();
            tokio::spawn(async move {
                let mut interval =
                    tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
                interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                // Skip first tick (don't run immediately on startup).
                interval.tick().await;
                loop {
                    tokio::select! {
                        _ = interval.tick() => {
                            match repos_bg.cleanup_analytics().await {
                                Ok(0) => {}
                                Ok(n) => info!(deleted = n, "analytics retention: cleaned up old records"),
                                Err(e) => warn!(error = %e, "analytics retention cleanup failed"),
                            }
                        }
                        _ = token.cancelled() => break,
                    }
                }
            });
        }

        let channels = EventChannels {
            intervention_rx,
            domain_event_bus,
            pipeline_rx,
            auto_focus_rx,
            nudge_rx,
            dashboard_tick_rx,
            dashboard_poll_interval_secs,
        };

        Ok((core, channels))
    }
}

/// Spawn the agent loop and channel manager tasks.
fn spawn_background(
    inbound_rx: mpsc::Receiver<bus::InboundMessage>,
    channel_manager: Arc<Mutex<ChannelManager>>,
    agent: &Arc<AgentLoop>,
    shutdown_token: &CancellationToken,
) {
    let agent_clone = Arc::clone(agent);
    let token = shutdown_token.clone();

    tokio::spawn(async move {
        tokio::select! {
            result = agent_clone.run_with_rx(inbound_rx) => {
                if let Err(e) = result {
                    error!("agent loop error: {}", e);
                }
            }
            _ = token.cancelled() => {
                info!("agent loop shutdown via token");
            }
        }
    });

    tokio::spawn(async move {
        if let Err(e) = channel_manager.lock().await.start_all().await {
            error!("channel manager error: {}", e);
        }
    });

    info!("background services spawned");
}

/// Register cron callbacks (must be called before wrapping CronService in Arc).
fn register_cron_callbacks(
    cron_service: &mut CronService,
    repos: &Repos,
    notification_dispatcher: &Arc<agent::NotificationDispatcher>,
    config: &config::Config,
    bus: &Arc<MessageBus>,
) {
    let todo_repo = repos.actions.clone();
    let dispatcher = Arc::clone(notification_dispatcher);
    let config_focus = config.todo.focus.clone();
    let bus_for_cron = bus.clone();
    let rt = tokio::runtime::Handle::current();

    cron_service.set_callback(Arc::new(move |job: &scheduling::CronJob| {
        let todo_repo = todo_repo.clone();
        let dispatcher = Arc::clone(&dispatcher);
        let config_focus = config_focus.clone();
        let bus = Arc::clone(&bus_for_cron);
        let job_name = job.name.clone();

        tokio::task::block_in_place(|| {
            rt.block_on(async move {
                match job_name.as_str() {
                    "todo_focus_check" => {
                        let focused: Vec<storage::ActionRow> = todo_repo.list_focused().await?;
                        for task in &focused {
                            if let Some(deadline) = task.focus_deadline {
                                let remaining = deadline - chrono::Utc::now();
                                let hours_left = remaining.num_hours();
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
                        let summary = todo_repo.summary().await?;
                        let overdue: Vec<storage::ActionRow> = todo_repo.overdue().await?;
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
                        let focused: Vec<storage::ActionRow> = todo_repo.list_focused().await?;
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
                    name if name.starts_with("__klyntbot_") => {
                        let (channel, msg_text) = match name {
                            "__klyntbot_weekly_report" => (
                                "weekly_report",
                                "Generate weekly progress report using the weekly-report skill",
                            ),
                            "__klyntbot_daily_planning" => ("daily_planning", "/daily-planning"),
                            "__klyntbot_finance_daily_review" => (
                                "finance_daily_review",
                                "Run finance daily review and send summary",
                            ),
                            "__klyntbot_finance_budget_check" => (
                                "finance_budget_check",
                                "Check budget thresholds and send alerts",
                            ),
                            "__klyntbot_finance_price_refresh" => {
                                ("finance_price_refresh", "Refresh investment prices")
                            }
                            "__klyntbot_finance_health_check" => {
                                ("finance_health_check", "Run finance data health check")
                            }
                            "__klyntbot_cognitive_weekly_reflection" => (
                                "cognitive_reflection",
                                "Run weekly cognitive reflection and consolidate learnings",
                            ),
                            _ => return Ok(None),
                        };
                        let msg = bus::InboundMessage::new(
                            "system",
                            "cron",
                            channel,
                            msg_text.to_string(),
                        );
                        bus.publish_inbound(msg).await.map_err(|e| {
                            common::KlyntbotError::Bus(format!(
                                "Failed to publish {name} message: {e}"
                            ))
                        })?;
                        Ok(Some(format!("{name} triggered")))
                    }
                    _ => Ok(None),
                }
            })
        })
    }));
}

/// Register default cron jobs (idempotent — skips existing).
async fn ensure_cron_jobs(
    cron_service: &Arc<CronService>,
    config: &config::Config,
) -> Result<(), common::KlyntbotError> {
    let existing: std::collections::HashSet<String> = cron_service
        .list_jobs(true)
        .await
        .into_iter()
        .map(|j| j.name)
        .collect();

    macro_rules! ensure_job {
        ($name:expr, $schedule:expr, $msg:expr) => {
            if !existing.contains($name) {
                cron_service
                    .add_job($name, $schedule, $msg, false, None, None, false)
                    .await?;
            }
        };
    }

    ensure_job!(
        "todo_focus_check",
        scheduling::CronSchedule::Every {
            every_ms: 30 * 60 * 1000,
        },
        "Check focus task deadlines"
    );
    ensure_job!(
        "todo_daily_digest",
        scheduling::CronSchedule::Cron {
            expr: "0 9 * * *".to_string(),
            tz: None,
        },
        "Daily task summary"
    );
    ensure_job!(
        "todo_overdue_check",
        scheduling::CronSchedule::Every {
            every_ms: 60 * 60 * 1000,
        },
        "Check for overdue focus tasks"
    );
    ensure_job!(
        "__klyntbot_weekly_report",
        scheduling::CronSchedule::Cron {
            expr: "0 18 * * 0".to_string(),
            tz: None,
        },
        "Generate weekly progress report"
    );

    // Daily planning
    if config.todo.daily_planning.enabled {
        if let Some(cron_expr) = parse_time_to_cron(&config.todo.daily_planning.planning_time) {
            ensure_job!(
                "__klyntbot_daily_planning",
                scheduling::CronSchedule::Cron {
                    expr: cron_expr,
                    tz: None,
                },
                "Generate daily planning notification"
            );
        }
    }

    // Weekly cognitive reflection
    {
        let reflection_schedule = config
            .cognitive
            .reflection_schedule
            .as_deref()
            .unwrap_or("0 9 * * 1"); // Monday 9am default
        ensure_job!(
            "__klyntbot_cognitive_weekly_reflection",
            scheduling::CronSchedule::Cron {
                expr: reflection_schedule.to_string(),
                tz: Some(config.timezone.clone()),
            },
            "Weekly cognitive reflection"
        );
    }

    // Finance cron jobs
    if config.finance.enabled && config.finance.proactivity_level != "reactive" {
        if let Some(cron_expr) = parse_time_to_cron(&config.finance.scheduling.daily_review_time) {
            ensure_job!(
                "__klyntbot_finance_daily_review",
                scheduling::CronSchedule::Cron {
                    expr: cron_expr,
                    tz: None,
                },
                "Daily financial review"
            );
        }
        ensure_job!(
            "__klyntbot_finance_budget_check",
            scheduling::CronSchedule::Every {
                every_ms: 6 * 60 * 60 * 1000,
            },
            "Check budget thresholds"
        );
        if config.finance.price_refresh.enabled {
            ensure_job!(
                "__klyntbot_finance_price_refresh",
                scheduling::CronSchedule::Every {
                    every_ms: config.finance.price_refresh.interval_hours as u64 * 60 * 60 * 1000,
                },
                "Refresh investment prices"
            );
        }
        ensure_job!(
            "__klyntbot_finance_health_check",
            scheduling::CronSchedule::Cron {
                expr: "0 0 * * *".to_string(),
                tz: None,
            },
            "Finance data health check"
        );
    }

    Ok(())
}

/// Parse "HH:MM" to cron expression "M H * * *".
fn parse_time_to_cron(time_str: &str) -> Option<String> {
    let parts: Vec<&str> = time_str.split(':').collect();
    if parts.len() != 2 {
        warn!("invalid time format '{}', expected HH:MM", time_str);
        return None;
    }
    let hour: u8 = parts[0].parse().ok()?;
    let minute: u8 = parts[1].parse().ok()?;
    if hour >= 24 || minute >= 60 {
        warn!("invalid time '{}': hour 0-23, minute 0-59", time_str);
        return None;
    }
    Some(format!("{} {} * * *", minute, hour))
}
