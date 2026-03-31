use std::sync::Arc;

use agent::{AgentLoop, PersonaManager};
use bus::{DomainEventBus, MessageBus};
use cognitive::situation::UserSituation;
use scheduling::CronService;
use storage::{Repos, StoragePool, VectorStore};
use tokio::sync::{mpsc, Mutex, RwLock};
use tracing::info;

/// Results from the agent initialization phase.
pub(super) struct AgentResult {
    pub cognitive_provider: Option<providers::DynProvider>,
    pub persona_manager: Arc<RwLock<PersonaManager>>,
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
    repos: &Repos,
    provider: providers::DynProvider,
    vector_store: Option<VectorStore>,
    bus: &Arc<MessageBus>,
    cognitive_provider: Option<providers::DynProvider>,
    domain_event_bus: &Arc<DomainEventBus>,
    cron_service: &Arc<CronService>,
    notification_dispatcher: &Arc<agent::NotificationDispatcher>,
    notification_sender: &Option<Arc<dyn common::NotificationSender>>,
    autotuner: Option<&Arc<agent::autotuner::AutoTunerOrchestrator>>,
    hot_config: Arc<RwLock<config::HotConfig>>,
    context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
    embedding_engine: Option<Arc<tools::EmbeddingEngine>>,
) -> Result<AgentResult, String> {
    // 7. Load personas
    let data_dir = config.data_dir_path();
    let personas_dir = data_dir.join("personas");
    let mut persona_manager = PersonaManager::load(&personas_dir).await;
    persona_manager.resolve_scopes(repos).await;
    let persona_manager = Arc::new(RwLock::new(persona_manager));
    info!("persona manager loaded");

    // Run activity-log migrations (unified activity log).
    StoragePool::run_feature_migrations(
        storage_pool.inner(),
        &activity_log::ActivityLog::migrations_static(),
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
        tokio::sync::broadcast::channel::<cognitive::PipelineEvent>(256);
    let pipeline_tx = pipeline_broadcast_tx.clone();
    let mut builder = AgentLoop::builder(bus.clone(), provider, config.clone())
        .with_pool(storage_pool.inner().clone())
        .with_cron_service(cron_service.clone())
        .with_notification_handle(notification_dispatcher.last_active_handle());

    if let Some(engine) = embedding_engine {
        builder = builder.with_embedding_engine(engine);
    }

    // Thread the custom notification sender (if provided) to the agent's ReminderEngine
    if let Some(ref sender) = notification_sender {
        builder = builder.with_notification_sender(Arc::clone(sender));
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
        persona_manager,
        agent,
        inbound_rx,
        pipeline_broadcast_tx,
        user_situation,
        active_view,
        activity_svc,
    })
}
