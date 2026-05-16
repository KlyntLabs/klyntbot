# Crate: `app-core`

> **Status:** 🟢 Stable
> **Subsystem:** [13 — Desktop App & Frontend](../subsystems/13-desktop-frontend.md)
> **Status last verified:** 2026-05-16
> **One-liner:** The actual integration crate — owns `AppCore`, every handler, the init sequence, and both `ThreadRuntime` impls (transport-agnostic, NOT `klyntbot`)

---

## TL;DR

`app-core` is where everything below the desktop binary actually integrates. Owns the `AppCore` struct (~50 fields, transport-agnostic — no `tauri::*` types), the 14-phase `init_with_sender` sequence, ~40 handler domains under `handlers/`, both `ThreadRuntime` implementations (`AssistantThreadRuntime` + `CodingThreadRuntime`) sharing `ActiveTurns` + `StreamGuard`, the coding-mode subsystem (`coding/`, `coding_memory/`), three tracing providers (Claude Code, Kimi, Klynt), and all `init/` modules that build cron, cognitive, launcher, coaching, productivity, etc.

**`app-core` is the true integration crate, NOT `klyntbot`.** The root `klyntbot` facade re-exports types for convenience but contains no logic. Anywhere you'd say "in the app-core" probably means here, not the facade.

---

## Module map

```
crates/app-core/src/
├── lib.rs                          ← Re-exports + AppMode helpers + init()/init_with_sender()
├── desktop_approval_channel.rs     ← DesktopApprovalChannel (oneshot-based modal bridge)
├── wake_orchestrator.rs            ← WakeOrchestrator + WakeDeliveryConfig integration
├── focus_timer.rs                  ← FocusTimer state
├── claude_code_integration.rs      ← run_first_launch_check (idempotent MCP registration)
│
├── handlers/
│   ├── agents/        coding_jobs/        knowledge_health/   reforge/
│   ├── annotations/   coding_plan/        launcher/           retention_history/
│   ├── areas/         coding_todo/        morning_briefing/   review_stats/
│   ├── atoms/         cognitive/          notes/              settings/
│   ├── autotuner/     columns/            objectives/         status/
│   ├── capture/       cron/               productivity/       subagent/
│   ├── chat/          distraction/        project_*/          tasks/
│   ├── coaching/      entities/                               timeline/
│   ├── …                                                      view/
│   ├── …                                                      voice/
│   ├── …                                                      voice_conversation*/
│   ├── …                                                      voice_echo/
│   ├── …                                                      work_context/
│   ├── …                                                      workflows/
│   └── …                                                      workspace/
│
├── coding/                         ← Coding-mode handlers (turn_handler, approval, etc.)
│   ├── turn_handler.rs             ← Main coding turn lifecycle
│   ├── approval_handler.rs
│   ├── chat_send_routing.rs
│   ├── steer_queue.rs
│   ├── title_service.rs            ← TODO: LLM call (auto-title stub)
│   └── … (20 more)
│
├── coding_memory/                  ← Coding-memory installer + recall + reforge bridge
│   ├── codex_installer.rs
│   ├── git_hook_installer.rs
│   ├── handlers.rs
│   ├── installer.rs
│   ├── mirror.rs
│   ├── opencode_installer.rs
│   ├── panels_phase5.rs
│   ├── recall.rs
│   └── reforge.rs
│
├── runtime/                        ← ThreadRuntime trait + impls
│   ├── mod.rs                      ← ThreadRuntime trait, ActiveTurns, StreamGuard
│   ├── assistant.rs                ← AssistantThreadRuntime
│   └── coding.rs                   ← CodingThreadRuntime
│
├── init/                           ← 14-phase init sequence
│   ├── mod.rs                      ← orchestrator
│   ├── storage.rs                  ← Phase 2: SQLite + LanceDB
│   ├── channels.rs                 ← Phase 3: EventChannels
│   ├── ai_pipeline.rs              ← Phase 4: AiFeatureRegistry + AgentLoop + tools
│   ├── cognitive.rs                ← Phase 5: cognitive graphs + mirror facade
│   ├── cron.rs                     ← Phase 6: CronExecutor + CronBridge + TemporalScheduler
│   ├── launcher.rs                 ← Phase 7: LauncherSearchEngine + tool wiring (Path C)
│   ├── coaching.rs                 ← Phase 8: signal accumulator + intervention router
│   ├── temporal_scheduler.rs       ← Phase 9: spawn background fire loop
│   ├── coding_subscribers.rs       ← Phase 10a
│   ├── coding_recall.rs            ← Phase 10b
│   ├── coding_retention.rs         ← Phase 10c
│   ├── coding_skills.rs            ← Phase 10d
│   ├── productivity.rs             ← Phase 11
│   └── dnd.rs                      ← Phase 12: DND end subscriber
│
├── adapters/                       ← Trait impls (avoid circular deps)
│   ├── approval_suggester.rs
│   ├── autotuner_bridge.rs
│   ├── cognitive_accessor.rs
│   ├── cross_domain_searcher.rs
│   ├── flashcard_accessor.rs
│   ├── insight_embedder.rs
│   ├── scope_resolver.rs
│   └── trial_evaluator.rs
│
└── tracing/                        ← Provider-based session import
    ├── provider.rs                 ← TracingProvider trait + TracingRegistry
    ├── providers/
    │   ├── claude_code/            ← Claude Code session importer
    │   ├── kimi/                   ← Kimi session importer
    │   └── klynt/                  ← Klynt session importer (this app's own sessions)
    └── …                           ← cache, categorize, discovery, import, loader, etc.
```

