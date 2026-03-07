# App-Core Extraction Design

## Problem

`AppCore` lives in the `desktop` crate and is coupled to `tauri::AppHandle`. This forces `dev-api` to duplicate the entire state struct (`DevState`) and all handler logic. The two implementations inevitably diverge — desktop showed stale/hardcoded data while dev-api had real implementations.

## Goal

Extract a framework-agnostic `app-core` crate that owns the application state and handler logic. Both `desktop` (Tauri) and `dev-api` (Axum) become thin wrappers.

## Architecture

```
Before:
  desktop (Tauri-coupled AppCore + 116 commands)
  dev-api (duplicate DevState + duplicate handlers)

After:
  app-core (framework-agnostic AppCore + cognitive/coaching handlers)
  desktop  (thin Tauri wrappers + TauriEmitter)
  dev-api  (thin Axum wrappers + SseEmitter)
```

### Layer placement

`app-core` sits at L7 alongside `desktop-shared`, `desktop`, and `dev-api`. It depends on:
- `common`, `config`, `bus`, `storage` (L0-L2)
- `cognitive`, `feature-coaching` (L4/L8)
- `desktop-shared` (L7 — DTOs only)
- `providers`, `agent` (L3/L5 — for initialization)

## Components

### 1. AppEventEmitter trait

Decouples event emission from Tauri:

```rust
pub trait AppEventEmitter: Send + Sync + 'static {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value);
}
```

Implementations:
- `TauriEmitter` — wraps `tauri::AppHandle`, calls `handle.emit(name, payload)`
- `SseEmitter` — wraps `Arc<Mutex<Vec<UnboundedSender<SseEvent>>>>`, broadcasts to SSE clients
- `NoopEmitter` — for tests

### 2. AppCore struct (framework-agnostic)

```rust
pub struct AppCore {
    // Public — needed by non-cognitive commands in desktop
    pub repos: Repos,
    pub agent: Arc<AgentLoop>,
    pub bus: Arc<MessageBus>,
    pub persona_manager: Arc<RwLock<PersonaManager>>,
    pub config: RwLock<config::Config>,
    pub note_repo: NoteRepo,
    pub active_streams: Arc<DashMap<String, CancellationToken>>,
    pub pending_interactions: Arc<DashMap<String, (String, oneshot::Sender<FormResponse>)>>,

    // Cognitive/coaching — accessed via methods
    domain_event_bus: Option<Arc<DomainEventBus>>,
    signal_accumulator: Option<Arc<Mutex<SignalAccumulator>>>,
    pattern_detector: Option<Arc<Mutex<PatternDetector>>>,
    intervention_router: Option<Arc<Mutex<InterventionRouter>>>,
    feedback_tracker: Option<Arc<Mutex<FeedbackTracker>>>,
    user_situation: Option<Arc<Mutex<UserSituation>>>,
    coaching_service: Option<Arc<Mutex<CoachingService>>>,
    has_cognitive_provider: bool,

    // Productivity
    productivity_repos: Option<ProductivityRepos>,
    focus_manager: Option<Arc<FocusManager>>,
    productivity_engine: Option<Arc<Mutex<ProductivityEngine>>>,
    aggregator: Option<Arc<DailyAggregator>>,
    nudge_service: Option<Arc<Mutex<NudgeService>>>,
    distraction_interceptor: Option<Arc<Mutex<DistractionInterceptor>>>,

    // Internal
    channel_manager: Arc<Mutex<ChannelManager>>,
    cron_service: Arc<CronService>,
    shutdown_token: CancellationToken,
}
```

### 3. Initialization

`AppCore::init()` takes a config struct instead of `tauri::AppHandle`:

