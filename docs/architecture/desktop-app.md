# Desktop Application Architecture

The desktop layer transforms Klyntbot's Rust agent engine into a native macOS application. Four components collaborate: a transport-agnostic business logic crate, a shared IPC type crate, a thin Tauri 2 adapter, and a React 19 frontend.

```
  desktop-ui/            crates/desktop/          crates/app-core/
  (React 19 + Vite)      (Tauri 2 adapter)        (business logic)
  +--------------+       +------------------+     +------------------+
  | useQuery()   | <---> | #[tauri::command] | --> | AppCore::         |
  | useMutation()| Tauri | emit_updates()    |     |   task_create()  |
  | useEvent()   | IPC   | focus_timer       |     |   chat_send()    |
  |              |       | tray_countdown    |     |   ...140+ fns    |
  +--------------+       +------------------+     +------------------+
        |                       |                        |
        | HTTP (dev)     crates/desktop-shared/          | uses
        +-------------> +------------------+    +--------+---------+
                        | Event constants  |    | storage, agent,  |
                        | Request/Response |    | cognitive, bus,  |
                        | ApiError, enums  |    | features, cron   |
                        +------------------+    +------------------+
```

## AppCore -- Transport-Agnostic Business Logic

**Crate:** `crates/app-core/`
**Key file:** `src/state.rs`

`AppCore` is a single struct with ~100 public fields holding the full application state. It has no Tauri or Axum dependencies -- both the desktop app and the dev HTTP server wrap it identically. This separation means business logic is testable without a windowing system.

### Key fields

| Field | Type | Purpose |
|-------|------|---------|
| `agent` | `Arc<AgentLoop>` | LLM agent runtime (skill routing, ReAct execution, tool calls) |
| `bus` | `Arc<MessageBus>` | Cross-feature message bus |
| `domain_event_bus` | `Option<Arc<DomainEventBus>>` | Cognitive domain events (broadcast, ~25 subscribers) |
| `repos` | `Repos` | SQLite repository access (tasks, sessions, config, etc.) |
| `storage_pool` | `StoragePool` | Underlying `SqlitePool` (Clone+Send+Sync) |
| `config` | `Arc<RwLock<Config>>` | Full config (structural changes require restart) |
| `hot_config` | `Arc<RwLock<HotConfig>>` | Hot-reloadable subset (model, temperature, budget) |
| `persona_manager` | `Arc<RwLock<PersonaManager>>` | Agent persona switching |
| `cron_service` | `Arc<CronService>` | Scheduled job execution |
| `channel_manager` | `Arc<Mutex<ChannelManager>>` | Platform integrations (Telegram, Discord, Slack, Email) |
| `active_streams` | `Arc<DashMap<String, CancellationToken>>` | Active chat streaming sessions |
| `pending_interactions` | `Arc<DashMap<String, (String, oneshot::Sender)>>` | Pending `ask_user` tool interactions |
| `mirror_facade` | `Option<Arc<MirrorFacade>>` | Self-reflection subsystem |
| `voice_service` | `Option<Arc<VoiceService>>` | Voice capture and synthesis |
| `event_emitter` | `Arc<dyn AppEventEmitter>` | Transport-agnostic event emission |

Optional fields (`Option<Arc<...>>`) represent features that require specific configuration or providers. Accessor methods return `Result<&T, ApiError>` with code `FEATURE_DISABLED` when unavailable.

### Initialization pipeline

`AppCore::init()` in `src/init/mod.rs` runs a multi-phase startup:

