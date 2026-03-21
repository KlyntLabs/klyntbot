use std::sync::Arc;

use bus::{DomainEventBus, MessageBus};
use feature_tasks::handlers::suggestion_applier::SuggestionApplier;
use feature_tasks::{DecompositionHandler, ForecastHandler, ProactiveHandler, TasksConfig};
use scheduling::CronService;
use storage::Repos;
use tracing::{error, info, warn};

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
    );

    // ── AutoTuner nightly cycle (conditional) ────────────────────────────
    let autotuner = if config.autotuner.enabled {
        let trial_repo = storage::TrialRepo::new(repos.pool().clone());
        trial_repo
            .migrate()
            .await
            .map_err(|e| format!("autotuner migration failed: {e}"))?;
        let strategy_repo = repos.strategies.clone();
        let event_log_repo = cognitive::EventLogRepo::new(repos.pool().clone());
        let usage_repo = repos.usage.clone();
        let fact_repo = cognitive::SemanticFactRepo::new(repos.pool().clone());
        let metric_source: Arc<dyn autotuner::MetricSource> = Arc::new(
            agent::autotuner::metric_collector::AgentMetricCollector::new(
                strategy_repo,
                event_log_repo,
                usage_repo,
                trial_repo.clone(),
                fact_repo,
            ),
        );
        let learning_state = repos.learning_state.clone();
        let champion =
            agent::autotuner::AutoTunerOrchestrator::load_champion(&learning_state).await;
        let episodic_memory_repo = cognitive::EpisodicMemoryRepo::new(repos.pool().clone());
        let memory_param_sink = Arc::new(std::sync::RwLock::new(None));
        let orchestrator = Arc::new(
            agent::autotuner::AutoTunerOrchestrator::new(
                champion,
                true,
                learning_state,
                trial_repo.clone(),
                autotuner_provider,
                config.agents.defaults.model.clone(),
            )
            .with_strategy_repo(repos.strategies.clone())
            .with_episodic_memory_repo(episodic_memory_repo)
            .with_session_repo(repos.sessions.clone())
            .with_memory_param_sink(Arc::clone(&memory_param_sink)),
        );
        // Spawn bootstrap replay as a background task to seed the first experiment
        // from historical session data (no-op if experiments already exist).
        {
            let orch_clone = Arc::clone(&orchestrator);
            tokio::spawn(async move {
                if let Err(e) = agent::autotuner::run_bootstrap_replay(&orch_clone).await {
                    warn!("bootstrap replay failed (non-fatal): {e}");
                }
            });
        }
        agent::autotuner::AutoTunerOrchestrator::register_nightly_cycle(
            Arc::clone(&orchestrator),
            &mut cron_service,
            config.autotuner.clone(),
            trial_repo,
            metric_source,
            Some(Arc::clone(domain_event_bus)),
        );
        info!("autotuner nightly cycle handler registered");
        Some(orchestrator)
    } else {
        None
    };

    let cron_service = Arc::new(cron_service);
    ensure_cron_jobs(&cron_service, config, &tasks_config)
        .await
        .map_err(|e| format!("cron job registration failed: {e}"))?;
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
const JOB_WEEKLY_REFLECTION: &str = "__klyntbot_cognitive_weekly_reflection";
const JOB_WEEKLY_REPORT: &str = "__klyntbot_weekly_report";
const JOB_DAILY_PLANNING: &str = "__klyntbot_daily_planning";
const JOB_FINANCE_DAILY_REVIEW: &str = "__klyntbot_finance_daily_review";
const JOB_ATOM_DECAY: &str = "__klyntbot_atom_decay_daily";
const JOB_FINANCE_BUDGET_CHECK: &str = "__klyntbot_finance_budget_check";
const JOB_FINANCE_PRICE_REFRESH: &str = "__klyntbot_finance_price_refresh";
const JOB_FINANCE_HEALTH_CHECK: &str = "__klyntbot_finance_health_check";

