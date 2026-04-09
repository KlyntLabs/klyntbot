use std::sync::Arc;

use bus::{DomainEventBus, MessageBus};
use feature_tasks::handlers::suggestion_applier::SuggestionApplier;
use feature_tasks::{DecompositionHandler, ForecastHandler, ProactiveHandler, TasksConfig};
use scheduling::CronService;
use storage::Repos;
use tracing::{info, warn};

/// Results from the cron initialization phase.
pub(super) struct CronResult {
    pub cron_service: Arc<CronService>,
    pub notification_dispatcher: Arc<agent::NotificationDispatcher>,
    pub proactive_handler: Option<Arc<dyn feature_tasks::ProactiveHandler>>,
    pub suggestion_applier: Option<Arc<dyn SuggestionApplier>>,
    pub decomposition_handler: Option<Arc<dyn DecompositionHandler>>,
    pub forecast_handler: Option<Arc<dyn ForecastHandler>>,
    pub autotuner: Option<Arc<agent::autotuner::AutoTunerOrchestrator>>,
}

pub const JOB_PROACTIVE_SCAN: &str = "proactive_scan";

/// Initialize cron service, register callbacks, and ensure default jobs.
#[allow(clippy::too_many_arguments)]
pub(super) async fn init_cron(
    config: &config::Config,
    repos: &Repos,
    bus: &Arc<MessageBus>,
    notification_sender: &Option<Arc<dyn common::NotificationSender>>,
    cognitive_provider: Option<providers::DynProvider>,
    provider: providers::DynProvider,
    domain_event_bus: &Arc<DomainEventBus>,
    tasks_config: TasksConfig,
    vector_store: Option<storage::VectorStore>,
) -> Result<CronResult, String> {
    // 6. Cron service — set callbacks BEFORE wrapping in Arc
    let mut cron_service = CronService::new(repos.cron.clone());
    cron_service
        .start()
        .await
        .map_err(|e| format!("cron start failed: {e}"))?;

    let notification_dispatcher = Arc::new(match notification_sender {
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

    // Build AI decomposition and forecast handlers.
    let decomposition_handler: Option<Arc<dyn DecompositionHandler>> = {
        let task_repo = repos.tasks.clone();
        Some(Arc::new(agent::handlers::LlmDecompositionHandler::new(
            provider.clone(),
            config.agents.defaults.model.clone(),
            task_repo,
            Some(Arc::clone(domain_event_bus)),
        )))
    };
    let forecast_handler: Option<Arc<dyn ForecastHandler>> = {
        let task_repo = repos.tasks.clone();
        Some(Arc::new(agent::handlers::LlmForecastHandler::new(
            provider.clone(),
            config.agents.defaults.model.clone(),
            task_repo,
        )))
    };

    // Build the proactive handler and applier for the cron job.
    // Uses its own LlmProactiveHandler instance (domain_bus=None — event emission
    // happens in run_proactive_scan after persist, not inside the handler).
    let autotuner_provider = provider.clone();
    let proactive_handler: Arc<dyn ProactiveHandler> = {
        let task_repo = repos.tasks.clone();
        Arc::new(agent::handlers::LlmProactiveHandler::new(
            provider,
            config.agents.defaults.model.clone(),
            task_repo,
            tasks_config.clone(),
        ))
    };
    let suggestion_applier: Option<Arc<dyn SuggestionApplier>> = {
        let task_repo = repos.tasks.clone();
        Some(Arc::new(agent::handlers::TaskSuggestionApplier::new(
            task_repo,
        )))
    };

    // Clone handlers before they're moved into cron closures so AppCore can hold refs too.
    let proactive_handler_out = Some(proactive_handler.clone());
    let suggestion_applier_out = suggestion_applier.clone();

    // ── AutoTuner setup (must happen before register_cron_callbacks) ─────
    let trial_repo = storage::TrialRepo::new(repos.pool().clone());
    trial_repo
        .migrate()
        .await
        .map_err(|e| format!("autotuner migration failed: {e}"))?;
    let strategy_repo = repos.strategies.clone();
    let event_log_repo = cognitive::EventLogRepo::new(repos.pool().clone());
    let usage_repo = repos.usage.clone();
    let fact_repo = cognitive::SemanticFactRepo::new(repos.pool().clone());
    let feedback_repo = storage::RetrievalFeedbackRepo::new(repos.pool().clone());
    let metric_source: Arc<dyn autotuner::MetricSource> = Arc::new(
        agent::autotuner::metric_collector::AgentMetricCollector::new(
            strategy_repo,
            event_log_repo,
            usage_repo,
            trial_repo.clone(),
            fact_repo,
        )
        .with_feedback_repo(feedback_repo),
    );
    let learning_state = repos.learning_state.clone();
    let champion = agent::autotuner::AutoTunerOrchestrator::load_champion(&learning_state).await;
    let episodic_memory_repo = cognitive::EpisodicMemoryRepo::new(repos.pool().clone());
    let memory_param_sink = Arc::new(std::sync::RwLock::new(None));
    let orchestrator = Arc::new(
        agent::autotuner::AutoTunerOrchestrator::new(
            champion,
            learning_state,
            trial_repo.clone(),
            autotuner_provider,
            config.agents.defaults.model.clone(),
        )
        .with_strategy_repo(repos.strategies.clone())
        .with_episodic_memory_repo(episodic_memory_repo)
        .with_memory_param_sink(Arc::clone(&memory_param_sink)),
    );
    // Build NightlyCycle + bridge for Reforge Phase 6.
    let nightly_cycle = Arc::new(autotuner::NightlyCycle::new(
        config.autotuner.clone(),
        trial_repo.clone(),
        Arc::clone(&metric_source),
    ));
    let autotuner_bridge: Option<Arc<dyn cognitive::services::reforge::AutotunerBridge>> =
        Some(Arc::new(
            agent::adapters::autotuner_bridge::AgentAutotunerBridge::new(
                Arc::clone(&orchestrator),
                nightly_cycle,
                Some(Arc::clone(domain_event_bus)),
            ),
        ));
    info!("autotuner orchestrator + Reforge Phase 6 bridge built");
    let autotuner = Some(orchestrator);

    register_cron_callbacks(
        &mut cron_service,
        repos,
        &notification_dispatcher,
        config,
        bus,
        cognitive_provider,
        domain_event_bus,
        tasks_config.clone(),
        proactive_handler,
        suggestion_applier,
        vector_store,
        autotuner_bridge,
        metric_source,
        trial_repo,
        autotuner.clone(),
    );

    let cron_service = Arc::new(cron_service);
    ensure_cron_jobs(&cron_service, config)
        .await
        .map_err(|e| format!("cron job registration failed: {e}"))?;
    set_default_intent_windows(&cron_service).await;
    info!("cron service started");

    Ok(CronResult {
        cron_service,
        notification_dispatcher,
        proactive_handler: proactive_handler_out,
        suggestion_applier: suggestion_applier_out,
        decomposition_handler,
        forecast_handler,
        autotuner,
    })
}

// ── Cron job name constants ──────────────────────────────────────────────────
// Shared between `register_cron_callbacks` and `ensure_cron_jobs` to prevent
// silent mismatches from typos.
const JOB_FOCUS_CHECK: &str = "todo_focus_check";
const JOB_DAILY_DIGEST: &str = "todo_daily_digest";
const JOB_OVERDUE_CHECK: &str = "todo_overdue_check";
const JOB_WEEKLY_REPORT: &str = "__klyntbot_weekly_report";
const JOB_DAILY_PLANNING: &str = "__klyntbot_daily_planning";
const JOB_FINANCE_DAILY_REVIEW: &str = "__klyntbot_finance_daily_review";
const JOB_ATOM_DECAY: &str = "__klyntbot_atom_decay_daily";
const JOB_ATOM_EXTRACTION_CATCHALL: &str = "__klyntbot_atom_extraction_catchall";
const JOB_FINANCE_BUDGET_CHECK: &str = "__klyntbot_finance_budget_check";
const JOB_FINANCE_PRICE_REFRESH: &str = "__klyntbot_finance_price_refresh";
const JOB_FINANCE_HEALTH_CHECK: &str = "__klyntbot_finance_health_check";
const JOB_MORNING_BRIEFING: &str = "__klyntbot_morning_briefing";
const JOB_WEEKLY_KNOWLEDGE_DIGEST: &str = "__klyntbot_weekly_knowledge_digest";

// ── Background service cron job constants ────────────────────────────────────
const JOB_SESSION_CLEANUP: &str = "__klyntbot_session_cleanup";
const JOB_MEMORY_MAINTENANCE: &str = "__klyntbot_memory_maintenance";
const JOB_ANALYTICS_CLEANUP: &str = "__klyntbot_analytics_cleanup";
const JOB_BLACKBOARD_CLEANUP: &str = "__klyntbot_blackboard_cleanup";
const JOB_REMINDER_CHECK: &str = "__klyntbot_reminder_check";
const JOB_RECURRING_TASKS: &str = "__klyntbot_recurring_tasks";
pub(super) const JOB_INSIGHT_REFRESH: &str = "__klyntbot_insight_refresh";
pub(super) const JOB_LEARNING_ANALYSIS: &str = "__klyntbot_learning_analysis";
pub(super) const JOB_CROSS_DOMAIN_NIGHTLY: &str = "__klyntbot_cross_domain_nightly";
const JOB_LAUNCHER_USAGE_PRUNE: &str = "__klyntbot_launcher_usage_prune";
const JOB_REFORGE_NIGHTLY: &str = "__klyntbot_reforge_nightly";

/// Register individual cron handlers.
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_arguments)]
fn register_cron_callbacks(
    cron_service: &mut CronService,
    repos: &Repos,
    notification_dispatcher: &Arc<agent::NotificationDispatcher>,
    config: &config::Config,
    bus: &Arc<MessageBus>,
    cognitive_provider: Option<providers::DynProvider>,
    domain_event_bus: &Arc<DomainEventBus>,
    tasks_config: TasksConfig,
    proactive_handler: Arc<dyn ProactiveHandler>,
    suggestion_applier: Option<Arc<dyn SuggestionApplier>>,
    vector_store: Option<storage::VectorStore>,
    autotuner_bridge: Option<Arc<dyn cognitive::services::reforge::AutotunerBridge>>,
    metric_source: Arc<dyn autotuner::MetricSource>,
    trial_repo: storage::TrialRepo,
    orchestrator: Option<Arc<agent::autotuner::AutoTunerOrchestrator>>,
) {
    let rt = tokio::runtime::Handle::current();

    // ── todo_focus_check ─────────────────────────────────────────────────
    {
        let todo_repo = repos.tasks.clone();
        let dispatcher = Arc::clone(notification_dispatcher);
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_FOCUS_CHECK,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let dispatcher = Arc::clone(&dispatcher);
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let focused: Vec<storage::TaskRow> = todo_repo.list_focused().await?;
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
        let todo_repo = repos.tasks.clone();
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
                        let overdue: Vec<storage::TaskRow> = todo_repo.overdue().await?;
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
        let todo_repo = repos.tasks.clone();
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
                        let focused: Vec<storage::TaskRow> = todo_repo.list_focused().await?;
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
                    // chat_id must be "channel:id" format for process_system_message routing.
                    // Cron jobs route to the "system" channel with the job channel as the id.
                    let chat_id = format!("system:{channel}");
                    let msg =
                        bus::InboundMessage::new("system", "cron", chat_id, msg_text.to_string());
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

    // ── atom_decay_daily ───────────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let bus = Arc::clone(domain_event_bus);
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_ATOM_DECAY,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let bus = Arc::clone(&bus);
                tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        if let Err(e) =
                            cognitive::services::atom_decay::run_decay_cycle(&pool, &bus).await
                        {
                            warn!("Atom decay cycle failed: {e}");
                        }
                        Ok(None)
                    })
                })
            }),
        );
    }

    // ── reforge_nightly ─────────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let repos_reforge = repos.clone();
        let cog_config = config.clone();
        let cog_provider = cognitive_provider.clone();
        let rt = rt.clone();
        let autotuner_bridge_for_reforge = autotuner_bridge.clone();
        let metric_source_for_reforge = metric_source;
        let trial_repo_for_reforge = trial_repo;
        let orchestrator_for_reforge = orchestrator.clone();
        cron_service.register_handler(
            JOB_REFORGE_NIGHTLY,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let repos_reforge = repos_reforge.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                let autotuner_bridge = autotuner_bridge_for_reforge.clone();
                let metric_source = metric_source_for_reforge.clone();
                let trial_repo = trial_repo_for_reforge.clone();
                let orchestrator = orchestrator_for_reforge.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
                        let episodic_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                        let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());
                        let mirror_repo = cognitive::mirror::MirrorRepo::new(
                            storage::StoragePool::from_existing(pool.clone()),
                        );
                        let feedback_repo = storage::RetrievalFeedbackRepo::new(pool.clone());

                        let handler = crate::handlers::cognitive::build_reforge_handler(
                            &cog_provider,
                            &cog_config,
                        );

                        let data_dir = cog_config.data_dir_path();
                        let skill_mgr =
                            cognitive::services::reforge::skill_files::SkillFileManager::new(
                                data_dir.join("skills"),
                            );

                        // Detect manual user edits before running the Reforge
                        // cycle so the synthesizer sees the latest on-disk content.
                        // The returned files are passed through to avoid a redundant
                        // `read_all()` inside the collector.
                        let pre_read_files =
                            cognitive::services::reforge::collector::detect_user_edits(
                                &skill_mgr,
                                &repos_reforge.skill_version,
                            )
                            .await;

                        // Load autotuner context for Phase 3 prompt
                        // (metric snapshots collected here where autotuner types are available).
                        let autotuner_ctx = if autotuner_bridge.is_some() {
                            let now = chrono::Utc::now();
                            let since_24h = now - chrono::Duration::hours(24);
                            let since_7d = now - chrono::Duration::days(7);

                            let m24 = metric_source
                                .collect_metrics(since_24h, None)
                                .await
                                .ok()
                                .map(|s| cognitive::services::reforge::types::MetricsSnapshot {
                                    correction_rate: s.correction_rate,
                                    retrieval_precision: s.retrieval_precision,
                                    avg_response_time_ms: s.avg_response_time_ms,
                                    avg_tokens_per_message: s.avg_tokens_per_message,
                                    routing_stability: s.routing_stability,
                                    memory_relevance: s.memory_relevance,
                                })
                                .unwrap_or_default();

                            let m7d = metric_source
                                .collect_metrics(since_7d, None)
                                .await
                                .ok()
                                .map(|s| cognitive::services::reforge::types::MetricsSnapshot {
                                    correction_rate: s.correction_rate,
                                    retrieval_precision: s.retrieval_precision,
                                    avg_response_time_ms: s.avg_response_time_ms,
                                    avg_tokens_per_message: s.avg_tokens_per_message,
                                    routing_stability: s.routing_stability,
                                    memory_relevance: s.memory_relevance,
                                })
                                .unwrap_or_default();

                            // Get actual champion params from the orchestrator.
                            let champion_trial_params = match orchestrator.as_ref() {
                                Some(orch) => {
                                    orch.current_champion_params().await.unwrap_or_default()
                                }
                                None => common::TrialParams::default(),
                            };

                            Some(
                                cognitive::services::reforge::collector::load_autotuner_context(
                                    &trial_repo,
                                    m24,
                                    m7d,
                                    &champion_trial_params,
                                )
                                .await,
                            )
                        } else {
                            None
                        };

                        let bridge_ref = autotuner_bridge.as_deref();

                        match cognitive::services::reforge::service::run_reforge(
                            &repos_reforge.reforge_state,
                            &repos_reforge.skill_version,
                            &repos_reforge.session_memory,
                            &fact_repo,
                            &episodic_repo,
                            &rule_repo,
                            handler.as_ref(),
                            &skill_mgr,
                            Some(pre_read_files),
                            Some(&mirror_repo),
                            Some(&feedback_repo),
                            bridge_ref,
                            autotuner_ctx,
                        )
                        .await
                        {
                            Some(result) => {
                                info!(
                                    facts_added = result.facts_added,
                                    facts_updated = result.facts_updated,
                                    rules_added = result.rules_added,
                                    skills_edited = result.skills_edited,
                                    errors = result.phase_errors.len(),
                                    "Reforge nightly cycle complete"
                                );
                                Ok(Some(format!(
                                    "Reforge: +{}f ~{}f +{}r {}sk {}err",
                                    result.facts_added,
                                    result.facts_updated,
                                    result.rules_added,
                                    result.skills_edited,
                                    result.phase_errors.len(),
                                )))
                            }
                            None => Ok(Some("Reforge: skipped (no new data)".to_string())),
                        }
                    })
                })
            }),
        );
    }

    // ── atom_extraction_catchall ─────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let bus = Arc::clone(domain_event_bus);
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_ATOM_EXTRACTION_CATCHALL,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let bus = Arc::clone(&bus);
                tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        let cache = cognitive::repos::AtomExtractionCache::new(pool);
                        match cache.find_unextracted_notes(50).await {
                            Ok(notes) => {
                                let count = notes.len();
                                for note_id in notes {
                                    bus.publish(bus::DomainEvent::NoteEditingFinished {
                                        note_id,
                                    });
                                }
                                if count > 0 {
                                    info!("Atom extraction catchall: queued {count} unextracted notes");
                                }
                                Ok(Some(format!("Queued {count} notes for extraction")))
                            }
                            Err(e) => {
                                warn!("Atom extraction catchall failed: {e}");
                                Ok(None)
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── morning_briefing ──────────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_MORNING_BRIEFING,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        let atom_repo = cognitive::KnowledgeAtomRepo::new(pool.clone());
                        let review_stats = cognitive::ReviewStatsRepo::new(pool.clone());

                        let (fading_res, streak_res) = tokio::join!(
                            atom_repo.list_fading_important(5),
                            review_stats.current_streak(),
                        );
                        let fading_count = fading_res.map(|v| v.len()).unwrap_or(0);
                        let streak = streak_res.unwrap_or(0);

                        if fading_count > 0 {
                            info!("Morning briefing: {fading_count} fading atoms, streak={streak}");
                        }

                        Ok(Some(format!(
                            "Morning briefing: {fading_count} fading, streak={streak}"
                        )))
                    })
                })
            }),
        );
    }

    // ── weekly_knowledge_digest ──────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_WEEKLY_KNOWLEDGE_DIGEST,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        let atom_repo = cognitive::KnowledgeAtomRepo::new(pool.clone());
                        let review_stats = cognitive::ReviewStatsRepo::new(pool.clone());

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

                        info!(
                            "Weekly knowledge digest: streak={streak}, reviews={reviews_week}, \
                             fading={fading_count}, topics={topic_count}",
                        );
                        Ok(Some(format!(
                            "Weekly digest: streak={streak}, fading={fading_count}"
                        )))
                    })
                })
            }),
        );
    }

    // ── proactive_scan ───────────────────────────────────────────────────
    if tasks_config.proactive_suggestions {
        let handler = proactive_handler.clone();
        let repos_clone = repos.clone();
        let bus_clone = Some(Arc::clone(domain_event_bus));
        let cfg = tasks_config.clone();
        let applier = suggestion_applier.clone();
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_PROACTIVE_SCAN,
            Arc::new(move |_job: &scheduling::CronJob| {
                let handler = handler.clone();
                let repos = repos_clone.clone();
                let bus = bus_clone.clone();
                let cfg = cfg.clone();
                let applier = applier.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        match crate::handlers::tasks::proactive::run_proactive_scan(
                            &handler, &repos, &bus, &cfg, &applier,
                        )
                        .await
                        {
                            Ok(n) => Ok(Some(format!(
                                "Proactive scan complete: {n} suggestions generated"
                            ))),
                            Err(e) => {
                                warn!("Proactive scan failed: {e}");
                                Ok(None)
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── session_cleanup ───────────────────────────────────────────────────
    {
        let session_repo = storage::SessionRepo::new(repos.pool().clone());
        let ttl_days = config.conversation.session.ttl_days;
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_SESSION_CLEANUP,
            Arc::new(move |_job: &scheduling::CronJob| {
                let session_repo = session_repo.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        match session_repo.delete_stale_sessions(ttl_days).await {
                            Ok(0) => Ok(Some("No stale sessions".to_string())),
                            Ok(n) => {
                                info!(
                                    deleted = n,
                                    ttl_days, "Session cleanup: deleted stale sessions"
                                );
                                Ok(Some(format!("Deleted {n} stale sessions")))
                            }
                            Err(e) => {
                                warn!(error = %e, "Session cleanup failed");
                                Ok(Some(format!("Session cleanup failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── blackboard_cleanup ─────────────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_BLACKBOARD_CLEANUP,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let repo = cognitive::BlackboardRepo::new(pool);
                        match repo.cleanup_stale(chrono::Duration::days(30)).await {
                            Ok(0) => Ok(Some("No stale blackboard entries".to_string())),
                            Ok(n) => {
                                info!(deleted = n, "Blackboard cleanup: removed stale entries");
                                Ok(Some(format!("Deleted {n} stale blackboard entries")))
                            }
                            Err(e) => {
                                warn!(error = %e, "Blackboard cleanup failed");
                                Ok(Some(format!("Blackboard cleanup failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── memory_maintenance ────────────────────────────────────────────────
    if let Some(vs) = vector_store {
        let max_age_days = config.conversation.memory.max_age_days;
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_MEMORY_MAINTENANCE,
            Arc::new(move |_job: &scheduling::CronJob| {
                let vs = vs.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let cutoff =
                            chrono::Utc::now() - chrono::Duration::days(max_age_days as i64);
                        let cutoff_str =
                            match storage::sanitize_predicate_value(&cutoff.to_rfc3339()) {
                                Ok(s) => s,
                                Err(e) => {
                                    return Ok(Some(format!(
                                        "Memory maintenance: invalid cutoff: {e}"
                                    )))
                                }
                            };
                        let predicate = format!("created_at < '{cutoff_str}'");

                        let before = vs.count("conv_embeddings").await.unwrap_or(0);
                        if let Err(e) = vs.delete_where("conv_embeddings", &predicate).await {
                            warn!(error = %e, "Memory maintenance: failed to prune");
                            return Ok(Some(format!("Memory maintenance failed: {e}")));
                        }
                        let after = vs.count("conv_embeddings").await.unwrap_or(before);
                        let deleted = before.saturating_sub(after);

                        // Dedup pass
                        for (table, ts_col) in [
                            ("conv_embeddings", "created_at"),
                            ("todo_embeddings", "updated_at"),
                            ("cognitive_fact_embeddings", "updated_at"),
                        ] {
                            if let Err(e) = vs.dedup_table(table, ts_col).await {
                                warn!(error = %e, table, "Memory maintenance: dedup failed");
                            }
                        }

                        // Compact Lance fragment files to reclaim memory
                        if let Err(e) = vs.optimize_all_tables().await {
                            warn!(error = %e, "Memory maintenance: LanceDB compaction failed");
                        }

                        if deleted > 0 {
                            info!(
                                deleted,
                                max_age_days, "Memory maintenance: pruned old embeddings"
                            );
                        }
                        Ok(Some(format!("Pruned {deleted} old embeddings")))
                    })
                })
            }),
        );
    }

    // ── analytics_cleanup ─────────────────────────────────────────────────
    {
        let repos_bg = repos.clone();
        let cog_pool = repos.pool().clone();
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_ANALYTICS_CLEANUP,
            Arc::new(move |_job: &scheduling::CronJob| {
                let repos_bg = repos_bg.clone();
                let cog_pool = cog_pool.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let cleaned = match repos_bg.cleanup_analytics().await {
                            Ok(n) => n,
                            Err(e) => {
                                warn!(error = %e, "Analytics cleanup failed");
                                0
                            }
                        };

                        // Prune low-salience semantic facts
                        let fact_repo = cognitive::SemanticFactRepo::new(cog_pool);
                        let pruned = match fact_repo.prune_low_salience(0.05, 180).await {
                            Ok(n) => n,
                            Err(e) => {
                                warn!(error = %e, "Fact pruning failed");
                                0
                            }
                        };

                        Ok(Some(format!(
                            "Analytics: {cleaned} records cleaned, {pruned} facts pruned"
                        )))
                    })
                })
            }),
        );
    }

    // ── reminder_check ────────────────────────────────────────────────────
    {
        let todo_repo = repos.tasks.clone();
        let dispatcher = Arc::clone(notification_dispatcher);
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_REMINDER_CHECK,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let dispatcher = Arc::clone(&dispatcher);
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        match agent::services::reminders::ReminderEngine::check_and_send_reminders_static(
                            &todo_repo,
                            &dispatcher,
                        )
                        .await
                        {
                            Ok(()) => Ok(Some("Reminder check complete".to_string())),
                            Err(e) => {
                                warn!("Reminder check failed: {e}");
                                Ok(Some(format!("Reminder check failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── launcher_usage_prune ──────────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_LAUNCHER_USAGE_PRUNE,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let repo = feature_launcher::FrequencyRepo::new(pool);
                        match repo.prune_old_entries().await {
                            Ok(0) => Ok(Some("No old launcher usage entries to prune".to_string())),
                            Ok(n) => {
                                info!(pruned = n, "Launcher usage prune: removed old entries");
                                Ok(Some(format!("Pruned {n} old launcher usage entries")))
                            }
                            Err(e) => {
                                warn!(error = %e, "Launcher usage prune failed");
                                Ok(Some(format!("Launcher usage prune failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── recurring_tasks ───────────────────────────────────────────────────
    {
        let todo_repo = repos.tasks.clone();
        let timezone = config.timezone.clone();
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_RECURRING_TASKS,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let timezone = timezone.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        match agent::services::recurring_tasks::RecurringTaskSpawner::check_and_spawn_static(
                            &todo_repo,
                            &timezone,
                        )
                        .await
                        {
                            Ok(()) => Ok(Some("Recurring task check complete".to_string())),
                            Err(e) => {
                                warn!("Recurring task check failed: {e}");
                                Ok(Some(format!("Recurring task check failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
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

    // System = protected (can't edit/delete), User = fully editable in the UI.
    // Creates the job if missing, or fixes origin if it was previously different.
    macro_rules! ensure_job {
        ($name:expr, $schedule:expr, $msg:expr, $origin:expr) => {
            if !existing.contains($name) {
                cron_service
                    .add_job($name, $schedule, $msg, false, None, None, false, $origin)
                    .await?;
            } else {
                cron_service.set_origin($name, $origin).await;
            }
        };
    }
    let system = scheduling::CronOrigin::System;

    // ── User-editable jobs ────────────────────────────────────────────────
    // All user jobs are now lazy-created via `ensure_lazy_job()` when
    // the feature is first used, or manually from the Automations page.
    // This keeps a fresh install clean.

    // ── Protected system jobs (AI background work, infrastructure) ────────

    // Finance price refresh (system — must run automatically when enabled)
    if config.finance.enabled && config.finance.price_refresh.enabled {
        ensure_job!(
            JOB_FINANCE_PRICE_REFRESH,
            scheduling::CronSchedule::Every {
                every_ms: config.finance.price_refresh.interval_hours as u64 * 60 * 60 * 1000,
            },
            "Refresh investment prices",
            system.clone()
        );
    }

    // Disabled: autotuner nightly subsumed by Reforge (Phase 6 deferred).
    // TODO(Task 12): integrate autotuner evaluation into Reforge Phase 6.
    // agent::autotuner::AutoTunerOrchestrator::ensure_nightly_job(
    //     cron_service,
    //     &config.autotuner.schedule,
    // )
    // .await?;

    // ── Reforge nightly ─────────────────────────────────────────────────
    ensure_job!(
        JOB_REFORGE_NIGHTLY,
        scheduling::CronSchedule::Cron {
            expr: "0 3 * * *".to_string(),
            tz: Some(config.timezone.clone()),
        },
        "Nightly Reforge: knowledge synthesis, skill improvement, compaction",
        system.clone()
    );
    ensure_job!(
        JOB_ATOM_DECAY,
        scheduling::CronSchedule::Cron {
            expr: "0 3 * * *".to_string(),
            tz: Some(config.timezone.clone()),
        },
        "Daily knowledge atom decay",
        system.clone()
    );
    ensure_job!(
        JOB_ATOM_EXTRACTION_CATCHALL,
        scheduling::CronSchedule::Cron {
            expr: "0 2 * * *".to_string(),
            tz: None
        },
        "Extract atoms from unprocessed notes",
        system.clone()
    );
    ensure_job!(
        JOB_SESSION_CLEANUP,
        scheduling::CronSchedule::Every {
            every_ms: config.conversation.session.cleanup_interval_hours as u64 * 60 * 60 * 1000,
        },
        "Delete stale sessions",
        system.clone()
    );
    ensure_job!(
        JOB_BLACKBOARD_CLEANUP,
        scheduling::CronSchedule::Cron {
            expr: "0 4 * * 0".to_string(),
            tz: Some(config.timezone.clone()),
        },
        "Clean stale blackboard entries",
        system.clone()
    );
    ensure_job!(
        JOB_MEMORY_MAINTENANCE,
        scheduling::CronSchedule::Every {
            every_ms: config.conversation.memory.maintenance_interval_hours as u64 * 60 * 60 * 1000,
        },
        "Prune old conversation embeddings",
        system.clone()
    );

    ensure_job!(
        JOB_ANALYTICS_CLEANUP,
        scheduling::CronSchedule::Every {
            every_ms: 24 * 60 * 60 * 1000
        },
        "Clean old analytics records and prune low-salience facts",
        system.clone()
    );
    ensure_job!(
        JOB_LEARNING_ANALYSIS,
        scheduling::CronSchedule::Every {
            every_ms: config.learning.analysis_interval_secs * 1000
        },
        "Analyze tool outcomes and adapt confidence threshold",
        system.clone()
    );
    ensure_job!(
        JOB_CROSS_DOMAIN_NIGHTLY,
        scheduling::CronSchedule::Cron {
            expr: "0 2 * * *".to_string(),
            tz: None
        },
        "Nightly cross-domain insight batch",
        system.clone()
    );
    ensure_job!(
        JOB_LAUNCHER_USAGE_PRUNE,
        scheduling::CronSchedule::Cron {
            expr: "0 3 * * 0".to_string(),
            tz: None
        },
        "Prune old launcher usage entries",
        system
    );

    Ok(())
}

/// Set default intent windows on AI-heavy cron jobs.
/// Called after ensure_cron_jobs to overlay intelligent scheduling.
async fn set_default_intent_windows(cron_service: &scheduling::CronService) {
    use scheduling::types::{CatchUpPriority, IntentTrigger, IntentWindow};
    use std::time::Duration;

    let windows: &[(&str, IntentWindow)] = &[
        // Disabled: autotuner nightly subsumed by Reforge (Phase 6 deferred).
        // (
        //     agent::autotuner::JOB_AUTOTUNER_NIGHTLY,
        //     IntentWindow {
        //         trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
        //         tolerance: Duration::from_secs(14400),
        //         catch_up: CatchUpPriority::WhenIdle,
        //     },
        // ),
        (
            JOB_REFORGE_NIGHTLY,
            IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
                tolerance: Duration::from_secs(14400),
                catch_up: CatchUpPriority::WhenIdle,
            },
        ),
        (
            JOB_PROACTIVE_SCAN,
            IntentWindow {
                trigger: IntentTrigger::MinActiveMinutes { minutes: 5 },
                tolerance: Duration::from_secs(3600),
                catch_up: CatchUpPriority::WhenPresent,
            },
        ),
        (
            JOB_INSIGHT_REFRESH,
            IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 600 },
                tolerance: Duration::from_secs(21600),
                catch_up: CatchUpPriority::WhenIdle,
            },
        ),
        (
            JOB_CROSS_DOMAIN_NIGHTLY,
            IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
                tolerance: Duration::from_secs(14400),
                catch_up: CatchUpPriority::WhenIdle,
            },
        ),
        (
            JOB_ATOM_EXTRACTION_CATCHALL,
            IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
                tolerance: Duration::from_secs(14400),
                catch_up: CatchUpPriority::WhenIdle,
            },
        ),
        (
            JOB_WEEKLY_REPORT,
            IntentWindow {
                trigger: IntentTrigger::UserPresent,
                tolerance: Duration::from_secs(7200),
                catch_up: CatchUpPriority::WhenPresent,
            },
        ),
    ];

    for (name, window) in windows {
        cron_service.set_intent_window(name, window.clone()).await;
    }
}

/// Refresh insight progress snapshots for all notes with insights.
pub async fn refresh_insight_progress(
    svc: &feature_insights::InsightService,
    note_repo: &feature_notes::repo::NoteRepo,
) -> Result<Option<String>, String> {
    let mut refreshed = 0u32;

    let all_notes = note_repo
        .list_notes(None)
        .await
        .map_err(|e| e.to_string())?;

    for note in &all_notes {
        if let Ok(Some(latest)) = svc.get_latest(&note.id).await {
            if let Err(e) = svc.compute_progress(&latest.id, &note.body, None).await {
                tracing::debug!("progress refresh failed for {}: {e}", note.id);
            } else {
                refreshed += 1;
            }
        }
    }

    if refreshed > 0 {
        Ok(Some(format!(
            "Refreshed {refreshed} insight progress snapshots"
        )))
    } else {
        Ok(None)
    }
}

/// Run the nightly cross-domain batch: collect today's dots and store polished
/// LLM-generated insight sentences for the next morning briefing.
///
/// When `provider` is `Some`, an LLM call generates 1-3 first-person insight
/// sentences. On LLM failure (or when no provider is available), falls back to
/// a simple concatenated summary so the user always gets *something*.
pub async fn run_nightly_batch(
    pool: &storage::StoragePool,
    provider: Option<&providers::DynProvider>,
    model: &str,
) -> Result<Option<String>, String> {
    let svc = feature_insights::nightly_batch::NightlyBatchService::new(pool.clone());

    let dots = svc.get_todays_dots().await.map_err(|e| e.to_string())?;
    if dots.is_empty() {
        return Ok(Some("No cross-domain dots today".to_string()));
    }

    let pairs: Vec<String> = dots.iter().map(|(pair, _)| pair.clone()).collect();
    let dot_refs = pairs.join(";");

    // Template fallback — always available.
    let fallback_summary = format!("Cross-domain connections detected: {}", pairs.join(", "));

    // Try LLM-enhanced insight generation.
    let insights = if let Some(provider) = provider {
        match generate_insights_via_llm(provider, model, &pairs).await {
            Ok(sentences) => sentences,
            Err(e) => {
                warn!("LLM insight generation failed, using template fallback: {e}");
                vec![fallback_summary.clone()]
            }
        }
    } else {
        vec![fallback_summary.clone()]
    };

    // Store each insight for tomorrow's morning briefing.
    let tomorrow = (chrono::Utc::now() + chrono::Duration::days(1))
        .format("%Y-%m-%d")
        .to_string();

    for insight in &insights {
        svc.store_insight(&tomorrow, insight, &dot_refs)
            .await
            .map_err(|e| e.to_string())?;
    }

    info!(
        dots = dots.len(),
        insights = insights.len(),
        "nightly cross-domain batch stored insights for {tomorrow}"
    );
    Ok(Some(format!(
        "Stored {} cross-domain insight(s) ({} dots) for {tomorrow}",
        insights.len(),
        dots.len()
    )))
}

/// Make a single lightweight LLM call to produce polished insight sentences
/// from today's cross-domain connections.
async fn generate_insights_via_llm(
    provider: &providers::DynProvider,
    model: &str,
    pairs: &[String],
) -> Result<Vec<String>, String> {
    let dot_list = pairs
        .iter()
        .map(|p| format!("- {p}"))
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "You are a personal second brain. Given these cross-domain connections the user \
         saw today, generate 1-3 polished insight sentences for tomorrow's morning \
         briefing. Be first-person, sparse, and action-oriented. Max one clause per \
         insight. Return ONLY the insight sentences, one per line.\n\n\
         Connections:\n{dot_list}"
    );

    let params = providers::ChatParams::new(model)
        .with_temperature(0.4)
        .with_max_tokens(500);

    let messages = vec![
        providers::Message::system(
            "You generate concise personal insight sentences. No preamble, no numbering, \
             no markdown. One sentence per line.",
        ),
        providers::Message::user(prompt),
    ];

    let response = provider
        .chat(&messages, None, &params)
        .await
        .map_err(|e| e.to_string())?;

    let content = response.content.unwrap_or_default();
    let sentences: Vec<String> = content
        .lines()
        .map(|l| l.trim().to_string())
        .filter(|l| !l.is_empty())
        .collect();

    if sentences.is_empty() {
        return Err("LLM returned empty response".to_string());
    }

    Ok(sentences)
}