---

## Public API surface

### `AppCore` struct (selected fields)

Transport-agnostic. **No `tauri::*` or `axum::*` types** in the struct itself.

```rust
pub struct AppCore {
    // Core infrastructure
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

    // Runtimes (lazy init)
    pub assistant_runtime: OnceLock<Arc<dyn ThreadRuntime>>,
    pub coding_runtime: OnceLock<Arc<dyn ThreadRuntime>>,

    // Coding mode
    pub thread_events: TypedBroker<ThreadEvent>,
    pub cost_events: TypedBroker<CostUpdate>,
    pub subagent_events: TypedBroker<SubagentEvent>,
    pub thread_subscriptions: Arc<DashMap<String, ThreadSubscription>>,
    pub steer_queue: Arc<SteerQueue>,
    pub tool_kit: Arc<RwLock<Option<Arc<ToolKitBuilder>>>>,
    pub desktop_approval_channel: Arc<DesktopApprovalChannel>,
    pub approval_grants_repo: Arc<GrantRepo>,
    pub coding_policies: Arc<RwLock<CodingPolicies>>,
    pub ingest_daemon: Option<Arc<IngestDaemon>>,
    pub distiller: Option<Arc<Distiller>>,
    pub recall: Option<Arc<CodingRecallService>>,
    pub coding_toolset: Option<Arc<CodingMemoryToolset>>,
    pub job_supervisor: Arc<dyn JobSupervisorHandle>,
    pub tracing_registry: Arc<TracingRegistry>,

    // Optional feature services (all Option<>)
    pub productivity: Option<Arc<ProductivityEngine>>,
    pub coaching: Option<Arc<CoachingService>>,
    pub cognitive: Option<Arc<CognitiveServices>>,
    pub mirror: Option<Arc<MirrorFacade>>,
    pub voice: Option<Arc<VoiceConversationManager>>,
    pub launcher: Option<Arc<LauncherSearchEngine>>,
    pub flashcard: Option<Arc<FlashcardService>>,
    pub knowledge_graph: Option<Arc<KnowledgeGraphService>>,
    pub autotuner: Option<Arc<AutotunerService>>,
    pub temporal_scheduler: Option<Arc<TemporalScheduler>>,
    pub notifications: Option<Arc<NotificationDispatcher>>,
    pub embedding: Option<Arc<dyn TextEmbedder>>,

    // Event emitter (transport-agnostic)
    pub event_emitter: Arc<dyn AppEventEmitter>,
}
```

### Init entry points

