use std::any::{Any, TypeId};
use std::sync::Arc;

use bus::DomainEvent;
use dashmap::DashMap;
use tracing::info;

use super::context::{AiFeatureRegistration, MetricRegistration, PluginContext, PluginDeps};
use super::AppCorePlugin;

/// A type-map of plugin handles.
///
/// Plugins insert typed handles during `init()`; callers retrieve them by type.
/// This replaces the 70-field `AppCore` god object with a dynamic but type-safe
/// lookup surface.
#[derive(Default, Clone)]
pub struct FeatureHost {
    handles: DashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl FeatureHost {
    pub fn new() -> Self {
        Self {
            handles: DashMap::new(),
        }
    }

    /// Insert a typed handle. If a handle of the same type already exists, it is overwritten.
    pub fn insert<T: Send + Sync + 'static>(&self, handle: Arc<T>) {
        self.handles.insert(TypeId::of::<T>(), handle);
    }

    /// Retrieve a typed handle.
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.handles
            .get(&TypeId::of::<T>())
            .and_then(|entry| entry.clone().downcast::<T>().ok())
    }

    /// Check whether a handle of the given type exists.
    pub fn has<T: Send + Sync + 'static>(&self) -> bool {
        self.handles.contains_key(&TypeId::of::<T>())
    }

    /// Retrieve a cloned value from the type-map, avoiding the `map(|arc| (*arc).clone())` pattern.
    pub fn get_cloned<T: Clone + Send + Sync + 'static>(&self) -> Option<T> {
        self.get::<T>().map(|arc| (*arc).clone())
    }

    pub(crate) fn insert_raw(&self, type_id: TypeId, handle: Arc<dyn Any + Send + Sync>) {
        self.handles.insert(type_id, handle);
    }
}

/// Result of building a feature host.
pub struct FeatureHostResult {
    pub host: FeatureHost,
    pub tools: tools_core::registry::ToolRegistry,
    pub context_sources: Option<Vec<Box<dyn context_engine::ContextSource>>>,
    pub signal_consumers: Vec<Arc<dyn ai_core::SignalConsumer>>,
    pub event_translators: Vec<super::context::EventTranslator>,
    pub ai_feature_registrations: Vec<AiFeatureRegistration>,
    pub metric_registrations: Vec<MetricRegistration>,
    pub cron_handlers: Vec<(String, scheduling::temporal::cron_executor::CronHandler)>,
    pub background_spawns: Vec<tokio::task::JoinHandle<()>>,
    plugins: Vec<Box<dyn super::AppCorePlugin>>,
}

impl FeatureHostResult {
    /// Build the workspace `AiFeatureRegistry` from all plugin registrations.
    pub fn build_feature_registry(&self) -> ai_core::AiFeatureRegistry {
        let mut reg = ai_core::AiFeatureRegistry::new();
        for f in &self.ai_feature_registrations {
            f(&mut reg);
        }
        reg
    }

    /// Build the workspace `MetricRegistry` from all plugin registrations.
    pub fn build_metric_registry(&self) -> ai_core::MetricRegistry {
        let mut reg = ai_core::MetricRegistry::new();
        for f in &self.metric_registrations {
            f(&mut reg);
        }
        reg
    }

    /// Translate a domain event using all registered plugin translators.
    pub fn translate_event(&self, event: &DomainEvent) -> Option<ai_core::AiSignal> {
        super::context::PluginContext::run_translators(&self.event_translators, event)
    }
}

impl FeatureHostResult {
    /// Run post-init for all plugins. Call this after AppCore is fully assembled.
    pub async fn run_post_init(&self, app: &crate::state::AppCore) -> common::Result<()> {
        for plugin in &self.plugins {
            tracing::info!(plugin = plugin.name(), "running post-init");
            plugin.post_init(app).await?;
        }
        Ok(())
    }
}

/// Orchestrates plugin initialization.
///
/// 1. Collects migrations from all plugins and runs them.
/// 2. Calls `plugin.init(ctx)` for each plugin in registration order.
/// 3. Returns the assembled host and all registered contributions.
pub struct FeatureHostBuilder {
    plugins: Vec<Box<dyn AppCorePlugin>>,
    pre_handles: Vec<(TypeId, Arc<dyn Any + Send + Sync>)>,
}

impl FeatureHostBuilder {
    pub fn new() -> Self {
        Self { plugins: vec![], pre_handles: vec![] }
    }

    /// Add a plugin to the host.
    pub fn plugin(mut self, plugin: impl AppCorePlugin) -> Self {
        self.plugins.push(Box::new(plugin));
        self
    }

    /// Pre-insert a typed handle into the host before any plugin runs.
    /// Useful for handles created before the plugin phase (e.g. cron-built orchestrators).
    pub fn with_handle<T: Send + Sync + 'static>(mut self, handle: Arc<T>) -> Self {
        self.pre_handles.push((TypeId::of::<T>(), handle));
        self
    }

    /// Build the host: run migrations, initialize all plugins, and return contributions.
    pub async fn build(self, deps: &PluginDeps) -> common::Result<FeatureHostResult> {
        // Phase 0: topological sort by declared dependencies
        let plugins = super::toposort::resolve_order(self.plugins)?;

        // Phase 1: collect and run migrations
        let mut all_migrations = Vec::new();
        for plugin in &plugins {
            let mut migs = plugin.migrations();
            for m in &mut migs {
                // Sanity check: migration feature name should match plugin name
                if m.feature_name != plugin.name() {
                    tracing::warn!(
                        plugin = plugin.name(),
                        migration_feature = %m.feature_name,
                        "migration feature name does not match plugin name"
                    );
                }
            }
            all_migrations.extend(migs);
        }

        if !all_migrations.is_empty() {
            storage::StoragePool::run_feature_migrations(deps.storage_pool.inner(), &all_migrations)
                .await
                .map_err(|e| {
                    common::KlyntbotError::Storage(format!(
                        "feature host migrations failed: {e}"
                    ))
                })?;
            info!(count = all_migrations.len(), "plugin migrations complete");
        }

        // Phase 2: initialize plugins in resolved order
        let mut tools = tools_core::registry::ToolRegistry::new();
        let mut context_sources = Vec::new();
        let mut signal_consumers = Vec::new();
        let mut event_translators = Vec::new();
        let mut ai_feature_registrations = Vec::new();
        let mut metric_registrations = Vec::new();
        let mut cron_handlers = Vec::new();
        let mut background_spawns = Vec::new();
        let mut host = FeatureHost::new();
        for (type_id, handle) in &self.pre_handles {
            host.insert_raw(*type_id, Arc::clone(handle));
        }

        for plugin in &plugins {
            let mut ctx = PluginContext::new(
                deps,
                &mut tools,
                &mut context_sources,
                &mut signal_consumers,
                &mut event_translators,
                &mut ai_feature_registrations,
                &mut metric_registrations,
                &mut cron_handlers,
                &mut background_spawns,
                &mut host,
            );

            info!(plugin = plugin.name(), "initializing plugin");
            plugin.init(&mut ctx).await?;
        }

        info!(
            plugins = plugins.len(),
            tools = tools.len(),
            context_sources = context_sources.len(),
            signal_consumers = signal_consumers.len(),
            "feature host built"
        );

        Ok(FeatureHostResult {
            host,
            tools,
            context_sources: Some(context_sources),
            signal_consumers,
            event_translators,
            ai_feature_registrations,
            metric_registrations,
            cron_handlers,
            background_spawns,
            plugins,
        })
    }
}
