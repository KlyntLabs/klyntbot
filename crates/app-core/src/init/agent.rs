use std::sync::Arc;

use agent::AgentLoop;
use bus::{DomainEventBus, MessageBus};
use cognitive::situation::UserSituation;
use scheduling::temporal::cron_executor::CronExecutor;
use storage::{Repos, StoragePool, VectorStore};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::info;

/// Results from the agent initialization phase.
pub(super) struct AgentResult {
    pub cognitive_provider: Option<providers::DynProvider>,
    pub agent: Arc<AgentLoop>,
    pub inbound_rx: mpsc::Receiver<bus::InboundMessage>,
}

/// Initialize persona manager, activity log, and agent loop.
///
/// The message bus, cognitive provider, and domain event bus are created by the
/// orchestrator and passed in, since they are shared with the cron phase.
#[allow(clippy::too_many_arguments)]
pub(super) async fn init_agent(
    config: &config::Config,
    storage_pool: &StoragePool,
    _repos: &Repos,
    provider: providers::DynProvider,
    vector_store: Option<VectorStore>,
    bus: &Arc<MessageBus>,
    cognitive_provider: Option<providers::DynProvider>,
    domain_event_bus: &Arc<DomainEventBus>,
    cron_executor: &Arc<CronExecutor>,
    cron_repo: &storage::repos::cron::CronRepo,
    autotuner: Option<&Arc<agent::autotuner::AutoTunerOrchestrator>>,
    hot_config: Arc<RwLock<config::HotConfig>>,
    context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
    embedding_engine: Option<Arc<tools::EmbeddingEngine>>,
    approval_channel: Option<Arc<dyn approval::ApprovalChannel>>,
    injector_registry: Option<bus::InjectorRegistry>,
    tool_registry: tools_core::registry::ToolRegistry,
    context_sources: Vec<Box<dyn context_engine::ContextSource>>,
    user_situation: Arc<Mutex<UserSituation>>,
    active_view: Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>,
    activity_svc: Arc<activity_log::ActivityIngestionService>,
    pipeline_broadcast_tx: tokio::sync::broadcast::Sender<cognitive::PipelineEvent>,
    cognitive_fact_repo: Option<cognitive::SemanticFactRepo>,
    cognitive_entity_repo: Option<cognitive::EntityRepo>,
    cognitive_embedder: Option<Arc<dyn cognitive::SemanticFactEmbedder>>,
    tool_kit: Option<Arc<klynt_core::ToolKitBuilder>>,
    hook_engine: Option<Arc<klynt_hooks::HookEngine>>,
) -> Result<AgentResult, String> {
    let pipeline_tx = pipeline_broadcast_tx.clone();
    let mut builder = AgentLoop::builder(bus.clone(), provider, config.clone())
        .with_pool(storage_pool.inner().clone())
        .with_cron_executor(Arc::clone(cron_executor), cron_repo.clone())
        .with_tool_registry(tool_registry)
        .with_context_sources(context_sources)
        .with_user_situation(user_situation.clone())
        .with_activity_service(Arc::clone(&activity_svc))
        .with_active_view(active_view.clone())
        .with_hot_config(hot_config);
    if let Some(repo) = cognitive_fact_repo {
        builder = builder.with_cognitive_fact_repo(repo);
    }
    if let Some(repo) = cognitive_entity_repo {
        builder = builder.with_cognitive_entity_repo(repo);
    }
    if let Some(embedder) = cognitive_embedder {
        builder = builder.with_cognitive_embedder(embedder);
    }

    if let Some(engine) = embedding_engine {
        builder = builder.with_embedding_engine(engine);
    }

    let mut builder = builder
        .with_domain_bus(Arc::clone(domain_event_bus))
        .with_cognitive_provider(cognitive_provider.clone())
        .with_pipeline_tx(pipeline_tx);

    if let Some(vs) = vector_store {
        builder = builder.with_vector_store(vs);
    }

    if let Some(orchestrator) = autotuner {
        builder = builder.with_autotuner(Arc::clone(orchestrator));
    }

    if let Some(queue) = context_update_queue {
        builder = builder.with_context_update_queue(queue);
    }

    if let Some(channel) = approval_channel {
        builder = builder.with_approval_channel(channel);
    }
    if let Some(registry) = injector_registry {
        builder = builder.with_injector_registry(registry);
    }
    if let Some(kit) = tool_kit {
        builder = builder.with_tool_kit(kit);
    }
    if let Some(engine) = hook_engine {
        builder = builder.with_hook_engine(engine);
    }

    let mut agent_loop_raw = builder
        .build()
        .await
        .map_err(|e| format!("agent build failed: {e}"))?;
    let inbound_rx = agent_loop_raw
        .take_inbound_rx()
        .expect("inbound receiver already taken");
    let agent = Arc::new(agent_loop_raw);
    info!("agent loop initialized");

    Ok(AgentResult {
        cognitive_provider,
        agent,
        inbound_rx,
    })
}