1. **Storage** -- Load config, connect SQLite pool, run migrations, create repos, initialize vector store (LanceDB), create LLM provider
2. **Hot config** -- Extract hot-reloadable subset, wire provider-degraded callback
3. **Shared infrastructure** -- Create `MessageBus` (100 slots), `DomainEventBus` (256 slots), `ContextUpdateQueue`
4. **Cron** -- Initialize scheduled jobs, notification dispatcher, proactive/suggestion/decomposition handlers, autotuner
5. **Embeddings** -- Create embedding engine (OpenAI or local), note embedding handler
6. **Agent** -- Build `AgentLoop` with tool registry, persona manager, skill catalog
7. **Features** -- Initialize coaching, productivity, launcher, cognitive, insights (each gated by config)
8. **Background services** -- Start deadline scheduler, mirror engine, config file watcher, lifecycle monitor

Returns `(AppCore, EventChannels)`. `EventChannels` bundles receiver ends of mpsc/broadcast channels that the transport layer (Tauri or dev server) wires to its event system.

### Handler pattern

All mutating handlers return `HandlerResult<T>`:

```rust
pub type HandlerResult<T> = Result<(T, Vec<EntityUpdate>), ApiError>;
```

The data value (`T`) is the response payload. The `Vec<EntityUpdate>` lists entities that changed, which the transport layer broadcasts as `entity:updated` events for frontend cache invalidation. This keeps business logic unaware of the event transport.

Handler modules live in `src/handlers/` (~46 modules) spanning subdomains: tasks, chat, notes, cognitive, productivity, coaching, finance, voice, workspace, and more.

### Adapters

`src/adapters/` contains bridge types that connect features without introducing direct dependencies:

- `cognitive_accessor` -- Accesses cognitive memory from non-cognitive features
- `flashcard_accessor` -- Bridges flashcard queries for the learning feature
- `trial_evaluator` -- Evaluates autotuner trial outcomes
- `cross_domain_searcher` -- Unified search across tasks, notes, finance
- `scope_resolver` -- Resolves entity scopes for permission checks
- `insight_embedder` -- Embeds insight content for semantic search
- `autotuner_bridge` -- Connects autotuner events to mirror engine

## Desktop-Shared -- IPC Types

**Crate:** `crates/desktop-shared/`

Defines the contract between backend and frontend with no runtime dependencies on either side.

### Event constants

59 typed event constants in `src/events.rs`, each with a corresponding payload struct. Categories:

| Prefix | Count | Examples |
|--------|-------|---------|
| `agent:*` | 29 | `content_chunk`, `tool_start`, `tool_end`, `done`, `error`, `memory_promoted` |
| `autotuner:*` | 3 | `report`, `promotion`, `rollback` |
| `entity:*` | 1 | `updated` (generic entity invalidation) |
| `chat:*` | 3 | `thread_created`, `thread_updated`, `message_added` |
| `mcp:*` | 4 | `oauth_complete`, `server_status`, `startup_complete` |
| `focus:*` | 7 | `state_changed`, `sync`, `phase_changed`, `warning`, `dnd_unavailable` |
| `productivity:*` | 2 | `distraction`, `nudge` |
| `activity:*` | 2 | `tick`, `switch` |
| `coaching:*` | 1 | `intervention` |
| `distraction:*` | 2 | `intervention`, `verdict` |
| Other | 5 | `score:updated`, `bucket:completed`, `insight:generated` |

### Command types

33+ modules in `src/commands/` define typed request/response structs for IPC. All use `#[serde(rename_all = "camelCase")]`. Examples: `tasks.rs` (TaskCreateParams, TaskResponse), `chat.rs` (ChatSendParams), `cognitive_graph.rs` (GraphQueryParams).

### Shared enums and types

- `EntityKind` -- 15 variants (Task, Project, Note, Finance, etc.) with `parse(s: &str)` for loose matching
- `MessageSegment` -- Tagged enum (Text | Tool) for structured assistant messages
- `ApiError` -- `{ code: String, message: String }` with HTTP status mapping in the dev server
- `TransparencyData` -- Accumulated pipeline metrics per assistant message (usage, cost, timing, tools, memory, skills)
- View/filter enums: `Priority`, `Status`, `AreaFilter`, `ViewMode`, `SidebarItem`

## Desktop Crate -- Tauri 2 Adapter

