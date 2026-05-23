use async_trait::async_trait;
use std::sync::Arc;
use tools_core::FeaturePackage;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

/// Plugin wrapper for the `feature-tasks` crate.
/// Spawns focus-alarm and side-effect background loops.
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

        tracing::info!("tasks plugin: focus alarm background loops spawned");
        Ok(())
    }
}
