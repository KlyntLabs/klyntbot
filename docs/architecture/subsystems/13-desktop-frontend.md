# Subsystem 13 — Desktop App & Frontend

> **Status:** 🟢 Stable — production deployable shape with active migrations: ThreadEvent v1 → v2 (assistant chat still on v1), MCP HTTP server is opt-in
> **Status last verified:** 2026-05-16
> **Crates / dirs:** `desktop`, `desktop-shared`, `desktop-macros`, `crates/desktop-ui` *(stub)*, `/desktop-ui` *(repo root TS)*, `app-core`, `klyntbot` *(facade)*, `klyntbot-server`
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

The integration cross-section of everything below. The **`desktop` binary** is the single deployable: it doubles as `klyntbot-hook` (sub-10ms short-circuit) and as the MCP stdio server (`mcp serve --stdio`). Startup runs through 17 steps including `pre_main_hardening` (load-bearing — must precede mimalloc), Tauri builder, lazy secondary windows, and an optional embedded Axum MCP server. **`app-core`** is the actual integration crate — it owns `AppCore`, all handlers, the init sequence, both `ThreadRuntime` impls (assistant + coding), and is transport-agnostic. The root **`klyntbot` facade** does `pub use` re-exports of every workspace crate plus convenience type-level re-exports. **`klyntbot-server`** is the MCP server library used by both stdio + HTTP modes.

The **frontend** at `/desktop-ui/` (repo root, **NOT** `crates/desktop-ui/`) is React 19 + Vite + Bun + **Tailwind** (CLAUDE.md says "plain CSS — no Tailwind" — wrong, both `@tailwindcss/vite` and explicit `tailwindcss()` plugin are present). 32 feature directories. `useChatStore` is a 3-slice Zustand store (Threads / Stream / Coding) with a 50ms-coalescer for stream snapshots and `@tanstack/react-virtual` for message virtualization.

