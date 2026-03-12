use std::sync::Arc;

use agent::{AgentLoop, PersonaManager};
use bus::{DomainEventBus, MessageBus};
use channels::ChannelManager;
use chrono::{Duration, Timelike, Utc};
use cognitive::situation::{compute_situation, SituationInputs, UserSituation};
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use feature_notes::repo::NoteRepo;
use feature_productivity::auto_focus::AutoFocusEvent;
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::tracker::categorizer::Categorizer;
use feature_productivity::{DailyAggregator, FocusManager, NudgeService, ProductivityEngine};
use scheduling::CronService;
use storage::{Repos, StoragePool, VectorStore};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};

use crate::state::AppCore;

/// Bundle of receiver channels that callers wire to their transport (Tauri, SSE, etc.).
pub struct EventChannels {
    pub intervention_rx: mpsc::Receiver<feature_coaching::router::DeliveredIntervention>,
    pub domain_event_bus: Arc<DomainEventBus>,
    pub pipeline_rx: tokio::sync::broadcast::Receiver<cognitive::PipelineEvent>,
    pub auto_focus_rx: Option<mpsc::Receiver<AutoFocusEvent>>,
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
        mode: common::AppMode,
        config_override: Option<config::Config>,
    ) -> Result<(Self, EventChannels), String> {
        Self::init_with_sender(mode, config_override, None).await
    }

    /// Initialize with an optional custom notification sender.
    ///
    /// When `sender` is `Some`, OS-native notifications are routed through it
    /// (e.g. Tauri's notification plugin, which shows the app icon). When
    /// `None`, the default platform command (`osascript` / `notify-send`) is used.
    pub async fn init_with_sender(
        mode: common::AppMode,
        config_override: Option<config::Config>,
        notification_sender: Option<Arc<dyn common::NotificationSender>>,
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

        // Run tasks feature migrations.
        StoragePool::run_feature_migrations(
            storage_pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "tasks".to_string(),
                version: 1,
                description: "Create agentic task tables".to_string(),
                sql: feature_tasks::TasksFeature::migration_sql().to_string(),
            }],
        )
        .await
        .map_err(|e| format!("tasks migration failed: {e}"))?;

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

        // 5. Create cognitive provider for LLM-backed handlers (needed by cron + agent)
        let cognitive_provider = providers::create_cognitive_provider(&config).ok().flatten();
        if cognitive_provider.is_some() {
            info!("cognitive provider created — using LLM handlers");
        } else {
            info!("no cognitive provider — using heuristic handlers");
        }

        // 6. Cron service — set callbacks BEFORE wrapping in Arc
        let mut cron_service = CronService::new(repos.cron.clone());
        cron_service
            .start()
            .await
            .map_err(|e| format!("cron start failed: {e}"))?;

        let notification_dispatcher = Arc::new(match &notification_sender {
            Some(sender) => agent::NotificationDispatcher::with_sender(
                bus.outbound_sender(),
                config.todo.notifications.clone(),
                Arc::clone(sender),
            ),
            None => agent::NotificationDispatcher::new(
                bus.outbound_sender(),
                config.todo.notifications.clone(),
            ),
        });

        register_cron_callbacks(
            &mut cron_service,
            &repos,
            &notification_dispatcher,
            &config,
            &bus,
            cognitive_provider.clone(),
        );

        let cron_service = Arc::new(cron_service);
        ensure_cron_jobs(&cron_service, &config)
            .await
            .map_err(|e| format!("cron job registration failed: {e}"))?;
        info!("cron service started");

        // 7. Load personas
        let personas_dir = data_dir.join("personas");
        let mut persona_manager = PersonaManager::load(&personas_dir).await;
        persona_manager.resolve_scopes(&repos).await;
        let persona_manager = Arc::new(RwLock::new(persona_manager));
        info!("persona manager loaded");

        // Run activity-log migrations (unified activity log).
        StoragePool::run_feature_migrations(
            storage_pool.inner(),
            &activity_log::ActivityLog::migrations_static(),
        )
        .await
        .map_err(|e| format!("activity-log migration failed: {e}"))?;
        let activity_svc = Arc::new(activity_log::ActivityIngestionService::new(
            storage_pool.clone(),
            activity_log::PrivacyFilter::default(),
        ));

        // 8. DomainEventBus for cross-feature communication (cognitive + coaching)
        let domain_event_bus = Arc::new(DomainEventBus::new(256));

        // Pre-create user situation (defaults now, recomputed with real data below
        // and every 2 min afterwards). Shared with CognitiveContextSource for
        // situational_boost in memory retrieval.
        let user_situation = Arc::new(Mutex::new(UserSituation::default()));

        // 8. Build AgentLoop
        let (pipeline_broadcast_tx, _) =
            tokio::sync::broadcast::channel::<cognitive::PipelineEvent>(256);
        let pipeline_tx = pipeline_broadcast_tx.clone();
        let mut builder = AgentLoop::builder(bus.clone(), provider, config.clone())
            .with_pool(storage_pool.inner().clone())
            .with_cron_service(cron_service.clone())
            .with_notification_handle(notification_dispatcher.last_active_handle());

        // Thread the custom notification sender (if provided) to the agent's ReminderEngine
        if let Some(ref sender) = notification_sender {
            builder = builder.with_notification_sender(Arc::clone(sender));
        }

        let mut builder = builder
            .with_domain_bus(Arc::clone(&domain_event_bus))
            .with_cognitive_provider(cognitive_provider.clone())
            .with_pipeline_tx(pipeline_tx)
            .with_user_situation(user_situation.clone())
            .with_activity_service(Arc::clone(&activity_svc));

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
                let mut engine = ProductivityEngine::new_full(
                    prod_config.clone(),
                    prod_repos.clone(),
                    categorizer,
                    Some(Arc::clone(&domain_event_bus)),
                    Some(Arc::clone(&activity_svc)),
                );

                // Take auto-focus receiver — caller wires to transport.
                let auto_focus_rx = engine.take_auto_focus_rx();

                // Subscribe to dashboard ticks — caller wires to DashboardEmitter.
                let dashboard_tick_rx = Some(engine.subscribe());

                engine.start();

                // Wire ProductivityIntelligenceLayer — subscribes to tick broadcast
                // for classification, session aggregation, quality scoring, and interventions.
                {
                    let prod_handler: Option<Arc<dyn feature_productivity::ProductivityHandler>> =
                        cognitive_provider.as_ref().map(|cp| {
                            let model = config.agents.defaults.model.clone();
                            Arc::new(agent::ProductivityHandlerImpl::new(cp.clone(), model))
                                as Arc<dyn feature_productivity::ProductivityHandler>
                        });

                    match feature_productivity::intelligence::ProductivityIntelligenceLayer::new(
                        engine.tick_sender(),
                        Arc::clone(&domain_event_bus),
                        prod_repos.clone(),
                        prod_handler,
                        shutdown_token.child_token(),
                    )
                    .await
                    {
                        Ok(layer) => {
                            layer.start();
                            info!("productivity intelligence layer started");
                        }
                        Err(e) => {
                            warn!("Failed to start intelligence layer: {e}");
                        }
                    }
                }

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

        // Always create the intervention channel pair (EventChannels requires it).
        let (intervention_tx, intervention_rx) =
            mpsc::channel::<feature_coaching::router::DeliveredIntervention>(64);

        let (
            signal_accumulator,
            pattern_detector,
            intervention_router,
            feedback_tracker,
            coaching_service,
        ) = if mode == common::AppMode::Desktop {
            // Initialize coaching engine state.
            let signal_accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
            let pattern_detector = Arc::new(Mutex::new(PatternDetector::new()));
            let intervention_router =
                Arc::new(Mutex::new(InterventionRouter::new(Default::default())));
            let coaching_repo = storage::CoachingStrategyRepo::new(storage_pool.inner().clone());
            let mut tracker = FeedbackTracker::new().with_repo(coaching_repo);
            tracker.load_from_db().await;
            let feedback_tracker = Arc::new(Mutex::new(tracker));

            // Compute real user situation now that productivity_repos is available.
            {
                let real_situation = build_situation_inputs(
                    productivity_repos.as_ref(),
                    &repos,
                    None, // router just created, no dismissals yet
                )
                .await;
                *user_situation.lock().await = real_situation;
            }

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
            let coaching_service = feature_coaching::CoachingService::start(
                domain_event_bus.subscribe(),
                signal_accumulator.clone(),
                pattern_detector.clone(),
                intervention_router.clone(),
                feedback_tracker.clone(),
                user_situation.clone(),
                coaching_reasoner,
                intervention_tx.clone(),
                coaching_cancel,
            );
            info!("coaching service started");

            (
                Some(signal_accumulator),
                Some(pattern_detector),
                Some(intervention_router),
                Some(feedback_tracker),
                Some(coaching_service),
            )
        } else {
            // Server mode: drop intervention_tx so intervention_rx.recv() returns None immediately.
            drop(intervention_tx);
            info!("coaching service skipped (server mode)");
            (None, None, None, None, None)
        };

        // Phase 3: Auto-generate ingestion token on first startup if missing.
        if config.capture.ingestion_api.enabled && config.capture.ingestion_api.token.is_none() {
            config.capture.ingestion_api.token = Some(uuid::Uuid::new_v4().to_string());
            if let Err(e) = config::save(&config).await {
                warn!("Failed to save auto-generated ingestion token: {e}");
            } else {
                info!("auto-generated ingestion API token");
            }
        }

        // Phase 3: Start file watcher if enabled.
        if config.capture.file_watcher.enabled {
            let dirs: Vec<std::path::PathBuf> = config
                .capture
                .file_watcher
                .directories
                .iter()
                .map(std::path::PathBuf::from)
                .collect();
            if !dirs.is_empty() {
                let fw = crate::file_watcher::FileWatcherService::new(
                    dirs,
                    Arc::clone(&activity_svc),
                    config.capture.file_watcher.ignore_patterns.clone(),
                    config.capture.file_watcher.debounce_ms,
                );
                let _fw_handle = fw.start(shutdown_token.child_token());
                info!("file watcher started");
            }
        }

        // Phase 2: Start Work Context inference engine + loop.
        if config.work_context.enabled {
            let inference_cfg =
                activity_log::inference::ContextInferenceConfig::from_work_context_config(
                    &config.work_context,
                );
            let embedding_engine = Arc::new(tools::EmbeddingEngine::new());
            let text_embedder = Arc::new(agent::TextEmbedderImpl::new(embedding_engine));
            let inference_engine = Arc::new(activity_log::inference::ContextInferenceEngine::new(
                storage_pool.clone(),
                text_embedder,
                None, // VectorStore already consumed by agent builder; centroids cached in-memory
                inference_cfg,
            ));
            let dormancy_days = config.work_context.max_dormancy_days as i64;
            let _inference_loop = activity_log::inference_loop::ContextInferenceLoop::start(
                inference_engine,
                storage_pool.clone(),
                config.work_context.inference_interval_mins,
                dormancy_days,
                shutdown_token.child_token(),
            );
            info!("work context inference loop started");
        }

        let core = AppCore {
            mode,
            repos,
            storage_pool: storage_pool.clone(),
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
            signal_accumulator,
            pattern_detector,
            intervention_router,
            feedback_tracker,
            user_situation: Some(user_situation),
            coaching_service: coaching_service.map(|cs| Arc::new(Mutex::new(cs))),
            cognitive_provider,
            pipeline_broadcast: Some(pipeline_broadcast_tx),
            event_log_repo: Some(cognitive::EventLogRepo::new(storage_pool.inner().clone())),
            consecutive_coaching_ignores: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            activity_ingestion_service: Some(Arc::clone(&activity_svc)),
        };

        // Start ActivityLogSubscriber for domain event normalization.
        let _activity_subscriber = activity_log::ActivityLogSubscriber::start(
            &domain_event_bus,
            activity_svc,
            shutdown_token.clone(),
        );

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

        // Spawn event log persistence — writes domain & pipeline events to DB.
        if let Some(ref event_log_repo) = core.event_log_repo {
            spawn_event_log_persistence(
                event_log_repo.clone(),
                core.domain_event_bus.as_ref().expect("initialized above"),
                core.pipeline_broadcast.as_ref().expect("initialized above"),
                &shutdown_token,
            );
        }

        // Spawn periodic situation recomputation (every 2 min).
        spawn_situation_recompute(
            core.productivity_repos.clone(),
            core.repos.clone(),
            core.intervention_router.clone(),
            core.user_situation.clone(),
            &shutdown_token,
        );

        let pipeline_rx = core
            .pipeline_broadcast
            .as_ref()
            .expect("pipeline broadcast initialized above")
            .subscribe();

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

