mod agent;
pub mod ai_pipeline;
mod channels;
mod coaching;
mod cognitive;
mod cron;
mod dnd;
mod launcher;
mod productivity;
mod storage;
mod temporal_scheduler;

use std::sync::Arc;

use ::agent::cognitive_handlers::LlmExtractionHandler;
use ::agent::cognitive_handlers::{HeuristicCoachingReasonerHandler, LlmCoachingReasonerHandler};
use ::agent::AgentLoop;
use ::channels::ChannelManager;
use ::cognitive::pipeline::{
    AtomCollector, ChatTurnCollector, CoachingCollector, RecallCollector, SessionCollector,
};
use bus::MessageBus;
use feature_productivity::auto_focus::AutoFocusEvent;
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::events::{AppEventEmitter, NoopEmitter};
use crate::state::AppCore;

/// Spawn a periodic timer that calls `f` every `interval_secs` until `token` is cancelled.
fn spawn_periodic_timer(
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
    pub auto_focus_rx: Option<mpsc::Receiver<AutoFocusEvent>>,
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
    pub async fn init(
        mode: common::AppMode,
        config_override: Option<config::Config>,
    ) -> Result<(Self, EventChannels), String> {
        Self::init_with_sender(mode, config_override, None, None).await
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
    pub async fn init_with_sender(
        mode: common::AppMode,
        config_override: Option<config::Config>,
        notification_sender: Option<Arc<dyn common::NotificationSender>>,
        event_emitter: Option<Arc<dyn AppEventEmitter>>,
    ) -> Result<(Self, EventChannels), String> {
        // ── Phase 1: Storage ─────────────────────────────────────────────
        let storage::StorageResult {
            mut config,
            storage_pool,
            repos,
            vector_store,
            note_repo,
            provider,
            provider_manager,
        } = storage::init_storage(config_override).await?;

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

        // ── TemporalScheduler (Phase 2, side-by-side with CronService) ──────
        let temporal_scheduler::TemporalSchedulerResult {
            scheduler: temporal_scheduler,
            scheduler_handle: temporal_scheduler_handle,
            wake_subscriber: temporal_wake_subscriber,
            cron_bridge,
        } = temporal_scheduler::init_temporal_scheduler(
            &repos,
            Arc::clone(&domain_event_bus),
            storage_pool.clone(),
        )
        .await?;

        // ── Note embedding handler (before vector_store is moved into agent) ──
        let embedding_provider = if config.embedding.provider == "openai" {
            let api_key = config.providers.openai.api_key.expose().to_string();
            if api_key.is_empty() {
                tracing::warn!("Embedding provider set to 'openai' but no API key configured — falling back to local");
                tools::embedding_engine::EmbeddingProvider::Local
            } else {
                tracing::info!("Using OpenAI text-embedding-3-small for embeddings");
                tools::embedding_engine::EmbeddingProvider::OpenAi {
                    api_key,
                    api_base: config.embedding.api_base.clone(),
                }
            }
        } else {
            tools::embedding_engine::EmbeddingProvider::Local
        };
        let embedding_engine = Arc::new(tools::embedding_engine::EmbeddingEngine::with_provider(
            embedding_provider,
        ));
        let note_embedding_handler: Option<
            Arc<dyn feature_notes::handlers::embedding::NoteEmbeddingHandler>,
        > = if let Some(ref vs) = vector_store {
            Some(Arc::new(
                ::agent::adapters::note_embedding::NoteEmbeddingAdapter::new(
                    Arc::clone(&embedding_engine),
                    vs.clone(),
                ),
            ))
        } else {
            None
        };
        // Keep a clone of embedding_engine and vector_store for AppCore fields
        // (used by flashcard embedding and compute_answer_similarity).
        let appcore_embedding_engine = Some(Arc::clone(&embedding_engine));
        let appcore_vector_store = vector_store.clone();

        // ── Insight embedder (reuses the same EmbeddingEngine) ──
        let insight_embedder: Arc<dyn feature_insights::InsightEmbedder> =
            if let Some(ref vs) = vector_store {
                Arc::new(crate::adapters::insight_embedder::InsightEmbedderImpl::new(
                    Arc::clone(&embedding_engine),
                    vs.clone(),
                ))
            } else {
                Arc::new(feature_insights::NoopInsightEmbedder)
            };

        // ── Cross-domain searcher (LanceDB vector search across domains) ──
        let cross_domain_searcher: Arc<dyn feature_insights::CrossDomainSearcher> =
            if let Some(ref vs) = vector_store {
                Arc::new(
                    crate::adapters::cross_domain_searcher::CrossDomainSearcherImpl::new(
                        vs.clone(),
                        Arc::clone(&embedding_engine),
                        repos.tasks.clone(),
                        note_repo.clone(),
                        storage_pool.inner().clone(),
                    ),
                )
            } else {
                Arc::new(feature_insights::NoopCrossDomainSearcher)
            };

        // ── Cognitive accessor for insight context injection ──
        let cognitive_accessor: Arc<dyn feature_insights::CognitiveAccessor> = Arc::new(
            crate::adapters::cognitive_accessor::CognitiveAccessorImpl::new(
                ::cognitive::SemanticFactRepo::new(storage_pool.inner().clone()),
                ::cognitive::EpisodicMemoryRepo::new(storage_pool.inner().clone()),
                ::cognitive::ProceduralRuleRepo::new(storage_pool.inner().clone()),
                ::cognitive::repos::EntityRepo::new(storage_pool.inner().clone()),
                ::cognitive::KnowledgeAtomRepo::new(storage_pool.inner().clone()),
            ),
        );

        // ── Scope resolver for insight context ──
        let scope_resolver: Arc<dyn feature_insights::ScopeResolver> =
            Arc::new(crate::adapters::scope_resolver::ScopeResolverImpl::new(
                note_repo.clone(),
                vector_store.clone(),
            ));

        // ── Phase 3: Agent ───────────────────────────────────────────────
        let agent::AgentResult {
            cognitive_provider,
            persona_manager,
            agent,
            inbound_rx,
            pipeline_broadcast_tx,
            user_situation,
            active_view,
            activity_svc,
        } = agent::init_agent(
            &config,
            &storage_pool,
            &repos,
            provider,
            vector_store,
            &bus,
            cognitive_provider,
            &domain_event_bus,
            &cron_executor,
            &repos.cron,
            autotuner.as_ref(),
            Arc::clone(&hot_config),
            Some(Arc::clone(&context_update_queue)),
            appcore_embedding_engine.clone(),
        )
        .await?;

        // ── Phase 4: Channel manager ─────────────────────────────────────
        let channel_manager = channels::init_channels(&config, &bus)?;

        let shutdown_token = CancellationToken::new();

        // Idle-unload for the ONNX embedding model — check every 60s.
        // The model auto-unloads after 15s idle; this timer just ensures
        // the check happens. 60s keeps wakeups low while still reclaiming
        // the ~420MB model within ~75s of last use.
        {
            let engine = Arc::clone(&embedding_engine);
            spawn_periodic_timer(&shutdown_token, 60, move || {
                engine.unload_if_idle();
            });
        }

        // Periodic LanceDB compaction — merge fragment files every 30 minutes
        // to prevent unbounded RSS growth from copy-on-write fragment accumulation.
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

        // ── DND focus session manager ─────────────────────────────────
        let dnd::DndResult {
            manager: dnd_manager,
            _end_subscriber_handle: _dnd_end_subscriber_handle,
        } = dnd::init_dnd(
            &storage_pool,
            &Some(Arc::clone(&domain_event_bus)),
            &shutdown_token,
        );

        // ── Focus alarms + AlarmFired side-effects ─────────────────────
        let _focus_alarms_handle = {
            let fire_store = Arc::new(scheduling::temporal::fire_store::FireStore::new(
                repos.scheduled_fires.clone(),
            ));
            feature_tasks::focus_alarms::spawn(
                Arc::clone(&domain_event_bus),
                fire_store,
                shutdown_token.clone(),
            )
        };
        let _alarm_side_effects_handle = feature_tasks::alarm_side_effects::spawn(
            Arc::clone(&domain_event_bus),
            repos.tasks.clone(),
            shutdown_token.clone(),
        );

        // ── Phases 5 & 8: Run independent init phases concurrently ─────
        let (productivity_result, launcher_result) = tokio::join!(
            productivity::init_productivity(
                &config,
                &storage_pool,
                &domain_event_bus,
                &activity_svc,
                &cognitive_provider,
                &shutdown_token,
            ),
            launcher::init_launcher(&config, &storage_pool, &shutdown_token),
        );

        let productivity::ProductivityResult {
            dashboard_poll_interval_secs,
            productivity_repos,
            focus_manager,
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
            distraction_alert_rx,
            auto_focus_rx,
            nudge_rx,
            dashboard_tick_rx,
        } = productivity_result;

        let launcher::LauncherResult { launcher_engine } = launcher_result;

        // ── Phase 6: Coaching ────────────────────────────────────────────
        let coaching::CoachingResult {
            intervention_rx,
            signal_accumulator,
            pattern_detector,
            intervention_router,
            feedback_tracker,
            coaching_intervention_log_repo,
            intervention_tx,
        } = coaching::init_coaching(
            mode,
            &config,
            &storage_pool,
            &repos,
            productivity_repos.as_ref(),
            &user_situation,
            &domain_event_bus,
            &cognitive_provider,
            &shutdown_token,
        )
        .await;

        // ── Phase 7: Cognitive (capture, file watcher, work context) ─────
        cognitive::init_cognitive(
            &mut config,
            &storage_pool,
            &activity_svc,
            &shutdown_token,
            Arc::clone(&embedding_engine),
        )
        .await;

        let pending_memory_repo = {
            let repo = ::cognitive::repos::PendingMemoryRepo::new(storage_pool.inner().clone());
            if let Err(e) = repo.migrate().await {
                tracing::warn!("Pending memory migration failed: {e}");
            }
            Some(repo)
        };

        // ── Phase 8: Mirror self-reflection layer (before SignalRouter so consumers can be wired in)
        let (mirror_facade, mirror_consumers, mirror_flush_handles, mirror_shutdown) = {
            let mirror_repo = ::cognitive::mirror::MirrorRepo::new(storage_pool.clone());
            let narrative_handler: Option<Arc<dyn ::cognitive::mirror::NarrativeHandler>> =
                cognitive_provider.as_ref().map(|cp| {
                    let model = config
                        .cognitive
                        .model
                        .as_deref()
                        .unwrap_or(&config.agents.defaults.model)
                        .to_string();
                    Arc::new(::agent::mirror_handlers::LlmNarrativeHandler::new(
                        cp.clone(),
                        model,
                    )) as Arc<dyn ::cognitive::mirror::NarrativeHandler>
                });
            let autotuner_bridge: Option<Arc<dyn ::cognitive::mirror::AutotunerBridge>> =
                autotuner.as_ref().map(|orch| {
                    Arc::new(crate::adapters::autotuner_bridge::AppAutotunerBridge::new(
                        Arc::clone(orch),
                    )) as Arc<dyn ::cognitive::mirror::AutotunerBridge>
                });
            let episodic_repo = Some(::cognitive::EpisodicMemoryRepo::new(
                storage_pool.inner().clone(),
            ));
            let rule_repo = Some(::cognitive::ProceduralRuleRepo::new(
                storage_pool.inner().clone(),
            ));
            let trial_evaluator: Option<Arc<dyn ::cognitive::mirror::EarlyTrialEvaluator>> = Some(
                Arc::new(crate::adapters::trial_evaluator::AppTrialEvaluator::new(
                    ::storage::StrategyRepo::new(storage_pool.inner().clone()),
                )),
            );
            let started = ::cognitive::mirror::MirrorEngine::start(
                mirror_repo.clone(),
                narrative_handler,
                autotuner_bridge,
                episodic_repo,
                rule_repo,
                trial_evaluator,
            );

            // Bootstrap brain version 1 on first run
            let bootstrap_archiver =
                ::cognitive::mirror::sources::ConfigArchiverSource::new(mirror_repo.clone(), None);
            tokio::spawn(async move {
                let _ = bootstrap_archiver.bootstrap(serde_json::json!({})).await;
            });

            // Spawn retention sweep
            let retention_cancel = shutdown_token.child_token();
            let retention_handle = ::cognitive::mirror::MirrorRetentionService::spawn(
                Arc::new(mirror_repo),
                ::cognitive::mirror::MirrorRetentionConfig::default(),
                retention_cancel.clone(),
            );

            let facade = {
                let text_embedder: Arc<dyn ::cognitive::TextEmbedder> = Arc::new(
                    ::agent::TextEmbedderImpl::new(Arc::clone(&embedding_engine)),
                );
                started.facade.with_text_embedder(text_embedder)
            };

            info!(
                consumer_count = started.consumers.len(),
                "mirror self-reflection engine started"
            );

            let mut all_handles = started.flush_handles;
            all_handles.push(retention_handle);

            (
                Some(Arc::new(facade)),
                started.consumers,
                all_handles,
                started.shutdown,
            )
        };

        // ── Phase 9: AI Pipeline — SignalRouter + all consumers ───────────
        let ai_pipeline_router = {
            let observation_repo =
                ::cognitive::repos::AccumulatedObservationRepo::new(storage_pool.inner().clone());
            let entity_repo = ::cognitive::repos::EntityRepo::new(storage_pool.inner().clone());
            let episodic_repo = ::cognitive::EpisodicMemoryRepo::new(storage_pool.inner().clone());
            let extraction_handler: Option<Arc<dyn ::cognitive::ExtractionHandler>> =
                cognitive_provider.as_ref().map(|cp| {
                    let params = providers::cognitive_chat_params(&config, 1024);
                    Arc::new(LlmExtractionHandler::new(cp.clone(), params))
                        as Arc<dyn ::cognitive::ExtractionHandler>
                });
            let ingestion: Arc<dyn ai_core::SignalConsumer> =
                Arc::new(::cognitive::consumers::IngestionConsumer::new(
                    observation_repo,
                    entity_repo,
                    episodic_repo,
                    extraction_handler,
                ));

            // 5 cognitive collectors — each SignalConsumer pushes CognitiveSignals
            // to the existing consolidator tx (signal_queue).
            let (cognitive_tx, cognitive_rx): (
                ::cognitive::pipeline::SignalSender,
                ::cognitive::pipeline::SignalReceiver,
            ) = ::cognitive::pipeline::signal_queue(128);
            let chat_turn: Arc<dyn ai_core::SignalConsumer> =
                Arc::new(ChatTurnCollector::new(cognitive_tx.clone()));
            let recall: Arc<dyn ai_core::SignalConsumer> =
                Arc::new(RecallCollector::new(cognitive_tx.clone()));
            let session: Arc<dyn ai_core::SignalConsumer> = Arc::new(SessionCollector::new(
                cognitive_tx.clone(),
                repos.session_memory.clone(),
            ));
            let atom: Arc<dyn ai_core::SignalConsumer> =
                Arc::new(AtomCollector::new(cognitive_tx.clone()));
            let coaching_collector: Arc<dyn ai_core::SignalConsumer> =
                Arc::new(CoachingCollector::new(cognitive_tx.clone()));

            // Coaching
            let (coaching_signal_tx, coaching_signal_rx) = tokio::sync::mpsc::channel(256);
            let coaching_consumer: Arc<dyn ai_core::SignalConsumer> = Arc::new(
                feature_coaching::CoachingSignalConsumer::new(coaching_signal_tx),
            );

            // Build consumer list: 7 base + mirror consumers
            let mut consumers: Vec<Arc<dyn ai_core::SignalConsumer>> = vec![
                ingestion,
                chat_turn,
                recall,
                session,
                atom,
                coaching_collector,
                coaching_consumer,
            ];
            consumers.extend(mirror_consumers.iter().cloned());

            let router = ai_pipeline::start(Arc::clone(&domain_event_bus), consumers);
            info!(
                "AI pipeline SignalRouter started with {} consumers (7 base + {} mirror)",
                7 + mirror_consumers.len(),
                mirror_consumers.len()
            );

            // Launch the cognitive consolidator task (reads cognitive_rx)
            let _cognitive_handle = {
                let repo = ::cognitive::SemanticFactRepo::new(storage_pool.inner().clone());
                let rule_repo = ::cognitive::ProceduralRuleRepo::new(storage_pool.inner().clone());
                let episodic_repo =
                    ::cognitive::EpisodicMemoryRepo::new(storage_pool.inner().clone());
                tokio::spawn(async move {
                    let mut rx: ::cognitive::pipeline::SignalReceiver = cognitive_rx;
                    let episodic = Some(episodic_repo);
                    while let Some(signal) = rx.recv().await {
                        let clusters = ::cognitive::pipeline::group_signals(vec![signal]);
                        let ops = ::cognitive::pipeline::heuristic_promote(&clusters);
                        if !ops.is_empty() {
                            ::cognitive::pipeline::execute_promotions(
                                &ops, &repo, &rule_repo, &episodic, None,
                            )
                            .await;
                        }
                    }
                })
            };

            // CoachingService now reads AiSignals instead of DomainEvents
            let _coaching_service = if let (
                Some(ref acc),
                Some(ref det),
                Some(ref router),
                Some(ref fb),
                Some(ref log_repo),
            ) = (
                &signal_accumulator,
                &pattern_detector,
                &intervention_router,
                &feedback_tracker,
                &coaching_intervention_log_repo,
            ) {
                let coaching_reasoner: Arc<dyn feature_coaching::CoachingReasonerHandler> =
                    if let Some(ref cp) = cognitive_provider {
                        let params = providers::cognitive_chat_params(&config, 1024);
                        Arc::new(LlmCoachingReasonerHandler::new(cp.clone(), params))
                    } else {
                        Arc::new(HeuristicCoachingReasonerHandler)
                    };

                let coaching_cancel = shutdown_token.child_token();
                let service = feature_coaching::CoachingService::start(
                    coaching_signal_rx,
                    Arc::clone(acc),
                    Arc::clone(det),
                    Arc::clone(router),
                    Arc::clone(fb),
                    user_situation.clone(),
                    coaching_reasoner,
                    intervention_tx.clone(),
                    Some(log_repo.clone()),
                    coaching_cancel,
                );
                info!("coaching service started (reading AiSignals via CoachingSignalConsumer)");
                Some(service)
            } else {
                None
            };

            Some(router)
        };

        // ── Journey tracker (needed by BrainVoice) ──────────────────────────
        let journey_tracker = crate::journey::JourneyTracker::new(storage_pool.clone());

        // ── Phase 10: BrainVoice signal router ─────────────────────────────
        let brain_voice = {
            let feedback_repo =
                ::storage::repos::BrainSignalFeedbackRepo::new(storage_pool.inner().clone());
            let emitter_for_brain: Arc<dyn crate::events::AppEventEmitter> = event_emitter
                .clone()
                .unwrap_or_else(|| Arc::new(NoopEmitter));
            let rx = domain_event_bus.subscribe();
            let bv = crate::brain_voice::BrainVoice::start(
                rx,
                feedback_repo,
                emitter_for_brain,
                crate::brain_voice::BrainVoiceConfig::default(),
                Some(journey_tracker.clone()),
            );
            info!("BrainVoice signal router started");
            Some(bv)
        };

        // ── Morning briefing: surface unsurfaced cross-domain insights ───
        {
            let pool = storage_pool.clone();
            let bus = Arc::clone(&domain_event_bus);
            tokio::spawn(async move {
                // Small delay to let BrainVoice finish subscribing.
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;

                let svc = feature_insights::nightly_batch::NightlyBatchService::new(pool);
                match svc.get_unsurfaced_insights().await {
                    Ok(insights) if !insights.is_empty() => {
                        for insight in &insights {
                            bus.publish(bus::DomainEvent::CrossDomainDotReady {
                                source_kind: "insight".into(),
                                source_id: insight.id.to_string(),
                                source_title: "Cross-domain insight".into(),
                                target_kind: "briefing".into(),
                                target_id: insight.date.clone(),
                                target_title: insight.date.clone(),
                                confidence: 1.0,
                                tooltip: insight.insight_text.clone(),
                                detail_route: None,
                            });
                            if let Err(e) = svc.mark_surfaced(insight.id).await {
                                tracing::warn!(
                                    "failed to mark insight {} surfaced: {e}",
                                    insight.id
                                );
                            }
                        }
                        info!(
                            count = insights.len(),
                            "morning briefing: surfaced cross-domain insights"
                        );
                    }
                    Ok(_) => {}
                    Err(e) => {
                        tracing::debug!("morning briefing insight check failed: {e}");
                    }
                }
            });
        }

        // ── Notification Dispatcher ───────────────────────────────────────
        // Run migration for notification tables (notification_log, held_notifications).
        ::storage::StoragePool::run_feature_migrations(
            storage_pool.inner(),
            &[notifications::migration()],
        )
        .await
        .map_err(|e| format!("notifications migration failed: {e}"))?;

        let notification_dispatcher_handle = {
            use notifications::channel::{
                os_native::OsNativeChannel, tray::TrayChannel, ChannelRegistry,
            };
            use notifications::held::HeldReleaseService;
            use notifications::quiet_hours::QuietHoursPolicy;
            use notifications::retry::RetryPolicy;
            use notifications::NotificationDispatcher;

            let notif_cfg = &config.notifications;
            let last_active: std::sync::Arc<
                tokio::sync::RwLock<Option<(common::ChannelName, common::ChatId)>>,
            > = std::sync::Arc::new(tokio::sync::RwLock::new(None));

            let mut registry = ChannelRegistry::new();

            // OS-native channel: delegate to provided sender or fallback.
            let os_sender: std::sync::Arc<dyn common::NotificationSender> =
                match &notification_sender {
                    Some(s) => std::sync::Arc::clone(s),
                    None => std::sync::Arc::new(common::notify::OsNotificationSender),
                };
            registry.register(std::sync::Arc::new(OsNativeChannel::new(os_sender)));
            registry.register(std::sync::Arc::new(TrayChannel::new(Arc::clone(
                &domain_event_bus,
            ))));
            // Outbound channels (telegram, discord, slack) — no-op until Phase 4 wires
            // last_active updates from the message router.
            for ch_name in ["telegram", "discord", "slack", "email"] {
                registry.register(std::sync::Arc::new(
                    notifications::channel::outbound::OutboundChannel::new(
                        ch_name,
                        bus.clone(),
                        Arc::clone(&last_active),
                    ),
                ));
            }

            let quiet_hours = if notif_cfg.quiet_hours.enabled {
                // TODO(phase-3.5): wire real user timezone when config has it.
                let tz = config.timezone.as_str();
                match QuietHoursPolicy::new(notif_cfg.quiet_hours.clone(), tz) {
                    Ok(qh) => Some(qh),
                    Err(e) => {
                        tracing::warn!("quiet hours policy init failed ({e}), disabling");
                        None
                    }
                }
            } else {
                None
            };

            let sf_repo = ::storage::repos::ScheduledFiresRepo::new(storage_pool.inner().clone());
            let fire_store = scheduling::FireStore::new(sf_repo);

            let dispatcher = NotificationDispatcher::new(
                Arc::clone(&domain_event_bus),
                registry,
                notif_cfg.default_channels.clone(),
                quiet_hours,
                ::storage::repos::NotificationLogRepo::new(storage_pool.inner().clone()),
                ::storage::repos::HeldNotificationsRepo::new(storage_pool.inner().clone()),
                HeldReleaseService::new(
                    ::storage::repos::HeldNotificationsRepo::new(storage_pool.inner().clone()),
                    fire_store,
                ),
                RetryPolicy::from_config(&notif_cfg.retry),
            );
            let handle = dispatcher.start();
            info!("NotificationDispatcher started (Phase 3)");
            Some(handle)
        };

        // ── Snapshot lifecycle config before moving config into Arc ──────
        let lifecycle_config_snapshot = config.lifecycle.clone();

        // ── Wrap config for shared ownership ─────────────────────────────
        let shared_config = Arc::new(RwLock::new(config));

        // ── Config file watcher (hot-reload) ──────────────────────────────
        let config_watcher_token = crate::infrastructure::config_watcher::start_config_watcher(
            Arc::clone(&shared_config),
            Arc::clone(&hot_config),
            shutdown_token.clone(),
        );

        // Clone mirror_facade before move into AppCore (needed for VoiceService echo provider)
        let mirror_facade_for_voice = mirror_facade.clone();

        // ── Start CronExecutor subscriber loop ───────────────────────────
        // Must start before AppCore is assembled so the executor can process
        // AlarmFired events from TemporalScheduler immediately after startup.
        let _cron_executor_handle = cron_executor.start(shutdown_token.clone());
        info!("CronExecutor subscriber started");

        // ── Assemble AppCore ─────────────────────────────────────────────
        // Clone cron_repo before repos is moved into AppCore.
        let cron_repo = repos.cron.clone();
        let mut core = AppCore {
            mode,
            repos,
            storage_pool: storage_pool.clone(),
            agent: Arc::clone(&agent),
            bus: bus.clone(),
            persona_manager,
            config: Arc::clone(&shared_config),
            hot_config: Arc::clone(&hot_config),
            channel_manager: channel_manager.clone(),
            cron_executor: cron_executor.clone(),
            cron_repo,
            cron_bridge: Arc::new(cron_bridge),
            shutdown_token: shutdown_token.clone(),
            active_streams: Arc::new(dashmap::DashMap::new()),
            pending_interactions: Arc::new(dashmap::DashMap::new()),
            note_repo,
            practice_repo: feature_notes::repo::PracticeSessionRepo::new(
                storage_pool.inner().clone(),
            ),
            productivity_repos,
            focus_manager,
            dnd_manager: Some(dnd_manager),
            productivity_engine,
            aggregator,
            nudge_service,
            distraction_interceptor,
            domain_event_bus: Some(Arc::clone(&domain_event_bus)),
            signal_accumulator,
            pattern_detector,
            intervention_router,
            feedback_tracker,
            coaching_intervention_log_repo,
            user_situation: Some(user_situation),
            active_view: Some(active_view),
            coaching_service: None,
            cognitive_provider,
            pipeline_broadcast: Some(pipeline_broadcast_tx),
            event_log_repo: Some(::cognitive::EventLogRepo::new(storage_pool.inner().clone())),
            consecutive_coaching_ignores: Arc::new(std::sync::atomic::AtomicI32::new(0)),
            activity_ingestion_service: Some(Arc::clone(&activity_svc)),
            event_emitter: event_emitter.unwrap_or_else(|| Arc::new(NoopEmitter)),
            note_embedding_handler,
            embedding_engine: appcore_embedding_engine,
            vector_store: appcore_vector_store,
            launcher_engine,
            insight_service: {
                let insight_repo =
                    feature_insights::InsightReviewRepo::new(storage_pool.inner().clone());
                Some(Arc::new(feature_insights::InsightService::new(
                    insight_repo.clone(),
                    feature_insights::InsightProgressRepo::new(storage_pool.inner().clone()),
                    scope_resolver,
                    feature_insights::SmartMergeEngine::new(insight_repo),
                    feature_insights::PromptBuilder::new(Arc::clone(&cognitive_accessor)),
                    Arc::new(
                        crate::adapters::flashcard_accessor::FlashcardAccessorImpl::new(
                            storage_pool.inner().clone(),
                        ),
                    ),
                    insight_embedder,
                    cross_domain_searcher,
                    feature_insights::ProgressWeights::default(),
                    Some(Arc::clone(&domain_event_bus)),
                )))
            },
            flashcard_repo: Some(::cognitive::FlashcardRepo::new(
                storage_pool.inner().clone(),
            )),
            knowledge_atom_repo: Some(::cognitive::KnowledgeAtomRepo::new(
                storage_pool.inner().clone(),
            )),
            persona_repo: Some(::cognitive::PersonaRepo::new(storage_pool.inner().clone())),
            squad_repo: Some(::cognitive::SquadRepo::new(storage_pool.inner().clone())),
            review_session_repo: Some(::cognitive::ReviewSessionRepo::new(
                storage_pool.inner().clone(),
            )),
            deck_preference_repo: Some(::cognitive::DeckPreferenceRepo::new(
                storage_pool.inner().clone(),
            )),
            autotuner,
            temporal_scheduler: Some(temporal_scheduler),
            _temporal_scheduler_handle: Some(temporal_scheduler_handle),
            _temporal_wake_subscriber: Some(temporal_wake_subscriber),
            _dnd_end_subscriber_handle: Some(_dnd_end_subscriber_handle),
            mirror_facade,
            pending_memory_repo,
            _mirror_handles: Some(mirror_flush_handles),
            _mirror_shutdown: Some(mirror_shutdown),
            notification_dispatcher_handle,
            _config_watcher_token: Some(config_watcher_token),
            _lifecycle_monitor: None,
            _wake_orchestrator_handle: None,
            voice_service: None,
            voice_conversation_manager: None,
            voice_loop_handle: None,
            brain_voice,
            journey_tracker: Some(journey_tracker),
            _ai_pipeline_router: ai_pipeline_router,
        };

        // ── Voice service initialization ────────────────────────────────
        {
            let config_guard = shared_config.read().await;
            let voice_config = config_guard.voice.clone();
            if voice_config.enabled {
                use voice_engine::*;
                let data_dir = config_guard.data_dir_path();

                let model_manager = ModelManager::new(&data_dir);
                let has_custom_personas = voice_config
                    .output
                    .personas
                    .values()
                    .any(|p| matches!(p, config::schema::VoicePersona::Custom { .. }));

                // STT: config-driven engine selection with deployment mode
                let stt_local: Option<Arc<dyn TranscriptionEngine>> = {
                    match voice_config.input.deployment {
                        config::schema::EngineDeployment::Local => {
                            match voice_engine::Qwen3AsrEngine::new(
                                model_manager.models_dir(),
                                voice_config.input.allowed_languages.clone(),
                            ) {
                                Ok(engine) => {
                                    info!("Qwen3-ASR engine ready (lazy-load on first use)");
                                    Some(Arc::new(engine) as Arc<dyn TranscriptionEngine>)
                                }
                                Err(e) => {
                                    warn!("Failed to create Qwen3-ASR: {e}");
                                    None
                                }
                            }
                        }
                        config::schema::EngineDeployment::Cloud {
                            ref api_url,
                            ref api_key,
                        } => Some(Arc::new(voice_engine::CloudAsrEngine::new(
                            api_url.clone(),
                            api_key.expose().to_string(),
                        )) as Arc<dyn TranscriptionEngine>),
                    }
                };

                drop(config_guard);

                // TTS: config-driven with engine manager wrapping primary + system fallback.
                // Temporary ref for one-shot preload only — idle unload routes
                // through VoiceService to avoid stale refs after hot-swap.
                let mut qwen3_tts_ref: Option<Arc<voice_engine::Qwen3TtsEngine>> = None;
                let system_tts = Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir));
                let tts: Option<Arc<dyn voice_engine::TtsEngine>> = {
                    match voice_config.output.deployment {
                        config::schema::EngineDeployment::Cloud {
                            ref api_url,
                            ref api_key,
                        } => {
                            let cloud = Arc::new(voice_engine::CloudTtsEngine::new(
                                api_url.clone(),
                                api_key.expose().to_string(),
                            ));
                            let manager = voice_engine::TtsEngineManager::new(
                                cloud,
                                Some(Arc::clone(&system_tts) as Arc<dyn voice_engine::TtsEngine>),
                            );
                            Some(Arc::new(manager) as Arc<dyn voice_engine::TtsEngine>)
                        }
                        config::schema::EngineDeployment::Local => {
                            let model_dir = match voice_config.output.tts_engine {
                                config::schema::TtsEngineKind::Qwen3 => model_manager
                                    .qwen3_tts_instruct_model_dir()
                                    .or_else(|| model_manager.qwen3_tts_model_dir()),
                                config::schema::TtsEngineKind::Qwen3Base => {
                                    model_manager.qwen3_tts_model_dir()
                                }
                                config::schema::TtsEngineKind::System => None,
                            };
                            if let Some(dir) = model_dir {
                                match voice_engine::Qwen3TtsEngine::new(&dir).await {
                                    Ok(engine) => {
                                        info!("Qwen3-TTS loaded — wrapping with system fallback");
                                        let engine = Arc::new(engine);
                                        qwen3_tts_ref = Some(Arc::clone(&engine));
                                        let manager = voice_engine::TtsEngineManager::new(
                                            engine,
                                            Some(Arc::clone(&system_tts)
                                                as Arc<dyn voice_engine::TtsEngine>),
                                        );
                                        Some(Arc::new(manager) as Arc<dyn voice_engine::TtsEngine>)
                                    }
                                    Err(e) => {
                                        warn!("Qwen3-TTS failed, using system TTS: {e}");
                                        Some(Arc::clone(&system_tts)
                                            as Arc<dyn voice_engine::TtsEngine>)
                                    }
                                }
                            } else {
                                Some(Arc::clone(&system_tts) as Arc<dyn voice_engine::TtsEngine>)
                            }
                        }
                    }
                };

                // Always create VoiceService — even without a local engine yet.
                // If the model is still downloading, voice_conversation_start will wait.
                let svc_config = VoiceServiceConfig {
                    capture: capture::CaptureConfig {
                        silence_threshold: 0.01,
                        silence_duration: std::time::Duration::from_secs_f32(
                            voice_config.input.silence_threshold_secs,
                        ),
                        selected_device: voice_config.input.selected_device.clone(),
                        noise_reduction: voice_config.input.noise_reduction,
                        ..Default::default()
                    },
                    privacy_mode: crate::handlers::voice::privacy_level(
                        voice_config.input.privacy_mode,
                    ),
                    data_dir: data_dir.clone(),
                    native_audio: true,
                    output_device: voice_config.output.selected_device.clone(),
                };

                let has_local_engine = stt_local.is_some();

                // Tier 3 memory recall: construct a MemoryRetriever from
                // cognitive UnifiedMemoryService (facts + conversation recall).
                let voice_memory_retriever: Option<Arc<dyn context_engine::MemoryRetriever>> = {
                    let fact_repo =
                        ::cognitive::SemanticFactRepo::new(storage_pool.inner().clone());
                    let retriever = ::cognitive::UnifiedMemoryService::new(fact_repo)
                        .with_co_activation_repo(::cognitive::CoActivationRepo::new(
                            storage_pool.inner().clone(),
                        ));
                    Some(Arc::new(retriever) as Arc<dyn context_engine::MemoryRetriever>)
                };

                let echo_provider: Arc<dyn voice_engine::MemoryEchoProvider> =
                    Arc::new(crate::handlers::voice_echo::AppMemoryEchoProvider::new(
                        mirror_facade_for_voice,
                        voice_memory_retriever,
                    ));

                let pronunciation_provider: Option<Arc<dyn voice_engine::PronunciationProvider>> = {
                    let ll_config = shared_config.read().await;
                    if ll_config.language_learning.enabled {
                        info!("Language learning enabled — wiring pronunciation provider");
                        Some(
                            Arc::new(feature_language_learning::AppPronunciationProvider::new())
                                as Arc<dyn voice_engine::PronunciationProvider>,
                        )
                    } else {
                        None
                    }
                };

                let service = VoiceService::new(
                    stt_local,
                    tts,
                    Some(Arc::clone(&echo_provider)),
                    pronunciation_provider,
                    model_manager,
                    svc_config,
                );

                let service = Arc::new(service);
                core.voice_service = Some(Arc::clone(&service));

                // STT and TTS models are lazy-loaded on first voice use.
                // Previously eagerly preloaded at startup, but the Qwen3-ASR model
                // alone is 1.7GB — loading it alongside the embedding model and
                // WebView caused a 15GB startup memory spike. Both engines already
                // support lazy-load, so first-use latency is the only tradeoff.
                let _ = (has_local_engine, &qwen3_tts_ref); // suppress unused warnings

                // ── Voice conversation manager ──────────────────────────
                let voice_config_arc = Arc::new(RwLock::new(voice_config.clone()));
                let voice_conv_manager = Arc::new(
                    crate::handlers::voice_conversation::VoiceConversationManager::new(
                        Arc::clone(&service),
                        core.repos.clone(),
                        Arc::clone(&core.agent),
                        Arc::clone(&core.event_emitter),
                        echo_provider,
                        voice_config_arc,
                    ),
                );
                let loop_handle = voice_conv_manager.spawn_supervised_loop().await;
                core.voice_loop_handle = Some(loop_handle);
                core.voice_conversation_manager = Some(voice_conv_manager);

                {
                    let svc = Arc::clone(&service);
                    spawn_periodic_timer(&shutdown_token, 300, move || svc.try_unload_idle_stt());
                }

                // Periodic idle unload for Qwen3-TTS model (reclaim GPU memory).
                // Routes through VoiceService so hot-swapped engines are reached.
                {
                    let svc_for_idle = Arc::clone(&service);
                    spawn_periodic_timer(&shutdown_token, 300, move || {
                        svc_for_idle.try_unload_idle_tts()
                    });
                }

                if has_local_engine {
                    info!("Voice service initialized (local engine ready)");
                } else {
                    // Auto-download Qwen3 models in background on first launch.
                    // After download, hot-swap engines into the live VoiceService.
                    if matches!(
                        voice_config.input.deployment,
                        config::schema::EngineDeployment::Local
                    ) {
                        let mm_dir = data_dir.clone();
                        let svc_for_swap = Arc::clone(&service);
                        let system_tts_for_swap =
                            Arc::clone(&system_tts) as Arc<dyn voice_engine::TtsEngine>;
                        let allowed_langs = voice_config.input.allowed_languages.clone();
                        let has_custom = has_custom_personas;
                        tokio::spawn(async move {
                            let mm = voice_engine::ModelManager::new(&mm_dir);
                            use voice_engine::model_manager::Qwen3Model;
                            let svc_for_asr = Arc::clone(&svc_for_swap);
                            let svc_for_tts = Arc::clone(&svc_for_swap);
                            let (asr, tts) = tokio::join!(
                                mm.download_model_with_progress(
                                    Qwen3Model::Asr,
                                    move |downloaded, total| {
                                        svc_for_asr.try_emit_event(
                                            voice_engine::VoiceEvent::DownloadProgress {
                                                model: "Qwen3-ASR".into(),
                                                downloaded,
                                                total,
                                            },
                                        );
                                    }
                                ),
                                mm.download_model_with_progress(
                                    Qwen3Model::Tts,
                                    move |downloaded, total| {
                                        svc_for_tts.try_emit_event(
                                            voice_engine::VoiceEvent::DownloadProgress {
                                                model: "Qwen3-TTS".into(),
                                                downloaded,
                                                total,
                                            },
                                        );
                                    }
                                ),
                            );
                            if let Err(e) = &asr {
                                warn!("Qwen3-ASR download: {e}");
                            }
                            if let Err(e) = &tts {
                                warn!("Qwen3-TTS download: {e}");
                            }

                            // Hot-swap STT engine after successful download
                            if asr.is_ok() {
                                if let Ok(engine) = voice_engine::Qwen3AsrEngine::new(
                                    mm.models_dir(),
                                    allowed_langs,
                                ) {
                                    svc_for_swap.set_local_engine(Arc::new(engine)
                                        as Arc<dyn voice_engine::TranscriptionEngine>);
                                    info!("Hot-swapped Qwen3-ASR into live VoiceService");
                                }
                            }

                            // Hot-swap TTS engine after successful download
                            if let Ok(ref dir) = tts {
                                match voice_engine::Qwen3TtsEngine::new(dir).await {
                                    Ok(engine) => {
                                        let manager = voice_engine::TtsEngineManager::new(
                                            Arc::new(engine),
                                            Some(system_tts_for_swap),
                                        );
                                        svc_for_swap
                                            .set_tts_engine(Arc::new(manager)
                                                as Arc<dyn voice_engine::TtsEngine>);
                                        info!("Hot-swapped Qwen3-TTS into live VoiceService");
                                    }
                                    Err(e) => warn!(
                                        "Qwen3-TTS engine creation after download failed: {e}"
                                    ),
                                }
                            }

                            // Download 1.7B instruct model when custom personas are configured
                            if has_custom {
                                if let Err(e) = mm.download_model(Qwen3Model::TtsInstruct).await {
                                    warn!("Qwen3-TTS 1.7B instruct download: {e}");
                                }
                            }
                        });
                    }
                    info!("Voice service initialized (no local STT — Qwen3 download pending)");
                }
            }
        }

        // ── Insight progress refresh (registered post-init — deps available now) ──
        if let Some(ref insight_svc) = core.insight_service {
            let svc = Arc::clone(insight_svc);
            let note_repo_clone = core.note_repo.clone();
            let rt = tokio::runtime::Handle::current();
            cron_executor.register(
                cron::JOB_INSIGHT_REFRESH,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let svc = Arc::clone(&svc);
                    let note_repo_clone = note_repo_clone.clone();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async move {
                            match cron::refresh_insight_progress(&svc, &note_repo_clone).await {
                                Ok(Some(msg)) => Ok(Some(msg)),
                                Ok(None) => Ok(Some("No insights to refresh".to_string())),
                                Err(e) => Ok(Some(format!("Insight refresh failed: {e}"))),
                            }
                        })
                    })
                }),
            );
        }

        // ── Nightly cross-domain batch (registered post-init — needs storage pool + LLM) ──
        {
            let pool = storage_pool.clone();
            let nightly_provider = core.cognitive_provider.clone();
            let nightly_model = {
                let cfg = core.config.read().await;
                cfg.cognitive
                    .model
                    .clone()
                    .unwrap_or_else(|| cfg.agents.defaults.model.clone())
            };
            let rt = tokio::runtime::Handle::current();
            cron_executor.register(
                cron::JOB_CROSS_DOMAIN_NIGHTLY,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let pool = pool.clone();
                    let provider = nightly_provider.clone();
                    let model = nightly_model.clone();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async move {
                            match cron::run_nightly_batch(&pool, provider.as_ref(), &model).await {
                                Ok(Some(msg)) => Ok(Some(msg)),
                                Ok(None) => Ok(Some("No cross-domain dots today".to_string())),
                                Err(e) => Ok(Some(format!("Nightly batch failed: {e}"))),
                            }
                        })
                    })
                }),
            );
        }

        // ── Start lifecycle monitor (macOS only) ─────────────────────────
        #[cfg(target_os = "macos")]
        {
            if let Some(ref bus) = core.domain_event_bus {
                let lifecycle_config = lifecycle_config_snapshot.clone();
                let bus_clone = bus.clone();

                let monitor_config = platform_macos::lifecycle::MonitorConfig {
                    idle_threshold_secs: lifecycle_config.idle_threshold_secs,
                    presence_threshold_secs: lifecycle_config.presence_threshold_secs,
                    wake_grace_period_secs: lifecycle_config.wake_grace_period_secs,
                    active_poll_interval_secs: lifecycle_config.active_poll_interval_secs,
                    idle_poll_interval_secs: lifecycle_config.idle_poll_interval_secs,
                };

                let monitor = platform_macos::lifecycle::LifecycleMonitor::start(
                    monitor_config,
                    move |event| {
                        use platform_macos::lifecycle::{LifecycleEvent as LE, WakeType as LWT};
                        let bus_wt = |wt: LWT| match wt {
                            LWT::FromSleep => bus::domain_events::WakeType::FromSleep,
                            LWT::FromIdle => bus::domain_events::WakeType::FromIdle,
                        };
                        match event {
                            LE::SystemWillSleep => {
                                // Publish SystemWillSleep so subscribers can react.
                                // CronService sleep-start tracking removed (4.4c) —
                                // TemporalScheduler's SystemDidWake subscriber handles wake catch-up.
                                bus_clone.publish(bus::DomainEvent::SystemWillSleep);
                            }
                            LE::SystemDidWake {
                                away_duration,
                                wake_type,
                            } => {
                                let away_secs = away_duration.as_secs();
                                // SystemDidWake triggers TemporalScheduler::wake() via its own
                                // bus subscriber (init_temporal_scheduler). No CronService
                                // miss-classification needed — TemporalScheduler reconciles.
                                bus_clone.publish(bus::DomainEvent::SystemDidWake {
                                    away_secs,
                                    wake_type: bus_wt(wake_type),
                                });
                            }
                            LE::UserBecameIdle { idle_secs } => {
                                bus_clone.publish(bus::DomainEvent::UserBecameIdle { idle_secs });
                            }
                            LE::UserReturned {
                                absence_duration,
                                wake_type,
                            } => {
                                bus_clone.publish(bus::DomainEvent::UserReturned {
                                    absence_secs: absence_duration.as_secs(),
                                    wake_type: bus_wt(wake_type),
                                });
                            }
                        }
                    },
                );
                core._lifecycle_monitor = Some(monitor);
                info!("lifecycle monitor started");
            }
        }

        // ── Start wake orchestrator ───────────────────────────────────────
        if let Some(ref bus) = core.domain_event_bus {
            let orchestrator = crate::wake_orchestrator::WakeOrchestrator::new(
                bus.clone(),
                lifecycle_config_snapshot.wake_delivery.clone(),
            );
            core._wake_orchestrator_handle = Some(orchestrator.start());
            info!("wake orchestrator started");
        }

        // ── Register MirrorTool in agent's tool registry (post-init) ──────
        if let Some(ref facade) = core.mirror_facade {
            let reg = core.agent.tool_registry();
            let mut registry = reg.write().await;
            registry.register(tools::MirrorTool::new(Arc::clone(facade)));
            info!("Mirror tool registered");
        }

        // ── Register TemporalTool in agent's tool registry (post-init) ────
        {
            let temporal_service = ::cognitive::TemporalService::new(
                ::cognitive::SemanticFactRepo::new(storage_pool.inner().clone()),
            )
            .with_changelog(::cognitive::FactChangelogRepo::new(
                storage_pool.inner().clone(),
            ));
            let reg = core.agent.tool_registry();
            let mut registry = reg.write().await;
            registry.register(tools::TemporalTool::new(temporal_service));
            info!("Temporal tool registered");
        }

        // ── Background note embedding catch-up ────────────────────────────
        if let Some(ref handler) = core.note_embedding_handler {
            let handler = Arc::clone(handler);
            let repo = core.note_repo.clone();
            let token = shutdown_token.clone();
            tokio::spawn(async move {
                // Delay long enough for the app to settle. The ONNX embedding
                // model is ~420 MB; loading it at startup inflates idle RSS.
                // 120s matches EMBEDDING_IDLE_SECS so the model unloads quickly
                // if no further embeddings are needed.
                tokio::time::sleep(std::time::Duration::from_secs(120)).await;
                if token.is_cancelled() {
                    return;
                }

                match repo.list_notes_needing_embedding(50).await {
                    Ok(notes) => {
                        if !notes.is_empty() {
                            info!(
                                count = notes.len(),
                                "embedding notes without embeddings (background)"
                            );
                        }
                        for note in notes {
                            if token.is_cancelled() {
                                break;
                            }
                            if let Err(e) = handler.embed_note(&note).await {
                                tracing::debug!("background embed failed for {}: {e}", note.id);
                            } else {
                                let _ = repo.update_embedding_timestamp(&note.id).await;
                            }
                        }
                    }
                    Err(e) => tracing::warn!("failed to list notes for embedding: {e}"),
                }
            });
        }

        // ── Post-core background services ────────────────────────────────
        cognitive::spawn_post_core_services(
            &core,
            &domain_event_bus,
            activity_svc,
            &shutdown_token,
        );

        // Spawn background services (agent loop + channel manager).
        spawn_background(inbound_rx, channel_manager, &agent, &shutdown_token);

        // Spawn periodic situation recomputation (every 2 min).
        coaching::spawn_situation_recompute(
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
            distraction_alert_rx,
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