**Crate:** `crates/desktop/`
**Entry point:** `src/main.rs`

### Dual-mode binary

The binary supports two modes via CLI subcommands:

- **Desktop** (default) -- Full Tauri windowed application
- **MCP stdio** (`mcp serve --stdio`) -- Headless MCP server for Claude Code / Cursor integration

### Runtime configuration

- **Allocator:** mimalloc with aggressive purge (0ms delay, eager abandoned page cleanup, no large OS pages)
- **Tokio runtime:** 4 workers, 2MB stacks (vs Rust default of 1-per-core, 8MB). Leaked intentionally (lives for process lifetime)
- **Memory hook:** `common::memory::set_purge_hook(purge_mimalloc)` registered globally so lower-layer crates can trigger OS page return after large transient allocations

### Command modules

50+ modules in `src/commands/` -- thin wrappers that delegate to `AppCore` methods. The pattern is consistent:

```rust
#[tauri::command]
async fn task_create(
    state: State<'_, Arc<AppCore>>,
    app: AppHandle,
    params: TaskCreateParams,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_create(params).await?;
    super::emit_updates(&app, &updates);
    Ok(result)
}
```

Each module exports `pub const DEV_COMMANDS: &[&str]` listing its command names. A compile-time parity test (`dev_server_covers_all_tauri_commands`) ensures every Tauri command has a dev server HTTP equivalent, and vice versa.

### Event emission

`emit_updates(app, updates)` iterates `Vec<EntityUpdate>` and emits `entity:updated` Tauri events per entity. The frontend's `useQuery` hook listens for these to auto-invalidate its SWR cache.

### Focus timer

`src/focus_timer.rs` -- Backend-owned Pomodoro state machine.

Phases: **Working** -> **BreakPending** (5s transition) -> **Break** -> **Working** (auto-continues).

- Configurable durations (default: 45min work, 5min short break, 15min long break)
- macOS DND integration via `platform-macos` crate
- Emits `focus:sync` (5s interval), `focus:phase_changed`, `focus:warning` events
- Coordinates with tray countdown via `FOCUS_ACTIVE` atomic flag

### Tray countdown

`src/tray_countdown.rs` -- Live countdown in the macOS menu bar showing the next calendar event or task deadline (e.g. `"24:57 -- Standup"`).

- Polls DB every 30s for the next upcoming item due today
- Ticks every 1s when counting down, 10s when idle
- Yields to focus timer and voice session via 3 atomic flags: `FOCUS_ACTIVE`, `VOICE_ACTIVE`, `VOICE_PHASE`
- Uses `tauri::async_runtime::spawn` (not `tokio::spawn`) because it starts during Tauri's `setup` hook

### Tauri plugins

Registered in the builder:

- `tauri-plugin-global-shortcut` -- Hotkeys (dashboard, launcher, voice)
- `tauri-plugin-notification` -- OS notifications with app icon
- `tauri-plugin-updater` -- Auto-update checks
- `tauri-plugin-dialog` -- Native file/folder dialogs
- `tauri-plugin-process` -- Process info and exit

### Other desktop modules

- `oauth.rs` -- MCP OAuth flow handling (browser redirect capture)
- `shortcuts.rs` -- Global shortcut registration from config
- `lazy_window.rs` -- Deferred window creation (voice orb, launcher)
- `notify.rs` -- Notification bridging to Tauri's notification plugin

## Dev Server

**Location:** `src/dev_server/` (debug builds only, `#[cfg(debug_assertions)]`)

An Axum HTTP server on port 3456 that mirrors all Tauri IPC commands as REST endpoints. This enables browser-only development: run `bun run dev` (Vite on 1420) + `cargo tauri dev` (which starts the dev server), then open `localhost:1420` in Chrome with full API access.

### Endpoints