```rust
/// Top-level — used by the desktop binary.
pub async fn init(handle: tauri::AppHandle) -> Result<(
    Arc<AppCore>,
    mpsc::Sender<TauriEvent>,
    Arc<DesktopApprovalChannel>,
)>;

/// Lower-level — testable, used by MCP child + tests + dev server.
impl AppCore {
    pub async fn init_with_sender(
        mode: AppMode,
        config_override: Option<Config>,
        notification_sender: Option<Arc<dyn NotificationSender>>,
        event_emitter: Option<Arc<dyn AppEventEmitter>>,
        approval_channel: Option<Arc<dyn ApprovalChannel>>,
        provider_override: Option<DynProvider>,
    ) -> Result<(Self, EventChannels), String>;

    pub async fn shutdown(&self);
}
```

### `ThreadRuntime` trait

```rust
#[async_trait]
pub trait ThreadRuntime: Send + Sync {
    async fn start_turn(&self, req: StartTurnRequest) -> Result<StartTurnOutcome, ApiError>;
    async fn cancel_turn(&self, turn_id: &str) -> Result<()>;
    fn is_active(&self, thread_id: &str) -> bool;
    fn active_turns(&self) -> Vec<String>;
}

pub struct StartTurnParams {
    pub thread_id: String,
    pub session_key: SessionKey,
    pub message: String,
    pub generation: u32,
    // ...
}

pub struct TurnHandle {
    pub turn_id: String,
    pub cancel_token: CancellationToken,
}
```

Two concrete impls:
- `AssistantThreadRuntime` — assistant mode (`runtime/assistant.rs`)
- `CodingThreadRuntime` — coding mode (`runtime/coding.rs`)

Both share `ActiveTurns` + `StreamGuard`.

### `ActiveTurns` + `StreamGuard`

```rust
pub type ActiveTurns = Arc<DashMap<String, ActiveTurnEntry>>;

pub struct ActiveTurnEntry {
    pub guard_id: u64,                    // monotonic from STREAM_GUARD_COUNTER
    pub turn_id: String,
    pub cancel_token: CancellationToken,
    pub started_at: Timestamp,
}

pub struct StreamGuard {
    map: ActiveTurns,
    key: String,
    guard_id: u64,
    pending: Arc<DashMap<...>>,
}

impl Drop for StreamGuard {
    fn drop(&mut self) {
        // Value-identity removal: only delete if guard_id still matches.
        // Prevents a new turn from being cleaned up by an old guard's deferred drop.
        if let Some(entry) = self.map.get(&self.key) {
            if entry.guard_id == self.guard_id {
                self.map.remove(&self.key);
            }
        }
    }
}

static STREAM_GUARD_COUNTER: AtomicU64 = AtomicU64::new(0);
```

**The value-identity drop is load-bearing.** Without it, an old guard whose drop is deferred could clean up the active turn entry that a *new* turn just inserted.

### `DesktopApprovalChannel`

```rust
pub struct DesktopApprovalChannel {
    pending: DashMap<String, oneshot::Sender<ApprovalDecision>>,
    event_emitter: Arc<dyn AppEventEmitter>,
}

impl DesktopApprovalChannel {
    pub fn new(event_emitter: Arc<dyn AppEventEmitter>) -> Self;

    /// Resolve a pending approval — wakes the parked future.
    pub fn resolve(&self, request_id: &str, decision: ApprovalDecision);
}

#[async_trait]
impl ApprovalChannel for DesktopApprovalChannel {
    async fn request(&self, req: ApprovalRequest) -> Result<ApprovalDecision> {
        let id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();
        self.pending.insert(id.clone(), tx);
        self.event_emitter.emit("approval-requested", &json!({
            "request_id": id, "tool": req.tool, "class": req.class, ...
        }));
        tokio::time::timeout(Duration::from_secs(600), rx)
            .await
            .map(|r| r.unwrap_or(ApprovalDecision::Decline { reason: "channel closed".into() }))
            .map_err(|_| ApprovalDecision::Decline { reason: "timeout".into() })
    }
    // ...
}
```