/// Register individual cron handlers (must be called before wrapping CronService in Arc).
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
) {
    let rt = tokio::runtime::Handle::current();

    // ── todo_focus_check ─────────────────────────────────────────────────
    {
        let todo_repo = repos.actions.clone();
        let dispatcher = Arc::clone(notification_dispatcher);
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_FOCUS_CHECK,
            Arc::new(move |_job: &scheduling::CronJob| {
                let todo_repo = todo_repo.clone();
                let dispatcher = Arc::clone(&dispatcher);
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let focused: Vec<storage::ActionRow> = todo_repo.list_focused().await?;
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
        let todo_repo = repos.actions.clone();
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
                    })
                })
            }),
        );
    }

    // ── todo_overdue_check ───────────────────────────────────────────────
    {
        let todo_repo = repos.actions.clone();
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
                    })
                })
            }),
        );
    }

    // ── __klyntbot_cognitive_weekly_reflection ───────────────────────────
    {
        let pool = repos.pool().clone();
        let cog_config = config.clone();
        let cog_provider = cognitive_provider;
        let rt = rt.clone();
        cron_service.register_handler(
            JOB_WEEKLY_REFLECTION,
            Arc::new(move |_job: &scheduling::CronJob| {
                let pool = pool.clone();
                let cog_config = cog_config.clone();
                let cog_provider = cog_provider.clone();
                tokio::task::block_in_place(|| {
                    rt.block_on(async move {
                        let fact_repo = cognitive::SemanticFactRepo::new(pool.clone());
                        let episodic_repo = cognitive::EpisodicMemoryRepo::new(pool.clone());
                        let rule_repo = cognitive::ProceduralRuleRepo::new(pool.clone());

                        let (reflection_handler, consolidation_handler) =
                            crate::handlers::cognitive::build_reflection_handlers(
                                &cog_provider,
                                &cog_config,
                            );

                        match cognitive::reflection::run_weekly_reflection(
                            reflection_handler.as_ref(),
                            consolidation_handler.as_ref(),
                            &fact_repo,
                            &episodic_repo,
                            &rule_repo,
                            None,
                        )
                        .await
                        {
                            Ok(output) => {
                                info!(
                                    "Weekly reflection complete: {} facts, {} rules — {}",
                                    output.fact_updates.len(),
                                    output.rule_updates.len(),
                                    output.summary,
                                );
                                Ok(Some(format!(
                                    "Weekly reflection: {} fact updates, {} rule updates",
                                    output.fact_updates.len(),
                                    output.rule_updates.len(),
                                )))
                            }
                            Err(e) => {
                                error!("Weekly reflection failed: {e}");
                                Ok(Some(format!("Weekly reflection failed: {e}")))
                            }
                        }
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
                    let msg =
                        bus::InboundMessage::new("system", "cron", channel, msg_text.to_string());
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
}

/// Register default cron jobs (idempotent — skips existing).
async fn ensure_cron_jobs(
    cron_service: &Arc<CronService>,
    config: &config::Config,
    tasks_config: &TasksConfig,
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
                    .add_job(
                        $name,
                        $schedule,
                        $msg,
                        false,
                        None,
                        None,
                        false,
                        scheduling::CronOrigin::System,
                    )
                    .await?;
            }
        };
    }

    ensure_job!(
        JOB_FOCUS_CHECK,
        scheduling::CronSchedule::Every {
            every_ms: 30 * 60 * 1000,
        },
        "Check focus task deadlines"
    );
    ensure_job!(
        JOB_DAILY_DIGEST,
        scheduling::CronSchedule::Cron {
            expr: "0 9 * * *".to_string(),
            tz: None,
        },
        "Daily task summary"
    );
    ensure_job!(
        JOB_OVERDUE_CHECK,
        scheduling::CronSchedule::Every {
            every_ms: 60 * 60 * 1000,
        },
        "Check for overdue focus tasks"
    );
    ensure_job!(
        JOB_WEEKLY_REPORT,
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
                JOB_DAILY_PLANNING,
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
            JOB_WEEKLY_REFLECTION,
            scheduling::CronSchedule::Cron {
                expr: reflection_schedule.to_string(),
                tz: Some(config.timezone.clone()),
            },
            "Weekly cognitive reflection"
        );
    }

    // Finance cron jobs
    if config.finance.enabled && config.finance.proactivity_level != "reactive" {
        if let Some(cron_expr) = parse_time_to_cron(&config.finance.scheduling.daily_review_time) {
            ensure_job!(
                JOB_FINANCE_DAILY_REVIEW,
                scheduling::CronSchedule::Cron {
                    expr: cron_expr,
                    tz: None,
                },
                "Daily financial review"
            );
        }
        ensure_job!(
            JOB_FINANCE_BUDGET_CHECK,
            scheduling::CronSchedule::Every {
                every_ms: 6 * 60 * 60 * 1000,
            },
            "Check budget thresholds"
        );
        if config.finance.price_refresh.enabled {
            ensure_job!(
                JOB_FINANCE_PRICE_REFRESH,
                scheduling::CronSchedule::Every {
                    every_ms: config.finance.price_refresh.interval_hours as u64 * 60 * 60 * 1000,
                },
                "Refresh investment prices"
            );
        }
        ensure_job!(
            JOB_FINANCE_HEALTH_CHECK,
            scheduling::CronSchedule::Cron {
                expr: "0 0 * * *".to_string(),
                tz: None,
            },
            "Finance data health check"
        );
    }

    // Daily atom decay (3 AM local)
    ensure_job!(
        JOB_ATOM_DECAY,
        scheduling::CronSchedule::Cron {
            expr: "0 3 * * *".to_string(),
            tz: Some(config.timezone.clone()),
        },
        "Daily knowledge atom decay"
    );

    // Proactive suggestion scan (every 4 hours)
    if tasks_config.proactive_suggestions {
        ensure_job!(
            JOB_PROACTIVE_SCAN,
            scheduling::CronSchedule::Cron {
                expr: "0 */4 * * *".to_string(),
                tz: None,
            },
            "Proactive task suggestion scan"
        );
    }

    // Autotuner nightly evaluation cycle
    if config.autotuner.enabled {
        agent::autotuner::AutoTunerOrchestrator::ensure_nightly_job(
            cron_service,
            &config.autotuner.schedule,
        )
        .await?;
    }

    Ok(())
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
