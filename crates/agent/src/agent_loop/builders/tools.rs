//! Feature-tool registration for the agent loop.
//!
//! Registers memory, productivity, MCP, skill-reference, and recurring-task tools.

use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

use config::Config;
use providers::DynProvider;

/// Inputs for the feature-tool build phase.
pub(crate) struct ToolsBuildInput<'a> {
    pub config: &'a Config,
    pub tool_registry: &'a mut tools::registry::ToolRegistry,
    pub conversation_recall_handler: &'a Option<Arc<dyn tools::ConversationRecallHandler>>,
    pub todo_embedding_handler: &'a Option<Arc<dyn tools::EmbeddingHandler>>,
    pub repos: &'a storage::Repos,
    pub prod_repos: Option<feature_productivity::repos::ProductivityRepos>,
    pub vector_store: &'a Option<storage::VectorStore>,
    pub domain_event_bus: &'a Option<Arc<bus::DomainEventBus>>,
    pub latest_enhancement_trace: &'a Arc<context_engine::enhancement::LatestEnhancementTrace>,
    pub provider: &'a DynProvider,
    pub skill_reference_index: &'a Arc<tools::SkillReferenceIndex>,
}

/// Result of the feature-tool build phase.
pub(crate) struct ToolsBuildResult {
    pub mcp_manager: Option<mcp::McpManager>,
    pub recurring_task_spawner: Option<Arc<RwLock<crate::RecurringTaskSpawner>>>,
}

/// Register feature-specific tools and start ancillary services.
pub(crate) async fn build_feature_tools(
    input: ToolsBuildInput<'_>,
) -> common::Result<ToolsBuildResult> {
    let ToolsBuildInput {
        config,
        tool_registry,
        conversation_recall_handler,
        todo_embedding_handler,
        repos,
        prod_repos,
        vector_store,
        domain_event_bus,
        latest_enhancement_trace,
        provider,
        skill_reference_index,
    } = input;

    // ── Memory tool ───────────────────────────────────────────────────
    if config.conversation.search.enabled {
        if let Some(ref handler) = conversation_recall_handler {
            let mut memory_tool = tools::MemoryTool::new()
                .with_conversation_handler(Arc::clone(handler))
                .with_todo_repo(repos.tasks.clone())
                .with_threshold(config.conversation.search.semantic_threshold)
                .with_rrf_k(config.todo.search.rrf_k);

            if let (Some(ref h), Some(ref vs)) = (todo_embedding_handler, vector_store) {
                memory_tool = memory_tool
                    .with_todo_embedding_handler(Arc::clone(h))
                    .with_embedding_store(vs.clone());
            }

            if let Some(ref bus) = domain_event_bus {
                memory_tool = memory_tool.with_domain_bus(Arc::clone(bus));
            }

            memory_tool =
                memory_tool.with_enhancement_trace_store(Arc::clone(latest_enhancement_trace));

            tool_registry.register(memory_tool);
        }
    }

    // ── Productivity tool ─────────────────────────────────────────────
    if let Some(prod_repos) = prod_repos {
        let focus_mgr = Arc::new(feature_productivity::FocusManager::new(
            prod_repos.clone(),
            config.productivity.focus.clone(),
        ));
        let prod_handler = Arc::new(crate::adapters::productivity::ProductivityHandlerImpl::new(
            provider.clone(),
            config.agents.defaults.model.clone(),
        ));
        let mut daily_agg = feature_productivity::DailyAggregator::new(prod_repos.clone())
            .with_handler(prod_handler);
        if let Some(ref bus) = domain_event_bus {
            daily_agg = daily_agg.with_domain_bus(Arc::clone(bus));
        }
        let aggregator = Arc::new(daily_agg);
        let productivity_tool =
            feature_productivity::ProductivityTool::new(prod_repos, focus_mgr, aggregator);
        tool_registry.register(productivity_tool);
    }

    // ── MCP tools (Model Context Protocol) ──────────────────────────
    let mcp_manager = if config.mcp.has_active_servers() {
        // connect_all logs startup progress internally via tracing
        let manager =
            mcp::McpManager::connect_all(&config.mcp, None, mcp::McpClientOptions::default()).await;
        let mcp_tools = manager.tools();
        let tool_count = mcp_tools.len();
        for tool in mcp_tools {
            tool_registry.register_dyn(tool as tools_core::DynTool);
        }
        if tool_count > 0 {
            info!(
                servers = manager.connected_count(),
                tools = tool_count,
                "MCP tools registered"
            );
        }
        Some(manager)
    } else {
        None
    };

    // ── Skill reference tool (progressive loading) ───────────────────
    tool_registry.register(tools::SkillReferenceTool::new(Arc::clone(
        skill_reference_index,
    )));

    // ── Recurring task spawner ────────────────────────────────────────
    let mut recurring_spawner = crate::RecurringTaskSpawner::new(
        repos.tasks.clone(),
        config.timezone.clone(),
        std::time::Duration::from_secs(60),
    );
    recurring_spawner.start();
    let recurring_task_spawner = Some(Arc::new(RwLock::new(recurring_spawner)));

    Ok(ToolsBuildResult {
        mcp_manager,
        recurring_task_spawner,
    })
}