`resolve` is called by the Tauri command `approval_respond` (or `approval_channel_respond` in some surfaces).

### `AppEventEmitter` (transport adapter trait)

```rust
#[async_trait]
pub trait AppEventEmitter: Send + Sync {
    async fn emit(&self, event: &str, payload: &serde_json::Value);
    async fn emit_to(&self, window: &str, event: &str, payload: &serde_json::Value);
}
```

Impls:
- Tauri adapter (in desktop crate) — emits via `app_handle.emit`
- `SocketBridgeEmitter` (in mcp-bridge) — serializes as `BridgeFrame` over Unix socket
- Dev HTTP emitter (in `desktop::dev_server`) — broadcasts to SSE clients

---

## Init sequence — 14 phases

`AppCore::init_with_sender` orchestrates via `init/mod.rs`. Order is load-bearing.

| # | Phase | Sets up | File |
|---:|---|---|---|
| 1 | Config | Load + env overrides + watcher | (in lib.rs) |
| 2 | Storage | SQLite pool + migrations + `Repos` + `VectorStore` | `init/storage.rs` |
| 3 | Channels | `EventChannels { intervention_rx, pipeline_rx }` | `init/channels.rs` |
| 4 | AI pipeline | `AiFeatureRegistry`, `AgentLoop`, tools | `init/ai_pipeline.rs` |
| 5 | Cognitive | cognitive graphs, semantic facts repo, embedding, mirror facade | `init/cognitive.rs` |
| 6 | Cron | `CronExecutor`, `CronBridge`, `TemporalScheduler` | `init/cron.rs` |
| 7 | Launcher | `LauncherSearchEngine` + tool registration (Path C) | `init/launcher.rs` |
| 8 | Coaching | signal accumulator, pattern detector, intervention router | `init/coaching.rs` |
| 9 | Temporal scheduler | spawn background fire loop | `init/temporal_scheduler.rs` |
| 10 | Coding | distiller + recall + retention + skills (4 sub-init modules) | `init/coding_*.rs` |
| 11 | Productivity | productivity engine, focus manager, DND manager | `init/productivity.rs` |
| 12 | DND | DND end subscriber | `init/dnd.rs` |
| 13 | Watchers | Config + data-version + lifecycle + wake orchestrator | (in lib.rs) |
| 14 | Voice | `VoiceService` + `VoiceConversationManager` | (in lib.rs) |

Most phases are `Option`-returning — features can be disabled via config.

---

## Handler architecture

`handlers/` has ~40 domain modules. Each module:
1. Owns `AppCore` methods that implement business logic
2. Is annotated with `#[tracing::instrument(skip(self), err)]` on every public method (project convention; enforced by code review)
3. Is **transport-agnostic** — returns domain types, not Tauri-specific types

### Handler module categories

| Category | Modules |
|---|---|
| Chat / Threads | `chat/{mod,sessions,streaming,thread_event_v2_translator,threads}` |
| Agents / Subagents | `agents`, `subagent` |
| Tasks / OKR | `tasks/`, `objectives`, `key_results`, `areas`, `projects`, `project_sources`, `project_conversations`, `project_memories` |
| Notes / Knowledge | `notes/`, `entities`, `entity_links`, `atoms`, `annotations`, `columns` |
| Productivity / Coaching | `productivity/`, `coaching`, `distraction`, `morning_briefing`, `voice`, `voice_conversation`, `voice_conversation_commands`, `voice_echo` |
| Finance | `finance/` |
| Cognitive / Reforge | `cognitive/`, `reforge`, `autotuner`, `retention_history`, `review_stats`, `knowledge_health` |
| Coding | `coding_jobs`, `coding_plan`, `coding_todo`, `coding/` *(separate subdir)*, `coding_memory/` *(separate subdir)* |
| Launcher | `launcher/` |
| Cron / Workflows | `cron`, `workflows` |
| Settings / Integrations | `settings/`, `integrations`, `capture`, `git`, `groups`, `fabric` |
| Workspace / Status | `workspace`, `status`, `view`, `timeline`, `work_context` |

