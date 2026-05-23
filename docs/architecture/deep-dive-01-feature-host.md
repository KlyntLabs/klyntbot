# Deep Dive: Extract a FeatureHost from the dual init monoliths

> Status: Design crystallized. See ADR-0001 and CONTEXT.md for recorded decisions.

## Decisions Made

| Question | Decision | Rationale |
|----------|----------|-----------|
| Extend `FeaturePackage` or new trait? | **New `AppCorePlugin` trait** | `FeaturePackage` lives in `tools-core` and is consumed by the tool registry. Extending it would couple tool metadata to AppCore lifecycle across crate boundaries. |
| Passive or active plugins? | **Active** (`plugin.init(ctx)` calls `ctx.register_tool(...)`) | Plugins can conditionally register based on config without the host knowing the conditions. Long-term flexibility. |
| `AppCore` Option A or B? | **Option A with delegation** | Shrink `AppCore` to core fields + `FeatureHost` with type-map handles. Keep accessor methods as thin delegates during migration to avoid breaking the command layer, then remove them once callers migrate. |
| Migration strategy? | **Incremental, simplest first** | `feature-focus` → `feature-language-learning` → `feature-alarms` → `feature-notes` → `feature-tasks` → `feature-productivity` → `feature-launcher` → `feature-coaching` → `cognitive` → `agent` |

---

## Current State (the problem)

There are **four independent wiring sites** that each know about every feature crate:

### Site A — Migrations (`app-core/src/init/storage.rs`)
```rust
run_migration!("notes", feature_notes::notes_migrations());
run_migration!("tasks", tasks_feature.migrations());
run_migration!("language-learning", feature_language_learning::language_learning_migrations());
run_migration!("focus", <feature_focus::FocusFeature as FeaturePackage>::migrations(&feature_focus::FocusFeature));
run_migration!("learning", <feature_learning::LearningFeature as FeaturePackage>::migrations(&feature_learning::LearningFeature::default()));
run_migration!("cognitive", cognitive::cognitive_migrations());
// Plus inline scheduling migration
```

### Site B — Agent tools + context sources (`agent/src/agent_loop/builder.rs`)
```rust
// ~1,500 lines constructing:
// - Context sources (soul, skills, identity, bootstrap, session, cognitive, project, annotation, productivity, work-context)
// - Background cognitive services (consolidation, session memory, work-context inference)
// - Tool registry entries (subagents, alarms, cron, tasks, notes, language-learning, work-context, productivity, MCP, skills, memory, calendar, OKR, area, project, annotate, learning, bash, temporal, mirror)
// - Query enhancement pipeline (signal enrichment, PRF, multi-query, reranking)
// - Tree builder subscribers (note, task, entity-linker, community, productivity, OKR, learning)
// - InsightForge + domain searchers
```

### Site C — AppCore post-init orchestration (`app-core/src/init/mod.rs`)
```rust
// ~1,750 lines:
// - Phases 1–10: storage, cron, temporal, embedding, agent, channels, productivity+launcher, coaching, cognitive, mirror, AI pipeline, BrainVoice
// - Launcher tool registration (post-agent)
// - Notification dispatcher assembly
// - Voice service + conversation manager (post-assembly)
// - Cron job registration (insight refresh, nightly batch)
// - Lifecycle monitor + wake orchestrator
// - Post-init tool registration (mirror, bash toolkit, temporal)
// - Background embedding catch-up
```

### Site D — AI pipeline (`app-core/src/init/ai_pipeline.rs`)
```rust
pub fn build_feature_registry() -> AiFeatureRegistry {
    let mut reg = ai_core::AiFeatureRegistry::new();
    feature_tasks::TasksFeature::register(&mut reg);
    feature_productivity::ProductivityFeature::register(&mut reg);
    feature_notes::NotesFeature::register(&mut reg);
    feature_learning::LearningFeature::register(&mut reg);
    feature_language_learning::LanguageLearningFeature::register(&mut reg);
    feature_coaching::CoachingFeature::register(&mut reg);
    reg
}

pub fn translate(event: &DomainEvent) -> Option<AiSignal> {
    if let Some(e) = feature_tasks::events::try_from_domain_event(event) { ... }
    if let Some(e) = feature_coaching::events::try_from_domain_event(event) { ... }
    if let Some(e) = feature_productivity::events::try_from_domain_event(event) { ... }
    if let Some(e) = feature_notes::events::try_from_domain_event(event) { ... }
    if let Some(e) = feature_learning::try_from_domain_event(event) { ... }
    if let Some(e) = feature_language_learning::try_from_domain_event(event) { ... }
    // ...
}
```

