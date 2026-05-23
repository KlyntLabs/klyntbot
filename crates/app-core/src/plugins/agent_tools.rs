use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Plugin that registers simple domain tools that don't depend on agent-internal
/// services (subagent manager, conversation recall, memory retrievers, etc.).
pub struct AgentToolsPlugin;

#[async_trait]
impl AppCorePlugin for AgentToolsPlugin {
    fn name(&self) -> &str {
        "agent-tools"
    }

    async fn init(&self, _ctx: &mut PluginContext) -> common::Result<()> {
        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        let work_context_enabled = app.config.read().await.work_context.enabled;
        let repos = &app.repos;

        let reg = app.agent.tool_registry();
        let mut registry = reg.write().await;

        // Area tool
        registry.register(tools::area_tool::AreaTool::new(repos.areas.clone()));

        // Project tool
        registry.register(tools::project_tool::ProjectTool::new(
            repos.projects.clone(),
            repos.tasks.clone(),
        ));

        // Alarm tool (standalone reminders)
        {
            let fire_store = Arc::new(scheduling::temporal::fire_store::FireStore::new(
                repos.scheduled_fires.clone(),
            ));
            let mut alarm_tool =
                feature_alarms::AlarmTool::new(fire_store, repos.scheduled_fires.clone());
            if let Some(ref domain_bus) = app.domain_event_bus {
                alarm_tool = alarm_tool.with_domain_bus(Arc::clone(domain_bus));
            }
            registry.register(alarm_tool);
        }

        // Work context tool
        if work_context_enabled {
            registry.register(activity_log::WorkContextTool::new(app.storage_pool.clone()));
        }

        tracing::info!("Agent domain tools registered");
        Ok(())
    }
}