### Tauri command shims (one layer up)

Desktop command shims in `crates/desktop/src/commands/` are *thin adapters* that delegate to `AppCore` handler methods:

```rust
#[klynt_command]
pub async fn task_create(state: State<Arc<AppCore>>, req: CreateTaskRequest)
    -> CommandResult<Task>
{
    state.task_create(req).await.map_err(ApiError::from)
}
```

**The trace span lives in the handler**, not in the Tauri shim. This is the convention enforced by code review.

### `coding/` directory (separate from `handlers/`)

| File | Purpose |
|---|---|
| `turn_handler.rs` | Main coding-turn lifecycle |
| `approval_handler.rs` | Per-tool approval orchestration |
| `chat_send_routing.rs` | Coding-thread message routing |
| `doctor_handler.rs` | Health check for coding mode |
| `help_handler.rs` | Help text emission |
| `mcp_handler.rs` | MCP-related coding actions |
| `metadata_handler.rs` | Coding metadata queries |
| `model_list_handler.rs` | Available coding models |
| `providers_handler.rs` | Provider selection |
| `recall_stats_handler.rs` | Recall telemetry (TODO at L33) |
| `resume_handler.rs` | Resume an interrupted session |
| `review_handler.rs` | Coding review flow |
| `review_prompt.rs` + `review_types.rs` | Review prompt builders + types |
| `sessions_handler.rs` | Coding session CRUD |
| `skills_handler.rs` | Coding-skill management |
| `status_handler.rs` | Coding-mode status |
| `steer_queue.rs` | Mid-stream steer queue |
| `subagent_handler.rs` | Coding subagent ops |
| `subscription.rs` | Thread subscription per Tauri connection |
| `thread_handler.rs` | Thread CRUD |
| `title_service.rs` | **🔴 TODO: LLM call** at L50 — auto-title stub |
| `workspace_handler.rs` | Workspace ops |
| `app_icon_handler.rs` | App icon lookup |

### `coding_memory/` directory

| File | Purpose |
|---|---|
| `codex_installer.rs` | Codex CLI integration installer (strips legacy hooks) |
| `git_hook_installer.rs` | Installs `.git/hooks/post-commit` for `git_post_commit` adapter |
| `handlers.rs` | Coding-memory handler methods on AppCore |
| `installer.rs` | Generic installer logic |
| `mirror.rs` | Mirror integration for coding-memory signals |
| `opencode_installer.rs` | Opencode installer |
| `panels_phase5.rs` | Phase 5 panel logic |
| `recall.rs` | Recall service wiring |
| `reforge.rs` | Coding-mode reforge phase hookup |

### `adapters/` directory

Trait implementations that avoid circular dependencies. Pattern: a lower-layer crate (e.g., `cognitive`) defines a trait (`AutotunerBridge`), and `app-core` provides the concrete impl that depends on both `cognitive` and `autotuner`.

| File | Implements |
|---|---|
| `approval_suggester.rs` | `approval::ApprovalSuggester` |
| `autotuner_bridge.rs` | `cognitive::reforge::AutotunerBridge` AND `cognitive::mirror::AutotunerBridge` (two distinct traits with same name) |
| `cognitive_accessor.rs` | Cognitive query trait |
| `cross_domain_searcher.rs` | Cross-domain search trait |
| `flashcard_accessor.rs` | Flashcard read access |
| `insight_embedder.rs` | Embeds insights for vector search |
| `scope_resolver.rs` | Resolves scope IDs (project / area / notebook) |
| `trial_evaluator.rs` | `cognitive::mirror::EarlyTrialEvaluator` |

### `tracing/` directory

Provider-based session import for Claude Code, Kimi, and Klynt agents.

| Component | Purpose |
|---|---|
| `provider.rs` | `TracingProvider` trait + `TracingRegistry` |
| `providers/claude_code/` | Claude Code session importer (cache + categorize + discovery + import + loader + stats + subagent_loader + summary) |
| `providers/kimi/` | Kimi session importer + context_loader + state_loader |
| `providers/klynt/` | Klynt agent's own session importer + context_loader + state_loader |