// ── Cron job name constants ──────────────────────────────────────────────────
// Shared between `register_cron_callbacks` and `ensure_cron_jobs` to prevent
// silent mismatches from typos.
const JOB_FOCUS_CHECK: &str = "todo_focus_check";
const JOB_DAILY_DIGEST: &str = "todo_daily_digest";
const JOB_OVERDUE_CHECK: &str = "todo_overdue_check";
const JOB_WEEKLY_REFLECTION: &str = "__klyntbot_cognitive_weekly_reflection";
const JOB_WEEKLY_REPORT: &str = "__klyntbot_weekly_report";
const JOB_DAILY_PLANNING: &str = "__klyntbot_daily_planning";
const JOB_FINANCE_DAILY_REVIEW: &str = "__klyntbot_finance_daily_review";
const JOB_FINANCE_BUDGET_CHECK: &str = "__klyntbot_finance_budget_check";
const JOB_FINANCE_PRICE_REFRESH: &str = "__klyntbot_finance_price_refresh";
const JOB_FINANCE_HEALTH_CHECK: &str = "__klyntbot_finance_health_check";

/// Register individual cron handlers (must be called before wrapping CronService in Arc).
fn register_cron_callbacks(
    cron_service: &mut CronService,
    repos: &Repos,
    notification_dispatcher: &Arc<agent::NotificationDispatcher>,
    config: &config::Config,
    bus: &Arc<MessageBus>,
    cognitive_provider: Option<providers::DynProvider>,
) {
    let rt = tokio::runtime::Handle::current();

    // ── todo_focus_check ─────────────────────────────────────────────────
    {
        let todo_repo = repos.actions.clone();
        let dispatcher = Arc::clone(notification_dispatcher);
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_FOCUS_CHECK,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let dispatcher = Arc::clone(&dispatcher);
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
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
                    })
                })
            }),
        );
    }

    // ── todo_daily_digest ────────────────────────────────────────────────
    {
        let todo_repo = repos.actions.clone();
        let dispatcher = Arc::clone(notification_dispatcher);
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_DAILY_DIGEST,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let dispatcher = Arc::clone(&dispatcher);
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
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
                    })
                })
            }),
        );
    }

    // ── todo_overdue_check ───────────────────────────────────────────────
    {
        let todo_repo = repos.actions.clone();
        let dispatcher = Arc::clone(notification_dispatcher);
        let config_focus = config.todo.focus.clone();
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_OVERDUE_CHECK,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let dispatcher = Arc::clone(&dispatcher);
                let config_focus = config_focus.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
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
                    })
                })
            }),
        );
    }

    // ── __klyntbot_cognitive_weekly_reflection ───────────────────────────
    {
        let pool = repos.pool().clone();
        let cog_config = config.clone();
        let cog_provider = cognitive_provider;
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_WEEKLY_REFLECTION,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
                        let episodic_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                        let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());

                        let (reflection_handler, consolidation_handler) =
                            crate::handlers::cognitive::build_reflection_handlers(
                                &cog_provider,
                                &cog_config,
                            );

                        match cognitive::reflection::run_weekly_reflection(
                            reflection_handler.as_ref(),
                            consolidation_handler.as_ref(),
                            &fact_repo,
                            &episodic_repo,
                            &rule_repo,
                            None,
                        )
                        .await
                        {
                            Ok(output) => {
                                info!(
                                    "Weekly reflection complete: {} facts, {} rules — {}",
                                    output.fact_updates.len(),
                                    output.rule_updates.len(),
                                    output.summary,
                                );
                                Ok(Some(format!(
                                    "Weekly reflection: {} fact updates, {} rule updates",
                                    output.fact_updates.len(),
                                    output.rule_updates.len(),
                                )))
                            }
                            Err(e) => {
                                error!("Weekly reflection failed: {e}");
                                Ok(Some(format!("Weekly reflection failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── __klyntbot_* bus-routed jobs (shared handler) ────────────────────
    //
    // These jobs publish an InboundMessage to the bus, routing to the agent.
    // A single fallback handler dispatches by job name.
    {
        let bus = bus.clone();
        let rt = rt.clone();
        cron_service.set_callback(Arc::new(move |job: &scheduling::CronJob| {
            let bus = Arc::clone(&bus);
            let job_name = job.name.clone();
            tokio::task::block_in_place(|| {
                rt.block_on(async move {
                    let (channel, msg_text) = match job_name.as_str() {
                        JOB_WEEKLY_REPORT => (
                            "weekly_report",
                            "Generate weekly progress report using the weekly-report skill",
                        ),
                        JOB_DAILY_PLANNING => ("daily_planning", "/daily-planning"),
                        JOB_FINANCE_DAILY_REVIEW => (
                            "finance_daily_review",
                            "Run finance daily review and send summary",
                        ),
                        JOB_FINANCE_BUDGET_CHECK => (
                            "finance_budget_check",
                            "Check budget thresholds and send alerts",
                        ),
                        JOB_FINANCE_PRICE_REFRESH => {
                            ("finance_price_refresh", "Refresh investment prices")
                        }
                        JOB_FINANCE_HEALTH_CHECK => {
                            ("finance_health_check", "Run finance data health check")
                        }
                        _ => return Ok(None),
                    };
                    let msg =
                        bus::InboundMessage::new("system", "cron", channel, msg_text.to_string());
                    bus.publish_inbound(msg).await.map_err(|e| {
                        common::KlyntbotError::Bus(format!(
                            "Failed to publish {job_name} message: {e}"
                        ))
                    })?;
                    Ok(Some(format!("{job_name} triggered")))
                })
            })
        }));
    }
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
                    .add_job(
                        $name,
                        $schedule,
                        $msg,
                        false,
                        None,
                        None,
                        false,
                        scheduling::CronOrigin::System,
                    )
                    .await?;
            }
        };
    }

    ensure_job!(
        JOB_FOCUS_CHECK,
        scheduling::CronSchedule::Every {
            every_ms: 30 * 60 * 1000,
        },
        "Check focus task deadlines"
    );
    ensure_job!(
        JOB_DAILY_DIGEST,
        scheduling::CronSchedule::Cron {
            expr: "0 9 * * *".to_string(),
            tz: None,
        },
        "Daily task summary"
    );
    ensure_job!(
        JOB_OVERDUE_CHECK,
        scheduling::CronSchedule::Every {
            every_ms: 60 * 60 * 1000,
        },
        "Check for overdue focus tasks"
    );
    ensure_job!(
        JOB_WEEKLY_REPORT,
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
                JOB_DAILY_PLANNING,
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
            JOB_WEEKLY_REFLECTION,
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
                JOB_FINANCE_DAILY_REVIEW,
                scheduling::CronSchedule::Cron {
                    expr: cron_expr,
                    tz: None,
                },
                "Daily financial review"
            );
        }
        ensure_job!(
            JOB_FINANCE_BUDGET_CHECK,
            scheduling::CronSchedule::Every {
                every_ms: 6 * 60 * 60 * 1000,
            },
            "Check budget thresholds"
        );
        if config.finance.price_refresh.enabled {
            ensure_job!(
                JOB_FINANCE_PRICE_REFRESH,
                scheduling::CronSchedule::Every {
                    every_ms: config.finance.price_refresh.interval_hours as u64 * 60 * 60 * 1000,
                },
                "Refresh investment prices"
            );
        }
        ensure_job!(
            JOB_FINANCE_HEALTH_CHECK,
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

/// Spawn background tasks that persist domain events and pipeline events to the DB.
fn spawn_event_log_persistence(
    repo: cognitive::EventLogRepo,
    domain_bus: &Arc<DomainEventBus>,
    pipeline_tx: &tokio::sync::broadcast::Sender<cognitive::PipelineEvent>,
    shutdown: &CancellationToken,
) {
    // Domain events → domain_event_log
    {
        let repo = repo.clone();
        let mut rx = domain_bus.subscribe();
        let token = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    result = rx.recv() => {
                        match result {
                            Ok(event) => {
                                let salience = cognitive::salience::evaluate_salience(&event);
                                let domain = domain_for_event(&event);
                                let salience_str = match salience {
                                    cognitive::types::SalienceVerdict::Extract => "extract",
                                    cognitive::types::SalienceVerdict::Accumulate => "accumulate",
                                    cognitive::types::SalienceVerdict::Discard => "discard",
                                };
                                let event_type = format!("{:?}", event)
                                    .split('{')
                                    .next()
                                    .unwrap_or("Unknown")
                                    .trim()
                                    .to_string();
                                let payload = serde_json::to_string(&event).unwrap_or_default();
                                let ts = chrono::Utc::now().to_rfc3339();
                                let id = uuid::Uuid::new_v4().to_string();

                                if let Err(e) = repo
                                    .insert_domain_event(&id, &event_type, domain, salience_str, &payload, &ts)
                                    .await
                                {
                                    warn!("failed to persist domain event: {e}");
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!("event log persistence lagged by {n} domain events");
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });
    }

    // Pipeline events → pipeline_event_log
    {
        let repo = repo.clone();
        let mut rx = pipeline_tx.subscribe();
        let token = shutdown.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    result = rx.recv() => {
                        match result {
                            Ok(pe) => {
                                let ts = chrono::Utc::now().to_rfc3339();
                                let id = uuid::Uuid::new_v4().to_string();

                                let result = match &pe {
                                    cognitive::PipelineEvent::Extraction {
                                        observation,
                                        facts_extracted,
                                        ..
                                    } => {
                                        repo.insert_pipeline_event(
                                            &id,
                                            "extraction",
                                            Some(observation.as_str()),
                                            Some(*facts_extracted as i64),
                                            None,
                                            None,
                                            &ts,
                                        )
                                        .await
                                    }
                                    cognitive::PipelineEvent::Consolidation {
                                        operation,
                                        fact,
                                        ..
                                    } => {
                                        repo.insert_pipeline_event(
                                            &id,
                                            "consolidation",
                                            None,
                                            None,
                                            Some(operation.as_str()),
                                            Some(fact.as_str()),
                                            &ts,
                                        )
                                        .await
                                    }
                                    _ => {
                                        // BatchStarted, DeadLetterQueued, DeadLetterReprocessed — log but don't persist
                                        continue;
                                    }
                                };

                                if let Err(e) = result {
                                    warn!("failed to persist pipeline event: {e}");
                                }
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                                warn!("event log persistence lagged by {n} pipeline events");
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        }
                    }
                }
            }
        });
    }
}

/// Build a `UserSituation` from real productivity + task data.
///
/// Called at startup and periodically to keep the situation accurate.
async fn build_situation_inputs(
    prod_repos: Option<&ProductivityRepos>,
    repos: &Repos,
    router: Option<&Arc<Mutex<InterventionRouter>>>,
) -> UserSituation {
    let now = Utc::now();
    let hour_of_day = now.hour();
    let today_start = now
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .map(|dt| dt.and_utc())
        .unwrap_or(now);
    let thirty_min_ago = now - Duration::minutes(30);

    let mut inputs = SituationInputs {
        hour_of_day,
        coaching_intensity: 0.7,
        ..Default::default()
    };

    // Query productivity data if available.
    if let Some(pr) = prod_repos {
        // Hours active today
        if let Ok(active_secs) = pr.events.total_active_secs(&today_start, &now).await {
            inputs.hours_active_today = active_secs as f64 / 3600.0;
        }

        // Context switches in last 30 min
        if let Ok(switches) = pr
            .events
            .count_context_switches(&thirty_min_ago, &now)
            .await
        {
            inputs.context_switches_last_30min = switches as i32;
        }

        // Historical average context switches (from last 7 days of daily summaries)
        let week_ago_date = (now - Duration::days(7)).format("%Y-%m-%d").to_string();
        let today_date = now.format("%Y-%m-%d").to_string();
        if let Ok(summaries) = pr.summaries.list_range(&week_ago_date, &today_date).await {
            if !summaries.is_empty() {
                let total_switches: i64 = summaries.iter().map(|s| s.context_switches).sum();
                inputs.historical_avg_context_switches =
                    total_switches as f64 / summaries.len() as f64;
            }
        }

        // Productive ratio today (productive_secs / total_active_secs)
        if let Ok(by_cat) = pr.events.aggregate_by_category(&today_start, &now).await {
            let total: i64 = by_cat.iter().map(|(_, secs)| *secs).sum();
            if total > 0 {
                // Categories named "productive" or starting with "coding"/"development" are productive.
                // For now, just use the ratio of non-idle time vs total, or check daily summary.
                if let Ok(Some(summary)) = pr.summaries.get(&today_date).await {
                    let total_work =
                        summary.productive_secs + summary.distracting_secs + summary.neutral_secs;
                    if total_work > 0 {
                        inputs.productive_ratio_today =
                            summary.productive_secs as f64 / total_work as f64;
                    }
                }
            }
        }

        // Distraction count in last 30 min
        if let Ok(patterns) = pr
            .distraction_patterns
            .list_range(&today_date, &today_date)
            .await
        {
            let recent_count = patterns
                .iter()
                .filter(|p| p.created_at >= thirty_min_ago)
                .count();
            inputs.distraction_count_last_30min = recent_count as i32;
        }

        // Focus session status
        if let Ok(Some(session)) = pr.sessions.get_active().await {
            inputs.is_in_focus_session = true;
            inputs.focus_quality = session.quality_score;

            // mins_since_break = time since focus session started (no break during focus)
            let focus_mins = (now - session.started_at).num_minutes() as f64;
            inputs.mins_since_break = focus_mins;
        } else {
            // mins_since_break: time since last idle event
            if let Ok(idle_secs) = pr.events.total_idle_secs(&today_start, &now).await {
                if idle_secs > 0 {
                    // Check most recent events to find last idle gap
                    if let Ok(recent) = pr.events.list_recent(50).await {
                        let last_idle = recent.iter().find(|e| e.is_idle);
                        if let Some(idle_event) = last_idle {
                            let idle_end = idle_event.ended_at.unwrap_or(now);
                            inputs.mins_since_break = (now - idle_end).num_minutes() as f64;
                        } else {
                            // No idle events found, use hours active as proxy
                            inputs.mins_since_break = inputs.hours_active_today * 60.0;
                        }
                    }
                } else {
                    // No idle time at all today
                    inputs.mins_since_break = inputs.hours_active_today * 60.0;
                }
            }
        }
    }

    // Task pressure: overdue tasks
    if let Ok(overdue) = repos.actions.overdue().await {
        inputs.overdue_task_count = overdue.len() as i32;
    }

    // Tasks due within 24h
    let tomorrow = now + Duration::hours(24);
    let filter = storage::ActionFilter {
        due_after: Some(now),
        due_before: Some(tomorrow),
        ..Default::default()
    };
    if let Ok(upcoming) = repos.actions.list(&filter).await {
        inputs.tasks_due_within_24h = upcoming.len() as i32;
    }

    // Dismissals from intervention router
    if let Some(router) = router {
        let r = router.lock().await;
        inputs.recent_dismissals = r.total_dismissals();
    }

    // Peak hour match — default to common productive hours (9-12, 14-17)
    // A future enhancement could read this from semantic facts.
    let is_peak = matches!(hour_of_day, 9..=12 | 14..=17);
    inputs.peak_hour_match = is_peak;

    compute_situation(&inputs)
}

/// Spawn a periodic task that recomputes UserSituation from real data every 2 minutes.
fn spawn_situation_recompute(
    prod_repos: Option<ProductivityRepos>,
    repos: Repos,
    router: Option<Arc<Mutex<InterventionRouter>>>,
    situation: Option<Arc<Mutex<UserSituation>>>,
    shutdown: &CancellationToken,
) {
    let situation = match situation {
        Some(s) => s,
        None => return,
    };
    let token = shutdown.clone();

    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(120));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        // Skip the immediate first tick (initial situation already computed).
        interval.tick().await;
        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let new_sit = build_situation_inputs(
                        prod_repos.as_ref(),
                        &repos,
                        router.as_ref(),
                    ).await;
                    debug!(
                        energy = format!("{:.2}", new_sit.energy_level),
                        focus = format!("{:.2}", new_sit.focus_state),
                        hours_active = format!("{:.1}", new_sit.hours_active_today),
                        "situation recomputed"
                    );
                    *situation.lock().await = new_sit;
                }
                _ = token.cancelled() => break,
            }
        }
    });
}

