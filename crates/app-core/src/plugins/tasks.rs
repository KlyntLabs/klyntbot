use async_trait::async_trait;
use std::sync::Arc;
use tools_core::FeaturePackage;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Plugin wrapper for the `feature-tasks` crate.
/// Registers task/OKR tools and spawns focus-alarm background loops.
pub struct TasksPlugin;

#[async_trait]
impl AppCorePlugin for TasksPlugin {
    fn name(&self) -> &str {
        "tasks"
    }

    fn migrations(&self) -> Vec<tools_core::FeatureMigration> {
        feature_tasks::TasksFeature::new().migrations()
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        ctx.register_ai_feature(|reg| feature_tasks::TasksFeature::register(reg));
        ctx.register_metrics(|reg| reg.register_all(feature_tasks::TaskEvent::FEATURE_METRICS));
        ctx.add_feature_translator(
            feature_tasks::events::try_from_domain_event,
            ai_core::RecallDomain::Tasks,
        );

        let pool = ctx.deps.pool();
        let config = ctx.deps.config.read().await;

        // ── Task tool ────────────────────────────────────────────────────
        let task_repo = storage::TaskRepo::new(pool.clone());
        let area_repo = storage::AreaRepo::new(pool.clone());
        let mut task_tool = feature_tasks::TaskTool::new(
            task_repo,
            config.todo.focus.max_slots,
            config.todo.focus.deadline_hours,
            config.timezone.clone(),
        )
        .with_area_repo(area_repo);

        // Task embedding (semantic search)
        if let (true, Some(vs)) = (config.todo.search.enabled, ctx.deps.vector_store.clone()) {
            let task_embed_impl =
                Arc::new(::agent::adapters::task_embedding::TaskEmbeddingAdapter::new(
                    Arc::clone(ctx.deps.embedding_engine.as_ref().expect("embedding engine for tasks")),
                    vs.clone(),
                ));
            task_tool = task_tool
                .with_embedding_handler(
                    Arc::clone(&task_embed_impl) as Arc<dyn feature_tasks::EmbeddingHandler>
                )
                .with_embedding_store(vs)
                .with_search_config(
                    config.todo.search.semantic_threshold,
                    config.todo.search.rrf_k,
                );
        }

        // Inject progress handler for KR→Objective cascade
        let progress_handler: Arc<dyn tools_core::ProgressHandler> =
            Arc::new(::agent::adapters::progress::ProgressHandlerImpl::new(
                ctx.deps.repos.key_results.clone(),
                ctx.deps.repos.objectives.clone(),
                ctx.deps.repos.tasks.clone(),
            ));
        task_tool = task_tool.with_progress_handler(Arc::clone(&progress_handler));

        // Wire DomainEventBus for task lifecycle events
        if let Some(ref domain_bus) = ctx.deps.domain_event_bus {
            task_tool = task_tool.with_domain_bus(Arc::clone(domain_bus));
        }

        // Wire alarm writer
        task_tool = task_tool.with_alarm_writer(
            ctx.deps.repos.task_alarms.clone(),
            ctx.deps.fire_store(),
        );

        ctx.register_tool(task_tool);

        // ── OKR tool ─────────────────────────────────────────────────────
        ctx.register_tool(
            tools::okr_tool::OkrTool::new(
                ctx.deps.repos.objectives.clone(),
                ctx.deps.repos.key_results.clone(),
            )
            .with_progress_handler(Arc::clone(&progress_handler)),
        );

        drop(config);

        let Some(ref domain_bus) = ctx.deps.domain_event_bus else {
            tracing::warn!("tasks plugin: no domain event bus, skipping background spawns");
            return Ok(());
        };

        // Focus alarms — materializes warning + expire alarms into scheduled_fires.
        let fire_store = ctx.deps.fire_store();
        let _focus_alarms_handle = feature_tasks::focus_alarms::spawn(
            Arc::clone(domain_bus),
            fire_store,
            ctx.deps.shutdown_token.clone(),
        );

        // AlarmFired side-effects — unfocuses expired tasks, updates last_reminded_at.
        let _alarm_side_effects_handle = feature_tasks::alarm_side_effects::spawn(
            Arc::clone(domain_bus),
            ctx.deps.repos.tasks.clone(),
            ctx.deps.shutdown_token.clone(),
        );

        // Focus deadline watcher — polls every 60s for expired focus sessions.
        let _focus_watcher_handle = feature_tasks::focus_watcher::spawn(
            Arc::clone(domain_bus),
            ctx.deps.repos.tasks.clone(),
            ctx.deps.shutdown_token.clone(),
        );

        tracing::info!("tasks plugin: tools registered + focus alarm background loops spawned");
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        use crate::init::cron::{
            publish_cron_alarm, JOB_DAILY_DIGEST, JOB_FOCUS_CHECK, JOB_OVERDUE_CHECK,
            JOB_RECURRING_TASKS,
        };

        let (deadline_hours, timezone) = {
            let config = app.config.read().await;
            (config.todo.focus.deadline_hours, config.timezone.clone())
        };

        // ── recurring_tasks (no domain bus required — register unconditionally) ──
        {
            let todo_repo = app.repos.tasks.clone();
            let rt = tokio::runtime::Handle::current();
            app.cron_executor.register(
                JOB_RECURRING_TASKS,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let todo_repo = todo_repo.clone();
                    let timezone = timezone.clone();
                    tokio::task::block_in_place(|| {
                        rt.block_on(async move {
                            match ::agent::services::recurring_tasks::RecurringTaskSpawner::check_and_spawn_static(
                                &todo_repo,
                                &timezone,
                            )
                            .await
                            {
                                Ok(()) => Ok(Some("Recurring task check complete".to_string())),
                                Err(e) => {
                                    tracing::warn!("Recurring task check failed: {e}");
                                    Ok(Some(format!("Recurring task check failed: {e}")))
                                }
                            }
                        })
                    })
                }),
            );
        }

        // The remaining task cron handlers publish AlarmFired events, so they
        // require the domain bus.
        let Ok(domain_bus) = app.domain_event_bus() else {
            tracing::warn!("tasks plugin: no domain event bus, skipping alarm-publishing cron handlers");
            return Ok(());
        };

        // ── todo_focus_check ──────────────────────────────────────────────
        {
            let todo_repo = app.repos.tasks.clone();
            let domain_bus = Arc::clone(&domain_bus);
            let rt = tokio::runtime::Handle::current();
            app.cron_executor.register(
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

        // ── todo_daily_digest ─────────────────────────────────────────────
        {
            let todo_repo = app.repos.tasks.clone();
            let domain_bus = Arc::clone(&domain_bus);
            let rt = tokio::runtime::Handle::current();
            app.cron_executor.register(
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

        // ── todo_overdue_check ────────────────────────────────────────────
        {
            let todo_repo = app.repos.tasks.clone();
            let domain_bus = Arc::clone(&domain_bus);
            let rt = tokio::runtime::Handle::current();
            app.cron_executor.register(
                JOB_OVERDUE_CHECK,
                Arc::new(move |_job: &scheduling::CronJob| {
                    let todo_repo = todo_repo.clone();
                    let domain_bus = Arc::clone(&domain_bus);
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
                                    expired_count, deadline_hours
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

        Ok(())
    }
}
