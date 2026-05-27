use async_trait::async_trait;
use std::sync::Arc;

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;

/// Plugin that registers simple domain tools that don't depend on agent-internal
/// services (subagent manager, conversation recall, memory retrievers, etc.).
pub struct AgentToolsPlugin;

#[async_trait]
impl AppCorePlugin for AgentToolsPlugin {
    fn name(&self) -> &str {
        "agent-tools"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let work_context_enabled = ctx.deps.config.read().await.work_context.enabled;
        let repos = &ctx.deps.repos;

        // Area tool
        ctx.register_tool(tools::area_tool::AreaTool::new(repos.areas.clone()));

        // Project tool
        ctx.register_tool(tools::project_tool::ProjectTool::new(
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
            if let Some(ref domain_bus) = ctx.deps.domain_event_bus {
                alarm_tool = alarm_tool.with_domain_bus(Arc::clone(domain_bus));
            }
            ctx.register_tool(alarm_tool);
        }

        // Work context tool
        if work_context_enabled {
            ctx.register_tool(activity_log::WorkContextTool::new(
                ctx.deps.storage_pool.clone(),
            ));
        }

        tracing::info!("Agent domain tools registered");
        Ok(())
    }
}
