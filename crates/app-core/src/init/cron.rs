use std::sync::Arc;

use bus::{DomainEventBus, MessageBus};
use scheduling::temporal::cron_executor::CronExecutor;
use storage::Repos;
use tracing::{debug, info, warn};

/// Publish an `AlarmFired` event routed through `NotificationDispatcher`.
///
/// This ensures all cron-sourced notifications honour quiet hours, retry,
/// and idempotency instead of bypassing the dispatcher pipeline.
fn publish_cron_alarm(
    bus: &DomainEventBus,
    cron_job_id: Option<String>,
    title: impl Into<String>,
    body: impl Into<String>,
) {
    let payload = serde_json::json!({
        "title": title.into(),
        "body": body.into(),
        "channel_mask": 0,
        "priority_override": null,
    })
    .to_string();
    bus.publish(bus::DomainEvent::AlarmFired {
        fire_id: uuid::Uuid::new_v4().to_string(),
        kind: "cron".to_string(),
        ref_id: cron_job_id,
        payload_json: payload,
        fired_at_ms: jiff::Timestamp::now().as_millisecond(),
    });
}

/// Results from the cron initialization phase.
pub(super) struct CronResult {
    pub cron_executor: Arc<CronExecutor>,
    pub autotuner: Option<Arc<agent::autotuner::AutoTunerOrchestrator>>,
}

