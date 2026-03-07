//! Central application state — embeds AgentLoop, Bus, Channels, Cron within Tauri.
//!
//! Mirrors the initialization sequence from `cli/src/serve.rs` but adapted for
//! the Tauri desktop lifecycle.

use std::sync::Arc;

use agent::{AgentLoop, PersonaManager};
use bus::{DomainEventBus, MessageBus};
use channels::ChannelManager;
use cognitive::situation::UserSituation;
use common::FormResponse;
use desktop_shared::errors::ApiError;
use desktop_shared::events;
use feature_coaching::{FeedbackTracker, InterventionRouter, PatternDetector, SignalAccumulator};
use feature_notes::repo::NoteRepo;
use feature_productivity::dashboard_emitter::{DashboardEmitter, DashboardEvent};
use feature_productivity::repos::ProductivityRepos;
use feature_productivity::{DailyAggregator, FocusManager, NudgeService, ProductivityEngine};
use scheduling::CronService;
use storage::{Repos, StoragePool, VectorStore};
use tauri::Emitter;
use tokio::sync::{mpsc, oneshot, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Central application state managed by Tauri.
pub struct AppCore {
    pub repos: Repos,
    pub agent: Arc<AgentLoop>,
    pub bus: Arc<MessageBus>,
    pub persona_manager: Arc<RwLock<PersonaManager>>,
    pub config: RwLock<config::Config>,

    // Private — managed internally
    channel_manager: Arc<Mutex<ChannelManager>>,
    cron_service: Arc<CronService>,
    shutdown_token: CancellationToken,
    /// Active streaming cancellation tokens keyed by session_key.
    pub active_streams: Arc<dashmap::DashMap<String, CancellationToken>>,
    /// Pending ask_user interaction oneshot senders keyed by session_key.
    /// Value is (request_id, sender). Only one interaction can be pending per session
    /// because the ask_user tool blocks the agent loop until answered.
    pub pending_interactions:
        Arc<dashmap::DashMap<String, (String, oneshot::Sender<FormResponse>)>>,
    /// Notes repo (always available).
    pub note_repo: NoteRepo,
    /// Productivity repos (None if feature is disabled or no pool).
    productivity_repos: Option<ProductivityRepos>,
    /// Focus session manager (None if feature is disabled or no pool).
    focus_manager: Option<Arc<FocusManager>>,
    /// Productivity engine — owns tracker + all subscribers (macOS only, None if disabled).
    productivity_engine: Option<Arc<Mutex<ProductivityEngine>>>,
    /// Daily aggregator for live productivity summaries.
    aggregator: Option<Arc<DailyAggregator>>,
    /// Nudge service for break reminders and burnout alerts.
    nudge_service: Option<Arc<Mutex<NudgeService>>>,
    /// Distraction interceptor for overlay decisions.
    distraction_interceptor:
        Option<Arc<Mutex<feature_productivity::distraction::DistractionInterceptor>>>,
    /// Cognitive domain event bus.
    pub domain_event_bus: Option<Arc<DomainEventBus>>,
    /// Coaching signal accumulator.
    signal_accumulator: Option<Arc<Mutex<SignalAccumulator>>>,
    /// Coaching pattern detector.
    pattern_detector: Option<Arc<Mutex<PatternDetector>>>,
    /// Coaching intervention router.
    intervention_router: Option<Arc<Mutex<InterventionRouter>>>,
    /// Coaching feedback tracker.
    feedback_tracker: Option<Arc<Mutex<FeedbackTracker>>>,
    /// Cached user situation.
    user_situation: Option<Arc<Mutex<UserSituation>>>,
    /// Coaching service — processes domain events through coaching pipeline.
    coaching_service: Option<Arc<Mutex<feature_coaching::CoachingService>>>,
    /// Whether a cognitive LLM provider is active (affects component status reporting).
    pub has_cognitive_provider: bool,
}

impl AppCore {
    /// Initialize the full agent stack.
    ///
    /// Mirrors the initialization order from `serve.rs`:
    /// config → storage → bus → provider → cron → persona → agent → channels
    pub async fn init(app_handle: tauri::AppHandle) -> Result<Self, String> {
        // 1. Load config
        let mut config = config::load_with_env_overrides()
            .await
            .map_err(|e| format!("config load failed: {e}"))?;
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

        // 3. Create LLM provider
        let (provider, resolved_model) = providers::create_provider(&config)
            .map_err(|e| format!("provider init failed: {e}"))?;
        config.agents.defaults.model = resolved_model;
        info!(provider = %provider.name(), "provider ready");

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

        Self::register_cron_callbacks(
            &mut cron_service,
            &repos,
            &notification_dispatcher,
            &config,
            &bus,
        );

        let cron_service = Arc::new(cron_service);
        Self::ensure_cron_jobs(&cron_service, &config)
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
        let (pipeline_tx, mut pipeline_rx) =
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

        // 8. Channel manager
        let channel_manager = Arc::new(Mutex::new(
            ChannelManager::new(Arc::new(config.clone()), bus.clone())
                .map_err(|e| format!("channel manager init failed: {e}"))?,
        ));

        let shutdown_token = CancellationToken::new();

        // Initialize productivity feature (optional — requires enabled config).
        let (
            productivity_repos,
            focus_manager,
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
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
                (None, None, None, None, None, None)
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
                let categorizer =
                    feature_productivity::tracker::categorizer::Categorizer::new(categories);
                let mut engine = ProductivityEngine::new_with_bus(
                    prod_config.clone(),
                    prod_repos.clone(),
                    categorizer,
                    Some(Arc::clone(&domain_event_bus)),
                );

                // Wire auto-focus session receiver to Tauri events.
                if let Some(mut auto_focus_rx) = engine.take_auto_focus_rx() {
                    let handle = app_handle.clone();
                    let cancel = shutdown_token.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                _ = cancel.cancelled() => break,
                                Some(session) = auto_focus_rx.recv() => {
                                    let payload = events::AutoFocusPayload {
                                        started_at: session.started_at.to_rfc3339(),
                                        ended_at: session.ended_at.to_rfc3339(),
                                        duration_mins: session.total_secs / 60,
                                        dominant_app: session.dominant_app,
                                        productive_ratio: session.productive_ratio,
                                    };
                                    if let Err(e) = handle.emit(events::FOCUS_AUTO_DETECTED, payload) {
                                        warn!("failed to emit auto-focus event: {e}");
                                    }
                                }
                            }
                        }
                    });
                }

                // DashboardEmitter — forwards ticks as Tauri events.
                let tick_rx = engine.subscribe();
                let emit_handle = app_handle.clone();
                let _dashboard_emitter = DashboardEmitter::start(
                    tick_rx,
                    Box::new(move |event| {
                        let res = match event {
                            DashboardEvent::ActivitySwitch {
                                from_app,
                                to_app,
                                to_site,
                                category_type,
                            } => emit_handle.emit(
                                events::ACTIVITY_SWITCH,
                                events::ActivitySwitchPayload {
                                    from_app,
                                    to_app,
                                    to_site,
                                    category_type,
                                },
                            ),
                            DashboardEvent::ScoreUpdated {
                                score,
                                productive_secs,
                                distracting_secs,
                            } => emit_handle.emit(
                                events::SCORE_UPDATED,
                                events::ScorePayload {
                                    score,
                                    productive_secs,
                                    distracting_secs,
                                },
                            ),
                            DashboardEvent::FocusStateChanged { state, since } => emit_handle.emit(
                                events::FOCUS_STATE_CHANGED,
                                events::FocusStatePayload { state, since },
                            ),
                        };
                        if let Err(e) = res {
                            warn!("DashboardEmitter: failed to emit event: {e}");
                        }
                    }),
                    prod_config.tracking.poll_interval_secs,
                    shutdown_token.clone(),
                );

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

                // Forward nudges as Tauri events.
                Self::spawn_channel_forwarder(
                    nudge_rx,
                    &app_handle,
                    &shutdown_token,
                    |handle, nudge| {
                        if let Err(e) = handle.emit(
                            events::PRODUCTIVITY_NUDGE,
                            events::NudgePayload {
                                nudge_type: nudge.nudge_type.to_string(),
                                message: nudge.message,
                            },
                        ) {
                            warn!("failed to emit nudge event: {e}");
                        }
                    },
                );

                (
                    Some(prod_repos),
                    Some(mgr),
                    Some(engine),
                    Some(agg),
                    Some(nudge_svc),
                    Some(interceptor),
                )
            }
        } else {
            (None, None, None, None, None, None)
        };

        // Initialize coaching engine state.
        let signal_accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
        let pattern_detector = Arc::new(Mutex::new(PatternDetector::new()));
        let intervention_router = Arc::new(Mutex::new(InterventionRouter::new(Default::default())));
        let feedback_tracker = Arc::new(Mutex::new(FeedbackTracker::new()));
        let user_situation = Arc::new(Mutex::new(UserSituation {
            energy_level: 0.7,
            focus_state: 0.5,
            coaching_receptivity: 0.7,
            ..Default::default()
        }));

        // Start CoachingService — processes domain events through coaching pipeline
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

        // Forward coaching interventions to Tauri frontend
        Self::spawn_channel_forwarder(
            intervention_rx,
            &app_handle,
            &shutdown_token,
            |handle, intervention| {
                if let Err(e) =
                    handle.emit(desktop_shared::events::COACHING_INTERVENTION, &intervention)
                {
                    warn!("failed to emit coaching intervention: {e}");
                }
            },
        );

        // Forward domain events to frontend for debug dashboard
        {
            let mut event_rx = domain_event_bus.subscribe();
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                loop {
                    match event_rx.recv().await {
                        Ok(event) => {
                            let salience = cognitive::salience::evaluate_salience(&event);
                            let domain = match &event {
                                bus::DomainEvent::TaskCreated { .. }
                                | bus::DomainEvent::TaskCompleted { .. }
                                | bus::DomainEvent::TaskDeferred { .. }
                                | bus::DomainEvent::GoalProgress { .. } => "work",
                                bus::DomainEvent::ActivitySessionCompleted { .. }
                                | bus::DomainEvent::FocusSessionEnded { .. }
                                | bus::DomainEvent::DistractionDetected { .. }
                                | bus::DomainEvent::ProductivityScoreComputed { .. } => "energy",
                                bus::DomainEvent::TransactionRecorded { .. }
                                | bus::DomainEvent::BudgetAlert { .. } => "finance",
                                bus::DomainEvent::UserStatedFact { domain, .. } => domain.as_str(),
                                bus::DomainEvent::UserCorrectedAI { .. } => "learning",
                                bus::DomainEvent::CoachingFeedback { .. } => "coaching",
                            };
                            let salience_str = match salience {
                                cognitive::types::SalienceVerdict::Extract => "extract",
                                cognitive::types::SalienceVerdict::Accumulate => "accumulate",
                                cognitive::types::SalienceVerdict::Discard => "discard",
                            };
                            let payload = desktop_shared::cognitive_commands::DomainEventPayload {
                                event_type: format!("{:?}", event)
                                    .split('{')
                                    .next()
                                    .unwrap_or("Unknown")
                                    .trim()
                                    .to_string(),
                                salience: salience_str.to_string(),
                                domain: domain.to_string(),
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                payload: serde_json::to_value(&event).unwrap_or_default(),
                            };
                            let _ = app_handle_clone.emit("cognitive:domain_event", &payload);
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            warn!("debug event forwarder lagged by {n} events");
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            });
        }

        // Forward pipeline events (extraction + consolidation) to frontend
        {
            let app_handle_clone = app_handle.clone();
            tokio::spawn(async move {
                while let Some(pe) = pipeline_rx.recv().await {
                    let event_name = match &pe {
                        cognitive::PipelineEvent::Extraction { .. } => "cognitive:extraction",
                        cognitive::PipelineEvent::Consolidation { .. } => "cognitive:consolidation",
                    };
                    let _ = app_handle_clone.emit(event_name, &pe);
                }
            });
        }

        let core = Self {
            repos,
            agent: Arc::clone(&agent),
            bus: bus.clone(),
            persona_manager,
            config: RwLock::new(config.clone()),
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

        // Spawn background services immediately
        core.spawn_background(inbound_rx, channel_manager, &agent, &shutdown_token);

        Ok(core)
    }

    /// Spawn the agent loop and channel manager tasks.
    fn spawn_background(
        &self,
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

    /// Spawn a background task that receives from a channel and emits Tauri events.
    fn spawn_channel_forwarder<T: Send + 'static>(
        mut rx: mpsc::Receiver<T>,
        app_handle: &tauri::AppHandle,
        shutdown_token: &CancellationToken,
        emit_fn: impl Fn(&tauri::AppHandle, T) + Send + 'static,
    ) {
        let handle = app_handle.clone();
        let token = shutdown_token.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => break,
                    msg = rx.recv() => {
                        let Some(msg) = msg else { break };
                        emit_fn(&handle, msg);
                    }
                }
            }
        });
    }

    /// Return productivity repos or a "feature disabled" error.
    pub fn productivity_repos(&self) -> Result<&ProductivityRepos, ApiError> {
        self.productivity_repos
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return focus manager or a "feature disabled" error.
    pub fn focus_manager(&self) -> Result<&Arc<FocusManager>, ApiError> {
        self.focus_manager
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return daily aggregator or a "feature disabled" error.
    pub fn aggregator(&self) -> Result<&Arc<DailyAggregator>, ApiError> {
        self.aggregator
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "productivity feature is not enabled"))
    }

    /// Return distraction interceptor or a "not available" error.
    pub fn distraction_interceptor(
        &self,
    ) -> Result<&Arc<Mutex<feature_productivity::distraction::DistractionInterceptor>>, ApiError>
    {
        self.distraction_interceptor.as_ref().ok_or_else(|| {
            ApiError::new("NOT_AVAILABLE", "Distraction interceptor not initialized")
        })
    }

    pub fn signal_accumulator(&self) -> Result<&Arc<Mutex<SignalAccumulator>>, ApiError> {
        self.signal_accumulator
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn pattern_detector(&self) -> Result<&Arc<Mutex<PatternDetector>>, ApiError> {
        self.pattern_detector
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn intervention_router(&self) -> Result<&Arc<Mutex<InterventionRouter>>, ApiError> {
        self.intervention_router
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn feedback_tracker(&self) -> Result<&Arc<Mutex<FeedbackTracker>>, ApiError> {
        self.feedback_tracker
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn user_situation(&self) -> Result<&Arc<Mutex<UserSituation>>, ApiError> {
        self.user_situation
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "coaching engine is not available"))
    }

    pub fn domain_event_bus(&self) -> Result<&Arc<DomainEventBus>, ApiError> {
        self.domain_event_bus
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "domain event bus is not available"))
    }

    /// Graceful shutdown.
    pub async fn shutdown(&self) {
        info!("shutting down app core");
        // Stop productivity engine first to flush pending events.
        if let Some(ref engine) = self.productivity_engine {
            engine.lock().await.stop().await;
        }
        // Stop nudge service.
        if let Some(ref nudge) = self.nudge_service {
            nudge.lock().await.stop().await;
        }
        // Stop coaching service.
        if let Some(ref coaching) = self.coaching_service {
            coaching.lock().await.stop().await;
        }
        if let Err(e) = self.agent.shutdown().await {
            error!("agent shutdown error: {}", e);
        }
        self.shutdown_token.cancel();
        self.cron_service.stop().await;
        info!("app core stopped");
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
                                                &format!(
                                                    "\"{}\" — deadline approaching!",
                                                    task.title
                                                ),
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
                                "__klyntbot_daily_planning" => {
                                    ("daily_planning", "/daily-planning")
                                }
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
            if let Some(cron_expr) =
                parse_time_to_cron(&config.finance.scheduling.daily_review_time)
            {
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
                        every_ms: config.finance.price_refresh.interval_hours as u64
                            * 60
                            * 60
                            * 1000,
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
