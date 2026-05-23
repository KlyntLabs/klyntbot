use std::sync::Arc;

use ai_core::SignalConsumer;
use bus::{DomainEvent, MessageBus};
use context_engine::ContextSource;
use scheduling::temporal::cron_executor::{CronExecutor, CronHandler};
use storage::{Repos, StoragePool, VectorStore};
use tokio_util::sync::CancellationToken;
use tools::embedding_engine::EmbeddingEngine;
use tools_core::registry::ToolRegistry;

use super::host::FeatureHost;

/// Shared infrastructure available to every plugin during initialization.
///
/// This is intentionally wide — init is wide. But it is a *single* wide interface,
/// not four separate files that a feature author must edit.
pub struct PluginDeps {
    pub mode: common::AppMode,
    pub config: Arc<tokio::sync::RwLock<config::Config>>,
    pub hot_config: Arc<tokio::sync::RwLock<config::HotConfig>>,
    pub storage_pool: StoragePool,
    pub repos: Repos,
    pub provider: providers::DynProvider,
    pub cognitive_provider: Option<providers::DynProvider>,
    pub vector_store: Option<VectorStore>,
    pub embedding_engine: Option<Arc<EmbeddingEngine>>,
    pub domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    pub bus: Arc<MessageBus>,
    pub cron_executor: Arc<CronExecutor>,
    pub activity_svc: Option<Arc<activity_log::ActivityIngestionService>>,
    pub user_situation: Option<Arc<tokio::sync::Mutex<cognitive::situation::UserSituation>>>,
    pub active_view: Option<Arc<tokio::sync::RwLock<Option<context_engine::ActiveView>>>>,
    pub agent: Option<Arc<agent::AgentLoop>>,
    pub autotuner: Option<Arc<agent::autotuner::AutoTunerOrchestrator>>,
    pub event_emitter: Option<Arc<dyn crate::events::AppEventEmitter>>,
    pub notification_sender: Option<Arc<dyn common::NotificationSender>>,
    pub cognitive_fact_embedder: Option<Arc<dyn ::cognitive::SemanticFactEmbedder>>,
    pub pipeline_broadcast: Option<tokio::sync::broadcast::Sender<::cognitive::PipelineEvent>>,
    pub shutdown_token: CancellationToken,
}

/// Event translator: converts a `DomainEvent` into an optional `AiSignal`.
pub type EventTranslator = Box<dyn Fn(&DomainEvent) -> Option<ai_core::AiSignal> + Send + Sync>;

/// The registration surface passed to `AppCorePlugin::init()`.
///
/// Plugins use mutable references to register their contributions. The host owns
/// all collections and assembles them after all plugins have run.
pub struct PluginContext<'a> {
    pub deps: &'a PluginDeps,
    pub tools: &'a mut ToolRegistry,
    pub context_sources: &'a mut Vec<Box<dyn ContextSource>>,
    pub signal_consumers: &'a mut Vec<Arc<dyn SignalConsumer>>,
    pub event_translators: &'a mut Vec<EventTranslator>,
    pub cron_handlers: &'a mut Vec<(String, CronHandler)>,
    pub background_spawns: &'a mut Vec<tokio::task::JoinHandle<()>>,
    pub host: &'a mut FeatureHost,
}

impl<'a> PluginContext<'a> {
    pub fn new(
        deps: &'a PluginDeps,
        tools: &'a mut ToolRegistry,
        context_sources: &'a mut Vec<Box<dyn ContextSource>>,
        signal_consumers: &'a mut Vec<Arc<dyn SignalConsumer>>,
        event_translators: &'a mut Vec<EventTranslator>,
        cron_handlers: &'a mut Vec<(String, CronHandler)>,
        background_spawns: &'a mut Vec<tokio::task::JoinHandle<()>>,
        host: &'a mut FeatureHost,
    ) -> Self {
        Self {
            deps,
            tools,
            context_sources,
            signal_consumers,
            event_translators,
            cron_handlers,
            background_spawns,
            host,
        }
    }

    /// Register a tool in the agent's tool registry.
    pub fn register_tool(&mut self, tool: impl tools_core::Tool + 'static) {
        self.tools.register(tool);
    }

    /// Register a dynamic tool.
    pub fn register_dyn_tool(&mut self, tool: tools_core::DynTool) {
        self.tools.register_dyn(tool);
    }

    /// Add a context source for the `ContextEngine`.
    pub fn add_context_source(&mut self, source: Box<dyn ContextSource>) {
        self.context_sources.push(source);
    }

    /// Add a signal consumer for the AI pipeline.
    pub fn add_signal_consumer(&mut self, consumer: Arc<dyn SignalConsumer>) {
        self.signal_consumers.push(consumer);
    }

    /// Add an event translator for `DomainEvent → AiSignal` conversion.
    pub fn add_event_translator(&mut self, translator: EventTranslator) {
        self.event_translators.push(translator);
    }

    /// Register a cron handler.
    pub fn register_cron_handler(&mut self, name: impl Into<String>, handler: CronHandler) {
        self.cron_handlers.push((name.into(), handler));
    }

    /// Spawn a background task tied to the app lifecycle.
    pub fn spawn_background(&mut self, task: impl std::future::Future<Output = ()> + Send + 'static) {
        let handle = tokio::spawn(task);
        self.background_spawns.push(handle);
    }

    /// Insert a typed handle into the host for cross-plugin retrieval.
    pub fn insert_handle<T: Send + Sync + 'static>(&mut self, handle: Arc<T>) {
        self.host.insert(handle);
    }
}
