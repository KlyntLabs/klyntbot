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
    pub pipeline_broadcast_tx: tokio::sync::broadcast::Sender<cognitive::PipelineEvent>,
    pub user_situation: Arc<Mutex<UserSituation>>,
    pub active_view: Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>,
    pub activity_svc: Arc<activity_log::ActivityIngestionService>,
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
    coding_recall: Option<Arc<coding_memory::recall::CodingRecallService>>,
    approval_channel: Option<Arc<dyn approval::ApprovalChannel>>,
    coding_policies: Option<
        Arc<dashmap::DashMap<String, Arc<parking_lot::RwLock<approval::CodingApprovalPolicy>>>>,
    >,
    injector_registry: Option<bus::InjectorRegistry>,
) -> Result<AgentResult, String> {
    // Run activity-log migrations (unified activity log).
    StoragePool::run_feature_migrations(
        storage_pool.inner(),
        &activity_log::activity_log_migrations(),
    )
    .await
    .map_err(|e| format!("activity-log migration failed: {e}"))?;
    let activity_svc = Arc::new(activity_log::ActivityIngestionService::new(
        storage_pool.clone(),
        activity_log::PrivacyFilter::default(),
    ));

    // Pre-create user situation (defaults now, recomputed with real data below
    // and every 2 min afterwards). Shared with CognitiveContextSource for
    // situational_boost in memory retrieval.
    let user_situation = Arc::new(Mutex::new(UserSituation::default()));

    // Pre-create active view (None until frontend pushes a view).
    // Shared with AgentRuntime for RetrievalContext.active_view.
    let active_view: Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>> =
        Arc::new(tokio::sync::RwLock::new(None));

    // 8. Build AgentLoop
    let (pipeline_broadcast_tx, _) =
        tokio::sync::broadcast::channel::<cognitive::PipelineEvent>(64);
    let pipeline_tx = pipeline_broadcast_tx.clone();
    let mut builder = AgentLoop::builder(bus.clone(), provider, config.clone())
        .with_pool(storage_pool.inner().clone())
        .with_cron_executor(Arc::clone(cron_executor), cron_repo.clone());

    if let Some(engine) = embedding_engine {
        builder = builder.with_embedding_engine(engine);
    }

    let mut builder = builder
        .with_domain_bus(Arc::clone(domain_event_bus))
        .with_cognitive_provider(cognitive_provider.clone())
        .with_pipeline_tx(pipeline_tx)
        .with_user_situation(user_situation.clone())
        .with_activity_service(Arc::clone(&activity_svc))
        .with_active_view(active_view.clone())
        .with_hot_config(hot_config);

    if let Some(vs) = vector_store {
        builder = builder.with_vector_store(vs);
    }

    if let Some(orchestrator) = autotuner {
        builder = builder.with_autotuner(Arc::clone(orchestrator));
    }

    if let Some(queue) = context_update_queue {
        builder = builder.with_context_update_queue(queue);
    }

    builder = builder.with_coding_recall_service(coding_recall);

    if let Some(channel) = approval_channel {
        builder = builder.with_approval_channel(channel);
    }
    if let Some(policies) = coding_policies {
        builder = builder.with_coding_policies(policies);
    }
    if let Some(registry) = injector_registry {
        builder = builder.with_injector_registry(registry);
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
        pipeline_broadcast_tx,
        user_situation,
        active_view,
        activity_svc,
    })
}