/// Map a DomainEvent to its domain string (shared with dev_server).
fn domain_for_event(event: &bus::DomainEvent) -> &'static str {
    match event {
        bus::DomainEvent::TaskCreated { .. }
        | bus::DomainEvent::TaskCompleted { .. }
        | bus::DomainEvent::TaskDeferred { .. }
        | bus::DomainEvent::GoalProgress { .. }
        | bus::DomainEvent::TaskDecomposed { .. }
        | bus::DomainEvent::TaskExecutionStarted { .. }
        | bus::DomainEvent::TaskExecutionCompleted { .. }
        | bus::DomainEvent::TaskExecutionFailed { .. }
        | bus::DomainEvent::TaskBlocked { .. }
        | bus::DomainEvent::TaskUnblocked { .. }
        | bus::DomainEvent::DayPlanGenerated { .. }
        | bus::DomainEvent::ProactiveSuggestionCreated { .. }
        | bus::DomainEvent::TaskFocusStarted { .. }
        | bus::DomainEvent::TaskFocusEnded { .. }
        | bus::DomainEvent::EstimationRecorded { .. }
        | bus::DomainEvent::TaskExecutionProgress { .. } => "work",
        bus::DomainEvent::ActivitySessionCompleted { .. }
        | bus::DomainEvent::FocusSessionStarted { .. }
        | bus::DomainEvent::FocusSessionEnded { .. }
        | bus::DomainEvent::DistractionDetected { .. }
        | bus::DomainEvent::ProductivityScoreComputed { .. } => "energy",
        bus::DomainEvent::TransactionRecorded { .. } | bus::DomainEvent::BudgetAlert { .. } => {
            "finance"
        }
        bus::DomainEvent::UserStatedFact { .. } => "general",
        bus::DomainEvent::UserCorrectedAI { .. } => "learning",
        bus::DomainEvent::CoachingFeedback { .. } => "coaching",
        bus::DomainEvent::ChatTurnCompleted { .. } => "general",
        bus::DomainEvent::NoteCreated { .. } | bus::DomainEvent::NoteUpdated { .. } => "notes",
        bus::DomainEvent::SessionCreated { .. }
        | bus::DomainEvent::SessionEnded { .. }
        | bus::DomainEvent::QualityScored { .. } => "energy",
        bus::DomainEvent::BehavioralPatternDetected { .. } => "learning",
        bus::DomainEvent::PredictiveAlert { .. } | bus::DomainEvent::NarrativeGenerated { .. } => {
            "general"
        }
        _ => "general",
    }
}
