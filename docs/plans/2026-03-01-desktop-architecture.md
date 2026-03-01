# Desktop Application Architecture

**Date:** 2026-03-01
**Status:** Approved
**Companion:** [Desktop App Design](./2026-03-01-desktop-app-design.md) (UI/UX spec)

## Decisions

| Decision | Choice |
|----------|--------|
| Channels | Desktop primary, channels feature-flagged optional |
| IPC layer | Thin Tauri commands delegating to AppCore |
| Frontend state | Event-driven Leptos signals |
| Error handling | Typed `Result<T, AppError>` across IPC |
| Shared types | `desktop-shared` crate (IPC contract) |
| Background services | Tokio tasks in AppCore with CancellationToken |
| Frontend routing | Signal-based (no URL router) |
| Architecture style | Flat module structure per crate |

## Crate Structure

```
crates/
  desktop-shared/          # Layer 0: IPC contract (types only, no logic)
    Cargo.toml             # deps: serde, chrono, uuid
    src/
      lib.rs
      error.rs             # AppError enum (Serialize + Deserialize)
      commands/
        mod.rs
        chat.rs            # ChatSendArgs, ChatResponse
        tasks.rs           # TaskListArgs, TaskCreateArgs, TaskResponse
        okr.rs             # OkrListArgs, ObjectiveResponse, KeyResultResponse
        areas.rs           # AreaListArgs, AreaResponse
        projects.rs        # ProjectListArgs, ProjectResponse
        sessions.rs        # SessionListArgs, SessionResponse
        settings.rs        # SettingsGetArgs, SettingsUpdateArgs
      events.rs            # AgentEventPayload, EntityUpdated, FocusChanged
      types.rs             # Shared display types (Priority, Status, ViewMode)

  desktop/                 # Layer 7: Tauri backend
    Cargo.toml             # deps: tauri v2, desktop-shared, agent, storage, config, common, tokio
    tauri.conf.json
    src/
      main.rs              # Tauri builder: setup, managed state, handlers, tray
      app_core.rs          # AppCore: owns AgentLoop + Repos + Config + background handles
      setup.rs             # Initialization: config, storage, agent, services
      commands/
        mod.rs             # register all handlers
        chat.rs            # chat_send, chat_cancel, chat_history
        tasks.rs           # task_list, task_create, task_update, task_delete, task_complete
        okr.rs             # objective_list, kr_update, kr_progress
        areas.rs           # area_list, area_create
        projects.rs        # project_list, project_create
        sessions.rs        # session_list, session_get
        settings.rs        # settings_get, settings_update
      events.rs            # emit_agent_event(), emit_entity_updated()
      streaming.rs         # Bridges AgentEvent stream to Tauri events

  desktop-ui/              # Layer 7: Leptos WASM frontend
    Cargo.toml             # deps: leptos 0.7, desktop-shared, serde, wasm-bindgen
    Trunk.toml
    index.html
    styles/input.css       # Tailwind v4 with @theme tokens
    src/
      lib.rs               # Entry: mount based on window label
      app.rs               # MainApp root component + signal providers
      router.rs            # Signal-based section routing
      ipc.rs               # Typed invoke/listen wrappers (sole WASM-JS boundary)
      signals/
        mod.rs
        chat.rs            # ChatSignals: messages, streaming_content, tool_activity
        tasks.rs           # TaskSignals: task_list, selected_task, view_mode
        okr.rs             # OkrSignals: objectives, key_results
        navigation.rs      # NavigationSignals: section, area, project, breadcrumb
        focus.rs           # FocusSignals: focus_task, timer
      views/
        mod.rs
        chat.rs            # Full chat view (thread list + conversation)
        tasks.rs           # Task list/board/tree views
        task_detail.rs     # Slide-over detail panel
        okr.rs             # OKR dashboard
        projects.rs        # Project cards grid
        settings.rs        # Settings page
        launcher.rs        # Launcher window content
        tray.rs            # System tray popup content
      components/
        mod.rs
        chat_input.rs
        chat_message.rs
        task_card.rs
        task_row.rs
        progress_bar.rs
        breadcrumb.rs
        priority_badge.rs
        status_toggle.rs
        entity_card.rs
        tool_indicator.rs
        toast.rs
```

### Dependency Graph

```
desktop-shared <-- desktop      (backend reads/writes contract types)
desktop-shared <-- desktop-ui   (frontend reads/writes contract types)
agent, storage, config, common <-- desktop  (backend integrates with core)
desktop-ui has NO dependency on agent/storage/config
```

`desktop-shared` must avoid system-specific deps (no tokio, no sqlx). Only `serde`, `chrono`, `uuid`.