Each provider exposes:
- Session discovery (find session files on disk)
- Categorization (group by project / topic / time)
- Import (parse and store in coding-ingest event log)
- Summary (LLM-driven session summary)
- Subagent loader (extract subagent traces)

---

## Internals

### Why the `klyntbot` facade is not the integration crate

The root `klyntbot` crate (`src/lib.rs`) does `pub use crate_name;` for each workspace member (full re-exports) plus convenience type-level re-exports. **It has no logic.** `AppCore` is constructed in `app-core::init_with_sender`. Anyone who imports `klyntbot::AppCore` gets it via the re-export chain from `app_core`.

**`klyntbot::*` is for convenience; `app_core::AppCore` is the real thing.**

### `OnceLock` for runtimes

`assistant_runtime` and `coding_runtime` are `OnceLock<Arc<dyn ThreadRuntime>>`. They're constructed lazily on first use (after `init_with_sender` returns) because constructing them requires fully-initialized `AgentLoop` + `ThreadEvent` broker + `ApprovalGate` references.

Pattern:
```rust
let runtime = self.assistant_runtime.get_or_init(|| {
    Arc::new(AssistantThreadRuntime::new(/* deps */))
}).clone();
```

### `StreamGuard` Drop semantics

The Drop impl uses **value-identity comparison** (`guard_id == self.guard_id`). Why this matters:

```
T1: User sends message → guard_a inserted with guard_id=1
T2: Stream completes → guard_a drops → removes entry (guard_id 1 matches)
T3: User sends new message → guard_b inserted with guard_id=2

Without value identity:
T4: An OLD deferred drop fires (e.g., from a delayed Tokio task) → blindly removes entry
    → wipes guard_b's entry — turn now appears inactive but is still running.

With value identity:
T4: Old drop checks: stored guard_id is 2, but my guard_id is 1. Don't remove.
```

### `desktop_approval_channel` is bidirectional

The channel uses a `DashMap<request_id, oneshot::Sender>` plus a global event emitter:
1. `request()` inserts a pending entry, emits `approval-requested` event to frontend, parks on the `oneshot::Receiver`.
2. Frontend modal renders the request, user clicks "Allow Always."
3. Tauri command `approval_respond(request_id, AllowAlways)` calls `core.respond_approval`, which calls `DesktopApprovalChannel::resolve` to send `ApprovalDecision::Session` on the oneshot.
4. The `request()` future resumes with the decision.
5. 600s timeout on the await.

### `event_emitter` is the transport seam

`AppCore` doesn't know whether it's running in Tauri or as an MCP stdio child. The `event_emitter` adapter is injected at construction. This is what makes `AppCore` truly transport-agnostic.

### `tracing::instrument` convention

Every `AppCore` handler method has `#[tracing::instrument(skip(self), err)]`. The Tauri command shims in `crates/desktop/src/commands/` do NOT — they're too thin. The span lives one layer down.

### Multi-CLI installers

`coding_memory/{codex,opencode,git_hook}_installer.rs` write configuration to user dotfiles so the corresponding CLIs emit events Klynt can ingest:
- `codex_installer` — strips any legacy hook block from `~/.codex/config.toml` (writes nothing new since codex is poll-only)
- `opencode_installer` — opencode is poll-only; this is a no-op installer
- `git_hook_installer` — writes `.git/hooks/post-commit` script to invoke `klyntbot-hook git-post-commit`

---

## Workflows

### App startup (driven by desktop binary)

```
desktop::run_desktop_app::setup:
   1. app_core::init(handle) → blocks
   2. AppCore::init_with_sender:
      a-n. 14 init phases in order
   3. Returns (core, global_event_tx, approval_channel)
   4. Spawns claude_code_integration::run_first_launch_check
   5. Optionally starts dev_server / embedded MCP HTTP server
   6. app.manage(core); app.manage(approval_channel); …
```

### Assistant chat turn (handler-level)

