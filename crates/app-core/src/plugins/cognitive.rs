use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Bundle of all cognitive initialization results for FeatureHost storage.
pub struct CognitiveInitResult {
    pub flashcard_repo: Option<::cognitive::FlashcardRepo>,
    pub knowledge_atom_repo: Option<::cognitive::KnowledgeAtomRepo>,
    pub review_session_repo: Option<::cognitive::ReviewSessionRepo>,
    pub deck_preference_repo: Option<::cognitive::DeckPreferenceRepo>,
    pub event_log_repo: Option<::cognitive::EventLogRepo>,
    pub cognitive_fact_embedder: Option<Arc<dyn ::cognitive::SemanticFactEmbedder>>,
    pub semantic_fact_repo: ::cognitive::SemanticFactRepo,
    pub entity_repo: ::cognitive::EntityRepo,
}

/// Plugin wrapper for the `cognitive` crate.
pub struct CognitivePlugin;

#[async_trait]
impl AppCorePlugin for CognitivePlugin {
    fn name(&self) -> &str {
        "cognitive"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        cognitive::cognitive_migrations()
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        ctx.add_feature_translator(
            cognitive::services::community_intelligence::events::try_from_domain_event,
            ai_core::RecallDomain::General,
        );
        ctx.add_feature_translator(
            cognitive::services::community_intelligence::co_activation_events::try_from_domain_event,
            ai_core::RecallDomain::General,
        );
        ctx.register_metrics(|reg| {
            reg.register_all(
                cognitive::services::community_intelligence::events::CommunityEvent::FEATURE_METRICS,
            )
        });
        ctx.register_metrics(|reg| {
            reg.register_all(
                cognitive::services::community_intelligence::co_activation_events::CoActivationEvent::FEATURE_METRICS,
            )
        });

        let repo =
            ::cognitive::repos::PendingMemoryRepo::new(ctx.deps.storage_pool.inner().clone());
        if let Err(e) = repo.migrate().await {
            tracing::warn!("Pending memory migration failed: {e}");
        }
        ctx.insert_handle(Arc::new(repo));

        let pool = ctx.deps.pool();

        let cognitive_fact_embedder: Option<Arc<dyn ::cognitive::SemanticFactEmbedder>> = ctx
            .with_embedding(|engine, vs| {
                Arc::new(
                    ::agent::adapters::cognitive_embedder::SemanticFactEmbedderImpl::new(
                        engine, vs,
                    ),
                ) as Arc<dyn ::cognitive::SemanticFactEmbedder>
            });

        // Register annotate tool
        ctx.register_tool(tools::AnnotateTool::new(::cognitive::AnnotationRepo::new(
            pool.clone(),
        )));

        ctx.insert_handle(Arc::new(CognitiveInitResult {
            flashcard_repo: Some(::cognitive::FlashcardRepo::new(pool.clone())),
            knowledge_atom_repo: Some(::cognitive::KnowledgeAtomRepo::new(pool.clone())),
            review_session_repo: Some(::cognitive::ReviewSessionRepo::new(pool.clone())),
            deck_preference_repo: Some(::cognitive::DeckPreferenceRepo::new(pool.clone())),
            event_log_repo: Some(::cognitive::EventLogRepo::new(pool.clone())),
            cognitive_fact_embedder,
            semantic_fact_repo: ::cognitive::SemanticFactRepo::new(pool.clone()),
            entity_repo: ::cognitive::EntityRepo::new(pool.clone()),
        }));
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        // Run cognitive init (skill seeding, ingestion token, file watcher, work context).
        {
            let mut config = app.config.write().await;
            self::init::init_cognitive(
                &mut config,
                &app.storage_pool,
                &app.activity_ingestion_service()
                    .expect("activity svc available"),
                &app.shutdown_token,
            )
            .await;
        }

        let Ok(domain_event_bus) = app.domain_event_bus() else {
            tracing::warn!("cognitive plugin: missing deps for post-core services, skipping");
            return Ok(());
        };
        let activity_svc = match app.activity_ingestion_service() {
            Ok(svc) => svc,
            Err(_) => {
                tracing::warn!("cognitive plugin: missing deps for post-core services, skipping");
                return Ok(());
            }
        };
        self::init::spawn_post_core_services(
            app,
            &domain_event_bus,
            Arc::clone(&activity_svc),
            &app.shutdown_token,
        );

        // ── Register insight progress refresh cron ────────────────────────
        if let Some(insight_svc) = app.insight_service() {
            let svc = Arc::clone(&insight_svc);
            let note_repo = app.note_repo.clone();
            let rt = tokio::runtime::Handle::current();
            app.cron_executor.register(
                crate::init::cron::JOB_INSIGHT_REFRESH,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let svc = Arc::clone(&svc);
                    let note_repo = note_repo.clone();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async move {
                            match crate::init::cron::refresh_insight_progress(&svc, &note_repo)
                                .await
                            {
                                Ok(Some(msg)) => Ok(Some(msg)),
                                Ok(None) => Ok(Some("No insights to refresh".to_string())),
                                Err(e) => Ok(Some(format!("Insight refresh failed: {e}"))),
                            }
                        })
                    })
                }),
            );
        }

        // ── Register nightly cross-domain batch cron ──────────────────────
        {
            let pool = app.storage_pool.clone();
            let nightly_provider = app.cognitive_provider();
            let nightly_model = {
                let cfg = app.config.read().await;
                cfg.cognitive
                    .model
                    .clone()
                    .unwrap_or_else(|| cfg.agents.defaults.model.clone())
            };
            let rt = tokio::runtime::Handle::current();
            app.cron_executor.register(
                crate::init::cron::JOB_CROSS_DOMAIN_NIGHTLY,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let pool = pool.clone();
                    let provider = nightly_provider.clone();
                    let model = nightly_model.clone();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async move {
                            match crate::init::cron::run_nightly_batch(
                                &pool,
                                provider.as_ref(),
                                &model,
                            )
                            .await
                            {
                                Ok(Some(msg)) => Ok(Some(msg)),
                                Ok(None) => Ok(Some("No cross-domain dots today".to_string())),
                                Err(e) => Ok(Some(format!("Nightly batch failed: {e}"))),
                            }
                        })
                    })
                }),
            );
        }

        // ── Cognitive maintenance cron handlers (migrated from init/cron.rs) ──
        {
            let pool = app.storage_pool.inner().clone();
            let cog_config = app.config.read().await.clone();
            let cog_provider = app.cognitive_provider();
            let domain_bus = app.domain_event_bus().ok();

            // atom_decay_daily
            if let Some(ref bus) = domain_bus {
                let pool = pool.clone();
                let bus = Arc::clone(bus);
                let rt = tokio::runtime::Handle::current();
                app.cron_executor.register(
                    crate::init::cron::JOB_ATOM_DECAY,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let pool = pool.clone();
                        let bus = Arc::clone(&bus);
                        tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                if let Err(e) =
                                    ::cognitive::services::atom_decay::run_decay_cycle(&pool, &bus)
                                        .await
                                {
                                    tracing::warn!("Atom decay cycle failed: {e}");
                                }
                                Ok(None)
                            })
                        })
                    }),
                );
            }

            // fsrs_optimize_weekly
            {
                let pool = pool.clone();
                let rt = tokio::runtime::Handle::current();
                app.cron_executor.register(
                    crate::init::cron::JOB_FSRS_OPTIMIZE,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let pool = pool.clone();
                        tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let storage_pool = storage::StoragePool::from_existing(pool.clone());
                                let repo = ::cognitive::FsrsParamsRepo::new(storage_pool.clone());
                                match ::agent::adapters::fsrs_writeback::train_fsrs_weights(
                                    &storage_pool,
                                    &repo,
                                )
                                .await
                                {
                                    Ok(true) => tracing::info!(
                                        "FSRS weekly optimization: weights improved and persisted"
                                    ),
                                    Ok(false) => tracing::info!(
                                        "FSRS weekly optimization: no improvement or insufficient data"
                                    ),
                                    Err(e) => {
                                        tracing::warn!("FSRS weekly optimization failed: {e}")
                                    }
                                }
                                Ok(None)
                            })
                        })
                    }),
                );
            }

            // micro_reforge
            {
                let pool = pool.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                let rt = tokio::runtime::Handle::current();
                app.cron_executor.register(
                    crate::init::cron::JOB_MICRO_REFORGE,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let pool = pool.clone();
                        let cog_config = cog_config.clone();
                        let cog_provider = cog_provider.clone();
                        tokio::task::block_in_place(|| {
                            rt.block_on(async move {
                                if !cog_config.cognitive.micro_reforge.enabled {
                                    return Ok(None);
                                }
                                let svc =
                                    ::cognitive::services::micro_reforge::MicroReforgeService::new(
                                        storage::StoragePool::from_existing(pool.clone()),
                                        cog_config.cognitive.micro_reforge.clone(),
                                    );
                                if !svc.should_run().await.unwrap_or(false) {
                                    return Ok(None);
                                }
                                let handler =
                                    crate::handlers::cognitive::build_micro_reforge_handler(
                                        &cog_provider,
                                        &cog_config,
                                    );
                                let rule_repo = ::cognitive::ProceduralRuleRepo::new(pool.clone());
                                let ep_repo = ::cognitive::EpisodicMemoryRepo::new(pool.clone());
                                let obs_repo =
                                    ::cognitive::AccumulatedObservationRepo::new(pool.clone());
                                match svc
                                    .run(
                                        "minute_threshold",
                                        handler,
                                        &rule_repo,
                                        &ep_repo,
                                        &obs_repo,
                                    )
                                    .await
                                {
                                    Ok(n) => {
                                        tracing::info!(accepted = n, "micro_reforge ran");
                                        Ok(Some(format!("Micro-Reforge: {} rules promoted", n)))
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, "micro_reforge failed");
                                        Ok(Some(format!("Micro-Reforge failed: {e}")))
                                    }
                                }
                            })
                        })
                    }),
                );
            }

            // episodic rollup hourly / daily / weekly
            for (job, kind) in [
                (crate::init::cron::JOB_EPISODIC_ROLLUP_HOURLY, "hourly"),
                (crate::init::cron::JOB_EPISODIC_ROLLUP_DAILY, "daily"),
                (crate::init::cron::JOB_EPISODIC_ROLLUP_WEEKLY, "weekly"),
            ] {
                let pool = pool.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                let rt = tokio::runtime::Handle::current();
                app.cron_executor.register(
                    job,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let pool = pool.clone();
                        let cog_config = cog_config.clone();
                        let cog_provider = cog_provider.clone();
                        tokio::task::block_in_place(|| {
                            rt.block_on(async move {
                                if !cog_config.cognitive.hierarchical.enabled {
                                    return Ok(None);
                                }
                                let repo = ::cognitive::EpisodicMemoryRepo::new(pool.clone());
                                let summarizer =
                                    crate::handlers::cognitive::build_hierarchical_summarizer(
                                        &cog_provider,
                                        &cog_config,
                                    );
                                let result = match kind {
                                    "hourly" => ::cognitive::services::hierarchical_compressor::roll_up_hourly(&repo, summarizer).await,
                                    "daily" => ::cognitive::services::hierarchical_compressor::roll_up_daily(&repo, summarizer).await,
                                    "weekly" => ::cognitive::services::hierarchical_compressor::roll_up_weekly(&repo, summarizer).await,
                                    other => {
                                        // Defensive: only hourly/daily/weekly are registered above.
                                        // Fail loud-but-safe rather than misrouting to a compressor.
                                        tracing::error!("unknown episodic rollup kind '{other}', skipping");
                                        return Ok(None);
                                    }
                                };
                                match result {
                                    Ok(n) => {
                                        tracing::info!(created = n, kind, "hierarchical rollup done");
                                        Ok(Some(format!("Hierarchical {kind}: {n} buckets created")))
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, kind, "hierarchical rollup failed");
                                        Ok(Some(format!("Hierarchical {kind} failed: {e}")))
                                    }
                                }
                            })
                        })
                    }),
                );
            }

            // atom_extraction_catchall
            if let Some(ref bus) = domain_bus {
                let pool = pool.clone();
                let bus = Arc::clone(bus);
                let rt = tokio::runtime::Handle::current();
                app.cron_executor.register(
                    crate::init::cron::JOB_ATOM_EXTRACTION_CATCHALL,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let pool = pool.clone();
                        let bus = Arc::clone(&bus);
                        tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let cache = ::cognitive::repos::AtomExtractionCache::new(pool);
                                match cache.find_unextracted_notes(50).await {
                                    Ok(notes) => {
                                        let count = notes.len();
                                        for note_id in notes {
                                            bus.publish(bus::DomainEvent::Note(bus::NoteEvent::NoteEditingFinished {
                                                note_id,
                                            }));
                                        }
                                        if count > 0 {
                                            tracing::info!(
                                                "Atom extraction catchall: queued {count} unextracted notes"
                                            );
                                        }
                                        Ok(Some(format!("Queued {count} notes for extraction")))
                                    }
                                    Err(e) => {
                                        tracing::warn!("Atom extraction catchall failed: {e}");
                                        Ok(None)
                                    }
                                }
                            })
                        })
                    }),
                );
            }

            // morning_briefing
            {
                let pool = pool.clone();
                let rt = tokio::runtime::Handle::current();
                app.cron_executor.register(
                    crate::init::cron::JOB_MORNING_BRIEFING,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let pool = pool.clone();
                        tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let atom_repo = ::cognitive::KnowledgeAtomRepo::new(pool.clone());
                                let review_stats = ::cognitive::ReviewStatsRepo::new(pool.clone());
                                let (fading_res, streak_res) = tokio::join!(
                                    atom_repo.list_fading_important(5),
                                    review_stats.current_streak(),
                                );
                                let fading_count = fading_res.map(|v| v.len()).unwrap_or(0);
                                let streak = streak_res.unwrap_or(0);
                                if fading_count > 0 {
                                    tracing::info!(
                                        "Morning briefing: {fading_count} fading atoms, streak={streak}"
                                    );
                                }
                                Ok(Some(format!(
                                    "Morning briefing: {fading_count} fading, streak={streak}"
                                )))
                            })
                        })
                    }),
                );
            }

            // weekly_knowledge_digest
            {
                let pool = pool.clone();
                let rt = tokio::runtime::Handle::current();
                app.cron_executor.register(
                    crate::init::cron::JOB_WEEKLY_KNOWLEDGE_DIGEST,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let pool = pool.clone();
                        tokio::task::block_in_place(|| {
                            rt.block_on(async {
                                let atom_repo = ::cognitive::KnowledgeAtomRepo::new(pool.clone());
                                let review_stats = ::cognitive::ReviewStatsRepo::new(pool.clone());
                                let topic_count_fut = sqlx::query_as::<_, (i64,)>(
                                    "SELECT COUNT(DISTINCT topic_id) FROM knowledge_atoms WHERE status = 'active' AND topic_id IS NOT NULL",
                                )
                                .fetch_one(&pool);
                                let (streak, topic_count, fading, daily) = tokio::join!(
                                    review_stats.current_streak(),
                                    topic_count_fut,
                                    atom_repo.list_fading_important(10),
                                    review_stats.daily_reviews(7),
                                );
                                let streak = streak.unwrap_or(0);
                                let topic_count = topic_count.map(|r| r.0).unwrap_or(0);
                                let fading_count = fading.unwrap_or_default().len();
                                let reviews_week: i64 =
                                    daily.unwrap_or_default().iter().map(|d| d.review_count).sum();
                                tracing::info!(
                                    "Weekly knowledge digest: streak={streak}, reviews={reviews_week}, fading={fading_count}, topics={topic_count}",
                                );
                                Ok(Some(format!(
                                    "Weekly digest: streak={streak}, fading={fading_count}"
                                )))
                            })
                        })
                    }),
                );
            }

            // analytics_cleanup
            {
                let repos_bg = app.repos.clone();
                let cog_pool = pool.clone();
                let rt = tokio::runtime::Handle::current();
                app.cron_executor.register(
                    crate::init::cron::JOB_ANALYTICS_CLEANUP,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let repos_bg = repos_bg.clone();
                        let cog_pool = cog_pool.clone();
                        tokio::task::block_in_place(|| {
                            rt.block_on(async move {
                                let cleaned = match repos_bg.cleanup_analytics().await {
                                    Ok(n) => n,
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Analytics cleanup failed");
                                        0
                                    }
                                };
                                let fact_repo = ::cognitive::SemanticFactRepo::new(cog_pool.clone());
                                let pruned = match fact_repo.prune_low_salience(0.05, 180).await {
                                    Ok(n) => n,
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Fact pruning failed");
                                        0
                                    }
                                };
                                let pending_repo =
                                    ::cognitive::repos::PendingMemoryRepo::new(cog_pool);
                                let pending_cleaned = match pending_repo.cleanup_older_than(30).await
                                {
                                    Ok(n) => n,
                                    Err(e) => {
                                        tracing::warn!(error = %e, "Pending memory cleanup failed");
                                        0
                                    }
                                };
                                Ok(Some(format!(
                                    "Analytics: {cleaned} records cleaned, {pruned} facts pruned, {pending_cleaned} stale pending memories removed"
                                )))
                            })
                        })
                    }),
                );
            }
        }

        Ok(())
    }
}

