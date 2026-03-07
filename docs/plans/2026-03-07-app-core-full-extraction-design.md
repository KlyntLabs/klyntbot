# App-Core Full Extraction Design

## Problem

Business logic is duplicated across three places:

1. **`crates/dev-api/src/main.rs`** — 3,600-line monolith with its own `DevState`, own initialization, ~75 command handlers inlined in a dispatch match.
2. **`crates/desktop/src/dev_server.rs`** — Debug-only Axum server inside Tauri that re-implements every handler as dispatch match arms (~800 lines).
3. **`crates/desktop/src/commands/*.rs`** — The "real" Tauri command handlers where business logic actually lives.

The standalone dev-api has its own state initialization, diverges from desktop on feature coverage, and is a maintenance burden. The desktop dev_server partially reuses command helpers but still duplicates orchestration logic.

## Goal

Extract a framework-agnostic `app-core` crate that owns all application state and handler logic. Delete the standalone `dev-api` crate entirely. Both `desktop` (Tauri commands) and `desktop/dev_server.rs` (debug HTTP) become thin adapters over `app-core`.

## Architecture

```
Before:
  dev-api   (DevState + 3600-line monolith, own init, own SSE)  <- DELETED
  desktop   (AppCore + 116 Tauri commands + dev_server dispatch copy)

After:
  app-core  (AppCore + ~116 handler methods + AppEventEmitter trait)
  desktop   (thin Tauri wrappers + TauriEmitter + simplified dev_server)
```

### Layer placement

`app-core` sits at L7 alongside `desktop-shared` and `desktop`:

```
L7: desktop-shared (DTOs only)
    app-core       (AppCore struct + all handler methods + AppEventEmitter trait)
    desktop        (Tauri wrappers + dev_server adapter)
```

Dependencies: `common`, `config`, `bus`, `storage`, `providers`, `agent`, `session`, `scheduling`, `cognitive`, `feature-coaching`, `feature-productivity`, `feature-notes`, `desktop-shared`.

## Components

### 1. AppEventEmitter trait

Decouples event emission from any specific transport:

```rust
// crates/app-core/src/events.rs
pub trait AppEventEmitter: Send + Sync + 'static {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value);
}
```

Implementations:
- `TauriEmitter` (in desktop crate) — wraps `tauri::AppHandle`, calls `handle.emit()`
- `SseEmitter` (in desktop dev_server) — broadcasts to SSE subscribers
- `NoopEmitter` (in app-core, for tests)

### 2. AppCore struct (framework-agnostic)

Moves from `desktop/src/app_core.rs` into `app-core` crate. Holds all subsystem state:

```rust
pub struct AppCore {
    // Public
    pub repos: Repos,
    pub agent: Arc<AgentLoop>,
    pub bus: Arc<MessageBus>,
    pub persona_manager: Arc<RwLock<PersonaManager>>,
    pub config: RwLock<config::Config>,
    pub note_repo: NoteRepo,
    pub active_streams: Arc<DashMap<String, CancellationToken>>,
    pub pending_interactions: Arc<DashMap<String, (String, oneshot::Sender<FormResponse>)>>,
    pub has_cognitive_provider: bool,

    // Feature-gated (accessed via Result-returning methods)
    domain_event_bus: Option<Arc<DomainEventBus>>,
    signal_accumulator: Option<Arc<Mutex<SignalAccumulator>>>,
    pattern_detector: Option<Arc<Mutex<PatternDetector>>>,
    intervention_router: Option<Arc<Mutex<InterventionRouter>>>,
    feedback_tracker: Option<Arc<Mutex<FeedbackTracker>>>,
    user_situation: Option<Arc<Mutex<UserSituation>>>,
    coaching_service: Option<Arc<Mutex<CoachingService>>>,
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
    pub emitter: Arc<dyn AppEventEmitter>,
}

pub struct EventChannels {
    pub inbound_rx: mpsc::Receiver<InboundMessage>,
    pub intervention_rx: mpsc::Receiver<DeliveredIntervention>,
    pub domain_event_rx: broadcast::Receiver<DomainEvent>,
    pub pipeline_rx: mpsc::UnboundedReceiver<PipelineEvent>,
    pub auto_focus_rx: Option<mpsc::Receiver<AutoFocusSession>>,
    pub nudge_rx: Option<mpsc::Receiver<NudgeRecord>>,
    pub dashboard_tick_rx: Option<broadcast::Receiver<DashboardTick>>,
}

impl AppCore {
    pub async fn init(opts: AppCoreConfig) -> Result<(Self, EventChannels), String> {
        // All shared init logic: config, storage, bus, provider, agent, coaching, productivity
        // Returns AppCore + EventChannels for the caller to wire up
    }
}
```

The caller (desktop `main.rs`) receives `EventChannels` and wires each to their transport (Tauri events or SSE).

### 4. Handler methods

Every command becomes a method on `AppCore`. Organized by domain in separate files:

```
crates/app-core/src/
  lib.rs              — re-exports
  state.rs            — AppCore struct + accessors
  init.rs             — AppCore::init() + cron registration
  events.rs           — AppEventEmitter trait + NoopEmitter
  handlers/
    mod.rs
    tasks.rs          — task_list, task_create, task_update, task_delete, ...
    projects.rs       — project_list, project_create, ...
    areas.rs          — area_list, area_create, ...
    objectives.rs     — objective_list, objective_create, ...
    key_results.rs    — key_result_create, key_result_update_metric, ...
    chat.rs           — chat_send, chat_threads, chat_messages, ...
    notes.rs          — note_list, note_create, note_update, ...
    finance.rs        — finance_accounts, finance_net_worth, ...
    productivity.rs   — productivity_today, productivity_focus_start, ...
    distraction.rs    — distraction_dismiss, distraction_allow_temp, ...
    cognitive.rs      — cognitive_user_model, cognitive_facts_list, ...
    coaching.rs       — coaching_situation, coaching_signals, ...
    settings.rs       — mcp_get_config, mcp_add_server, ...
    status.rs         — agent_status
```

#### Entity update pattern

Mutating handlers return entity updates alongside the result:

```rust
pub struct EntityUpdate {
    pub kind: EntityKind,
    pub id: String,
}

pub type HandlerResult<T> = Result<(T, Vec<EntityUpdate>), ApiError>;

impl AppCore {
    pub async fn task_create(&self, params: TaskCreateParams) -> HandlerResult<TaskResponse> {
        // ... business logic ...
        Ok((response, vec![EntityUpdate { kind: EntityKind::Task, id }]))
    }

    // Read-only handlers return plain Result
    pub async fn task_list(&self, area_id: Option<String>, ...) -> Result<Vec<TaskResponse>, ApiError> { ... }
}
```

The adapter layer emits entity:updated events from the returned `Vec<EntityUpdate>`.

#### Row-to-response converters

Functions like `action_to_task()`, `objective_to_response()`, `fact_to_response()` move into `app-core/src/handlers/` as private helpers.

### 5. Chat streaming

The most complex handler. `AppCore` sets up the stream and returns a handle:

```rust
pub struct ChatSendHandle {
    pub user_message: ChatMessageResponse,
    pub event_rx: mpsc::UnboundedReceiver<AgentEvent>,
    pub interaction_rx: mpsc::UnboundedReceiver<InteractionRequest>,
    pub cancel_token: CancellationToken,
}

impl AppCore {
    pub async fn chat_send(
        &self,
        session_key: String,
        content: String,
        context: Option<SessionContextInput>,
    ) -> Result<ChatSendHandle, ApiError> { ... }
}
```

TransparencyData accumulation + DB persistence lives in a shared function:

```rust
pub async fn relay_chat_stream(
    core: &AppCore,
    handle: ChatSendHandle,
    emitter: &dyn AppEventEmitter,
) { ... }
```

Both Tauri and dev_server call `relay_chat_stream()` with their emitter.

### 6. Consumer wrappers

**Tauri commands** (2-3 lines each):
```rust
#[tauri::command]
pub async fn task_create(
    state: State<'_, Arc<AppCore>>,
    app: AppHandle,
    params: TaskCreateParams,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_create(params).await?;
    emit_updates(&app, &updates);
    Ok(result)
}
```

**Dev server dispatch** (1 line each):
```rust
"task_list" => ok(core.task_list(get(&body, "area_id"), ...).await?),
```

## Deleted code

| Item | Approx lines |
|------|-------------|
| `crates/dev-api/` entire crate | ~3,600 |
| `DevState` struct + duplicate init | ~270 |
| Duplicate business logic in `dev_server.rs` dispatch | ~800 |
| Business logic in `desktop/src/commands/*.rs` (replaced by delegation) | ~2,000 |
| **Total removed** | **~6,670** |

## What stays in `desktop` crate

- `main.rs` — Tauri setup, windows, tray, shortcuts, event channel wiring
- `commands/*.rs` — thin wrappers (2-3 lines each)
- `dev_server.rs` — simplified dispatch delegating to `AppCore` methods
- `oauth/` — OAuth flow (browser launch, callback server)

## New crate structure

```
crates/app-core/
  Cargo.toml
  src/
    lib.rs
    state.rs
    init.rs
    events.rs
    handlers/
      mod.rs
      tasks.rs
      projects.rs
      areas.rs
      objectives.rs
      key_results.rs
      chat.rs
      notes.rs
      finance.rs
      productivity.rs
      distraction.rs
      cognitive.rs
      coaching.rs
      settings.rs
      status.rs
```

## Testing

- All existing tests continue to pass (logic moves, not changes)
- `app-core` handlers are testable without Tauri via `NoopEmitter` + `StoragePool::connect_in_memory()`
- `cargo build -p app-core -p desktop` must compile
- `cargo clippy --workspace` must pass with zero warnings
- Manual: `cargo tauri dev` → open localhost:1420 in Chrome → all pages work
