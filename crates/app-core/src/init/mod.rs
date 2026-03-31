mod agent;
mod channels;
mod coaching;
mod cognitive;
mod cron;
mod deadline;
mod launcher;
mod productivity;
mod storage;

use std::sync::Arc;

use ::agent::AgentLoop;
use ::channels::ChannelManager;
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

        // DomainEventBus is created before cron so the proactive scan callback
        // can capture it and emit ProactiveSuggestionCreated after persisting.
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
            cron_service,
            notification_dispatcher,
            proactive_handler,
            suggestion_applier,
            decomposition_handler,
            forecast_handler,
            autotuner,
        } = cron::init_cron(
            &config,
            &repos,
            &bus,
            &notification_sender,
            cognitive_provider.clone(),
            provider.clone(),
            &domain_event_bus,
            feature_tasks::TasksConfig::default(),
            vector_store.clone(),
        )
        .await?;

        // ── Note embedding handler (before vector_store is moved into agent) ──
        let embedding_engine = Arc::new(tools::embedding_engine::EmbeddingEngine::new());
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
            &cron_service,
            &notification_dispatcher,
            &notification_sender,
            autotuner.as_ref(),
            Arc::clone(&hot_config),
            Some(Arc::clone(&context_update_queue)),
            appcore_embedding_engine.clone(),
        )
        .await?;

        // ── Phase 4: Channel manager ─────────────────────────────────────
        let channel_manager = channels::init_channels(&config, &bus)?;

        let shutdown_token = CancellationToken::new();

        // Idle-unload for the ONNX embedding model (interval matches EMBEDDING_IDLE_SECS)
        {
            let engine = Arc::clone(&embedding_engine);
            spawn_periodic_timer(&shutdown_token, 120, move || {
                engine.unload_if_idle();
            });
        }

        // ── Deadline scheduler (event-driven timers) ────────────────────
        let deadline_scheduler = deadline::init_deadline_scheduler(
            &repos,
            &notification_dispatcher,
            &domain_event_bus,
            &config,
            &shutdown_token,
        )
        .await;

        // ── Phase 5: Productivity ────────────────────────────────────────
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
        } = productivity::init_productivity(
            &config,
            &storage_pool,
            &domain_event_bus,
            &activity_svc,
            &cognitive_provider,
            &shutdown_token,
        )
        .await;

        // ── Phase 6: Coaching ────────────────────────────────────────────
        let coaching::CoachingResult {
            intervention_rx,
            signal_accumulator,
            pattern_detector,
            intervention_router,
            feedback_tracker,
            coaching_service,
            coaching_intervention_log_repo,
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

        // ── Phase 8: Launcher ─────────────────────────────────────────────
        let launcher::LauncherResult { launcher_engine } =
            launcher::init_launcher(&config, &storage_pool, &shutdown_token).await;

        // ── Phase 9: Mirror self-reflection layer ────────────────────────
        let (mirror_facade, mirror_handles, mirror_shutdown) = {
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
            let (facade, handles, shutdown) = ::cognitive::mirror::MirrorEngine::start(
                mirror_repo,
                Arc::clone(&domain_event_bus),
                narrative_handler,
                autotuner_bridge,
                episodic_repo,
            );

            // Bootstrap brain version 1 on first run
            let bootstrap_repo = ::cognitive::mirror::MirrorRepo::new(storage_pool.clone());
            let bootstrap_archiver = ::cognitive::mirror::ConfigArchiver::new(bootstrap_repo, None);
            tokio::spawn(async move {
                let _ = bootstrap_archiver.bootstrap(serde_json::json!({})).await;
            });

            let facade = {
                let text_embedder: Arc<dyn ::cognitive::TextEmbedder> = Arc::new(
                    ::agent::TextEmbedderImpl::new(Arc::clone(&embedding_engine)),
                );
                facade.with_text_embedder(text_embedder)
            };

            info!("mirror self-reflection engine started");
            (Some(Arc::new(facade)), Some(handles), Some(shutdown))
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

        // ── Auto-create note on trial kill (fire-and-forget) ─────────────
        {
            let note_repo = note_repo.clone();
            let mut rx = domain_event_bus.subscribe();
            tokio::spawn(async move {
                while let Ok(event) = rx.recv().await {
                    if let bus::DomainEvent::MirrorTrialKilled { trial_id } = event {
                        let now = feature_notes::repo::utc_now_str();
                        let row = feature_notes::models::NoteRow {
                            id: uuid::Uuid::new_v4().to_string(),
                            notebook_id: None,
                            title: format!("Killed experiment: {trial_id}"),
                            body: "Manually killed this experiment trial from the Mirror."
                                .to_string(),
                            body_html: None,
                            body_json: None,
                            pinned: 0,
                            archived: 0,
                            icon: None,
                            color: None,
                            embedding_updated_at: None,
                            split_content: None,
                            split_mode: None,
                            perspective_config: None,
                            last_visited_at: None,
                            created_at: now.clone(),
                            updated_at: now,
                        };
                        if let Err(e) = note_repo.create_note(&row).await {
                            tracing::warn!(
                                "mirror: failed to auto-create note for killed trial {trial_id}: {e}"
                            );
                        }
                    }
                }
            });
        }

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

        // ── Assemble AppCore ─────────────────────────────────────────────
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
            cron_service: cron_service.clone(),
            shutdown_token: shutdown_token.clone(),
            active_streams: Arc::new(dashmap::DashMap::new()),
            pending_interactions: Arc::new(dashmap::DashMap::new()),
            note_repo,
            practice_repo: feature_notes::repo::PracticeSessionRepo::new(
                storage_pool.inner().clone(),
            ),
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
            coaching_intervention_log_repo,
            user_situation: Some(user_situation),
            active_view: Some(active_view),
            coaching_service: coaching_service.map(|cs| Arc::new(Mutex::new(cs))),
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
            proactive_handler,
            suggestion_applier,
            decomposition_handler,
            forecast_handler,
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
            deadline_scheduler: Some(deadline_scheduler),
            mirror_facade,
            _mirror_handles: mirror_handles,
            _mirror_shutdown: mirror_shutdown,
            _config_watcher_token: Some(config_watcher_token),
            _lifecycle_monitor: None,
            _wake_orchestrator_handle: None,
            voice_service: None,
            voice_conversation_manager: None,
            voice_loop_handle: None,
            brain_voice,
            journey_tracker: Some(journey_tracker),
        };

        // ── Voice service initialization ────────────────────────────────
        {
            let config_guard = shared_config.read().await;
            let voice_config = config_guard.voice.clone();
            if voice_config.enabled {
                use voice_engine::*;
                let data_dir = config_guard.data_dir_path();

                let model_manager = ModelManager::new(&data_dir);
                let model_needs_download = !model_manager.is_available(WhisperModelSize::Small);

                // Local STT: try to load WhisperLocalEngine if model exists
                let stt_local: Option<Arc<dyn TranscriptionEngine>> = model_manager
                    .model_path(WhisperModelSize::Small)
                    .and_then(|path| match engines::WhisperLocalEngine::new(&path) {
                        Ok(engine) => {
                            info!("Whisper local engine loaded from {}", path.display());
                            Some(Arc::new(engine) as Arc<dyn TranscriptionEngine>)
                        }
                        Err(e) => {
                            warn!("Failed to load local Whisper engine: {e}");
                            None
                        }
                    });

                drop(config_guard);

                // TTS: prefer Kokoro (if configured + model available), fall back to macOS AVSpeech
                let tts: Option<Arc<dyn voice_engine::TtsEngine>> = {
                    #[cfg(feature = "kokoro")]
                    {
                        match voice_config.output.tts_engine {
                            config::schema::TtsEngineKind::Kokoro => {
                                if let Some(kokoro_dir) = model_manager.kokoro_model_dir() {
                                    match voice_engine::KokoroTtsEngine::new(&kokoro_dir).await {
                                        Ok(engine) => {
                                            info!(
                                                "Kokoro TTS engine loaded from {}",
                                                kokoro_dir.display()
                                            );
                                            Some(Arc::new(engine)
                                                as Arc<dyn voice_engine::TtsEngine>)
                                        }
                                        Err(e) => {
                                            warn!(
                                                "Failed to load Kokoro TTS, falling back to system: {e}"
                                            );
                                            Some(Arc::new(voice_engine::AvSpeechTtsEngine::new(
                                                &data_dir,
                                            )))
                                        }
                                    }
                                } else {
                                    info!("Kokoro model not found, using system TTS");
                                    Some(Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir)))
                                }
                            }
                            _ => Some(Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir))),
                        }
                    }
                    #[cfg(not(feature = "kokoro"))]
                    {
                        Some(Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir)))
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
                        ..Default::default()
                    },
                    privacy_mode: match voice_config.input.privacy_mode {
                        config::schema::VoicePrivacyMode::Standard => PrivacyLevel::Standard,
                        config::schema::VoicePrivacyMode::Strict => PrivacyLevel::Strict,
                        config::schema::VoicePrivacyMode::Off => PrivacyLevel::Off,
                    },
                    data_dir: data_dir.clone(),
                };

                let has_local_engine = stt_local.is_some();

                // Tier 3 memory recall: construct a MemoryRetriever from
                // cognitive UnifiedMemoryService (facts + conversation recall).
                let voice_memory_retriever: Option<Arc<dyn context_engine::MemoryRetriever>> = {
                    let fact_repo =
                        ::cognitive::SemanticFactRepo::new(storage_pool.inner().clone());
                    let retriever = ::cognitive::UnifiedMemoryService::new(fact_repo);
                    Some(Arc::new(retriever) as Arc<dyn context_engine::MemoryRetriever>)
                };

                let echo_provider: Arc<dyn voice_engine::MemoryEchoProvider> =
                    Arc::new(crate::handlers::voice_echo::AppMemoryEchoProvider::new(
                        mirror_facade_for_voice,
                        voice_memory_retriever,
                    ));

                let service = VoiceService::new(
                    stt_local,
                    tts,
                    Some(Arc::clone(&echo_provider)),
                    model_manager,
                    svc_config,
                );

                let service = Arc::new(service);
                core.voice_service = Some(Arc::clone(&service));

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

                if !has_local_engine {
                    if model_needs_download {
                        // Auto-download whisper-small in background on first run.
                        // Voice will wait for download on first use.
                        // Once download completes, local engine becomes available.
                        info!("Whisper model not found — starting background download");
                        let svc = Arc::clone(&service);
                        let data_dir = data_dir.clone();
                        tokio::spawn(async move {
                            match svc.download_model(WhisperModelSize::Small).await {
                                Ok(()) => {
                                    info!("Whisper model downloaded — local voice available on next capture");
                                    // Hot-load: set local engine so next start_capture uses it.
                                    let model_path = data_dir
                                        .join("models")
                                        .join(WhisperModelSize::Small.filename());
                                    match engines::WhisperLocalEngine::new(&model_path) {
                                        Ok(engine) => {
                                            svc.set_local_engine(Arc::new(engine));
                                            info!("Local Whisper engine hot-loaded after download");
                                        }
                                        Err(e) => {
                                            warn!("Failed to hot-load Whisper engine after download: {e}");
                                        }
                                    }
                                }
                                Err(e) => {
                                    warn!("Background model download failed: {e}");
                                }
                            }
                        });
                    }
                    info!("Voice service initialized (waiting for model download)");
                } else {
                    info!("Voice service initialized (local engine ready)");
                }
            }
        }

        // ── Insight progress refresh (registered post-init — deps available now) ──
        if let Some(ref insight_svc) = core.insight_service {
            let svc = Arc::clone(insight_svc);
            let note_repo_clone = core.note_repo.clone();
            let rt = tokio::runtime::Handle::current();
            cron_service.register_handler(
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
            cron_service.register_handler(
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

        // ── Mirror cron jobs (registered post-init — needs mirror_facade) ──
        if let Some(ref facade) = core.mirror_facade {
            let rt = tokio::runtime::Handle::current();

            // Weekly narrative generation
            {
                let facade = Arc::clone(facade);
                let rt = rt.clone();
                cron_service.register_handler(
                    cron::JOB_MIRROR_WEEKLY_NARRATIVE,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let facade = Arc::clone(&facade);
                        tokio::task::block_in_place(|| {
                            rt.block_on(async move {
                                match facade.generate_weekly_narrative().await {
                                    Ok(narrative) => {
                                        info!(
                                            "Mirror weekly narrative generated: {}",
                                            narrative.id
                                        );
                                        Ok(Some(format!(
                                            "Mirror narrative generated: {}",
                                            narrative.id
                                        )))
                                    }
                                    Err(e) => {
                                        tracing::warn!("Mirror weekly narrative failed: {e}");
                                        Ok(Some(format!("Mirror narrative failed: {e}")))
                                    }
                                }
                            })
                        })
                    }),
                );
            }

            // Cleanup old snapshots and snippets (retain 90 days)
            {
                let mirror_repo = ::cognitive::mirror::MirrorRepo::new(storage_pool.clone());
                cron_service.register_handler(
                    cron::JOB_MIRROR_CLEANUP,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let mirror_repo = mirror_repo.clone();
                        tokio::task::block_in_place(|| {
                            rt.block_on(async move {
                                let snap_count =
                                    mirror_repo.cleanup_old_snapshots(90).await.unwrap_or(0);
                                let snip_count =
                                    mirror_repo.cleanup_old_snippets(90).await.unwrap_or(0);
                                let preview_count =
                                    mirror_repo.cleanup_old_trial_previews(90).await.unwrap_or(0);
                                Ok(Some(format!(
                                    "Mirror cleanup: deleted {snap_count} snapshots, {snip_count} snippets, {preview_count} trial previews"
                                )))
                            })
                        })
                    }),
                );
            }
        }

        // ── Start lifecycle monitor (macOS only) ─────────────────────────
        #[cfg(target_os = "macos")]
        {
            if let Some(ref bus) = core.domain_event_bus {
                let lifecycle_config = lifecycle_config_snapshot.clone();
                let bus_clone = bus.clone();
                let cron_clone = cron_service.clone();

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
                                bus_clone.publish(bus::DomainEvent::SystemWillSleep);
                                let cron = cron_clone.clone();
                                tokio::task::block_in_place(|| {
                                    tokio::runtime::Handle::current()
                                        .block_on(cron.on_system_will_sleep());
                                });
                            }
                            LE::SystemDidWake {
                                away_duration,
                                wake_type,
                            } => {
                                let away_secs = away_duration.as_secs();
                                bus_clone.publish(bus::DomainEvent::SystemDidWake {
                                    away_secs,
                                    wake_type: bus_wt(wake_type),
                                });
                                // Spawn classification + cheap job execution so the
                                // callback returns immediately without blocking the
                                // lifecycle polling thread.
                                let cron = cron_clone.clone();
                                let bus2 = bus_clone.clone();
                                tokio::spawn(async move {
                                    let (immediate, deferred, expired) =
                                        cron.on_system_did_wake().await;
                                    bus2.publish(bus::DomainEvent::CronCatchUpReady {
                                        immediate_count: immediate.len(),
                                        deferred_count: deferred.len(),
                                        expired_count: expired.len(),
                                    });
                                    for job in &immediate {
                                        let _ = cron.run_job(&job.id, false).await;
                                    }
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

        // ── Background note embedding catch-up ────────────────────────────
        if let Some(ref handler) = core.note_embedding_handler {
            let handler = Arc::clone(handler);
            let repo = core.note_repo.clone();
            let token = shutdown_token.clone();
            tokio::spawn(async move {
                // Small delay to let the app finish starting
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
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

    tokio::spawn(async move {
        if let Err(e) = channel_manager.lock().await.start_all().await {
            tracing::error!("channel manager error: {}", e);
        }
    });

    info!("background services spawned");
}