| Route | Method | Purpose |
|-------|--------|---------|
| `/api/{cmd}` | POST | Generic command dispatch (mirrors Tauri `invoke`) |
| `/api/events/{sessionKey}` | GET | SSE stream for chat streaming events |
| `/api/cognitive/stream` | GET | SSE stream for cognitive pipeline events |
| `/api/insight/events` | GET | SSE stream for insight events |
| `/api/brain/events` | GET | SSE stream for global app events |
| `/api/v1/ingest` | POST | Activity log ingestion |
| `/api/v1/ingest/batch` | POST | Batch activity ingestion |

### Architecture

`dispatch.rs` routes POST requests by command name to the corresponding `dispatch_dev` function in each command module. Entity updates from `HandlerResult` are discarded (no Tauri `AppHandle` available). Event delivery uses `SseEmitter` which implements `AppEventEmitter` via a tokio broadcast channel.

CORS is configured for `http://localhost:1420` (the Vite dev server). The Vite config proxies `/api/*` to port 3456.

### Parity enforcement

Two tests guarantee bidirectional parity:

- `dev_server_covers_all_tauri_commands` -- Every Tauri command has a dev dispatch (except `TAURI_ONLY` commands like `permissions_check_accessibility`, `quit_app`, focus session controls)
- `dev_server_has_no_orphan_commands` -- No dev dispatch exists without a corresponding Tauri command

## React 19 Frontend

**Location:** `desktop-ui/`
**Stack:** React 19, TypeScript, Vite, Tailwind v4, Biome 2.0

### Directory structure

```
desktop-ui/src/
  app/              -- Shell layout, router, providers, BrainEventBridge
  features/         -- 21 feature folders (chat, tasks, learn, finance, ...)
  shared/
    hooks/          -- 27 shared hooks (useQuery, useMutation, useEvent, ...)
    components/     -- Reusable UI components
    composites/     -- Multi-component compositions
    lib/            -- Utilities (dates, formatting, ipc transport)
    stores/         -- Zustand stores
    styles/         -- CSS variables, theme definitions
    types/          -- Shared TypeScript types
    ui/             -- Primitive UI components
```

### Data fetching

Three hooks form the data layer:

**`useQuery<T>(cmd, args)`** -- Read operations. SWR-style caching with 50-entry LRU cache, 1-minute TTL, 30-second stale time, request deduplication. Automatically refetches when `entity:updated` events match the query's entity type.

**`useMutation<T>(cmd)`** -- Write operations. Calls `ipc(cmd, args)`, then auto-invalidates related queries via entity update events emitted by the backend.

**`useEvent<T>(event, handler)`** -- Subscribes to Tauri events (or SSE in browser dev mode). Used for streaming chat content, focus timer sync, coaching nudges, and real-time activity updates.

### Dual-transport IPC

`useIpc.ts` exports `ipc<T>(cmd, args)` which auto-detects the environment:

- **Tauri:** Calls `invoke<T>(cmd, args)` directly
- **Browser dev mode:** POSTs to `/api/{cmd}` via Vite's proxy to port 3456

This is the only place transport selection happens. All feature code uses `ipc()` uniformly.

### Feature pages

| Feature | Route | Key capabilities |
|---------|-------|-----------------|
| Dashboard | `/day/:date`, `/week/:date`, `/month/:date`, `/year/:year` | Calendar views, daily agenda |
| Chat | `/chat` | Streaming responses, markdown rendering, persona debate, tool transparency |
| Tasks | `/tasks` | Kanban board, grid view, tree view, custom columns, drag-and-drop |
| Notes | `/notes` | Knowledge base, notebook organization, AI insight tabs |
| Learn | `/learn` | FSRS spaced repetition review, knowledge graph (d3/three.js), health metrics |
| Finance | `/finance`, `/finance/overview`, `/finance/investments`, `/finance/targets` | Cash flow, investment tracking, budget goals |
| Projects | `/projects`, `/project/:id/:tab` | Project detail with conversations, memories, sources |
| Brain | `/brain` | Mirror self-reflection: narratives, routing history, brain versions, meta-rules |
| Coaching | `/coaching`, `/coaching/patterns`, `/coaching/history` | Behavioral pattern detection, intervention history |
| Settings | `/settings/*` | General, configuration, MCP servers, integrations, voice, launcher (12 tabs) |
| System | `/system/:tab` | Work contexts, app categories, inference debug, event log |
| Automations | `/automations` | Cron jobs, scheduled tasks |
| Voice | `/voice-orb` | Voice orb overlay (separate window) |
| Launcher | `/launcher` | Spotlight-style search (separate window) |
| Tray | `/tray` | System tray popover |

