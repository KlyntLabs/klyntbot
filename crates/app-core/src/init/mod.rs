mod agent;
pub mod ai_pipeline;
mod channels;
pub(crate) mod cron;
pub mod event_channels;
mod storage;

use std::sync::Arc;

use ::agent::AgentLoop;
use ::channels::ChannelManager;
use bus::MessageBus;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::info;

use crate::events::{AppEventEmitter, NoopEmitter};
use crate::state::AppCore;
use event_channels::EventChannels;

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
    #[tracing::instrument(skip(notification_sender, event_emitter, provider_override), err)]
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
            metric_registry: reforge_metric_registry,
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
        let user_situation = Arc::new(tokio::sync::Mutex::new(
            ::cognitive::situation::UserSituation::default(),
        ));
        let active_view: Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>> =
            Arc::new(tokio::sync::RwLock::new(None));
        let (pipeline_broadcast_tx, _) =
            tokio::sync::broadcast::channel::<::cognitive::PipelineEvent>(64);

        let shutdown_token = CancellationToken::new();
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
            user_situation: Some(Arc::clone(&user_situation)),
            active_view: Some(Arc::clone(&active_view)),
            event_emitter: event_emitter.clone(),
            notification_sender: notification_sender.clone(),
            pipeline_broadcast: Some(pipeline_broadcast_tx.clone()),

            shutdown_token: shutdown_token.clone(),
        };

        let mut host_builder = crate::plugin::host::FeatureHostBuilder::new()
            .with_plugins(crate::plugins::all_plugins())
            .with_handle(Arc::clone(&tracing_registry))
            .with_handle(Arc::clone(&active_view))
            .with_handle(Arc::clone(&domain_event_bus))
            .with_handle(Arc::new(pipeline_broadcast_tx.clone()));
        if let Some(ref cp) = cognitive_provider {
            host_builder = host_builder.with_handle(Arc::new(cp.clone()));
        }
        if let Some(ref engine) = appcore_embedding_engine {
            host_builder = host_builder.with_handle(Arc::clone(engine));
        }
        if let Some(ref vs) = appcore_vector_store {
            host_builder = host_builder.with_handle(Arc::new(vs.clone()));
        }
        if let Some(ref orch) = autotuner {
            host_builder = host_builder.with_handle(Arc::clone(orch));
        }
        // ── Start CronExecutor subscriber loop ───────────────────────────
        // Must start before the FeatureHost is built: TemporalPlugin starts the
        // TemporalScheduler during build() and can publish AlarmFired events on
        // startup reconciliation. CronExecutor::start() subscribes to a broadcast
        // bus, which does not replay events emitted before the subscription —
        // so subscribing late would silently drop those startup cron fires.
        let _cron_executor_handle = cron_executor.start(shutdown_token.clone());
        info!("CronExecutor subscriber started");

        let mut host_result = host_builder
            .build(&plugin_deps)
            .await
            .map_err(|e| e.to_string())?;

        info!("FeatureHost built");

        // Fill the reforge metric-registry slot now that plugins have registered
        // their metrics. The nightly reforge job (registered in init_cron, above)
        // reads this slot at fire time so its feedback collector sees feature metrics.
        let _ = reforge_metric_registry.set(host_result.build_metric_registry());

        // Extract activity service from host (created by ActivityLogPlugin).
        let activity_svc = host_result
            .host
            .get::<activity_log::ActivityIngestionService>()
            .expect("ActivityLogPlugin must be registered");

        // Extract cognitive repos from plugins to eliminate agent-side duplication.
        let cog_init = host_result
            .host
            .get::<crate::plugins::cognitive::CognitiveInitResult>();
        let cognitive_fact_repo = cog_init.as_ref().map(|r| r.semantic_fact_repo.clone());
        let cognitive_entity_repo = cog_init.as_ref().map(|r| r.entity_repo.clone());
        let cognitive_embedder = cog_init
            .as_ref()
            .and_then(|r| r.cognitive_fact_embedder.clone());

        // Tool kit + hook engine were built by BashToolkitPlugin in Phase 4 and
        // stashed in the FeatureHost; inject them into the agent at construction
        // so the runtime is never observably half-wired.
        let tool_kit = host_result.host.get::<klynt_core::ToolKitBuilder>();
        let hook_engine = host_result.host.get::<klynt_hooks::HookEngine>();

        // ── Phase 5: Agent (consumes plugin-built tool registry) ──────────
        let agent::AgentResult {
            cognitive_provider: _,
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
            host_result.context_sources.take().unwrap_or_default(),
            Arc::clone(&user_situation),
            Arc::clone(&active_view),
            Arc::clone(&activity_svc),
            pipeline_broadcast_tx.clone(),
            cognitive_fact_repo,
            cognitive_entity_repo,
            cognitive_embedder,
            tool_kit,
            hook_engine,
        )
        .await?;

        // ── Phase 6: Channel manager ─────────────────────────────────────
        let channel_manager = channels::init_channels(&config, &bus)?;

        info!("FeatureHost built");

        let feature_registry = Arc::new(host_result.build_feature_registry());
        host_result.host.insert(Arc::clone(&feature_registry));

        // Wrap a clone of config for shared ownership in AppCore.
        // Mutations after this point are synced back to core.config manually.
        // ── Assemble AppCore ─────────────────────────────────────────────
        // Clone cron_repo before repos is moved into AppCore.
        let cron_repo = repos.cron.clone();
        let core = AppCore {
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

            user_situation: Some(user_situation.clone()),

            consecutive_coaching_ignores: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            event_emitter: event_emitter
                .clone()
                .unwrap_or_else(|| Arc::new(NoopEmitter)),

            host: host_result.host.clone(),
            assistant_runtime: std::sync::OnceLock::new(),
            mcp_exposure: std::sync::OnceLock::new(),
        };

        // Run plugin post-init hooks now that AppCore is assembled.
        host_result
            .run_post_init(&core)
            .await
            .map_err(|e| e.to_string())?;

        // Start the TemporalScheduler now that every plugin's post_init has
        // registered its cron handlers. Starting earlier (in TemporalPlugin::init)
        // would let a due/recovered fire be published before its handler exists,
        // dropping that execution.
        if let Some(temporal) = core
            .host
            .get::<crate::plugins::temporal::TemporalInitResult>()
        {
            crate::plugins::temporal::start_temporal_scheduler(&temporal);
        }

        // ── Phase 8: Mirror self-reflection layer (populated by MirrorPlugin) ──
        let mirror_result = host_result
            .host
            .get::<crate::plugins::mirror::MirrorInitResult>();
        let mirror_shutdown = mirror_result.as_ref().map(|r| r.shutdown.clone());

        // Store mirror shutdown token in FeatureHost for shutdown() to find.
        if let Some(token) = mirror_shutdown {
            core.host.insert(Arc::new(token));
        }

        // ── Phase 9: AI Pipeline — SignalRouter + all consumers ───────────
        tracing::info!(
            features = feature_registry.len(),
            "ai feature registry built"
        );

        // Spawn background services (agent loop + channel manager).
        spawn_background(inbound_rx, channel_manager, &agent, &shutdown_token);

        let pipeline_rx = core
            .pipeline_broadcast()
            .expect("pipeline broadcast initialized above")
            .subscribe();

        let channels = event_channels::build(&host_result.host, domain_event_bus, pipeline_rx);

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
