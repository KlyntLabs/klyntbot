use async_trait::async_trait;
use std::sync::Arc;
use tools_core::FeaturePackage;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

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

        let pool = ctx.deps.storage_pool.inner().clone();
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
        {
            let fire_store = Arc::new(scheduling::temporal::fire_store::FireStore::new(
                ctx.deps.repos.scheduled_fires.clone(),
            ));
            task_tool = task_tool.with_alarm_writer(ctx.deps.repos.task_alarms.clone(), fire_store);
        }

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
        let fire_store = Arc::new(scheduling::temporal::fire_store::FireStore::new(
            ctx.deps.repos.scheduled_fires.clone(),
        ));
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
}
