use std::sync::Arc;

use bus::DomainEventBus;
use storage::StoragePool;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::state::AppCore;

/// Initialize cognitive event log, pipeline, domain bus wiring, and ActivityIngestionService.
///
/// Also handles capture config (ingestion token, file watcher) and work context inference.
pub(super) async fn init_cognitive(
    config: &mut config::Config,
    storage_pool: &StoragePool,
    activity_svc: &Arc<activity_log::ActivityIngestionService>,
    shutdown_token: &CancellationToken,
    embedding_engine: Arc<tools::EmbeddingEngine>,
) {
    // Seed builtin personas (idempotent, safe on every startup)
    let persona_repo = cognitive::repos::PersonaRepo::new(storage_pool.inner().clone());
    if let Err(e) = persona_repo.seed_builtins().await {
        warn!("Failed to seed builtin personas: {e}");
    }

    // Seed builtin squads (idempotent, safe on every startup)
    let squad_repo = cognitive::SquadRepo::new(storage_pool.inner().clone());
    if let Err(e) = squad_repo.seed_builtins().await {
        warn!("Failed to seed builtin squads: {e}");
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

    // Phase 2: Start Work Context inference engine + loop.
    if config.work_context.enabled {
        let inference_cfg =
            activity_log::inference::ContextInferenceConfig::from_work_context_config(
                &config.work_context,
            );
        let text_embedder = Arc::new(agent::TextEmbedderImpl::new(embedding_engine.clone()));
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
}

/// Spawn post-core background services: activity subscriber, analytics retention, event log persistence.
pub(super) fn spawn_post_core_services(
    core: &AppCore,
    domain_event_bus: &Arc<DomainEventBus>,
    activity_svc: Arc<activity_log::ActivityIngestionService>,
    shutdown_token: &CancellationToken,
) {
    // Start ActivityLogSubscriber for domain event normalization.
    // The subscriber's background task runs until the shutdown token is cancelled.
    // We intentionally drop the handle here — the spawned task is self-contained
    // and will stop when the token fires.
    let _activity_subscriber = activity_log::ActivityLogSubscriber::start(
        domain_event_bus,
        activity_svc,
        shutdown_token.clone(),
    );

    // Analytics retention cleanup + semantic fact pruning is handled by CronService
    // (registered in init/cron.rs as __klyntbot_analytics_cleanup).

    // Start atom extraction service (auto-extract concepts from notes).
    if let Some(ref provider) = core.cognitive_provider {
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
    if let Some(ref event_log_repo) = core.event_log_repo {
        spawn_event_log_persistence(
            event_log_repo.clone(),
            core.domain_event_bus.as_ref().expect("initialized above"),
            core.pipeline_broadcast.as_ref().expect("initialized above"),
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
                                let salience = cognitive::salience::evaluate_salience(&event);
                                let domain = event.domain();
                                let salience_str = salience.as_str();
                                let event_type = event.variant_name().to_string();
                                // Store only the variant name — full serialization
                                // causes heap pressure with large payloads.
                                let payload = &event_type;
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
