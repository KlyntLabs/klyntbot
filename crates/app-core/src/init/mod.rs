mod agent;
pub mod ai_pipeline;
mod channels;
pub mod coaching;
pub mod cognitive;
pub(crate) mod cron;
pub mod launcher;
pub mod productivity;
mod storage;

use std::sync::Arc;

use ::agent::AgentLoop;
use ::channels::ChannelManager;
use bus::MessageBus;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::events::{AppEventEmitter, NoopEmitter};
use crate::state::AppCore;

/// Spawn a periodic timer that calls `f` every `interval_secs` until `token` is cancelled.
pub(crate) fn spawn_periodic_timer(
    token: &CancellationToken,
    interval_secs: u64,
    f: impl Fn() + Send + 'static,
) {
    let token = token.child_token();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(interval_secs));
        loop {
            tokio::select! {
                _ = token.cancelled() => break,
                _ = interval.tick() => f(),
            }
        }
    });
}

/// Bundle of receiver channels that callers wire to their transport (Tauri, SSE, etc.).
pub struct EventChannels {
    pub intervention_rx: mpsc::Receiver<feature_coaching::router::DeliveredIntervention>,
    pub domain_event_bus: Arc<bus::DomainEventBus>,
    pub pipeline_rx: tokio::sync::broadcast::Receiver<::cognitive::PipelineEvent>,
    pub nudge_rx: Option<mpsc::Receiver<feature_productivity::types::NudgeRecord>>,
    pub dashboard_tick_rx:
        Option<tokio::sync::broadcast::Receiver<feature_productivity::ActivityTick>>,
    pub dashboard_poll_interval_secs: u64,
    pub distraction_alert_rx:
        Option<tokio::sync::mpsc::Receiver<feature_productivity::distraction::DistractionAlert>>,
}

impl AppCore {
    /// Initialize the full agent stack.
    ///
    /// Mirrors the initialization order from `serve.rs`:
    /// config → storage → bus → provider → cron → persona → agent → channels
    ///
    /// Returns `(AppCore, EventChannels)`. The caller wires `EventChannels`
    /// receivers to their transport layer (Tauri events, SSE, etc.).
    #[tracing::instrument(err)]
    pub async fn init(
        mode: common::AppMode,
        config_override: Option<config::Config>,
    ) -> Result<(Self, EventChannels), String> {
        Self::init_with_sender(mode, config_override, None, None, None).await
    }