## The Existing `FeaturePackage` Trait (too shallow)

```rust
trait FeaturePackage {
    fn name(&self) -> &str;
    fn tools(&self) -> Vec<DynTool>;      // Empty for coaching, focus, learning
    fn migrations(&self) -> Vec<FeatureMigration>;
    async fn health_check(&self) -> Result<HealthStatus>;
}
```

It has no hooks for context sources, signal consumers, event translators, background services, cron handlers, or post-init callbacks. Several features return `Vec::new()` for tools and admit in comments that their real logic lives elsewhere.

## Proposed Design

### 1. `AppCorePlugin` — new trait at the app-core seam

```rust
#[async_trait]
pub trait AppCorePlugin: Send + Sync + 'static {
    fn name(&self) -> &str;
    fn migrations(&self) -> Vec<FeatureMigration> { vec![] }
    fn ai_feature(&self) -> Option<AiFeatureRecord> { None }

    /// Active registration: plugin calls ctx.register_tool(), ctx.add_context_source(), etc.
    async fn init(&self, ctx: &mut PluginContext) -> Result<()>;

    /// Run after AppCore is fully assembled. For tools that need AppCore itself.
    async fn post_init(&self, app: &AppCore) -> Result<()> { Ok(()) }
}
```

### 2. `PluginContext` — the active registration surface

```rust
pub struct PluginContext<'a> {
    pub deps: &'a PluginDeps,
    pub tools: &'a mut ToolRegistry,
    pub context_sources: &'a mut Vec<Box<dyn ContextSource>>,
    pub signal_consumers: &'a mut Vec<Arc<dyn SignalConsumer>>,
    pub event_translators: &'a mut Vec<Box<dyn EventTranslator>>,
    pub cron_handlers: &'a mut Vec<(String, Arc<dyn CronHandler>)>,
    pub background_spawns: &'a mut Vec<tokio::task::JoinHandle<()>>,
    pub host: &'a mut FeatureHost,
}

pub struct PluginDeps {
    pub config: Arc<RwLock<Config>>,
    pub hot_config: Arc<RwLock<HotConfig>>,
    pub storage_pool: StoragePool,
    pub repos: Repos,
    pub provider: DynProvider,
    pub cognitive_provider: Option<DynProvider>,
    pub vector_store: Option<VectorStore>,
    pub embedding_engine: Option<Arc<EmbeddingEngine>>,
    pub domain_event_bus: Option<Arc<DomainEventBus>>,
    pub bus: Arc<MessageBus>,
    pub cron_executor: Arc<CronExecutor>,
    pub shutdown_token: CancellationToken,
    pub mode: AppMode,
}
```

### 3. `FeatureHost` — type-map for plugin handles

```rust
pub struct FeatureHost {
    handles: DashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl FeatureHost {
    pub fn insert<T: Send + Sync + 'static>(&self, handle: Arc<T>) {
        self.handles.insert(TypeId::of::<T>(), handle);
    }
    
    pub fn get<T: Send + Sync + 'static>(&self) -> Option<Arc<T>> {
        self.handles.get(&TypeId::of::<T>())
            .and_then(|h| h.clone().downcast::<T>().ok())
    }
}
```

Plugins that need to expose state insert a typed handle during `init()`:
```rust
// Inside feature_productivity::ProductivityPlugin::init()
let repos = ProductivityRepos::new(ctx.deps.storage_pool.inner().clone());
ctx.host.insert(Arc::new(repos.clone()));
// ... use repos to build tools, context sources, etc.
```

Callers access it via type:
```rust
let repos = core.host().get::<ProductivityRepos>()
    .ok_or(ApiError::new("FEATURE_DISABLED", "productivity not available"))?;
```

### 4. `AppCore` shrinks incrementally

During migration, keep accessor methods as thin delegates to avoid breaking the command layer:

```rust
impl AppCore {
    pub fn productivity_repos(&self) -> Result<Arc<ProductivityRepos>, ApiError> {
        self.host.get::<ProductivityRepos>()
            .ok_or(ApiError::new("FEATURE_DISABLED", "productivity not available"))
    }
}
```

Once all callers migrate to `core.host().get::<T>()`, delete the accessors.

