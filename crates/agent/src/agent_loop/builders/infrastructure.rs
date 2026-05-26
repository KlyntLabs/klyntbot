//! Infrastructure service builder for the agent loop.
//!
//! Assembles the session manager, subagent manager, base tool registry,
//! learning outcome store, and conversation recall service.

use std::sync::Arc;
use tokio::sync::RwLock;

use bus::MessageBus;
use config::Config;
use providers::DynProvider;
use session::SessionManager;
use tools::{cron_tool::CronTool, registry::ToolRegistry, subagents::SubagentsTool};

use crate::{CronHandlerAdapter, SubagentManager};

/// Result of the infrastructure build phase.
pub(crate) struct InfrastructureResult {
    pub session_manager: SessionManager,
    pub subagent_manager: Arc<SubagentManager>,
    pub tool_registry: ToolRegistry,
    pub outcome_store: Option<Arc<RwLock<crate::learning::OutcomeStore>>>,
    pub recall_service: Option<Arc<cognitive::ConversationRecallService>>,
}

/// Build core infrastructure services.
pub(crate) async fn build_infrastructure(
    config: &Config,
    provider: &DynProvider,
    bus: &Arc<MessageBus>,
    workspace: &std::path::Path,
    storage_pool: &storage::StoragePool,
    repos: &storage::Repos,
    embedding_engine: &Arc<tools::EmbeddingEngine>,
    vector_store: &Option<storage::VectorStore>,
    cron_executor: &Option<(
        Arc<scheduling::temporal::cron_executor::CronExecutor>,
        storage::repos::cron::CronRepo,
    )>,
    job_supervisor: &Option<tools_core::DynJobSupervisor>,
    tool_registry: Option<tools_core::registry::ToolRegistry>,
    tool_kit: Option<Arc<klynt_core::ToolKitBuilder>>,
    hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
) -> InfrastructureResult {
    // ── Session manager (SQL-backed) ──────────────────────────────────
    let session_manager = SessionManager::from_repo(
        storage::SessionRepo::new(storage_pool.inner().clone()),
        config.conversation.session.max_cache_size,
    )
    .await;

    // ── Subagent manager ──────────────────────────────────────────────
    let mut subagent_builder =
        SubagentManager::builder(Arc::clone(provider), workspace.to_path_buf())
            .inbound_sender(bus.inbound_sender())
            .model(config.agents.defaults.model.clone())
            .max_concurrent_subagents(config.agents.defaults.max_concurrent_subagents)
            .agent_task_repo(repos.agent_tasks.clone())
            .job_supervisor(job_supervisor.clone())
            .repos(repos.clone());
    if let Some(kit) = tool_kit {
        subagent_builder = subagent_builder.tool_kit(kit);
    }
    if let Some(engine) = hook_engine {
        subagent_builder = subagent_builder.hook_engine(engine);
    }
    let subagent_manager = Arc::new(subagent_builder.build());

    // ── Tool registry ─────────────────────────────────────────────────
    let mut tool_registry = tool_registry.unwrap_or_else(ToolRegistry::new);

    // Subagents tool
    tool_registry.register(SubagentsTool::with_handler(
        Arc::clone(&subagent_manager) as Arc<dyn tools::subagents::SubagentsHandler>
    ));

    // Cron tool (optional)
    if let Some((ref executor, ref repo)) = cron_executor {
        let adapter: Arc<dyn tools::cron_tool::CronHandler> =
            Arc::new(CronHandlerAdapter::new(Arc::clone(executor), repo.clone()));
        tool_registry.register(CronTool::with_handler(adapter));
    }

    // ── Learning: outcome store ────────────────────────────
    let outcome_store = if config.learning.enabled {
        Some(Arc::new(RwLock::new(crate::learning::OutcomeStore::new(
            repos.outcomes.clone(),
        ))))
    } else {
        None
    };

    // ── Create ConversationRecallService (shared by retriever + handler) ──
    let recall_service: Option<Arc<cognitive::ConversationRecallService>> =
        if let (true, Some(vs)) = (config.conversation.embedding.enabled, vector_store.clone()) {
            let text_embedder =
                Arc::new(crate::adapters::cognitive_embedder::TextEmbedderImpl::new(
                    Arc::clone(embedding_engine),
                ));
            Some(Arc::new(cognitive::ConversationRecallService::new(
                vs,
                text_embedder,
                cognitive::RecallConfig {
                    decay_half_life_days: config.conversation.memory.decay_half_life_days as f64,
                    default_threshold: config.conversation.search.semantic_threshold as f32,
                    ..cognitive::RecallConfig::default()
                },
            )))
        } else {
            None
        };

    InfrastructureResult {
        session_manager,
        subagent_manager,
        tool_registry,
        outcome_store,
        recall_service,
    }
}
