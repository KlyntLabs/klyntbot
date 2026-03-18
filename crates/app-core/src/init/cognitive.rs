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
}

/// Spawn post-core background services: activity subscriber, analytics retention, event log persistence.
pub(super) fn spawn_post_core_services(
    core: &AppCore,
    domain_event_bus: &Arc<DomainEventBus>,
    activity_svc: Arc<activity_log::ActivityIngestionService>,
    shutdown_token: &CancellationToken,
) {
    // Start ActivityLogSubscriber for domain event normalization.
    let _activity_subscriber = activity_log::ActivityLogSubscriber::start(
        domain_event_bus,
        activity_svc,
        shutdown_token.clone(),
    );

    // Spawn daily analytics retention cleanup + semantic fact pruning.
    {
        let repos_bg = core.repos.clone();
        let cog_pool = core.repos.pool().clone();
        let token = shutdown_token.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(24 * 60 * 60));
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

                        // Prune low-salience semantic facts (confidence < 0.05, untouched for 180+ days).
                        let fact_repo = cognitive::SemanticFactRepo::new(cog_pool.clone());
                        match fact_repo.prune_low_salience(0.05, 180).await {
                            Ok(0) => {}
                            Ok(n) => info!(pruned = n, "cognitive: pruned low-salience facts"),
                            Err(e) => warn!(error = %e, "cognitive: fact pruning failed"),
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