### 5. `FeatureHostBuilder` — declarative startup

```rust
let host = FeatureHostBuilder::new(core_deps)
    .plugin(feature_focus::FocusPlugin::default())
    .plugin(feature_language_learning::LanguageLearningPlugin::default())
    .plugin(feature_alarms::AlarmPlugin::default())
    .plugin(feature_notes::NotesPlugin::default())
    .plugin(feature_tasks::TaskPlugin::default())
    .plugin(feature_productivity::ProductivityPlugin::default())
    .plugin(feature_launcher::LauncherPlugin::default())
    .plugin(feature_coaching::CoachingPlugin::default())
    .plugin(cognitive::CognitivePlugin::default())
    .plugin(agent::AgentPlugin::default())
    .build()
    .await?;
```

The builder:
1. Collects all migrations, runs them via `StoragePool`
2. Calls `plugin.init(ctx)` in dependency order (topological sort based on `PluginDeps` usage — or explicit ordering)
3. Assembles `AppCore` from core fields + host
4. Calls `plugin.post_init(&app_core)` for each plugin

### 6. What happens to the four wiring sites

| Site | Before | After |
|------|--------|-------|
| A — migrations | Hardcoded list in `storage.rs` | `host.collect_migrations()` iterates plugins |
| B — agent builder | 2,000-line `AgentLoopBuilder::build()` | Split: `AgentPlugin` owns tool registration; subsystem builders (cognitive, context engine) become internal modules |
| C — AppCore init | 1,750-line `init_with_sender()` | ~200-line orchestrator: build deps → build host → assemble AppCore → post-init |
| D — AI pipeline | Hardcoded `build_feature_registry()`, `translate()`, `build_metric_registry()` | `host.register_ai_features(&mut reg)`; `host.translate_event(event)` iterates plugin translators |

## Migration Phases

| Phase | Feature | Complexity | What moves to plugin |
|-------|---------|-----------|---------------------|
| 1 | `feature-focus` | Trivial | Migrations only (no tools, no-op health check) |
| 2 | `feature-language-learning` | Low | One tool, simple migrations |
| 3 | `feature-alarms` | Low | One tool, no cross-cognitive deps |
| 4 | `feature-notes` | Medium | NoteRepo, embedding handler, NotesTool, post-init backfill |
| 5 | `feature-tasks` | Medium | TaskRepo, FireStore, progress handler, TaskTool, OKR tool, area/project tools |
| 6 | `feature-productivity` | Medium-High | ProductivityRepos, focus manager, engine, aggregator, nudge service, distraction interceptor |
| 7 | `feature-launcher` | Medium | Engine registry, calendar fetcher, LauncherTool, attention rebuild |
| 8 | `feature-coaching` | High | Signal accumulator, pattern detector, intervention router, feedback tracker, coaching service |
| 9 | `cognitive` | Very High | 15+ handlers, background consolidation, session memory, tree builders, insight forge, query pipeline |
| 10 | `agent` | Very High | 2,000-line builder split into `AgentPlugin` + internal subsystem builders |

Each phase is a standalone PR. The host supports both plugins and hardcoded init simultaneously during transition.

## What Tests Would Survive

Current state:
- `app-core/tests/` — 3 files (proptest, integration, stub shadowing)
- `ai_pipeline.rs` — unit tests for `translate()` and `build_feature_registry()`
- `agent_loop/builder.rs` — **zero tests**
- `app-core/src/init/mod.rs` — **zero tests**

With the host:
- Each plugin tests its own `init()` and `post_init()` with a mock `PluginContext`
- `FeatureHostBuilder` tests dependency resolution and ordering
- Integration tests build the full host with a subset of plugins (e.g., no voice, no cognitive)
- The 1,750-line and 2,000-line functions become declarative — test the builder, not the assembly

## Interface Design (see INTERFACE-DESIGN.md)

The interface is the test surface. A mock `PluginContext` records what each plugin registers:

```rust
#[tokio::test]
async fn notes_plugin_registers_notes_tool() {
    let mut ctx = MockPluginContext::new();
    let plugin = feature_notes::NotesPlugin::default();
    plugin.init(&mut ctx).await.unwrap();
    
    assert!(ctx.has_tool("notes"));
    assert!(ctx.has_context_source::<feature_notes::context::NotesContextSource>());
}
```

The `MockPluginContext` is an in-memory stand-in (local-substitutable). No I/O, no real storage.