## AppCore

Central struct owned by Tauri managed state. All three windows share one instance.

```rust
pub struct AppCore {
    agent_loop: AgentLoop,
    repos: Repos,
    config: Config,
    cancel_token: CancellationToken,
    active_streams: DashMap<String, CancellationToken>,
}
```

### Public API

```rust
impl AppCore {
    // Lifecycle
    pub async fn initialize(config: Config) -> Result<Self, AppError>;
    pub async fn shutdown(&self);

    // Chat (streaming via Tauri events)
    pub async fn chat_send(&self, content: String, session_key: String, app: AppHandle) -> Result<(), AppError>;
    pub fn chat_cancel(&self, session_key: &str) -> Result<(), AppError>;

    // CRUD (thin wrappers around Repos)
    pub async fn task_list(&self, area_id: Option<Uuid>, project_id: Option<Uuid>) -> Result<Vec<TaskResponse>, AppError>;
    pub async fn task_create(&self, args: TaskCreateArgs) -> Result<TaskResponse, AppError>;
    pub async fn task_update(&self, id: Uuid, args: TaskUpdateArgs) -> Result<TaskResponse, AppError>;
    pub async fn task_complete(&self, id: Uuid) -> Result<TaskResponse, AppError>;

    // OKR
    pub async fn objective_list(&self, project_id: Uuid) -> Result<Vec<ObjectiveResponse>, AppError>;
    pub async fn kr_update_progress(&self, id: Uuid, value: f64) -> Result<KeyResultResponse, AppError>;

    // Areas & Projects
    pub async fn area_list(&self) -> Result<Vec<AreaResponse>, AppError>;
    pub async fn project_list(&self, area_id: Option<Uuid>) -> Result<Vec<ProjectResponse>, AppError>;

    // Settings
    pub async fn settings_get(&self) -> Result<SettingsResponse, AppError>;
    pub async fn settings_update(&self, args: SettingsUpdateArgs) -> Result<(), AppError>;
}
```

### Command Pattern

Commands are one-liners that delegate to AppCore:

```rust
#[tauri::command]
async fn task_list(
    state: tauri::State<'_, AppCore>,
    args: TaskListArgs,
) -> Result<Vec<TaskResponse>, AppError> {
    state.task_list(args.area_id, args.project_id).await
}
```

AppCore does NOT hold an `AppHandle`. Commands pass it when event emission is needed. This keeps AppCore testable without a Tauri runtime.

## Event System

### Event Taxonomy

| Event | Payload | Purpose |
|-------|---------|---------|
| `agent:content_chunk` | `{ session_key, data }` | Token-by-token streaming |
| `agent:tool_start` | `{ session_key, name, args }` | Tool activity indicator |
| `agent:tool_end` | `{ session_key, name, success, duration_ms }` | Complete indicator |
| `agent:done` | `{ session_key, content }` | Finalize response |
| `agent:error` | `{ session_key, error }` | Error in chat |
| `entity:created` | `{ entity_type, id, summary }` | Refresh lists |
| `entity:updated` | `{ entity_type, id }` | Invalidate + re-fetch |
| `entity:deleted` | `{ entity_type, id }` | Remove from signals |
| `focus:changed` | `{ task_id, task }` | Update tray + focus UI |

### Streaming Bridge

```rust
pub fn spawn_event_relay(
    app: AppHandle,
    session_key: String,
    mut event_rx: mpsc::Receiver<AgentEvent>,
    cancel_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => break,
                event = event_rx.recv() => {
                    match event {
                        Some(agent_event) => {
                            let payload = AgentEventPayload::from(agent_event, &session_key);
                            let _ = app.emit(&payload.event_name(), &payload);
                        }
                        None => break,
                    }
                }
            }
        }
    })
}
```

### Frontend Signal Pattern

Entity events trigger re-fetch (not patch). Backend is single source of truth. IPC cost of re-fetching from SQLite is negligible (<1ms).

```rust
listen("entity:updated", move |_| {
    spawn_local(async move {
        if let Ok(updated) = ipc::call::<_, Vec<TaskResponse>>("task_list", &()).await {
            tasks.set(updated);
        }
    });
});
```

Optimistic updates for user-initiated actions: mutate signal immediately, fire IPC in background, revert on error.

## Error Handling