mod init {
    use std::sync::Arc;

    use bus::DomainEventBus;
    use storage::StoragePool;
    use tokio_util::sync::CancellationToken;
    use tracing::{info, warn};

    use crate::state::AppCore;

    /// Initialize cognitive event log, pipeline, domain bus wiring, and ActivityIngestionService.
    ///
    /// Also handles capture config (ingestion token, file watcher).
    pub(crate) async fn init_cognitive(
        config: &mut config::Config,
        storage_pool: &StoragePool,
        activity_svc: &Arc<activity_log::ActivityIngestionService>,
        shutdown_token: &CancellationToken,
    ) {
        // Seed compiled default skills to disk on first run (skills dir empty).
        // Records v1 in skill_versions for each seeded file so the Reforge cycle
        // can detect user edits against the known baseline.
        {
            let skills_dir = config.data_dir_path().join("skills");
            let skill_mgr =
                cognitive::services::reforge::skill_files::SkillFileManager::new(skills_dir);
            let defaults = skill_system::compiled_skill_defaults();
            match skill_mgr.seed_if_empty(&defaults) {
                Ok(0) => {
                    // Already seeded on a previous run — nothing to do.
                }
                Ok(seeded) => {
                    info!("Seeded {seeded} skills to disk");
                    // Record v1 versions for all seeded files so detect_user_edits
                    // has a baseline to diff against.
                    let version_repo =
                        storage::repos::SkillVersionRepo::new(storage_pool.inner().clone());
                    let all_files = skill_mgr.read_all();
                    for (skill_name, files) in &all_files {
                        for file in files {
                            let row = storage::rows::SkillVersionRow {
                                id: uuid::Uuid::new_v4().to_string(),
                                skill_name: skill_name.clone(),
                                version: 1,
                                file_path: file.file_path.clone(),
                                content: file.content.clone(),
                                diff: None,
                                source: "Seed".to_string(),
                                reason: Some("Initial skill from compiled defaults".to_string()),
                                created_at: jiff::Timestamp::now().to_string(),
                            };
                            if let Err(e) = version_repo.insert(&row).await {
                                warn!(
                                    "Failed to record seed version for {}/{}: {e}",
                                    skill_name, file.file_path
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to seed skills to disk: {e}");
                }
            }
        }

        // Phase 3: Auto-generate ingestion token on first startup if missing.
        if config.capture.ingestion_api.enabled && config.capture.ingestion_api.token.is_none() {
            config.capture.ingestion_api.token = Some(uuid::Uuid::new_v4().to_string());
            if let Err(e) = config::save(config).await {
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
                let fw = crate::infrastructure::file_watcher::FileWatcherService::new(
                    dirs,
                    Arc::clone(activity_svc),
                    config.capture.file_watcher.ignore_patterns.clone(),
                    config.capture.file_watcher.debounce_ms,
                );
                let _fw_handle = fw.start(shutdown_token.child_token());
                info!("file watcher started");
            }
        }

        // NOTE: Work Context inference engine + loop are started in the agent builder
        // (agent/src/agent_loop/builders/context_sources.rs), where the WorkContextSource
        // is also registered and the real VectorStore is available. Do not start a second
        // loop here — it duplicates the inference work with a degraded (None) vector store.
    }

    /// Spawn post-core background services: activity subscriber, analytics retention, event log persistence.
    pub fn spawn_post_core_services(
        core: &AppCore,
        domain_event_bus: &Arc<DomainEventBus>,
        _activity_svc: Arc<activity_log::ActivityIngestionService>,
        shutdown_token: &CancellationToken,
    ) {
        // Activity-log normalization is now handled by NormalizerSignalConsumer
        // registered with the SignalRouter in init/mod.rs (Phase 9).
        // The legacy ActivityLogSubscriber bus subscription has been removed.

        // Analytics retention cleanup + semantic fact pruning is handled by CronService
        // (registered in init/cron.rs as __klyntbot_analytics_cleanup).

        // Start atom extraction service (auto-extract concepts from notes).
        if let Some(provider) = core.cognitive_provider() {
            // try_read() is fine here — config lock is uncontested during init
            if let Ok(config) = core.config.try_read() {
                let extraction_config = config.cognitive.atom_extraction.clone();
                drop(config);
                if extraction_config.enabled {
                    let pool = core.storage_pool.inner().clone();
                    let bus = Arc::clone(domain_event_bus);
                    let token = shutdown_token.child_token();
                    cognitive::services::atom_extraction::AtomExtractionService::start(
                        pool,
                        provider.clone(),
                        bus,
                        extraction_config,
                        token,
                    );
                    info!("atom extraction service started");
                }
            }
        }

        // Spawn event log persistence — writes domain & pipeline events to DB.
        if let Some(event_log_repo) = core.event_log_repo() {
            spawn_event_log_persistence(
                event_log_repo.clone(),
                &core.domain_event_bus().expect("initialized above"),
                &core.pipeline_broadcast().expect("initialized above"),
                shutdown_token,
            );
        }
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
                                    let domain = event.domain();
                                    let event_type = event.variant_name().to_string();
                                    let payload = serde_json::to_string(&event)
                                        .unwrap_or_else(|e| {
                                            tracing::warn!(error = %e, "serialize DomainEvent for event log failed");
                                            format!("{{\"_kind\":{:?}}}", event_type)
                                        });
                                    let ts = jiff::Timestamp::now().to_string();
                                    let id = uuid::Uuid::new_v4().to_string();

                                    if let Err(e) = repo
                                        .insert_domain_event(&id, &event_type, &domain, "extract", &payload, &ts)
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
                                    let ts = jiff::Timestamp::now().to_string();
                                    let id = uuid::Uuid::new_v4().to_string();

                                    let result = match &pe {
                                        cognitive::PipelineEvent::Extraction {
                                            observation,
                                            facts_extracted,
                                            ..
                                        } => {
                                            repo.insert_pipeline_event(
                                                &cognitive::PipelineEventRecord {
                                                    id: &id,
                                                    event_kind: "extraction",
                                                    observation: Some(observation.as_str()),
                                                    facts_extracted: Some(*facts_extracted as i64),
                                                    operation: None,
                                                    fact_triple: None,
                                                    timestamp: &ts,
                                                },
                                            )
                                            .await
                                        }
                                        cognitive::PipelineEvent::Consolidation {
                                            operation,
                                            fact,
                                            ..
                                        } => {
                                            repo.insert_pipeline_event(
                                                &cognitive::PipelineEventRecord {
                                                    id: &id,
                                                    event_kind: "consolidation",
                                                    observation: None,
                                                    facts_extracted: None,
                                                    operation: Some(operation.as_str()),
                                                    fact_triple: Some(fact.as_str()),
                                                    timestamp: &ts,
                                                },
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
}