```rust
pub struct AppCoreConfig {
    pub data_dir: PathBuf,
    pub emitter: Box<dyn AppEventEmitter>,
}

impl AppCore {
    pub async fn init(opts: AppCoreConfig) -> Result<(Self, EventChannels), String> {
        // Shared init logic (config, storage, bus, provider, agent, coaching)
        // Returns AppCore + EventChannels for the caller to wire up
    }
}

/// Channels the caller wires to their event system (Tauri emit, SSE, etc.)
pub struct EventChannels {
    pub inbound_rx: mpsc::Receiver<InboundMessage>,
    pub intervention_rx: mpsc::Receiver<DeliveredIntervention>,
    pub domain_event_bus: Arc<DomainEventBus>,
    pub pipeline_rx: mpsc::UnboundedReceiver<cognitive::PipelineEvent>,
}
```

The caller (desktop or dev-api) receives `EventChannels` and wires them to their own event transport.

### 4. Handler methods

22 cognitive/coaching handlers become methods on `AppCore`:

```rust
// crates/app-core/src/cognitive.rs
impl AppCore {
    pub async fn cognitive_user_model(&self) -> Result<UserModelSummaryResponse, ApiError> { ... }
    pub async fn cognitive_facts_list(&self) -> Result<Vec<SemanticFactResponse>, ApiError> { ... }
    pub async fn cognitive_system_status(&self) -> Result<SystemStatusResponse, ApiError> { ... }
    pub async fn cognitive_inject_event(&self, event_type: String, payload: serde_json::Value) -> Result<bool, ApiError> { ... }
    // ... etc
}

// crates/app-core/src/coaching.rs
impl AppCore {
    pub async fn coaching_situation(&self) -> Result<UserSituationResponse, ApiError> { ... }
    pub async fn coaching_signals(&self) -> Result<SignalWindowResponse, ApiError> { ... }
    // ... etc
}
```

### 5. Consumer wrappers

**Desktop (Tauri):**
```rust
#[tauri::command]
pub async fn cognitive_system_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<SystemStatusResponse, ApiError> {
    state.cognitive_system_status().await
}
```

**Dev-API (Axum):**
```rust
"cognitive_system_status" => ok(core.cognitive_system_status().await?),
```

## Scope

Phase 1 (this plan): Extract only the 22 cognitive/coaching handlers and the state fields they need. The other 94 commands (tasks, notes, chat, etc.) remain in `desktop` and can migrate later.

Phase 2 (future): Migrate remaining commands to `app-core` methods, fully remove `DevState`, and potentially remove `dev-api` entirely.

## File Changes

### New files
- `crates/app-core/Cargo.toml`
- `crates/app-core/src/lib.rs`
- `crates/app-core/src/state.rs` — AppCore struct + accessors
- `crates/app-core/src/init.rs` — shared initialization
- `crates/app-core/src/cognitive.rs` — cognitive handler methods
- `crates/app-core/src/coaching.rs` — coaching handler methods
- `crates/app-core/src/events.rs` — AppEventEmitter trait

### Modified files
- `Cargo.toml` (workspace) — add `app-core` member
- `crates/desktop/Cargo.toml` — add `app-core` dependency
- `crates/desktop/src/app_core.rs` — thin wrapper around `app_core::AppCore`, Tauri event wiring
- `crates/desktop/src/commands/cognitive.rs` — 21 commands become one-line delegations
- `crates/dev-api/Cargo.toml` — add `app-core` dependency, remove duplicated deps
- `crates/dev-api/src/main.rs` — drop `DevState`, use `app_core::AppCore`, slim handlers

### Deleted code
- `DevState` struct in dev-api (~25 fields)
- ~400 lines of duplicate handler logic in dev-api
- ~300 lines of handler logic in desktop/commands/cognitive.rs (replaced by delegations)

## Testing

- All existing 415 tests continue to pass (no logic changes, only code movement)
- `cargo build -p app-core -p desktop -p dev-api` must compile
- `cargo clippy --workspace` must pass
- Manual verification: `cargo tauri dev` + `cargo run -p dev-api` both show identical debug dashboard data