### AppError

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AppError {
    NotFound { entity: String, id: String },
    Validation { field: String, message: String },
    AgentBusy { session_key: String },
    ProviderUnavailable { provider: String, reason: String },
    StorageError { message: String },
    ConfigError { message: String },
    Internal { message: String },
}
```

Adjacently tagged JSON for ergonomic frontend matching. Maps from `KlyntbotError` via `From` impl.

### Error-to-UI Mapping

| Variant | UI Treatment |
|---------|-------------|
| `NotFound` | Empty state or inline message |
| `Validation` | Inline field error |
| `AgentBusy` | Thinking indicator, disable send |
| `ProviderUnavailable` | Toast with retry |
| `StorageError` | Toast |
| `ConfigError` | Toast + settings link |
| `Internal` | Generic toast |

## IPC Wrapper

Single module (`ipc.rs`) isolates all WASM-JS interop:

```rust
pub async fn call<A: Serialize, R: DeserializeOwned>(
    cmd: &str,
    args: &A,
) -> Result<R, AppError>;

pub fn on<P: DeserializeOwned + 'static>(
    event: &str,
    handler: impl FnMut(P) + 'static,
);
```

Every other frontend file uses pure Rust types.

## Initialization & Lifecycle

### Startup

```
Launch
  -> load config -> connect SQLite -> run migrations
  -> build AgentLoop -> start background services (tokio tasks)
  -> manage(AppCore) -> create windows (hidden)
  -> register global shortcuts -> show tray icon
```

### Window Lifecycle

- **Launcher**: created once, shown/hidden via `Option+Space`
- **Main app**: `prevent_close()` on close request, hides to tray
- **Tray popup**: toggle visibility on tray icon click

### Shutdown

```
Quit (tray menu or Cmd+Q)
  -> cancel active chat streams
  -> cancel background services (CancellationToken)
  -> drop pool (connections close)
  -> exit(0)
```

## Frontend Architecture

### Multi-Window

Single WASM bundle, three entry points via `?window=` query param:

```rust
match window_label.as_str() {
    "launcher" => view! { <LauncherApp /> },
    "tray"     => view! { <TrayApp /> },
    _          => view! { <MainApp /> },
}
```

### Signal Groups

| Group | Used By | Contents |
|-------|---------|----------|
| `ChatSignals` | Main, Launcher | messages, streaming_content, tool_activity, is_streaming |
| `TaskSignals` | Main | tasks, selected, view_mode, filters |
| `OkrSignals` | Main | objectives, key_results, expanded_objective |
| `NavigationSignals` | Main | section, current_area, current_project, breadcrumb |
| `FocusSignals` | Main, Tray | focus_task, timer_elapsed |

Provided via `provide_context` at root. Each window gets independent signal instances. Data consistency from shared backend, not shared frontend state.

### Signal-Based Routing

No URL router. Navigation via signals:

```rust
move || match nav.section.get() {
    Section::Chat     => view! { <ChatView /> }.into_any(),
    Section::Tasks    => view! { <TasksView /> }.into_any(),
    Section::Okr      => view! { <OkrView /> }.into_any(),
    Section::Calendar => view! { <CalendarView /> }.into_any(),
    Section::Settings => view! { <SettingsView /> }.into_any(),
}
```

## Build System

### Pipeline

```
desktop-shared  ->  desktop-ui (Trunk -> WASM + Tailwind)
                ->  desktop (Tauri CLI, embeds WASM dist)
                ->  .app bundle
```

### Commands

```bash
# Dev (two terminals)
cd crates/desktop-ui && trunk serve --watch
cd crates/desktop && cargo tauri dev

# Production
cd crates/desktop-ui && trunk build --release
cd crates/desktop && cargo tauri build
```

### Constraints

- `desktop-ui` excluded from `default-members` (targets `wasm32-unknown-unknown`)
- Built exclusively via Trunk, not `cargo build --workspace`
- Tailwind v4 uses `@theme` in CSS for design tokens (no JS config)

### Tauri Windows

| Window | Size | Decorations | Transparent | Behavior |
|--------|------|-------------|-------------|----------|
| main | 1200x800 | yes | no | Hide on close |
| launcher | 700x500 | no | yes | Global shortcut toggle |
| tray | 320x400 | no | yes | Tray icon toggle |

### Tailwind Theme

```css
@theme {
  --color-base:       #0D1117;
  --color-surface:    #161B22;
  --color-elevated:   #1C2128;
  --color-overlay:    #21262D;
  --color-orange-500: #F97316;
  --color-orange-400: #FB923C;
  --color-orange-600: #EA580C;
  --color-primary:    #E6EDF3;
  --color-secondary:  #8B949E;
  --color-muted:      #484F58;
  --color-border:     #21262D;
  --color-border-emphasis: #30363D;
  --color-success:    #22C55E;
  --color-warning:    #EAB308;
  --color-error:      #EF4444;
}
```