```
desktop::commands::chat::chat_send (Tauri shim)
   → AppCore::chat_send(thread_id, message)
      → AppCore.assistant_runtime.get_or_init(…)
      → AssistantThreadRuntime::start_turn(params)
         → inserts ActiveTurnEntry with new guard_id
         → spawns task that:
            → AgentLoop::process_direct_streaming(content, session_key)
            → forwards AgentEvent → ThreadEvent v2 via translator
            → publishes to TypedBroker<ThreadEvent>
            → Tauri adapter emits "thread:event"
         → returns TurnHandle
   → returns CommandResult<TurnHandle>
```

### Coding turn (handler-level)

```
desktop::commands::coding::coding_thread_send (Tauri shim)
   → AppCore::coding_thread_send(thread_id, message)
      → AppCore.coding_runtime.get_or_init(…)
      → CodingThreadRuntime::start_turn(params)
         → app-core::coding::turn_handler::handle_turn
            → walks AGENTS.md tree via coding_agents_md
            → AgentLoop::process_direct_streaming with CODING_CHANNEL
            → klynt-core tools executed under sandbox + execpolicy
            → coding-ingest normalizes events → AgentEvent
            → Distiller buffers + fires distill_turn
         → emit ThreadEvent::Terminal on completion
```

### Approval flow (handler-level)

```
ApprovalGate::check(req) (in execute_loop)
   → DesktopApprovalChannel::request(req)
      → insert pending entry
      → event_emitter.emit("approval-requested", {request_id, …})
      → park on oneshot::Receiver (600s timeout)

(meanwhile in frontend modal)
   user clicks "Allow this session"
   → invoke("approval_respond", {request_id, decision: "session"})
      → AppCore::respond_approval(request_id, AllowSession)
         → DesktopApprovalChannel::resolve(request_id, ApprovalDecision::Session)
            → oneshot::Sender.send(decision)
   → the parked request() resolves with ApprovalDecision::Session
   → grant persisted to approval_grants
   → tool executes
```

### Cron job firing (handler-level)

```
TemporalScheduler ticks at fire_at
   → publishes DomainEvent::AlarmFired { kind="cron_job", ref_id=job_id }
   → CronExecutor (bus subscriber) filters kind == "cron_job"
   → cron_repo.find(job_id) → CronJobRow
   → registered CronHandler invoked via spawn_blocking
   → handler does its work (typically an AppCore method call)
```

---

## Testing approach

### `AppCore` in tests via `init_with_sender`

```rust
use app_core::{AppCore, AppMode};
use approval::BlockingFallbackChannel;
use bus::AppEventEmitter;

#[tokio::test]
async fn test_thing() {
    let temp = tempfile::tempdir().unwrap();
    let mut config = Config::default();
    config.data_dir = Some(temp.path().to_path_buf());

    let event_emitter = Arc::new(TestEmitter::new());
    let approval = Arc::new(BlockingFallbackChannel::new(/* ... */));

    let core = AppCore::init_with_sender(
        AppMode::Server,
        Some(config),
        None,
        Some(event_emitter),
        Some(approval),
        None,
    ).await.unwrap();

    // ... drive core via handler methods
}
```

`AppMode::Server` skips desktop-only init phases (channels, productivity).

### Mock `AppEventEmitter`

```rust
struct TestEmitter { events: Mutex<Vec<(String, Value)>> }

#[async_trait]
impl AppEventEmitter for TestEmitter {
    async fn emit(&self, event: &str, payload: &Value) {
        self.events.lock().await.push((event.into(), payload.clone()));
    }
    async fn emit_to(&self, _: &str, event: &str, payload: &Value) {
        self.emit(event, payload).await;
    }
}
```

### Mock `ApprovalChannel`

`BlockingFallbackChannel` always declines — good default for tests that shouldn't hit interactive flows. For tests that need to simulate user approval, write a `MockApprovalChannel` that returns canned decisions.

### Test handlers in isolation

Handlers are async methods on `AppCore`. Test by:
1. Constructing `AppCore` via `init_with_sender`
2. Calling the handler method directly
3. Asserting on the return value + side effects (DB rows, events)