/// Initialize cron service, register callbacks, and ensure default jobs.
#[allow(clippy::too_many_arguments)]
pub(super) async fn init_cron(
    config: &config::Config,
    repos: &Repos,
    bus: &Arc<MessageBus>,
    cognitive_provider: Option<providers::DynProvider>,
    provider: providers::DynProvider,
    domain_event_bus: &Arc<DomainEventBus>,
    vector_store: Option<storage::VectorStore>,
    symbol_extractor: Option<Arc<dyn coding_memory::symbols::SymbolExtractor>>,
    causal_edge_repo: Option<Arc<coding_memory::causal::CausalEdgeRepo>>,
) -> Result<CronResult, String> {
    // 6. CronExecutor — handler registration only; TemporalScheduler drives firing.
    let cron_executor = CronExecutor::new(repos.cron.clone(), Arc::clone(domain_event_bus));

    let autotuner_provider = provider.clone();

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
    let fsrs_params_repo = Arc::new(cognitive::FsrsParamsRepo::new(
        storage::StoragePool::from_existing(repos.pool().clone()),
    ));
    let autotuner_bridge: Option<Arc<dyn cognitive::services::reforge::AutotunerBridge>> =
        Some(Arc::new(
            agent::adapters::autotuner_bridge::AgentAutotunerBridge::new(
                Arc::clone(&orchestrator),
                nightly_cycle,
                Some(Arc::clone(domain_event_bus)),
                Some(fsrs_params_repo),
            ),
        ));
    info!("autotuner orchestrator + Reforge Phase 6 bridge built");
    let autotuner = Some(orchestrator);

    register_cron_callbacks(
        &cron_executor,
        repos,
        config,
        bus,
        cognitive_provider,
        domain_event_bus,
        vector_store,
        autotuner_bridge,
        metric_source,
        trial_repo,
        autotuner.clone(),
        symbol_extractor,
        causal_edge_repo,
    );

    let cron_executor = Arc::new(cron_executor);
    ensure_cron_jobs(&repos.cron, config)
        .await
        .map_err(|e| format!("cron job registration failed: {e}"))?;
    set_default_intent_windows(&repos.cron).await;
    info!("cron executor initialized");

    Ok(CronResult {
        cron_executor,
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
const JOB_RECURRING_TASKS: &str = "__klyntbot_recurring_tasks";
pub(super) const JOB_INSIGHT_REFRESH: &str = "__klyntbot_insight_refresh";
pub(super) const JOB_LEARNING_ANALYSIS: &str = "__klyntbot_learning_analysis";
pub(super) const JOB_CROSS_DOMAIN_NIGHTLY: &str = "__klyntbot_cross_domain_nightly";
const JOB_LAUNCHER_USAGE_PRUNE: &str = "__klyntbot_launcher_usage_prune";
const JOB_LAUNCHER_ATTENTION_REBUILD: &str = "__klyntbot_launcher_attention_rebuild";
const JOB_REFORGE_NIGHTLY: &str = "__klyntbot_reforge_nightly";
const JOB_MICRO_REFORGE: &str = "__klyntbot_micro_reforge";
const JOB_EPISODIC_ROLLUP_HOURLY: &str = "__klyntbot_episodic_rollup_hourly";
const JOB_EPISODIC_ROLLUP_DAILY: &str = "__klyntbot_episodic_rollup_daily";
const JOB_EPISODIC_ROLLUP_WEEKLY: &str = "__klyntbot_episodic_rollup_weekly";
const JOB_FSRS_OPTIMIZE: &str = "__klyntbot_fsrs_optimize_weekly";

/// Register individual cron handlers.
#[allow(clippy::too_many_arguments)]
fn register_cron_callbacks(
    cron_executor: &CronExecutor,
    repos: &Repos,
    config: &config::Config,
    bus: &Arc<MessageBus>,
    cognitive_provider: Option<providers::DynProvider>,
    domain_event_bus: &Arc<DomainEventBus>,
    vector_store: Option<storage::VectorStore>,
    autotuner_bridge: Option<Arc<dyn cognitive::services::reforge::AutotunerBridge>>,
    metric_source: Arc<dyn autotuner::MetricSource>,
    trial_repo: storage::TrialRepo,
    orchestrator: Option<Arc<agent::autotuner::AutoTunerOrchestrator>>,
    symbol_extractor: Option<Arc<dyn coding_memory::symbols::SymbolExtractor>>,
    causal_edge_repo: Option<Arc<coding_memory::causal::CausalEdgeRepo>>,
) {
    let rt = tokio::runtime::Handle::current();

    // ── todo_focus_check ─────────────────────────────────────────────────
    {
        let todo_repo = repos.tasks.clone();
        let domain_bus = Arc::clone(domain_event_bus);
        let rt = rt.clone();
        cron_executor.register(
            JOB_FOCUS_CHECK,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let domain_bus = Arc::clone(&domain_bus);
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let focused: Vec<storage::TaskRow> = todo_repo.list_focused().await?;
                        for task in &focused {
                            if let Some(deadline) = task.focus_deadline {
                                let hours_left = (deadline.as_millisecond()
                                    - jiff::Timestamp::now().as_millisecond())
                                    / 3_600_000;
                                if hours_left <= 1 && hours_left > 0 {
                                    publish_cron_alarm(
                                        &domain_bus,
                                        Some(JOB_FOCUS_CHECK.to_string()),
                                        "⏰ Focus Deadline: 1h left",
                                        format!("\"{}\" — deadline approaching!", task.title),
                                    );
                                } else if hours_left <= 3 && hours_left > 1 {
                                    publish_cron_alarm(
                                        &domain_bus,
                                        Some(JOB_FOCUS_CHECK.to_string()),
                                        "⏰ Focus Deadline: 3h left",
                                        format!("\"{}\" — stay on track", task.title),
                                    );
                                } else if hours_left <= 6 && hours_left > 3 {
                                    publish_cron_alarm(
                                        &domain_bus,
                                        Some(JOB_FOCUS_CHECK.to_string()),
                                        "⏰ Focus Deadline: 6h left",
                                        format!("\"{}\" — keep going", task.title),
                                    );
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
        let domain_bus = Arc::clone(domain_event_bus);
        let rt = rt.clone();
        cron_executor.register(
            JOB_DAILY_DIGEST,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let domain_bus = Arc::clone(&domain_bus);
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
                        publish_cron_alarm(
                            &domain_bus,
                            Some(JOB_DAILY_DIGEST.to_string()),
                            "📋 Daily Task Digest",
                            body,
                        );
                        Ok(Some("Daily digest sent".to_string()))
                    })
                })
            }),
        );
    }

    // ── todo_overdue_check ───────────────────────────────────────────────
    {
        let todo_repo = repos.tasks.clone();
        let domain_bus = Arc::clone(domain_event_bus);
        let config_focus = config.todo.focus.clone();
        let rt = rt.clone();
        cron_executor.register(
            JOB_OVERDUE_CHECK,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let domain_bus = Arc::clone(&domain_bus);
                let config_focus = config_focus.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let focused: Vec<storage::TaskRow> = todo_repo.list_focused().await?;
                        let now_jiff = jiff::Timestamp::now();
                        let mut expired_count = 0u32;
                        for task in &focused {
                            if task.focus_deadline.map(|d| *d < now_jiff).unwrap_or(false) {
                                let _ = todo_repo.unfocus(&task.id).await;
                                expired_count += 1;
                            }
                        }
                        if expired_count > 0 {
                            let body = format!(
                                "{} task(s) auto-unfocused due to {}h deadline",
                                expired_count, config_focus.deadline_hours
                            );
                            publish_cron_alarm(
                                &domain_bus,
                                Some(JOB_OVERDUE_CHECK.to_string()),
                                "⏰ Focus Tasks Expired",
                                body,
                            );
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
    // Each has an explicit handler registered with CronExecutor.
    {
        macro_rules! register_bus_job {
            ($name:expr, $channel:expr, $msg_text:expr) => {{
                let bus = bus.clone();
                let rt = rt.clone();
                let job_name = $name;
                cron_executor.register(
                    job_name,
                    Arc::new(move |_job: &scheduling::CronJob| {
                        let bus = Arc::clone(&bus);
                        let channel = $channel;
                        let msg_text = $msg_text;
                        let job_name = job_name;
                        tokio::task::block_in_place(|| {
                            rt.block_on(async move {
                                let chat_id = format!("system:{channel}");
                                let msg = bus::InboundMessage::new(
                                    "system",
                                    "cron",
                                    chat_id,
                                    msg_text.to_string(),
                                );
                                bus.publish_inbound(msg).await.map_err(|e| {
                                    common::KlyntbotError::Bus(format!(
                                        "Failed to publish {job_name} message: {e}"
                                    ))
                                })?;
                                Ok(Some(format!("{job_name} triggered")))
                            })
                        })
                    }),
                );
            }};
        }

        register_bus_job!(
            JOB_WEEKLY_REPORT,
            "weekly_report",
            "Generate weekly progress report using the weekly-report skill"
        );
        register_bus_job!(
            JOB_FINANCE_DAILY_REVIEW,
            "finance_daily_review",
            "Run finance daily review and send summary"
        );
        register_bus_job!(
            JOB_FINANCE_BUDGET_CHECK,
            "finance_budget_check",
            "Check budget thresholds and send alerts"
        );
        register_bus_job!(
            JOB_FINANCE_PRICE_REFRESH,
            "finance_price_refresh",
            "Refresh investment prices"
        );
        register_bus_job!(
            JOB_FINANCE_HEALTH_CHECK,
            "finance_health_check",
            "Run finance data health check"
        );
    }

    // ── atom_decay_daily ───────────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let bus = Arc::clone(domain_event_bus);
        let rt = rt.clone();
        cron_executor.register(
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

    // ── fsrs_optimize_weekly ────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let rt = rt.clone();
        cron_executor.register(
            JOB_FSRS_OPTIMIZE,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        let storage_pool = storage::StoragePool::from_existing(pool.clone());
                        let repo = cognitive::FsrsParamsRepo::new(storage_pool.clone());
                        match agent::adapters::fsrs_writeback::train_fsrs_weights(
                            &storage_pool,
                            &repo,
                        )
                        .await
                        {
                            Ok(true) => {
                                info!("FSRS weekly optimization: weights improved and persisted")
                            }
                            Ok(false) => info!(
                                "FSRS weekly optimization: no improvement or insufficient data"
                            ),
                            Err(e) => warn!("FSRS weekly optimization failed: {e}"),
                        }
                        Ok(None)
                    })
                })
            }),
        );
    }

    // ── autotuner_nightly ────────────────────────────────────────────
    {
        let bridge = autotuner_bridge.clone();
        let rt = rt.clone();
        cron_executor.register(
            agent::autotuner::JOB_AUTOTUNER_NIGHTLY,
            Arc::new(move |_job: &scheduling::CronJob| {
                let bridge = bridge.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async {
                        if let Some(ref b) = bridge {
                            match cognitive::services::reforge::service::run_phase6_autotuner(
                                b.as_ref(),
                            )
                            .await
                            {
                                Ok(eval) => {
                                    info!(
                                        evaluated = eval.evaluated_count,
                                        promoted = eval.promoted,
                                        regression = eval.regression,
                                        "Autotuner nightly evaluation complete"
                                    );
                                }
                                Err(e) => {
                                    warn!("Autotuner nightly evaluation failed: {e}");
                                }
                            }
                        } else {
                            debug!("Autotuner nightly: skipped (no bridge)");
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
        let domain_event_bus_for_reforge = Arc::clone(domain_event_bus);
        let symbol_extractor_for_reforge = symbol_extractor.clone();
        let causal_edge_repo_for_reforge = causal_edge_repo.clone();
        cron_executor.register(
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
                let domain_event_bus = domain_event_bus_for_reforge.clone();
                let symbol_extractor = symbol_extractor_for_reforge.clone();
                let causal_edge_repo = causal_edge_repo_for_reforge.clone();
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
                            let since_24h =
                                jiff::Timestamp::now() - jiff::SignedDuration::from_hours(24);
                            let since_7d =
                                jiff::Timestamp::now() - jiff::SignedDuration::from_hours(168);

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

                        let event_log_repo = cognitive::EventLogRepo::new(pool.clone());
                        let co_activation_repo = cognitive::CoActivationRepo::new(pool.clone());
                        let suggestion_repo = storage::ReforgeSuggestionRepo::new(pool.clone());
                        let density_repo = cognitive::ConversationDensityRepo::new(pool.clone());
                        let entity_repo = cognitive::EntityRepo::new(pool.clone());
                        let snapshot_repo = cognitive::KnowledgeSnapshotRepo::new(pool.clone());
                        let metric_repo = cognitive::MetricRepo::new(pool.clone());
                        let metric_registry = ai_core::MetricRegistry::new();
                        let feedback_sources =
                            cognitive::services::reforge::collector::FeedbackSources {
                                outcome_repo: Some(&repos_reforge.outcomes),
                                event_log_repo: Some(&event_log_repo),
                                co_activation_repo: Some(&co_activation_repo),
                                suggestion_repo: Some(&suggestion_repo),
                                pool: Some(&pool),
                                density_repo: Some(&density_repo),
                                metric_repo: Some(&metric_repo),
                                metric_registry: Some(&metric_registry),
                            };

                        // Build coding-specific Phase 2.5 / 3.5 handlers.
                        let synth_handler: Option<
                            std::sync::Arc<dyn coding_memory::reforge::CodingSynthesisHandler>,
                        > = cog_provider.as_ref().map(|p| {
                            let model = cog_config
                                .coding_memory
                                .reforge
                                .synth_model
                                .clone()
                                .unwrap_or_else(|| p.default_model().to_string());
                            let params = providers::ChatParams::new(model)
                                .with_temperature(0.2)
                                .with_response_format(providers::ResponseFormat::JsonObject);
                            std::sync::Arc::new(
                                agent::handlers::coding_synthesis::CodingSynthesisHandlerImpl::new(
                                    p.clone(),
                                    params,
                                ),
                            )
                                as std::sync::Arc<
                                    dyn coding_memory::reforge::CodingSynthesisHandler,
                                >
                        });
                        let rules_handler: Option<
                            std::sync::Arc<dyn coding_memory::reforge::RuleArtifactsHandler>,
                        > = cog_provider.as_ref().map(|p| {
                            let model = cog_config
                                .coding_memory
                                .reforge
                                .rules_model
                                .clone()
                                .unwrap_or_else(|| p.default_model().to_string());
                            let params = providers::ChatParams::new(model)
                                .with_temperature(0.2)
                                .with_response_format(providers::ResponseFormat::JsonObject);
                            std::sync::Arc::new(
                                agent::handlers::rule_artifacts::RuleArtifactsHandlerImpl::new(
                                    p.clone(),
                                    params,
                                ),
                            )
                                as std::sync::Arc<dyn coding_memory::reforge::RuleArtifactsHandler>
                        });

                        let enabled_artifacts = {
                            let r = &cog_config.coding_memory.reforge;
                            let mut v = Vec::new();
                            if r.rule_artifacts.claude_md {
                                v.push("claude_md".into());
                            }
                            if r.rule_artifacts.agents_md {
                                v.push("agents_md".into());
                            }
                            if r.rule_artifacts.cursorrules {
                                v.push("cursorrules".into());
                            }
                            if r.rule_artifacts.continue_rules {
                                v.push("continue_rules".into());
                            }
                            v
                        };

                        let coding_runner =
                            crate::coding_memory::reforge::CodingPhaseRunnerImpl::new(
                                storage::StoragePool::from_existing(pool.clone()),
                                synth_handler,
                                rules_handler,
                                enabled_artifacts,
                                Some(domain_event_bus.clone()),
                                cog_config
                                    .coding_memory
                                    .reforge
                                    .cross_session_dedup_threshold,
                                cog_config.coding_memory.reforge.selective_delete_threshold,
                            )
                            .with_symbol_extractor(symbol_extractor)
                            .with_causal_repo(causal_edge_repo);

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
                            Some(&feedback_sources),
                            crate::handlers::cognitive::build_graph_enrichment_handler(
                                &cog_provider,
                                &cog_config,
                            )
                            .as_deref(),
                            Some(&density_repo),
                            Some(&entity_repo),
                            Some(&snapshot_repo),
                            crate::handlers::cognitive::build_community_intelligence_handler(
                                &cog_provider,
                                &cog_config,
                            )
                            .as_deref(),
                            Some(&cognitive::CommunityRepo::new(pool.clone())),
                            Some(&co_activation_repo),
                            Some(domain_event_bus),
                            Some(&coding_runner),
                            Some(&coding_runner),
                            Some(&coding_runner),
                        )
                        .await
                        {
                            Some(result) => {
                                // Clean up old suggestions (90 day retention)
                                if let Err(e) = suggestion_repo
                                    .delete_older_than(90, jiff::Timestamp::now())
                                    .await
                                {
                                    tracing::warn!("Reforge: failed to clean up suggestions: {e}");
                                }
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

    // ── micro_reforge ────────────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let cog_config = config.clone();
        let cog_provider = cognitive_provider.clone();
        let rt = rt.clone();
        cron_executor.register(
            JOB_MICRO_REFORGE,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        if !cog_config.cognitive.micro_reforge.enabled {
                            return Ok(None);
                        }
                        let svc = cognitive::services::micro_reforge::MicroReforgeService::new(
                            storage::StoragePool::from_existing(pool.clone()),
                            cog_config.cognitive.micro_reforge.clone(),
                        );
                        if !svc.should_run().await.unwrap_or(false) {
                            return Ok(None);
                        }
                        let handler = crate::handlers::cognitive::build_micro_reforge_handler(
                            &cog_provider,
                            &cog_config,
                        );
                        let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());
                        let ep_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                        let obs_repo = cognitive::AccumulatedObservationRepo::new(pool.clone());
                        match svc
                            .run("minute_threshold", handler, &rule_repo, &ep_repo, &obs_repo)
                            .await
                        {
                            Ok(n) => {
                                info!(accepted = n, "micro_reforge ran");
                                Ok(Some(format!("Micro-Reforge: {} rules promoted", n)))
                            }
                            Err(e) => {
                                warn!(error = %e, "micro_reforge failed");
                                Ok(Some(format!("Micro-Reforge failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── episodic rollup hourly ───────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let cog_config = config.clone();
        let cog_provider = cognitive_provider.clone();
        let rt = rt.clone();
        cron_executor.register(
            JOB_EPISODIC_ROLLUP_HOURLY,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        if !cog_config.cognitive.hierarchical.enabled {
                            return Ok(None);
                        }
                        let repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                        let summarizer = crate::handlers::cognitive::build_hierarchical_summarizer(
                            &cog_provider,
                            &cog_config,
                        );
                        match cognitive::services::hierarchical_compressor::roll_up_hourly(
                            &repo,
                            summarizer,
                        )
                        .await
                        {
                            Ok(n) => {
                                info!(created = n, "hierarchical rollup hourly done");
                                Ok(Some(format!("Hierarchical hourly: {} buckets created", n)))
                            }
                            Err(e) => {
                                warn!(error = %e, "hierarchical rollup hourly failed");
                                Ok(Some(format!("Hierarchical hourly failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── episodic rollup daily ────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let cog_config = config.clone();
        let cog_provider = cognitive_provider.clone();
        let rt = rt.clone();
        cron_executor.register(
            JOB_EPISODIC_ROLLUP_DAILY,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        if !cog_config.cognitive.hierarchical.enabled {
                            return Ok(None);
                        }
                        let repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                        let summarizer = crate::handlers::cognitive::build_hierarchical_summarizer(
                            &cog_provider,
                            &cog_config,
                        );
                        match cognitive::services::hierarchical_compressor::roll_up_daily(
                            &repo,
                            summarizer,
                        )
                        .await
                        {
                            Ok(n) => {
                                info!(created = n, "hierarchical rollup daily done");
                                Ok(Some(format!("Hierarchical daily: {} buckets created", n)))
                            }
                            Err(e) => {
                                warn!(error = %e, "hierarchical rollup daily failed");
                                Ok(Some(format!("Hierarchical daily failed: {e}")))
                            }
                        }
                    })
                })
            }),
        );
    }

    // ── episodic rollup weekly ───────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let cog_config = config.clone();
        let cog_provider = cognitive_provider.clone();
        let rt = rt.clone();
        cron_executor.register(
            JOB_EPISODIC_ROLLUP_WEEKLY,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        if !cog_config.cognitive.hierarchical.enabled {
                            return Ok(None);
                        }
                        let repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                        let summarizer = crate::handlers::cognitive::build_hierarchical_summarizer(
                            &cog_provider,
                            &cog_config,
                        );
                        match cognitive::services::hierarchical_compressor::roll_up_weekly(
                            &repo,
                            summarizer,
                        )
                        .await
                        {
                            Ok(n) => {
                                info!(created = n, "hierarchical rollup weekly done");
                                Ok(Some(format!("Hierarchical weekly: {} buckets created", n)))
                            }
                            Err(e) => {
                                warn!(error = %e, "hierarchical rollup weekly failed");
                                Ok(Some(format!("Hierarchical weekly failed: {e}")))
                            }
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
        cron_executor.register(
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
        cron_executor.register(
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
        cron_executor.register(
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

    // ── session_cleanup ───────────────────────────────────────────────────
    {
        let session_repo = storage::SessionRepo::new(repos.pool().clone());
        let ttl_days = config.conversation.session.ttl_days;
        let rt = rt.clone();
        cron_executor.register(
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

    // ── memory_maintenance ────────────────────────────────────────────────
    if let Some(vs) = vector_store {
        let max_age_days = config.conversation.memory.max_age_days;
        let rt = rt.clone();
        cron_executor.register(
            JOB_MEMORY_MAINTENANCE,
            Arc::new(move |_job: &scheduling::CronJob| {
                let vs = vs.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let cutoff = jiff::Timestamp::now()
                            .checked_sub(jiff::SignedDuration::from_secs(
                                max_age_days as i64 * 86400,
                            ))
                            .unwrap_or_else(|_| jiff::Timestamp::now());
                        let cutoff_str =
                            match storage::sanitize_predicate_value(&cutoff.to_string()) {
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
        cron_executor.register(
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
                        let fact_repo = cognitive::SemanticFactRepo::new(cog_pool.clone());
                        let pruned = match fact_repo.prune_low_salience(0.05, 180).await {
                            Ok(n) => n,
                            Err(e) => {
                                warn!(error = %e, "Fact pruning failed");
                                0
                            }
                        };

                        // Clean stale pending memories (unreviewed for >30 days)
                        let pending_repo = cognitive::repos::PendingMemoryRepo::new(cog_pool);
                        let pending_cleaned = match pending_repo.cleanup_older_than(30).await {
                            Ok(n) => n,
                            Err(e) => {
                                warn!(error = %e, "Pending memory cleanup failed");
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

    // ── launcher_usage_prune ──────────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let rt = rt.clone();
        cron_executor.register(
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

    // ── launcher_attention_rebuild ────────────────────────────────────────
    {
        let pool = repos.pool().clone();
        let rt = rt.clone();
        cron_executor.register(
            JOB_LAUNCHER_ATTENTION_REBUILD,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let aggregator = feature_launcher::AttentionAggregator::new(pool);
                        match aggregator.rebuild_from_activity(90).await {
                            Ok(0) => Ok(Some("No attention data to rebuild".to_string())),
                            Ok(n) => {
                                info!(rows = n, "Launcher attention rebuild complete");
                                Ok(Some(format!("Rebuilt attention for {n} entities")))
                            }
                            Err(e) => {
                                warn!(error = %e, "Launcher attention rebuild failed");
                                Ok(Some(format!("Launcher attention rebuild failed: {e}")))
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
        cron_executor.register(
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

/// Register default cron jobs directly via `CronRepo` (idempotent — skips existing).
///
/// Uses `CronRepo::upsert` rather than `CronService::add_job` so this path
/// does not depend on `CronService` internals. `CronBridge::reconcile_all()`
/// is called once at the end (by the `init_temporal_scheduler` step that follows).
async fn ensure_cron_jobs(
    cron_repo: &storage::repos::cron::CronRepo,
    config: &config::Config,
) -> Result<(), common::KlyntbotError> {
    use jiff::Timestamp;

    use crate::handlers::cron::new_cron_id;

    // Build a map of existing jobs by name so we can skip or fix them.
    let existing_rows: std::collections::HashMap<String, storage::rows::cron::CronJobRow> =
        cron_repo
            .list()
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("cron list failed: {e}")))?
            .into_iter()
            .map(|r| (r.name.clone(), r))
            .collect();

    // Creates the job if missing, or fixes origin if it was previously different.
    macro_rules! ensure_job {
        ($name:expr, $schedule:expr, $msg:expr, $origin_str:expr) => {{
            if let Some(existing) = existing_rows.get($name as &str) {
                // Fix origin mismatch (e.g. job was created as "user" but should be "system").
                if existing.origin != $origin_str {
                    let mut fixed = existing.clone();
                    fixed.origin = $origin_str.to_string();
                    fixed.updated_at_ms = Timestamp::now().as_millisecond();
                    cron_repo.upsert(&fixed).await.map_err(|e| {
                        common::KlyntbotError::Storage(format!("cron upsert failed: {e}"))
                    })?;
                }
            } else {
                let now_ms = Timestamp::now().as_millisecond();
                let row = storage::rows::cron::CronJobRow {
                    id: new_cron_id(),
                    name: $name.to_string(),
                    enabled: true,
                    origin: $origin_str.to_string(),
                    schedule: serde_json::to_value(&$schedule)
                        .expect("CronSchedule serialization is infallible"),
                    payload: serde_json::json!({
                        "kind": "agent_turn",
                        "message": $msg,
                        "deliver": false,
                    }),
                    next_run_at_ms: None,
                    last_run_at_ms: None,
                    last_status: None,
                    last_error: None,
                    created_at_ms: now_ms,
                    updated_at_ms: now_ms,
                    delete_after_run: false,
                    intent_window: None,
                    intent_pending_since_ms: None,
                };
                cron_repo.upsert(&row).await.map_err(|e| {
                    common::KlyntbotError::Storage(format!("cron upsert failed: {e}"))
                })?;
            }
        }};
    }
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
            "system"
        );
    }

    // Autotuner nightly — now delegated to run_phase6_autotuner via the
    // Reforge Phase 6 bridge (Task 12 complete). The cron row is kept so
    // the callback below fires on schedule.
    agent::autotuner::AutoTunerOrchestrator::ensure_nightly_job(
        cron_repo,
        &config.autotuner.schedule,
    )
    .await?;

    // ── Reforge nightly ─────────────────────────────────────────────────
    ensure_job!(
        JOB_REFORGE_NIGHTLY,
        scheduling::CronSchedule::Cron {
            expr: "0 0 3 * * *".to_string(),
            tz: Some(config.timezone.clone()),
        },
        "Nightly Reforge: knowledge synthesis, skill improvement, compaction",
        "system"
    );
    if config.cognitive.micro_reforge.enabled {
        ensure_job!(
            JOB_MICRO_REFORGE,
            scheduling::CronSchedule::Every {
                every_ms: 5 * 60 * 1000, // every 5 minutes
            },
            "Micro-Reforge timer (KCA Track 4)",
            "system"
        );
    }
    if config.cognitive.hierarchical.enabled {
        ensure_job!(
            JOB_EPISODIC_ROLLUP_HOURLY,
            scheduling::CronSchedule::Cron {
                expr: config.cognitive.hierarchical.hourly_schedule.clone(),
                tz: Some(config.timezone.clone()),
            },
            "Hourly episodic compression (KCA Track 8)",
            "system"
        );
        ensure_job!(
            JOB_EPISODIC_ROLLUP_DAILY,
            scheduling::CronSchedule::Cron {
                expr: config.cognitive.hierarchical.daily_schedule.clone(),
                tz: Some(config.timezone.clone()),
            },
            "Daily episodic compression (KCA Track 8)",
            "system"
        );
        ensure_job!(
            JOB_EPISODIC_ROLLUP_WEEKLY,
            scheduling::CronSchedule::Cron {
                expr: config.cognitive.hierarchical.weekly_schedule.clone(),
                tz: Some(config.timezone.clone()),
            },
            "Weekly episodic compression (KCA Track 8)",
            "system"
        );
    }
    ensure_job!(
        JOB_ATOM_DECAY,
        scheduling::CronSchedule::Cron {
            expr: "0 0 3 * * *".to_string(),
            tz: Some(config.timezone.clone()),
        },
        "Daily knowledge atom decay",
        "system"
    );
    ensure_job!(
        JOB_FSRS_OPTIMIZE,
        scheduling::CronSchedule::Cron {
            expr: "0 0 4 * * 7".to_string(), // Sunday 04:00 local (7 = Sunday in this parser)
            tz: Some(config.timezone.clone()),
        },
        "Weekly FSRS-5 weight optimisation",
        "system"
    );
    // Fixup: if the FSRS row was created with the old invalid "0" day-of-week,
    // patch it to "7" so the scheduler doesn't panic on startup.
    if let Ok(rows) = cron_repo.list().await {
        if let Some(row) = rows.iter().find(|r| r.name == JOB_FSRS_OPTIMIZE) {
            if row.schedule.get("expr").and_then(|v| v.as_str()) == Some("0 0 4 * * 0") {
                let mut fixed = row.clone();
                fixed.schedule = serde_json::to_value(&scheduling::CronSchedule::Cron {
                    expr: "0 0 4 * * 7".to_string(),
                    tz: Some(config.timezone.clone()),
                })
                .expect("CronSchedule serialization is infallible");
                fixed.updated_at_ms = jiff::Timestamp::now().as_millisecond();
                let _ = cron_repo.upsert(&fixed).await;
            }
        }
    }
    ensure_job!(
        JOB_ATOM_EXTRACTION_CATCHALL,
        scheduling::CronSchedule::Cron {
            expr: "0 0 2 * * *".to_string(),
            tz: None
        },
        "Extract atoms from unprocessed notes",
        "system"
    );
    ensure_job!(
        JOB_SESSION_CLEANUP,
        scheduling::CronSchedule::Every {
            every_ms: config.conversation.session.cleanup_interval_hours as u64 * 60 * 60 * 1000,
        },
        "Delete stale sessions",
        "system"
    );
    ensure_job!(
        JOB_MEMORY_MAINTENANCE,
        scheduling::CronSchedule::Every {
            every_ms: config.conversation.memory.maintenance_interval_hours as u64 * 60 * 60 * 1000,
        },
        "Prune old conversation embeddings",
        "system"
    );

    ensure_job!(
        JOB_ANALYTICS_CLEANUP,
        scheduling::CronSchedule::Every {
            every_ms: 24 * 60 * 60 * 1000
        },
        "Clean old analytics records and prune low-salience facts",
        "system"
    );
    ensure_job!(
        JOB_LEARNING_ANALYSIS,
        scheduling::CronSchedule::Every {
            every_ms: config.learning.analysis_interval_secs * 1000
        },
        "Analyze tool outcomes and adapt confidence threshold",
        "system"
    );
    ensure_job!(
        JOB_CROSS_DOMAIN_NIGHTLY,
        scheduling::CronSchedule::Cron {
            expr: "0 0 2 * * *".to_string(),
            tz: None
        },
        "Nightly cross-domain insight batch",
        "system"
    );
    ensure_job!(
        JOB_LAUNCHER_USAGE_PRUNE,
        scheduling::CronSchedule::Cron {
            expr: "0 0 3 * * SUN".to_string(),
            tz: None
        },
        "Prune old launcher usage entries",
        "system"
    );
    ensure_job!(
        JOB_LAUNCHER_ATTENTION_REBUILD,
        scheduling::CronSchedule::Cron {
            expr: "0 0 3 * * *".to_string(),
            tz: None
        },
        "Rebuild launcher attention from activity events",
        "system"
    );

    Ok(())
}

/// Set default intent windows on AI-heavy cron jobs via direct SQL update.
/// Called after ensure_cron_jobs to overlay intelligent scheduling.
async fn set_default_intent_windows(cron_repo: &storage::repos::cron::CronRepo) {
    use scheduling::types::{CatchUpPriority, IntentTrigger, IntentWindow};
    use std::time::Duration;

    let windows: &[(&str, IntentWindow)] = &[
        (
            agent::autotuner::JOB_AUTOTUNER_NIGHTLY,
            IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
                tolerance: Duration::from_secs(14400),
                catch_up: CatchUpPriority::WhenIdle,
            },
        ),
        (
            JOB_REFORGE_NIGHTLY,
            IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 300 },
                tolerance: Duration::from_secs(14400),
                catch_up: CatchUpPriority::WhenIdle,
            },
        ),
        (
            JOB_MICRO_REFORGE,
            IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 60 },
                tolerance: Duration::from_secs(300),
                catch_up: CatchUpPriority::WhenIdle,
            },
        ),
        (
            JOB_FSRS_OPTIMIZE,
            IntentWindow {
                trigger: IntentTrigger::UserIdle { min_idle_secs: 600 },
                tolerance: Duration::from_secs(86400),
                catch_up: CatchUpPriority::WhenIdle,
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
        let json = match serde_json::to_string(window) {
            Ok(j) => j,
            Err(e) => {
                tracing::warn!("Failed to serialize intent window for '{}': {}", name, e);
                continue;
            }
        };
        if let Err(e) = cron_repo.set_intent_window_by_name(name, Some(&json)).await {
            tracing::warn!("Failed to set intent window for '{}': {}", name, e);
        }
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
    let tomorrow = jiff::Timestamp::now()
        .checked_add(jiff::SignedDuration::from_secs(86400))
        .unwrap_or_else(|_| jiff::Timestamp::now())
        .strftime("%Y-%m-%d")
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