### Theming

All visual tokens are CSS variables defined in `src/shared/styles/theme.css`. Uses OKLch color space with two themes ("dark" and "retro"). Tailwind v4 consumes these via `@theme inline` -- no `tailwind.config.js`. The `glass-panel` class provides glassmorphism for dropdowns, popups, and dialogs.

### React Compiler

Enabled via `babel-plugin-react-compiler` in `vite.config.ts`. Auto-memoizes components -- manual `React.memo`, `useMemo`, and `useCallback` are unnecessary unless profiling shows a specific need.

## Startup Sequence

The full desktop startup, in order:

1. **`main.rs`** -- Configure mimalloc, parse CLI, set up tokio runtime (4 workers, 2MB stacks)
2. **Tauri builder** -- Register plugins (shortcuts, notifications, updater, dialog, process)
3. **`.setup()` hook** -- Call `app_core::init()` which runs the full initialization pipeline (see above), returns `(AppCore, global_event_tx)`
4. **Store state** -- `app.manage(Arc::new(core))` makes `AppCore` available to all commands
5. **Dev server** (debug only) -- Spawn Axum server on port 3456
6. **MCP HTTP server** (if `mcp.server.enabled`) -- Spawn embedded MCP server with bearer auth
7. **Register shortcuts** -- Global hotkeys from config (or defaults on failure)
8. **Register voice hotkey** -- Separate from the 3-shortcut system, context-aware (focus -> quick journal, launcher -> hands-free search, normal -> voice orb toggle)
9. **Build tray icon** -- System tray with click-to-toggle dashboard, right-click menu
10. **Spawn tray countdown** -- Background task for menu bar countdown
11. **Start focus timer** -- Background Pomodoro state machine

Background services spawned during `AppCore::init` (step 3) run on the tokio runtime: cron scheduler, deadline scheduler, mirror engine subscribers, config file watcher, lifecycle monitor.

## Extensibility

### Adding a new IPC command

1. **Desktop-shared:** Define request/response types in `crates/desktop-shared/src/commands/your_domain.rs`
2. **AppCore:** Implement the handler in `crates/app-core/src/handlers/your_domain.rs`, returning `HandlerResult<T>`
3. **Desktop command:** Create thin wrapper in `crates/desktop/src/commands/your_domain.rs` with `#[tauri::command]`, add to `DEV_COMMANDS`
4. **Register:** Add the command to the `generate_handler!` list in `main.rs`, add the module to `dev_server/dispatch.rs`
5. **Frontend:** Call via `ipc<ResponseType>("your_command", params)` using `useQuery` or `useMutation`

The parity test will fail if step 4 is incomplete.

### Adding a new feature page

1. Create a folder in `desktop-ui/src/features/your_feature/`
2. Export the page component from the feature's `index.ts`
3. Add a lazy import and route in `desktop-ui/src/app/router.tsx`
4. Use `useQuery(cmd, args)` for reads and `useMutation(cmd)` for writes -- both auto-handle cache invalidation

---

See also:
- [Core Infrastructure](core-infrastructure.md) -- Storage, bus, config, providers
- [Features](features.md) -- Feature package system, individual feature details
- [Agent Runtime](agent-runtime.md) -- Skill routing, ReAct execution, context engine