    /// Initialize with an optional custom notification sender and event emitter.
    ///
    /// When `sender` is `Some`, OS-native notifications are routed through it
    /// (e.g. Tauri's notification plugin, which shows the app icon). When
    /// `None`, the default platform command (`osascript` / `notify-send`) is used.
    ///
    /// When `event_emitter` is `Some`, entity update events from MCP tool
    /// mutations are forwarded to the frontend. When `None`, a no-op emitter
    /// is used (CLI / standalone MCP server).
    #[tracing::instrument(
        skip(
            notification_sender,
            event_emitter,
            provider_override
        ),
        err
    )]
    pub async fn init_with_sender(
        mode: common::AppMode,
        config_override: Option<config::Config>,
        notification_sender: Option<Arc<dyn common::NotificationSender>>,
        event_emitter: Option<Arc<dyn AppEventEmitter>>,
        provider_override: Option<providers::DynProvider>,
    ) -> Result<(Self, EventChannels), String> {
        // ── Phase 1: Storage ─────────────────────────────────────────────
        let storage::StorageResult {
            config,
            storage_pool,
            repos,
            vector_store,
            note_repo,
            provider,
            provider_manager,
        } = storage::init_storage(config_override).await?;

        // Subagent zombie sweep: flip any `running` rows older than 5 min to `failed`.
        // Mirrors the zombie-session detector documented in CLAUDE.md.
        if let Err(e) = repos.subagent_instances.sweep_zombies(300_000).await {
            tracing::warn!(error = %e, "subagent zombie sweep failed at startup");
        }

        let provider = provider_override.unwrap_or(provider);

        // Keep a clone for the Distiller (constructed after agent init).
        let provider_clone = provider.clone();

        // Wire provider-degraded event forwarding to the frontend.
        // Done here (not inside init_storage) because the emitter isn't available there.
        if let Some(ref manager) = provider_manager {
            if let Some(ref emitter) = event_emitter {
                let emitter_clone = Arc::clone(emitter);
                let degraded_cb: providers::OnProviderDegraded = Arc::new(move |level| {
                    let payload = match level {
                        providers::DegradationLevel::Fallback => {
                            serde_json::json!({ "level": "fallback" })
                        }
                        providers::DegradationLevel::Offline => {
                            serde_json::json!({ "level": "offline" })
                        }
                    };
                    emitter_clone.emit_event(crate::events::PROVIDER_DEGRADED, payload);
                });
                manager.set_provider_degraded_callback(degraded_cb).await;
            }
        }

        // ── Hot-reloadable config subset (shared between agent + AppCore) ──
        let hot_config = Arc::new(RwLock::new(config::HotConfig::from(&config)));

        // ── Shared: Bus + cognitive provider + domain event bus ──────────
        // Created once here in the orchestrator. Both cron and agent receive
        // references to the same instances (matches the original single-fn flow).
        let bus = Arc::new(MessageBus::new(100));

        let cognitive_provider = providers::create_cognitive_provider(&config).ok().flatten();
        if cognitive_provider.is_some() {
            info!("cognitive provider created — using LLM handlers");
        } else {
            info!("no cognitive provider — using heuristic handlers");
        }

        // DomainEventBus — 256 slots give ~25 subscribers enough headroom for
        // bursty tool-call sequences without Lagged errors while staying bounded.
        // Payload reduction (no user_message in ChatTurnCompleted, capped
        // args_preview in ToolCallExecuted) keeps per-slot clone cost low.
        let domain_event_bus = Arc::new(bus::DomainEventBus::new(256));

        // Context update queue for live context refresher (shared between agent + background services).
        let context_update_queue = Arc::new(bus::ContextUpdateQueue::new());

        // ── Startup recovery — DND crash safety net ──────────────────────
        if let Ok(Some(dnd_row)) = repos.dnd_override.get().await {
            tracing::warn!(
                "Recovering DND state from interrupted focus session (overridden at {})",
                dnd_row.overridden_at
            );
            tracing::warn!("DND restore not yet implemented — cleared orphaned override record");
            let _ = repos.dnd_override.clear().await;
        }

        // ── Phase 2: Cron ────────────────────────────────────────────────
        let cron::CronResult {
            cron_executor,
            autotuner,
        } = cron::init_cron(
            &config,
            &repos,
            &bus,
            cognitive_provider.clone(),
            provider.clone(),
            &domain_event_bus,
            vector_store.clone(),
        )
        .await?;

        // ── Note embedding handler (before vector_store is moved into agent) ──
        let embedding_provider = if config.embedding.provider == "openai" {
            let api_key = config.providers.openai.api_key.expose().to_string();
            if api_key.is_empty() {
                tracing::warn!(
                    "Embedding provider set to 'openai' but no API key configured — falling back to local"
                );
                tools::embedding_engine::EmbeddingProvider::Local
            } else {
                tracing::info!(
                    model = %config.cognitive.openai_embedding_model,
                    "Using OpenAI embedding model"
                );
                tools::embedding_engine::EmbeddingProvider::OpenAi {
                    api_key,
                    api_base: config.embedding.api_base.clone(),
                }
            }
        } else {
            tools::embedding_engine::EmbeddingProvider::Local
        };
        let embedding_engine = Arc::new(
            tools::embedding_engine::EmbeddingEngine::with_provider(embedding_provider)
                .with_openai_model(config.cognitive.openai_embedding_model.clone()),
        );
        // Keep a clone of embedding_engine and vector_store for AppCore fields
        // (used by flashcard embedding and compute_answer_similarity).
        let appcore_embedding_engine = Some(Arc::clone(&embedding_engine));
        let appcore_vector_store = vector_store.clone();

        // Insight infrastructure, note embedding handler, cognitive fact embedder,
        // and cognitive repos are all initialized by their respective plugins.

        // ── Phase 3: Shared services (needed by plugins + agent) ─────────
        // Run activity-log migrations before plugins need the service.
        ::storage::StoragePool::run_feature_migrations(
            storage_pool.inner(),
            &activity_log::activity_log_migrations(),
        )
        .await
        .map_err(|e| format!("activity-log migration failed: {e}"))?;
        let activity_svc = Arc::new(activity_log::ActivityIngestionService::new(
            storage_pool.clone(),
            activity_log::PrivacyFilter::default(),
        ));
        let user_situation = Arc::new(tokio::sync::Mutex::new(::cognitive::situation::UserSituation::default()));
        let active_view: Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>> =
            Arc::new(tokio::sync::RwLock::new(None));
        let (pipeline_broadcast_tx, _) =
            tokio::sync::broadcast::channel::<::cognitive::PipelineEvent>(64);

        let shutdown_token = CancellationToken::new();
        let mut config = config;
        let plugin_config = Arc::new(RwLock::new(config.clone()));
        let tracing_registry = Arc::new(crate::tracing::TracingRegistry::new());

        // Idle-unload for the ONNX embedding model — check every 60s.
        {
            let engine = Arc::clone(&embedding_engine);
            spawn_periodic_timer(&shutdown_token, 60, move || {
                engine.unload_if_idle();
            });
        }

        // Periodic LanceDB compaction — merge fragment files every 30 minutes.
        if let Some(vs) = &appcore_vector_store {
            let vs_compact = vs.clone();
            spawn_periodic_timer(&shutdown_token, 1800, move || {
                let vs = vs_compact.clone();
                tokio::spawn(async move {
                    if let Err(e) = vs.optimize_all_tables().await {
                        tracing::warn!("Periodic LanceDB compaction failed: {e}");
                    }
                    common::memory::purge_freed_memory();
                });
            });
        }

        // ── Phase 4: Build FeatureHost (plugins run BEFORE agent) ─────────
        let plugin_deps = crate::plugin::context::PluginDeps {
            mode,
            config: Arc::clone(&plugin_config),
            hot_config: Arc::clone(&hot_config),
            storage_pool: storage_pool.clone(),
            repos: repos.clone(),
            provider: provider_clone,
            cognitive_provider: cognitive_provider.clone(),
            vector_store: appcore_vector_store.clone(),
            embedding_engine: appcore_embedding_engine.clone(),
            domain_event_bus: Some(Arc::clone(&domain_event_bus)),
            bus: bus.clone(),
            cron_executor: cron_executor.clone(),
            activity_svc: Some(Arc::clone(&activity_svc)),
            user_situation: Some(Arc::clone(&user_situation)),
            active_view: Some(Arc::clone(&active_view)),
            autotuner: autotuner.clone(),
            event_emitter: event_emitter.clone(),
            notification_sender: notification_sender.clone(),
            pipeline_broadcast: Some(pipeline_broadcast_tx.clone()),
            shutdown_token: shutdown_token.clone(),
        };

        let host_result = crate::plugin::host::FeatureHostBuilder::new()
            .plugin(crate::plugins::focus::FocusPlugin)
            .plugin(crate::plugins::notes::NotesPlugin)
            .plugin(crate::plugins::tasks::TasksPlugin)
            .plugin(crate::plugins::language_learning::LanguageLearningPlugin)
            .plugin(crate::plugins::learning::LearningPlugin)
            .plugin(crate::plugins::insights::InsightsPlugin)
            .plugin(crate::plugins::cognitive::CognitivePlugin)
            .plugin(crate::plugins::agent_tools::AgentToolsPlugin)
            .plugin(crate::plugins::productivity::ProductivityPlugin)
            .plugin(crate::plugins::launcher::LauncherPlugin)
            .plugin(crate::plugins::coaching::CoachingPlugin)
            .plugin(crate::plugins::mirror::MirrorPlugin)
            .plugin(crate::plugins::brain_voice::BrainVoicePlugin)
            .plugin(crate::plugins::voice::VoicePlugin)
            .plugin(crate::plugins::briefing::BriefingPlugin)
            .plugin(crate::plugins::lifecycle::LifecyclePlugin)
            .plugin(crate::plugins::notifications::NotificationPlugin)
            .plugin(crate::plugins::bash_toolkit::BashToolkitPlugin)
            .plugin(crate::plugins::temporal::TemporalPlugin)
            .plugin(crate::plugins::ai_pipeline::AiPipelinePlugin)
            .build(&plugin_deps)
            .await
            .map_err(|e| e.to_string())?;

        info!("FeatureHost built");

        // ── Phase 5: Agent (consumes plugin-built tool registry) ──────────
        let agent::AgentResult {
            cognitive_provider,
            agent,
            inbound_rx,
        } = agent::init_agent(
            &config,
            &storage_pool,
            &repos,
            provider,
            vector_store,
            &bus,
            cognitive_provider.clone(),
            &domain_event_bus,
            &cron_executor,
            &repos.cron,
            autotuner.as_ref(),
            Arc::clone(&hot_config),
            Some(Arc::clone(&context_update_queue)),
            appcore_embedding_engine.clone(),
            None,
            None,
            host_result.tools.clone(),
            Arc::clone(&user_situation),
            Arc::clone(&active_view),
            Arc::clone(&activity_svc),
            pipeline_broadcast_tx.clone(),
        )
        .await?;

        // ── Phase 6: Channel manager ─────────────────────────────────────
        let channel_manager = channels::init_channels(&config, &bus)?;

        info!("FeatureHost built");

        let feature_registry = Arc::new(host_result.build_feature_registry());

        // Wrap a clone of config for shared ownership in AppCore.
        // Mutations after this point are synced back to core.config manually.
        // ── Assemble AppCore ─────────────────────────────────────────────
        // Clone cron_repo before repos is moved into AppCore.
        let cron_repo = repos.cron.clone();
        let mut core = AppCore {
            mode,
            repos: repos.clone(),
            storage_pool: storage_pool.clone(),
            agent: Arc::clone(&agent),
            bus: bus.clone(),
            config: Arc::clone(&plugin_config),
            hot_config: Arc::clone(&hot_config),
            channel_manager: channel_manager.clone(),
            cron_executor: cron_executor.clone(),
            cron_repo,
            cron_bridge: host_result
                .host
                .get::<crate::plugins::temporal::TemporalInitResult>()
                .map(|r| Arc::new(r.cron_bridge.clone()))
                .expect("TemporalPlugin should have initialized"),
            shutdown_token: shutdown_token.clone(),
            active_streams: Arc::new(dashmap::DashMap::new()),
            pending_interactions: Arc::new(dashmap::DashMap::new()),
            note_repo,
            practice_repo: feature_notes::repo::PracticeSessionRepo::new(
                storage_pool.inner().clone(),
            ),
            productivity_repos: host_result
                .host
                .get::<feature_productivity::repos::ProductivityRepos>(),
            focus_manager: host_result.host.get::<feature_productivity::FocusManager>(),
            dnd_manager: host_result.host.get::<feature_focus::DndManager>(),
            productivity_engine: host_result
                .host
                .get::<Arc<tokio::sync::Mutex<feature_productivity::ProductivityEngine>>>()
                .map(|arc| (*arc).clone()),
            aggregator: host_result.host.get::<feature_productivity::DailyAggregator>(),
            nudge_service: host_result
                .host
                .get::<Arc<tokio::sync::Mutex<feature_productivity::NudgeService>>>()
                .map(|arc| (*arc).clone()),
            distraction_interceptor: host_result
                .host
                .get::<Arc<tokio::sync::Mutex<feature_productivity::distraction::DistractionInterceptor>>>()
                .map(|arc| (*arc).clone()),
            domain_event_bus: Some(Arc::clone(&domain_event_bus)),
            signal_accumulator: host_result
                .host
                .get::<tokio::sync::Mutex<feature_coaching::SignalAccumulator>>(),
            pattern_detector: host_result
                .host
                .get::<tokio::sync::Mutex<feature_coaching::PatternDetector>>(),
            intervention_router: host_result
                .host
                .get::<tokio::sync::Mutex<feature_coaching::InterventionRouter>>(),
            feedback_tracker: host_result
                .host
                .get::<tokio::sync::Mutex<feature_coaching::FeedbackTracker>>(),
            coaching_intervention_log_repo: host_result
                .host
                .get::<::storage::CoachingInterventionLogRepo>(),
            user_situation: Some(user_situation.clone()),
            active_view: Some(active_view),
            coaching_service: None,
            cognitive_provider: cognitive_provider.clone(),
            pipeline_broadcast: Some(pipeline_broadcast_tx.clone()),
            event_log_repo: host_result
                .host
                .get::<crate::plugins::cognitive::CognitiveInitResult>()
                .and_then(|r| r.event_log_repo.clone()),
            consecutive_coaching_ignores: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            activity_ingestion_service: Some(Arc::clone(&activity_svc)),
            event_emitter: event_emitter.clone().unwrap_or_else(|| Arc::new(NoopEmitter)),
            note_embedding_handler: host_result
                .host
                .get::<crate::plugins::notes::NotesInitResult>()
                .and_then(|r| r.note_embedding_handler.clone()),
            embedding_engine: appcore_embedding_engine,
            vector_store: appcore_vector_store,
            launcher_engine: host_result
                .host
                .get::<crate::handlers::launcher::LauncherSearchEngine>(),
            insight_service: host_result
                .host
                .get::<feature_insights::InsightService>(),
            flashcard_repo: host_result
                .host
                .get::<crate::plugins::cognitive::CognitiveInitResult>()
                .and_then(|r| r.flashcard_repo.clone()),
            knowledge_atom_repo: host_result
                .host
                .get::<crate::plugins::cognitive::CognitiveInitResult>()
                .and_then(|r| r.knowledge_atom_repo.clone()),
            review_session_repo: host_result
                .host
                .get::<crate::plugins::cognitive::CognitiveInitResult>()
                .and_then(|r| r.review_session_repo.clone()),
            deck_preference_repo: host_result
                .host
                .get::<crate::plugins::cognitive::CognitiveInitResult>()
                .and_then(|r| r.deck_preference_repo.clone()),
            autotuner: autotuner.clone(),
            temporal_scheduler: host_result
                .host
                .get::<crate::plugins::temporal::TemporalInitResult>()
                .as_ref()
                .map(|r| r.scheduler.clone()),
            _temporal_scheduler_handle: host_result
                .host
                .get::<crate::plugins::temporal::TemporalInitResult>()
                .and_then(|r| r.scheduler_handle.lock().unwrap().take()),
            _temporal_wake_subscriber: host_result
                .host
                .get::<crate::plugins::temporal::TemporalInitResult>()
                .and_then(|r| r.wake_subscriber.lock().unwrap().take()),
            _dnd_end_subscriber_handle: None,
            mirror_facade: None,
            pending_memory_repo: host_result
                .host
                .get::<::cognitive::repos::PendingMemoryRepo>()
                .map(|arc| (*arc).clone()),
            _mirror_handles: None,
            _mirror_shutdown: None,
            notification_dispatcher_handle: None,
            _config_watcher_token: None,
            _data_version_watcher_token: None,
            _lifecycle_monitor: None,
            _wake_orchestrator_handle: None,
            voice_service: None,
            voice_conversation_manager: None,
            voice_loop_handle: None,
            brain_voice: None,
            journey_tracker: None,
            _ai_pipeline_router: None,
            feature_registry: feature_registry.clone(),
            tracing_registry,
            host: host_result.host.clone(),
            assistant_runtime: std::sync::OnceLock::new(),
        };

        // Run plugin post-init hooks now that AppCore is assembled.
        host_result.run_post_init(&core).await.map_err(|e| e.to_string())?;

        // ── Phase 8: Mirror self-reflection layer (populated by MirrorPlugin) ──
        let mirror_result = host_result
            .host
            .get::<crate::plugins::mirror::MirrorInitResult>();
        let mirror_facade = mirror_result
            .as_ref()
            .map(|r| Arc::clone(&r.facade));
        let _mirror_consumers = mirror_result
            .as_ref()
            .map(|r| r.consumers.clone())
            .unwrap_or_default();
        let mirror_flush_handles = mirror_result
            .as_ref()
            .and_then(|r| r.flush_handles.lock().unwrap().take());
        let mirror_shutdown = mirror_result
            .as_ref()
            .map(|r| r.shutdown.clone());

        core.mirror_facade = mirror_facade.clone();
        core._mirror_handles = mirror_flush_handles;
        core._mirror_shutdown = mirror_shutdown;

        // Retrieve journey tracker and BrainVoice from FeatureHost.
        let journey_tracker = host_result
            .host
            .get::<crate::journey::JourneyTracker>()
            .map(|arc| (*arc).clone());
        core.journey_tracker = journey_tracker;

        let brain_voice = host_result
            .host
            .get::<crate::brain_voice::BrainVoice>();
        core.brain_voice = brain_voice;

        // ── Notification Dispatcher (populated by NotificationPlugin) ─────
        let notification_dispatcher_handle = host_result
            .host
            .get::<crate::plugins::notifications::NotificationInitResult>()
            .and_then(|r| r.dispatcher_handle.lock().unwrap().take());
        core.notification_dispatcher_handle = notification_dispatcher_handle;

        // ── Start CronExecutor subscriber loop ───────────────────────────
        // Must start before AppCore is assembled so the executor can process
        // AlarmFired events from TemporalScheduler immediately after startup.
        let _cron_executor_handle = cron_executor.start(shutdown_token.clone());
        info!("CronExecutor subscriber started");


        // ── Phase 9: AI Pipeline — SignalRouter + all consumers ───────────
        tracing::info!(
            features = feature_registry.len(),
            "ai feature registry built"
        );

        // ── Phase 9: AI Pipeline (populated by AiPipelinePlugin) ───────────
        let ai_pipeline_router = host_result
            .host
            .get::<crate::plugins::ai_pipeline::AiPipelineInitResult>()
            .map(|r| Arc::clone(&r.router));
        core._ai_pipeline_router = ai_pipeline_router;

        // ── Voice service (populated by VoicePlugin) ────────────────────
        if let Some(voice_result) = host_result.host.get::<crate::plugins::voice::VoiceInitResult>() {
            core.voice_service = voice_result.voice_service.clone();
            core.voice_conversation_manager = voice_result.voice_conversation_manager.clone();
            core.voice_loop_handle = voice_result.voice_loop_handle.lock().unwrap().take();
        }

        // ── Lifecycle (populated by LifecyclePlugin) ──────────────────────
        if let Some(lifecycle_result) = host_result.host.get::<crate::plugins::lifecycle::LifecycleInitResult>() {
            core._config_watcher_token = lifecycle_result.config_watcher_token.clone();
            core._lifecycle_monitor = lifecycle_result.lifecycle_monitor.lock().unwrap().take();
            core._wake_orchestrator_handle = lifecycle_result.wake_orchestrator_handle.lock().unwrap().take();
        }

        // Spawn background services (agent loop + channel manager).
        spawn_background(inbound_rx, channel_manager, &agent, &shutdown_token);

        let pipeline_rx = core
            .pipeline_broadcast
            .as_ref()
            .expect("pipeline broadcast initialized above")
            .subscribe();

        let prod_bundle = host_result
            .host
            .get::<crate::plugins::productivity::ProductivityInitResult>();

        let channels = EventChannels {
            intervention_rx: host_result
                .host
                .get::<crate::plugins::coaching::CoachingInitResult>()
                .and_then(|b| b.intervention_rx.lock().unwrap().take())
                .expect("coaching plugin always provides intervention_rx"),
            domain_event_bus,
            pipeline_rx,
            nudge_rx: prod_bundle
                .as_ref()
                .and_then(|b| b.nudge_rx.lock().unwrap().take()),
            dashboard_tick_rx: prod_bundle
                .as_ref()
                .and_then(|b| b.dashboard_tick_rx.lock().unwrap().take()),
            dashboard_poll_interval_secs: prod_bundle
                .as_ref()
                .map(|b| b.dashboard_poll_interval_secs)
                .unwrap_or(60),
            distraction_alert_rx: prod_bundle
                .as_ref()
                .and_then(|b| b.distraction_alert_rx.lock().unwrap().take()),
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
                    tracing::error!("agent loop error: {}", e);
                }
            }
            _ = token.cancelled() => {
                info!("agent loop shutdown via token");
            }
        }
    });

    let cm_token = shutdown_token.child_token();
    tokio::spawn(async move {
        tokio::select! {
            result = async { channel_manager.lock().await.start_all().await } => {
                if let Err(e) = result {
                    tracing::error!("channel manager error: {}", e);
                }
            }
            _ = cm_token.cancelled() => {
                info!("channel manager shutdown via token");
            }
        }
    });

    info!("background services spawned");
}