Two attribute macros (`#[klynt_command]` constrained, `#[klynt_raw_command]` unconstrained) plus two collection macros (`klynt_collect_commands!` + `klynt_collect_events!`) enforce the IPC surface. **Five CI tests guard the surface**: `no_raw_tauri_command_outside_macros`, `registration_drift`, `bindings_are_current`, `no_double_registration`, plus a `specta_builder_smoke` test.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef bin fill:#cfd8dc,stroke:#37474f,color:#263238
    classDef macro fill:#fff8e1,stroke:#f9a825,color:#f57f17
    classDef core fill:#fff3e0,stroke:#f57c00,color:#e65100
    classDef facade fill:#e8eaf6,stroke:#3949ab,color:#1a237e
    classDef ui fill:#e1f5fe,stroke:#0277bd,color:#01579b
    classDef server fill:#dcedc8,stroke:#7cb342,color:#33691e
    classDef test fill:#fce4ec,stroke:#c2185b,color:#880e4f

    DESK[desktop binary<br/><i>main.rs 17-step startup<br/>--hook short-circuit<br/>mcp serve subcommand<br/>5 secondary windows<br/>OAuth Axum server<br/>tray_countdown</i>]:::bin
    DSH[desktop-shared<br/><i>ThreadEvent v2 (26 variants)<br/>CommandResult / ApiError<br/>specta::Type DTOs</i>]:::core
    DMC[desktop-macros<br/><i>#[klynt_command]<br/>#[klynt_raw_command]<br/>klynt_collect_commands![]<br/>klynt_collect_events![]</i>]:::macro
    TST[5 CI guards<br/><i>no_raw_tauri_command_outside_macros<br/>registration_drift<br/>bindings_are_current<br/>no_double_registration<br/>specta_builder_smoke</i>]:::test

    AC[app-core<br/><i>AppCore struct (transport-agnostic)<br/>~40 handler domains<br/>init/ (14 phases)<br/>runtime/ (ThreadRuntime trait + 2 impls)<br/>coding/ + coding_memory/<br/>tracing providers (CC/Kimi/Klynt)</i>]:::core
    FAC[klyntbot facade<br/><i>pub use all 64 crates<br/>+ convenience type re-exports</i>]:::facade
    SRV[klyntbot-server<br/><i>KlyntbotServerHandler<br/>ToolRegistryBridge + AgentBridge<br/>Stdio + Embedded HTTP (Axum)</i>]:::server

    UI[/desktop-ui — repo root<br/><i>React 19 + Vite + Bun + Tailwind<br/>32 features<br/>useChatStore (3 slices)<br/>VirtualizedMessageList<br/>50ms coalescer + watchdog</i>]:::ui
    BIN_STUB[crates/desktop-ui<br/><i>REMOVED from workspace<br/>orphaned src/bindings.ts</i>]:::ui

    DESK --> AC
    DESK --> SRV
    DESK --> DMC
    AC --> FAC
    SRV --> AC
    DMC --> TST
    UI -.IPC: invoke().-> DESK
    UI -.events: thread:event.-> DESK
    DESK -.bindings.ts.-> BIN_STUB
    BIN_STUB -.imported by.-> UI
```

---

## Mental model

**Six conceptual roles in one subsystem:**

1. **The binary** (`desktop`) — single executable, triple-mode (Tauri app / `--hook` / `mcp serve`).
2. **Transport-agnostic core** (`app-core`) — all handlers, init, runtime. No Tauri code references in the struct.
3. **Macros** (`desktop-macros`) — enforce the IPC surface so the runtime dispatch table can't drift from the Specta TypeScript bindings.
4. **Shared DTOs** (`desktop-shared`) — `ThreadEvent v2`, `CommandResult`, all derive `specta::Type`.
5. **MCP server** (`klyntbot-server`) — exposes a subset of `AppCore` capabilities to external clients.
6. **Frontend** (`/desktop-ui`) — React 19 SPA. Single Zustand store. Coalesced stream renders.

### Three structural facts that surprise people

1. **`crates/desktop-ui/` is a stub.** It contains only `src/bindings.ts` (the auto-generated tauri-specta TypeScript file). The real React frontend is at the repo root `/desktop-ui/`. Anyone grepping `crates/desktop-ui/` for component source will find nothing useful.
2. **`app-core` is the integration crate, not `klyntbot`.** The root `klyntbot` crate at `src/lib.rs` does `pub use` re-exports of every workspace crate but contains zero logic. Importing `klyntbot::AppCore` works because of the re-export chain; `AppCore` actually lives in `app-core`.
3. **The frontend uses Tailwind.** CLAUDE.md says "Plain CSS. No Tailwind. All styles in `src/styles/*.css`. Class naming is BEM-ish." Actual: `@tailwindcss/vite` is in `vite.config.ts` plugins and Tailwind classes appear throughout the codebase. The BEM-ish naming exists for *legacy* styles in `src/styles/*.css`; new code uses Tailwind. Doc drift.

---

## Reference

### `desktop` — startup sequence (17 steps)

`fn main()` at `crates/desktop/src/main.rs`:

```
1.  --hook short-circuit
    raw_args[1] == "--hook" → coding_ingest::hook_cli::run() → std::process::exit
    (sub-10ms; no allocator, no clap, no Tauri)

2.  pre_main_hardening
    ptrace deny, RLIMIT_CORE=0, env scrub. MUST precede #3.

3.  configure_mimalloc
    MI_OPTION_PURGE_DELAY=0, ARENA_PURGE_MULT=1, ABANDONED_PAGE_PURGE=1.
    Disables large OS pages + eager commit. Minimizes RSS growth.

4.  Cli::parse
    Clap parses: `mcp serve --stdio`, `mcp tools --list`, or no subcommand → desktop app.

5.  run_desktop_app
    Register purge_mimalloc as global memory hook.
    Build capped 4-worker tokio runtime (2MB stacks), leak it.
    Init tracing to stderr.
    Run specta_builder::build_specta(); in debug builds, export desktop-ui/src/bindings.ts.

6.  Tauri builder
    Plugins: tauri-plugin-global-shortcut, -notification, -updater, -dialog, -process.

7.  setup closure
    specta.mount_events(app).
    app_core::init(handle) → AppCore::init_with_sender (blocking).
    Returns (core, global_event_tx, approval_channel).
    Spawns claude_code_integration::run_first_launch_check (idempotent MCP registration).
    In debug builds: optionally start dev_server::start.
    Optionally start embedded MCP HTTP server (Axum + StreamableHttpService).

8.  Managed state
    app.manage(core); app.manage(approval_channel); app.manage(Arc::new(FocusTimer::new())).

9.  Secondary windows (lazy)
    Registered but NOT created at startup. See [Secondary windows](#secondary-windows).

10. shortcuts::register_shortcuts
    Reads config.shortcuts. Unregisters all, re-parses, registers via global-shortcut plugin.

11. Voice hotkey (separate)
    Reads config.voice.input.hotkey. Context-aware handler at 3 levels:
      - Focus session active → quick voice capture (no orb)
      - Launcher visible → emit "voice-recording-start" event
      - Otherwise → toggle voice-orb window + VoiceConversationManager

12. macOS menu
    Cmd+Q maps to HIDE dashboard (not quit). Cmd+W intentionally unbound.
    CloseRequested event intercepted → hide window + set ActivationPolicy::Accessory.

13. Tray icon
    TrayIconBuilder::with_id("klynt-tray").
    Left-click: if VOICE_ACTIVE → voice pause/resume; else shortcuts::toggle_window(tray).

14. mimalloc compaction timer
    10s interval calling mi_collect(true) until shutdown_token fires.

15. tray_countdown::spawn
    Subscribes to DomainEventBus.
    Tick policy: 1s when countdown visible, 2s for voice, 60s for focus, 1h when idle.

16. OAuth (lazy)
    crates/desktop/src/oauth/ — local Axum HTTP server on FIXED CALLBACK_PORT.
    mcp_oauth_start command opens browser; callback handler exchanges code → tokens,
    stores via OAuthRegistry, emits McpOAuthCompletePayload.

17. Invoke handler
    .invoke_handler(specta_builder::klynt_invoke_handler())
    Runtime dispatch table built from KLYNT_COMMANDS linkme slice.
    Replaces tauri::generate_handler![].
```

### `desktop-macros` — 4 macros

| Macro | Constraints | Generates |
|---|---|---|
| `#[klynt_command]` | `pub async fn`, no `State` param, no `Result` return | Injects `state: tauri::State<'_, Arc<AppCore>>` as first arg. Wraps return as `CommandResult<T>`. Adds `#[tauri::command]` + `#[specta::specta]`. Emits `__klynt_dispatch_*` dispatcher. Emits `#[linkme::distributed_slice(KLYNT_COMMANDS)] static CommandRegistration { source: Klynt }`. |
| `#[klynt_raw_command]` | None | Leaves body unchanged. Emits same dispatcher + slice registration with `source: Raw`. Use for OAuth, streaming, custom state. |
| `klynt_collect_commands![paths...]` | Invoked once in `specta_builder.rs` | Emits `KLYNT_SPECTA_COMMAND_NAMES: &[&str]` (last path segment of each) + `__klynt_specta_commands()` for specta type export. |
| `klynt_collect_events![paths...]` | Invoked once | Emits the event registration array for specta event export. |

`KLYNT_SPECTA_COMMAND_NAMES` is re-exported as `SPECTA_COMMAND_NAMES` for backward compat.

### Four CI guards on the IPC surface

| Test | What it checks |
|---|---|
| `no_raw_tauri_command_outside_macros` | `rg`-scans `crates/desktop/src/commands/` + `crates/desktop/src/oauth/` for bare `#[tauri::command]` not wrapped in either macro. Fails if any found. |
| `registration_drift` | Compares `KLYNT_COMMANDS` (linkme slice, runtime truth) with `SPECTA_COMMAND_NAMES` (specta hand-list, FE binding truth) as `BTreeSet<&str>`. Fails with a diff if they diverge. |
| `bindings_are_current` | Calls `build_specta().export_str(Typescript::default())` and compares **byte-for-byte** against `desktop-ui/src/bindings.ts`. Writes the regenerated file on failure (so the next run is green if you commit it). Auto-regenerated by `cargo tauri dev` in debug builds. |
| `no_double_registration` | Guards against the same command appearing twice in the linkme slice. |

### Secondary windows (5)

All created lazily via `lazy_window::get_or_create_window(app, label)`.

| Label | Size | Behavior |
|---|---|---|
| `launcher` (WINDOW_LAUNCHER) | 660×580 | `hud_effects()`, dismiss-on-blur (also emits `voice-recording-reset`), `always_on_top`, `transparent`, no decorations, centered |
| `tray` (WINDOW_TRAY) | 320×600 | `hud_effects()`, dismiss-on-blur, `always_on_top`, `transparent`, `focused(false)` |
| `distraction-overlay` | 340×300 | `hud_effects()`, `always_on_top`, centered, `focused(true)` |
| `voice-orb` | 200×200 | Transparent, `always_on_top`, no decorations, no blur dismiss; positioned bottom-right of cursor monitor via `position_orb_bottom_right` |
| `coding:{repo_id}` | 1200×800 (min 700×500) | **Full decorations**, visible immediately, normal window. Label parsed by `parse_coding_label`. Per-repo, persists. CLAUDE.md doesn't list this one. |

**`hud_effects()` helper:** `EffectsBuilder::new().effect(HudWindow).state(Active).radius(16.0).build()`. macOS vibrancy HUD style.

**Drag handle pattern:** CSS class `.lc-drag-handle` with `-webkit-app-region: drag`. `useWindowDrag.ts` also supports `data-tauri-drag-region` attribute zones via `getCurrentWindow().startDragging()` (calls `startDraggingSafe()`).

**`position_on_cursor_monitor`:** Reads `cursor_position()`, finds the monitor containing it, centers the window on that monitor at 1/3 height (Spotlight-style). Falls back to `window.center()`.

### `app-core::AppCore` struct (selected fields)

Transport-agnostic. No Tauri/Axum types in the struct itself.

```rust
pub struct AppCore {
    // Core infra
    pub mode: AppMode,
    pub repos: Repos,
    pub storage_pool: StoragePool,
    pub agent: Arc<AgentLoop>,
    pub bus: Arc<MessageBus>,
    pub config: Arc<RwLock<Config>>,
    pub hot_config: Arc<RwLock<HotConfig>>,
    pub channel_manager: ChannelManager,
    pub cron_executor: Arc<CronExecutor>,
    pub cron_repo: CronRepo,
    pub cron_bridge: Option<CronBridge>,
    pub shutdown_token: CancellationToken,

    // Streaming
    pub active_streams: Arc<ActiveStreams>,
    pub pending_interactions: Arc<DashMap<String, (String, oneshot::Sender<FormResponse>)>>,

    // Runtimes (lazy)
    pub assistant_runtime: OnceLock<Arc<dyn ThreadRuntime>>,
    pub coding_runtime: OnceLock<Arc<dyn ThreadRuntime>>,

    // Coding mode
    pub thread_events: TypedBroker<ThreadEvent>,
    pub cost_events: TypedBroker<CostUpdate>,
    pub subagent_events: TypedBroker<SubagentEvent>,
    pub thread_subscriptions: ...,
    pub steer_queue: ...,
    pub tool_kit: ...,
    pub desktop_approval_channel: Arc<DesktopApprovalChannel>,
    pub approval_grants_repo: Arc<GrantRepo>,
    pub coding_policies: ...,
    pub ingest_daemon: ...,
    pub distiller: ...,
    pub recall: ...,
    pub coding_toolset: ...,
    pub job_supervisor: Arc<dyn JobSupervisorHandle>,
    pub tracing_registry: Arc<TracingRegistry>,

    // Optional feature services (all Option<>)
    pub productivity, coaching, cognitive, mirror, voice,
    pub launcher, flashcard, knowledge_graph, autotuner,
    pub temporal_scheduler, notifications, embedding,

    // Event emitter (transport-agnostic adapter)
    pub event_emitter: Arc<dyn AppEventEmitter>,
}
```

### `AppCore::init_with_sender` — 14 phases

`init/mod.rs` orchestrates. Phases in order:

```
1.  Config load + merge + env overrides
2.  init::storage             → SQLite pool, migrations, Repos
3.  init::channels            → EventChannels (intervention_rx, pipeline_rx)
4.  init::ai_pipeline         → AiFeatureRegistry, AgentLoop, tools
5.  init::cognitive           → cognitive graphs, semantic facts repo, embedding, mirror
6.  init::cron                → CronExecutor, CronBridge, TemporalScheduler
7.  init::launcher            → LauncherSearchEngine
8.  init::coaching            → signal accumulator, pattern detector, intervention router
9.  init::temporal_scheduler  → spawn background fire loop
10. init::coding_subscribers + coding_recall + coding_retention + coding_skills
                              → Distiller, recall service, skill activator
11. init::productivity        → productivity engine, focus manager, DND manager
12. init::dnd                 → DND end subscriber
13. File watchers             → config + data-version + lifecycle + wake orchestrator
14. Voice                     → VoiceService + VoiceConversationManager
```

### `app-core/handlers/` — ~40 domain handlers

`agents`, `annotations`, `areas`, `atoms`, `autotuner`, `capture`, `chat/`, `coaching`, `coding_jobs`, `coding_plan`, `coding_todo`, `cognitive/`, `columns`, `cron`, `distraction`, `entities`, `entity_links`, `fabric`, `finance/`, `git`, `groups`, `integrations`, `key_results`, `knowledge_health`, `launcher/`, `morning_briefing`, `notes/`, `objectives`, `productivity/`, `project_conversations`, `project_memories`, `project_sources`, `projects`, `reforge`, `retention_history`, `review_stats`, `settings/`, `status`, `subagent`, `tasks/`, `timeline`, `view`, `voice`, `voice_conversation`, `voice_conversation_commands`, `voice_echo`, `work_context`, `workflows`, `workspace`.

### `runtime/` — `ThreadRuntime` trait + impls

```rust
pub trait ThreadRuntime: Send + Sync {
    fn start_turn(&self, ...) -> ...;
    fn cancel_turn(&self, turn_id: &str);
    fn is_active(&self, thread_id: &str) -> bool;
    fn active_turns(&self) -> Vec<String>;
}
```

Two concrete impls: `AssistantThreadRuntime` (in `runtime/assistant.rs`) and `CodingThreadRuntime` (in `runtime/coding.rs`). Both share `ActiveTurns = Arc<DashMap<String, ActiveTurnEntry>>` and `StreamGuard`.

**`StreamGuard` value-identity pattern:** Each entry has a monotonic `guard_id: u64` (from `STREAM_GUARD_COUNTER`). `Drop` removes the entry **only if** the stored `guard_id` still matches — prevents a new turn from being cleaned up by an old guard's deferred drop. This is non-obvious but load-bearing for double-send safety.

### `ThreadEvent v2` — 26 variants

`desktop-shared/src/thread_event_v2.rs`. Replaces 50+ legacy `agent:*` events with a single tagged union (`#[serde(tag = "event", rename_all = "snake_case")]`):

| Group | Variants |
|---|---|
| Content | `ContentChunk` |
| Tools | `ToolStart`, `ToolEnd` |
| Memory | `EntityCreated`, `MemoryAccess`, `MemoryPromoted` |
| Pipeline | `PipelineStarted`, `ExecutionStarted`, `ContextAssembled`, `RetrievalEnhanced`, `IterationStart`, `ClassificationComplete` |
| Agents | `AgentSelected`, `SkillLoaded`, `LearningEvent`, `SubagentSpawned`, `DelegationStarted`, `DelegationCompleted` |
| Plan | `PlanGenerated`, `PlanStepCompleted` |
| Usage | `UsageReport`, `ConfidenceAssessed`, `BudgetWarning` |
| Interaction | `InteractionRequest` |
| Heartbeat | `Heartbeat` (30s keepalive) |
| Terminal | `Terminal { kind: TerminalKind }` — guaranteed on every exit |

`TerminalKind`: `Done { content, message_id? }`, `Error { message }`, `Cancelled { partial_content, partial_reasoning }`.

Every variant carries `generation: u32` + `session_key: String`. Frontend filters stale events by comparing `generation`.

Tauri channel name: `thread:event`.

### `klyntbot` facade (root `src/lib.rs`)

```rust
// Full crate re-exports (gives access to entire namespaces)
pub use activity_log;
pub use agent;
pub use app_core;
// ... all 60+ crates

// Convenience type-level re-exports at klyntbot::
pub use agent::{AgentEvent, AgentLoop, ProgressHandlerImpl, StreamingHandle, SubagentManager, ...};
pub use bus::{InboundMessage, MessageBus, OutboundMessage};
pub use channels::{Channel, ChannelManager, DynChannel};
pub use common::{Result, SessionKey, MessageRole, ChannelName, ChatId, ...};
pub use config::Config;
pub use providers::{LlmProvider, DynProvider, Message, ...};
pub use storage::{Repos, StoragePool};
pub use tools::{DynTool, Tool};
pub use scheduling::{CronExecutor, CronJob};
pub use session::{Session, SessionManager};
pub use notifications::{NotificationDispatcher, NotificationDispatcherHandle};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
```

**Selective at the type level, full at the crate level.** Both forms work: `klyntbot::AppCore` (via the `app_core` re-export) and `klyntbot::Config` (via the explicit `pub use config::Config`).

### `klyntbot-server`

```rust
KlyntbotServerHandler::new(app: Arc<AppCore>, whitelist: Vec<String>)
   ↓ implements rmcp::handler::server::ServerHandler

list_tools() → [get_status] + [agent?] + bridge.list_tools()
call_tool(name, args)  → dispatches to:
   - handle_get_status (always available)
   - agent_bridge.execute (if "agent" in whitelist)
   - bridge.execute (everything else)
```

**Resources exposed:** `klyntbot://status`, `klyntbot://memory/recent`, `klyntbot://tasks/today`, `klyntbot://config/skills`.

**Modes:**
- **Stdio** — `klyntbot_server::serve_stdio(app, whitelist)`, called from `run_mcp_stdio()` in `main.rs`. Uses `rmcp::transport::io::stdio()`. Drains event channels in a separate task. Calls `app.shutdown()` before returning.
- **Embedded HTTP** — Spawned in `run_desktop_app` if `config.mcp.server.enabled`. Uses `rmcp::transport::streamable_http_server::StreamableHttpService<KlyntbotServerHandler, LocalSessionManager>` mounted at `/mcp`. Optional bearer-token auth middleware. Bound to `config.mcp.server.host:port`.

**Post-mutation entity updates** are dispatched via `emit_entity_update_for_tool` using `AiFeatureRegistry` (primary) or `NON_FEATURE_TOOL_ENTITY_KINDS` fallback (OKR, project, area, work_context, productivity).

**No explicit `/health` route** — status via `get_status` MCP tool or `klyntbot://status` resource.

### `desktop-shared`

| Type | Purpose |
|---|---|
| `ThreadEvent` (v2) | 26-variant tagged union; canonical thread event surface |
| `CommandResult<T>` | `Result<T, ApiError>` — every `#[klynt_command]` returns this |
| `ApiError { code: String, message: String }` | Serde-tagged camelCase; exhaustive `From<KlyntbotError>` |
| Coding event types | Per-feature thread payload types |
| All derive `specta::Type` | Feeds `tauri-specta` type export → `bindings.ts` |

### `desktop-ui` (repo root)

| Aspect | Value |
|---|---|
| Location | `/Users/jayden/Projects/Klynt/bot/desktop-ui/` (**NOT** `crates/desktop-ui/`) |
| Stack | React 19, Vite 8, Bun (always; never npm), TypeScript, Vitest, ESLint |
| **CSS** | **Tailwind CSS via `@tailwindcss/vite` plugin.** CLAUDE.md claims "Plain CSS. No Tailwind" — wrong. |
| Manual chunks | `vendor-react`, `vendor-markdown`, `vendor-tauri`, `vendor-ui`, `vendor-xterm`, `vendor-mermaid`, `vendor-katex` |
| Worker format | ES modules |
| Build target | `es2022` |
| Bindings | `src/bindings.ts` — auto-generated by tauri-specta in debug Tauri builds + as a CI check |

**Path aliases** (`vite.config.ts`):
- `@/` → `src/` *(catch-all)*
- `@app/` → `src/features/app/`
- `@settings/` → `src/features/settings/`
- `@threads/` → `src/features/threads/`
- `@services/` → `src/services/`
- `@utils/` → `src/utils/`
- **No `@shared` or `@features` aliases** — those were old UI conventions.

**33 feature directories** (`src/features/`): about, app, apps, chat, coding, collaboration, composer, dashboard, debug, design-system, dictation, distraction, files, git, home, launcher, layout, messages, mobile, models, notifications, plan, plugins, prompts, settings, shared, skills, terminal, threads, tray, update, workspaces.

### `useChatStore` (Zustand, 3 slices)

Single `create<ChatStore>()` call combining:

| Slice | State | Key behaviors |
|---|---|---|
| **ThreadsSlice** | `ThreadState` from `useThreadsReducer` | `dispatchThreadAction` applies the reducer |
| **StreamSlice** | `streamSnapshots: Record<sessionKey, StreamSnapshot>`, `streamApprovals`, `streamFileEdits` | **Coalesced via `CoalescerRegistry<StreamSnapshot>` — 50ms max-wait per session.** Evicts at most 5 idle sessions on each stream start/completion. |
| **CodingSlice** | `CodingThreadState` managed by `applyThreadEvent` from `codingEventReducer` | Per-thread coding state |

Imports `chatStreamStore` as a side effect to register **legacy v1** `agent:*` Tauri event listeners until full v2 migration completes.

### Coalescer, virtualized list, watchdog

| Component | Purpose | Location |
|---|---|---|
| `CoalescerRegistry` | Throttles `_setStreamSnapshot` calls to max 50ms batches; flushes only last snapshot per batch | `src/features/threads/store/useChatStore.ts` |
| `VirtualizedMessageList` | `@tanstack/react-virtual` wrapping `Messages` | `src/features/messages/components/VirtualizedMessageList.tsx` |
| `useThreadWatchdog` | Monitors backend's 30s `Heartbeat`; fires `onFire` if no heartbeat while `isProcessing=true` | `src/features/threads/hooks/useThreadWatchdog.ts` |

### Approval modal locations

| Surface | File |
|---|---|
| Coding threads | `src/features/coding/components/ApprovalCard.tsx` + `useApprovalQueue.ts` |
| Assistant threads | `src/features/threads/hooks/useThreadApprovalEvents.ts` + `useThreadApprovals.ts` |
| App-level toasts | `src/features/app/components/ApprovalToasts.tsx` |

---

## Workflows

### A Tauri command from frontend to backend

```
1. Frontend: chatStreamStore.startStream(sessionKey)
   → commands.chatSend({...}) from auto-generated bindings.ts wrapper
   → invoke("chat_send", args)
2. Tauri IPC: serializes args to JSON, routes via klynt_invoke_handler
   → fn __klynt_dispatch_chat_send (generated by #[klynt_command])
3. Desktop command shim: crates/desktop/src/commands/chat.rs
   → pub async fn chat_send(state: State<Arc<AppCore>>, ...) ← injected by macro
   → state.handlers::chat::streaming::handle_send(...)
4. app-core handler:
   - Acquires AppCore state
   - Creates StreamGuard (monotonic guard_id)
   - Inserts ActiveTurnEntry in ActiveTurns DashMap
   - Starts agent loop
5. Events flow back via TypedBroker<ThreadEvent>
   - Tauri adapter: app_handle.emit("thread:event", payload)
   - Legacy v1 adapter: app_handle.emit("agent:content_chunk", ...) for assistant chat
6. Frontend: chatStreamStore.ts listeners (registered via listen("thread:event", handler))
   → useChatStore.getState()._setStreamSnapshot(...)
   → CoalescerRegistry batches to 50ms
   → React re-render once per batch
```

### Secondary window creation (lazy)

```
1. Trigger: global shortcut fires → shortcuts::toggle_window(app, "launcher")
2. lazy_window::get_or_create_window(app, "launcher"):
   - app.get_webview_window("launcher") → Some → use existing
   - None → call build_launcher(app):
     a. WebviewWindowBuilder::new(app, "launcher", WebviewUrl::App("/#/launcher"))
     b. .inner_size(660.0, 580.0)
     c. .effect(hud_effects())
     d. .transparent(true).always_on_top(true).decorations(false).visible(false)
     e. .on_window_event |event| → dismiss-on-blur + emit("voice-recording-reset")
     f. build() → WebviewWindow
3. position_on_cursor_monitor(&window) → centers on cursor's display, 1/3 from top
4. window.show() + window.set_focus() + window.emit("window-shown", ())
5. Frontend: React Router renders /#/launcher
   → src/features/launcher/ components mount
   → launcher_search command invoked for initial data
```

### OAuth flow

```
1. Frontend invokes mcp_oauth_start("my-server")
2. crates/desktop/src/oauth/ starts local Axum server on CALLBACK_PORT
3. Opens provider auth URL in default browser
4. User authorizes; provider redirects to http://localhost:<CALLBACK_PORT>/callback?code=...&state=...
5. Callback handler:
   - Validates state
   - Exchanges code for tokens via provider's token endpoint
   - Stores via OAuthRegistry
   - Emits McpOAuthCompletePayload event to frontend
   - Returns success HTML page
6. Frontend listens for McpOAuthCompletePayload → updates UI
```

**Gotcha:** `CALLBACK_PORT` is fixed. If another process is already bound to it, OAuth start fails. No retry/fallback today.

---

## Internals

### Why hardening must precede mimalloc

mimalloc reads `MALLOC_*` / `MallocStackLogging*` env vars at initialization. The hardening step at line 112 of `main.rs` scrubs these env vars. **The order is load-bearing** — reordering to "init allocator early for perf" would silently break the hardening. Documented earlier in [`10-sandboxing-security.md`](./10-sandboxing-security.md) but worth surfacing here too.

### The 4-worker capped tokio runtime

```rust
let rt = tokio::runtime::Builder::new_multi_thread()
    .worker_threads(4)
    .thread_stack_size(2 * 1024 * 1024)   // 2 MB
    .build()
    .unwrap();
```

Capped at 4 workers + 2 MB stacks (default is unbounded workers + 2 MB stacks). For a single-user desktop app, more workers don't help and burn memory. The runtime is leaked (`Box::leak`) because Tauri's lifecycle outlives any structured drop.

### Mimalloc explicit compaction hook

`common::memory::set_purge_hook(purge_mimalloc)` is called at startup so lower-layer crates (storage, agent) can trigger `mi_collect(true)` after large transient allocations (LanceDB compaction, index rebuilds) without going through a timer. The 10s timer (#14) is the fallback.

### Tray-icon left-click is context-aware

```rust
if VOICE_ACTIVE.load(Ordering::Relaxed) {
    voice_pause_resume()
} else {
    shortcuts::toggle_window(app, WINDOW_TRAY)
}
```

A user with active voice can pause/resume by clicking the tray. Otherwise the click opens the tray popup window.

### macOS menu Cmd+Q maps to hide

`CloseRequested` event is intercepted: instead of closing, the window is hidden and `ActivationPolicy::Accessory` is set (removes from Dock). Cmd+W is intentionally unbound so it's available for in-app navigation. This matches the menu-bar-app UX pattern (like Spotlight, Raycast).

### `chatStreamStore` is the legacy v1 bridge

Still active for assistant chat. Registers ~30 `agent:*` Tauri event listeners (one per legacy event type). Coding threads use v2 `ThreadEvent` via the typed broker. Migration is in progress — coding moved first; assistant chat planned for a later cut. The legacy listeners coexist with v2 in `useChatStore.ts`.

### Coding-thread events use per-connection subscription

`coding_thread_subscribe` command returns a `subscription_id`. Events arrive on `agent:thread_event#{subscription_id}` rather than a global event name. Each frontend session subscribes once; backend routes events only to subscribers.

### `bindings.ts` regeneration cadence

- `cargo tauri dev` debug build → auto-regenerates `desktop-ui/src/bindings.ts`
- `bindings_are_current` test → fails if file is stale, writes the new version so the next run is green
- Hand-editing `bindings.ts` is wasted work

### `tray_countdown` adaptive tick rate

Different visible state → different tick rate. Idle desktop with no upcoming events → 1-hour tick (negligible CPU). Active countdown to a calendar event → 1-second tick (smooth countdown). Voice/focus modes have intermediate rates.

### `app-core` is transport-agnostic

`AppCore` struct holds no `tauri::*` or `axum::*` types. The transport injects via `Arc<dyn AppEventEmitter>`. The Tauri adapter emits to Tauri windows; the MCP child uses `SocketBridgeEmitter`. Same `AppCore` can serve a desktop UI or a stdio MCP child — that's what makes the binary triple-mode.

---

## Dependencies & extension points

### Upstream deps

- `tauri = "2"` + plugins (global-shortcut, notification, updater, dialog, process)
- `tauri-specta` (type export + event mount)
- `linkme` (distributed slice for `KLYNT_COMMANDS`)
- `mimalloc` + custom allocator hooks
- `axum` (embedded MCP HTTP server, OAuth callback server)
- `rmcp` (MCP server + transports)
- `tokio` (the leaked runtime)
- `tracing` + `tracing-subscriber` (stderr output)
- `dashmap` (active turns, stream snapshots)
- Frontend: React 19, Vite 8, Tailwind, `@tanstack/react-virtual`, Zustand

### Adding a Tauri command (the only sanctioned path)

1. Pick the right macro:
   - `#[klynt_command]` for the happy path (`pub async`, no `state` param, bare `T` return).
   - `#[klynt_raw_command]` otherwise.
2. Add to `klynt_collect_commands![paths...]` in `specta_builder.rs`.
3. Run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`.
4. Commit the regenerated `bindings.ts`.
5. The `registration_drift` + `bindings_are_current` tests pass.

**Don't** use bare `#[tauri::command]` — the `no_raw_tauri_command_outside_macros` test will fail.

### Adding a secondary window

1. Add `WINDOW_<NAME>: &str = "<name>"` constant.
2. Add `build_<name>` function in `lazy_window.rs` using `WebviewWindowBuilder`.
3. Apply `hud_effects()` if it's a floating panel.
4. Register dismiss-on-blur if appropriate.
5. Add `get_or_create_window` arm.
6. Add a `toggle_<name>_window` command if you want shortcut-driven toggle.

### Adding an init phase

1. Create `crates/app-core/src/init/<phase>.rs`.
2. Add to `init/mod.rs` in the right slot (see [`AppCore::init_with_sender`](#appcoreinit_with_sender--14-phases)).
3. Set fields on the `AppCore` struct (mark feature `Option<>` if it can be disabled).
4. **Order matters** — read existing phases for dependency hints.

### Adding an `AppCore` handler

1. Create the handler module under `crates/app-core/src/handlers/<domain>/`.
2. Annotate every public method with `#[tracing::instrument(skip(self), err)]` (project convention).
3. Add a Tauri command shim in `crates/desktop/src/commands/<domain>.rs` using `#[klynt_command]`.
4. Register the command in `klynt_collect_commands![...]`.

### Exposing a tool via MCP

Already covered in [`11-channels-mcp.md`](./11-channels-mcp.md). Briefly: add registry name to `default_exposed_tools()` (in `crates/config/src/schema/mcp.rs`) OR rely on `AiFeatureRegistry::tool_names()` auto-inclusion + `EXPLICIT_TOOL_ALLOWLIST`.

---

## Open questions & debt

- **CLAUDE.md is wrong about CSS.** Says "Plain CSS. No Tailwind." Actual: Tailwind is wired. Critical doc drift — anyone writing new components from CLAUDE.md guidance will skip Tailwind.
- **CLAUDE.md misses the `coding:{repo_id}` window.** 5 secondary windows, not 4.
- **`crates/desktop-ui/` stub naming.** Bindings file in a near-empty crate is confusing. Consider renaming to `desktop-bindings`.
- **OAuth callback port is fixed.** Port conflict = silent OAuth failure. Add fallback or document.
- **Legacy v1 event bridge still active** for assistant chat (`chatStreamStore.ts`). Plan + schedule the v2 migration cut for assistant.
- **`#[tracing::instrument]` convention** is enforced by code review only — no CI gate. Could add a lint.
- **`bindings.ts` byte-comparison** in `bindings_are_current` is brittle to whitespace changes. Consider semantic comparison.
- **No `/health` route on embedded MCP HTTP server.** `klyntbot://status` resource works but external monitors expect HTTP.
- **OAuth registry persistence shape** not surfaced anywhere in user-facing docs.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #5 (doc drift), #2 (stubs) for specifics.

---

## Cross-references

- [`01-foundations.md`](./01-foundations.md) — `MessageBus`, `DomainEventBus` consumed by `AppCore`
- [`02-storage.md`](./02-storage.md) — `Repos`, `StoragePool` consumed by `AppCore`
- [`04-agent-runtime.md`](./04-agent-runtime.md) — `AgentLoop` constructed in `init::ai_pipeline`
- [`09-coding-mode.md`](./09-coding-mode.md) — coding runtime, `coding:{repo_id}` window
- [`10-sandboxing-security.md`](./10-sandboxing-security.md) — `pre_main_hardening` must precede mimalloc
- [`11-channels-mcp.md`](./11-channels-mcp.md) — `klyntbot-server` exposes via stdio + HTTP
- [`crates/app-core.md`](../crates/app-core.md) — *(planned)* deep crate-level reference
- [`crates/desktop.md`](../crates/desktop.md) — *(planned)* deep crate-level reference