No Tauri needed — that's why `AppCore` is transport-agnostic.

---

## Extension points

### Add a handler

1. Create `crates/app-core/src/handlers/<domain>/<file>.rs`.
2. Annotate every public method with `#[tracing::instrument(skip(self), err)]`.
3. Methods take `&self` and return `Result<T>`.
4. Add a Tauri command shim in `crates/desktop/src/commands/<domain>.rs` using `#[klynt_command]`.
5. Register the command in `klynt_collect_commands![...]`.
6. Run `cargo tauri dev` to regenerate `bindings.ts`.

### Add an init phase

1. Create `crates/app-core/src/init/<phase>.rs`.
2. Function signature: `pub async fn run(core: &mut AppCoreBuilder, …) -> Result<()>`.
3. Add to `init/mod.rs` in the correct slot (order matters — read existing phases for deps).
4. Set the corresponding field(s) on `AppCore`.
5. Mark the feature `Option<>` on `AppCore` if it can be disabled.

### Add a trait impl in `adapters/`

When a lower-layer crate (e.g., `cognitive`) defines a trait that needs to call into other crates (e.g., `autotuner`), implement it in `app-core::adapters` to avoid circular deps.

```rust
pub struct MyBridge { /* Arc<...> deps */ }

#[async_trait]
impl LowerCrate::SomeTrait for MyBridge {
    async fn do_something(&self, …) -> Result<…> {
        // calls into deps
    }
}
```

### Add an event-emitter consumer

If you need a new transport (e.g., WebSocket clients, gRPC stream), implement `AppEventEmitter` and inject it via `init_with_sender`.

### Add a tracing provider

```rust
// crates/app-core/src/tracing/providers/my_cli/mod.rs
#[async_trait]
impl TracingProvider for MyCliProvider {
    fn name(&self) -> &str { "my_cli" }
    async fn discover_sessions(&self, root: &Path) -> Vec<SessionInfo> { … }
    async fn import_session(&self, id: &str) -> Result<()> { … }
    // ...
}

// Register in TracingRegistry construction
```

### Add a `ThreadRuntime` impl

Implement the trait. Wire into `AppCore::init_with_sender` to set the appropriate `OnceLock`. Currently only assistant + coding exist; adding a third mode (e.g., "review-only") would follow this path.

---

## Open questions

- **`AppCore` has ~50 fields.** Could split into sub-structs (e.g., `AppCore { core: CoreServices, optional: OptionalServices }`). Cosmetic; defer.
- **`coding/` and `coding_memory/` are sibling top-level dirs** (not under `handlers/`). Inconsistent. Pick a structure.
- **`title_service.rs:50` has `// TODO: LLM call`** — auto-title stub. Implementation pending.
- **Multiple installers under `coding_memory/`** for codex / opencode / git-hook, but `codex_installer` is "strip-only" (no install). Naming misleading.
- **Two `AutotunerBridge` traits** with the same name implemented by the same adapter file. See [`TECH_DEBT.md`](../TECH_DEBT.md).
- **`tracing/` is huge** — three providers each with cache + categorize + discovery + import + loader + stats + subagent_loader + summary. Common abstractions could DRY this up.
- **`init/` has 14+ phases** with implicit ordering. Could formalize as a DAG with explicit deps.
- **`OnceLock` for runtimes** is a workaround for circular construction. Could refactor to inject runtimes into a separate phase.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #1 (TODOs) + #7 (anomalies) + #8 (naming).

---

## Cross-references

- [Subsystem 13 — Desktop App & Frontend](../subsystems/13-desktop-frontend.md) (parent)
- [`crates/agent.md`](./agent.md) — constructed in Phase 4
- [`crates/storage.md`](./storage.md) — `Repos` + `StoragePool` constructed in Phase 2
- [`crates/cognitive.md`](./cognitive.md) *(planned)* — services constructed in Phase 5
- [`crates/coding-memory.md`](./coding-memory.md) *(planned)* — distiller wired in Phase 10
- [`crates/desktop.md`](./desktop.md) *(planned)* — thin Tauri shell over this crate
