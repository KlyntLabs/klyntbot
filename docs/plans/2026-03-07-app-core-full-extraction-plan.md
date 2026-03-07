# App-Core Full Extraction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract all ~116 handlers from desktop Tauri commands into a shared `app-core` crate, delete the standalone `dev-api` crate, and make desktop commands thin 2-3 line adapters.

**Architecture:** New `app-core` crate at L7 owns `AppCore` struct, initialization, and all handler methods. `desktop` becomes thin wrappers that delegate to `app-core` and emit Tauri events. The `dev_server.rs` dispatch also delegates to `app-core`. `dev-api` is deleted.

**Tech Stack:** Rust, Tauri 2, Axum (dev_server only), tokio, serde, SQLite

**Design doc:** `docs/plans/2026-03-07-app-core-full-extraction-design.md`

---

### Task 1: Create app-core crate skeleton

**Files:**
- Create: `crates/app-core/Cargo.toml`
- Create: `crates/app-core/src/lib.rs`
- Create: `crates/app-core/src/events.rs`
- Create: `crates/app-core/src/errors.rs`
- Create: `crates/app-core/src/handlers/mod.rs`
- Modify: `Cargo.toml` (workspace members + `[workspace.dependencies]`)

**Step 1: Create `Cargo.toml` for app-core**

```toml
[package]
name = "app-core"
version.workspace = true
edition.workspace = true

[dependencies]
desktop-shared = { workspace = true }
agent = { workspace = true }
bus = { workspace = true }
channels = { workspace = true }
cognitive = { workspace = true }
common = { workspace = true }
config = { workspace = true }
feature-coaching = { workspace = true }
feature-notes = { workspace = true }
feature-productivity = { workspace = true }
providers = { workspace = true }
scheduling = { workspace = true }
session = { workspace = true }
storage = { workspace = true }
dashmap = { workspace = true }
futures-util = { workspace = true }
tokio = { workspace = true }
tokio-util = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
```

**Step 2: Create `src/events.rs` — AppEventEmitter trait + NoopEmitter**

```rust
/// Transport-agnostic event emitter.
pub trait AppEventEmitter: Send + Sync + 'static {
    fn emit_event(&self, event_name: &str, payload: serde_json::Value);
}

/// No-op emitter for tests.
pub struct NoopEmitter;

impl AppEventEmitter for NoopEmitter {
    fn emit_event(&self, _event_name: &str, _payload: serde_json::Value) {}
}
```

**Step 3: Create `src/errors.rs` — shared error mapping functions**

Move from `desktop/src/commands/mod.rs`:
- `map_storage_err`
- `map_prod_err`
- `map_cognitive_err`
- `map_config_save_err`
- `parse_date` / `parse_date_or_err`

These are pure functions with no Tauri dependency.

**Step 4: Create `src/handlers/mod.rs` — empty, ready for handler modules**

**Step 5: Create `src/lib.rs` — re-exports**

```rust
pub mod errors;
pub mod events;
pub mod handlers;
```

**Step 6: Add `app-core` to workspace**

In root `Cargo.toml`:
- Add `"crates/app-core"` to `members`
- Add `app-core = { path = "crates/app-core" }` to `[workspace.dependencies]`

**Step 7: Verify**

Run: `cargo build -p app-core`
Expected: compiles with zero errors

**Step 8: Commit**

```
feat(app-core): create crate skeleton with AppEventEmitter trait
```

---

### Task 2: Move AppCore struct and accessors

**Files:**
- Create: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/lib.rs`
- Modify: `crates/desktop/Cargo.toml` — add `app-core` dependency
- Modify: `crates/desktop/src/app_core.rs` — re-export from app-core

**Step 1: Create `src/state.rs`**

Move the `AppCore` struct definition and all accessor methods (`productivity_repos()`, `focus_manager()`, `aggregator()`, `distraction_interceptor()`, `signal_accumulator()`, `pattern_detector()`, `intervention_router()`, `feedback_tracker()`, `user_situation()`, `domain_event_bus()`, `shutdown()`) from `desktop/src/app_core.rs`.

Remove all `tauri::` references. The struct itself has no Tauri dependencies — only `init()` and event forwarding used `tauri::AppHandle`.

Also move to state.rs:
- `EntityUpdate` struct (new type)
- `HandlerResult<T>` type alias (new)

```rust
use desktop_shared::types::EntityKind;

pub struct EntityUpdate {
    pub kind: EntityKind,
    pub id: String,
}

pub type HandlerResult<T> = Result<(T, Vec<EntityUpdate>), desktop_shared::errors::ApiError>;
```

**Step 2: Add `app-core` dep to desktop**

In `crates/desktop/Cargo.toml`:
```toml
app-core = { workspace = true }
```

**Step 3: Update desktop's `app_core.rs`**

Replace the struct definition with a re-export:
```rust
pub use app_core::AppCore;
```

Keep only the Tauri-specific code:
- The `init()` wrapper that creates `TauriEmitter` and wires `EventChannels`
- The event forwarding spawns (domain events → Tauri emit, pipeline events → Tauri emit, etc.)

**Step 4: Verify**

Run: `cargo build -p app-core -p desktop`
Expected: compiles

**Step 5: Commit**

```
refactor(app-core): move AppCore struct and accessors from desktop
```

---

### Task 3: Move initialization logic

**Files:**
- Create: `crates/app-core/src/init.rs`
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/lib.rs`
- Modify: `crates/desktop/src/app_core.rs`

**Step 1: Create `EventChannels` struct in `init.rs`**

```rust
pub struct EventChannels {
    pub inbound_rx: tokio::sync::mpsc::Receiver<bus::InboundMessage>,
    pub intervention_rx: tokio::sync::mpsc::Receiver<feature_coaching::router::DeliveredIntervention>,
    pub domain_event_bus: std::sync::Arc<bus::DomainEventBus>,
    pub pipeline_rx: tokio::sync::mpsc::UnboundedReceiver<cognitive::PipelineEvent>,
    pub auto_focus_rx: Option<tokio::sync::mpsc::Receiver<feature_productivity::AutoFocusSession>>,
    pub nudge_rx: Option<tokio::sync::mpsc::Receiver<feature_productivity::types::NudgeRecord>>,
    pub dashboard_tick_rx: Option<tokio::sync::broadcast::Receiver<feature_productivity::dashboard_emitter::DashboardTick>>,
}
```

**Step 2: Move `AppCore::init()` logic**

Move the entire initialization sequence from `desktop/src/app_core.rs` into `app-core/src/init.rs`. The key change: replace `app_handle: tauri::AppHandle` with returning `EventChannels` for the caller to wire.

Specifically, remove from init:
- `DashboardEmitter::start()` with the Tauri emit closure
- Auto-focus receiver Tauri emit spawn
- Nudge Tauri emit spawn
- Coaching intervention Tauri emit spawn
- Domain event Tauri emit spawn
- Pipeline event Tauri emit spawn

Instead, return these receiver channels in `EventChannels` so the caller does the wiring.

Also move: `register_cron_callbacks()`, `ensure_cron_jobs()`, `parse_time_to_cron()`, `spawn_background()`.

Signature:
```rust
impl AppCore {
    pub async fn init(config_override: Option<config::Config>) -> Result<(Self, EventChannels), String> { ... }
}
```

**Step 3: Update desktop's `app_core.rs`**

Thin wrapper that:
1. Calls `AppCore::init(None)`
2. Creates `TauriEmitter` from `app_handle`
3. Wires each channel from `EventChannels` to Tauri events via `tokio::spawn`

This is ~100 lines of event-forwarding glue instead of ~600 lines of init + forwarding.

**Step 4: Verify**

Run: `cargo build -p app-core -p desktop`
Run: `cargo nextest run -p desktop` (if tests exist)

**Step 5: Commit**

```
refactor(app-core): move initialization logic, return EventChannels
```

---

### Task 4: Move simple CRUD handlers — tasks, areas, projects, objectives, key results

**Files:**
- Create: `crates/app-core/src/handlers/tasks.rs`
- Create: `crates/app-core/src/handlers/areas.rs`
- Create: `crates/app-core/src/handlers/projects.rs`
- Create: `crates/app-core/src/handlers/objectives.rs`
- Create: `crates/app-core/src/handlers/key_results.rs`
- Modify: `crates/app-core/src/handlers/mod.rs`
- Modify: `crates/desktop/src/commands/tasks.rs`
- Modify: `crates/desktop/src/commands/areas.rs`
- Modify: `crates/desktop/src/commands/projects.rs`
- Modify: `crates/desktop/src/commands/objectives.rs`
- Modify: `crates/desktop/src/commands/key_results.rs`

**Step 1: Move tasks handlers**

Move from `desktop/src/commands/tasks.rs` to `app-core/src/handlers/tasks.rs`:
- Row-to-response converters: `priority_label`, `action_to_task`, `action_to_today_task`, `objective_to_response`, `kr_to_response`
- Helpers: `rows_to_tasks`, `row_to_task`
- Handler methods on `AppCore`:
  - `task_get(id) -> Result<Option<TaskResponse>, ApiError>`
  - `task_list(area_id, project_id, status) -> Result<Vec<TaskResponse>, ApiError>`
  - `task_create(params) -> HandlerResult<TaskResponse>`
  - `task_update(params) -> HandlerResult<TaskResponse>`
  - `task_delete(id) -> HandlerResult<bool>`
  - `task_toggle_complete(id) -> HandlerResult<TaskResponse>`
  - `task_list_children(parent_id) -> Result<Vec<TaskResponse>, ApiError>`
  - `today_tasks() -> Result<Vec<TodayTaskResponse>, ApiError>`
  - `project_list_for_tasks(area_id) -> Result<Vec<ProjectResponse>, ApiError>` (was `project_list` in tasks.rs)
  - `objective_list_for_tasks(project_id) -> Result<Vec<ObjectiveResponse>, ApiError>` (was `objective_list` in tasks.rs)

Mutating handlers return `HandlerResult<T>` with `EntityUpdate`s.
Read-only handlers return plain `Result<T, ApiError>`.

**Step 2: Simplify desktop tasks.rs**

Each Tauri command becomes a 2-4 line adapter:

```rust
#[tauri::command]
pub async fn task_create(
    state: State<'_, Arc<AppCore>>,
    app: tauri::AppHandle,
    params: TaskCreateParams,
) -> Result<TaskResponse, ApiError> {
    let (result, updates) = state.task_create(params).await?;
    emit_updates(&app, &updates);
    Ok(result)
}
```

Add a shared `emit_updates()` helper in `desktop/src/commands/mod.rs`.

**Step 3: Repeat for areas, projects, objectives, key_results**

Same pattern for each:
- Move business logic + converters to `app-core/src/handlers/<domain>.rs`
- Simplify desktop command to thin adapter

**Step 4: Verify**

Run: `cargo build -p app-core -p desktop`
Run: `cargo clippy --workspace`

**Step 5: Commit**

```
refactor(app-core): move task/area/project/objective/kr handlers
```

---

### Task 5: Move finance and status handlers

**Files:**
- Create: `crates/app-core/src/handlers/finance.rs`
- Create: `crates/app-core/src/handlers/status.rs`
- Modify: `crates/desktop/src/commands/finance.rs`
- Modify: `crates/desktop/src/commands/status.rs`

**Step 1: Move finance handlers**

These are mostly read-only repo delegates. Move all 9 commands:
- `finance_accounts`, `finance_transactions`, `finance_budget_usage`, `finance_portfolios`, `finance_investments`, `finance_goals`, `finance_liabilities`, `finance_net_worth`, `finance_exchange_rates`

`finance_net_worth` has the multi-currency aggregation logic — this is pure business logic, no Tauri dependency.

**Step 2: Move status handler**

`agent_status` — queries focused tasks + summary, pure logic.

**Step 3: Simplify desktop commands**

Each becomes a one-liner delegate.

**Step 4: Verify & Commit**

```
refactor(app-core): move finance and status handlers
```

---

### Task 6: Move notes handlers

**Files:**
- Create: `crates/app-core/src/handlers/notes.rs`
- Modify: `crates/desktop/src/commands/notes.rs`

**Step 1: Move notes handlers**

16 commands including converters. Move:
- `note_row_to_response`, `note_with_tags`, `notebook_row_to_response`, `notes_with_tags_batch`
- All 16 commands: `note_list`, `note_get`, `note_create`, `note_update`, `note_delete`, `note_search`, `note_links_all`, `note_list_by_entity`, `note_version_list`, `note_version_create`, `note_version_restore`, `note_save_attachment`, `notebook_list`, `notebook_create`, `notebook_update`, `notebook_delete`

Notable: `note_update` includes wiki-link extraction (`link_parser::extract_links`) and entity mention parsing. `note_version_restore` does snapshot-before-restore. `notebook_update` has cycle detection. `note_save_attachment` decodes base64 and writes to filesystem. All of this is pure business logic.

Mutating commands return `HandlerResult` with `EntityUpdate { kind: EntityKind::Note, id }` or `EntityKind::Notebook`.

**Step 2: Simplify desktop commands**

**Step 3: Verify & Commit**

```
refactor(app-core): move notes handlers
```

---

### Task 7: Move productivity and distraction handlers

**Files:**
- Create: `crates/app-core/src/handlers/productivity.rs`
- Create: `crates/app-core/src/handlers/distraction.rs`
- Modify: `crates/desktop/src/commands/productivity.rs`
- Modify: `crates/desktop/src/commands/distraction.rs`

**Step 1: Move productivity handlers**

18 commands + converters (`summary_to_response`, `session_to_response`, `event_to_timeline`, `insight_to_response`, `project_to_response`).

All delegate to `ProductivityRepos`, `FocusManager`, `DailyAggregator` — no Tauri deps.

**Step 2: Move distraction handlers**

3 commands: `distraction_dismiss`, `distraction_allow_temp`, `distraction_allow_session`, `distraction_learned_rules`, `distraction_delete_rule`.

Delegate to `DistractionInterceptor` — no Tauri deps.

**Step 3: Simplify desktop commands**

**Step 4: Verify & Commit**

```
refactor(app-core): move productivity and distraction handlers
```

---

### Task 8: Move cognitive and coaching handlers

**Files:**
- Create: `crates/app-core/src/handlers/cognitive.rs`
- Create: `crates/app-core/src/handlers/coaching.rs`
- Modify: `crates/desktop/src/commands/cognitive.rs`

**Step 1: Move cognitive handlers**

12 commands including converters (`fact_to_response`, `rule_to_response`, `fact_preview`):
- Reads: `cognitive_user_model`, `cognitive_facts_list`, `cognitive_episodic_list`, `cognitive_rules_list`, `cognitive_memory_stats`, `cognitive_system_status`
- Mutations: `cognitive_fact_create`, `cognitive_fact_update`, `cognitive_fact_delete`, `cognitive_rule_create`, `cognitive_rule_deactivate`, `cognitive_run_compaction`, `cognitive_inject_event`

**Step 2: Move coaching handlers**

6 commands:
- `coaching_situation`, `coaching_signals`, `coaching_patterns`, `coaching_feedback_stats`, `coaching_router_status`
- Mutations: `coaching_reset_dismissals`, `coaching_clear_signals`

**Step 3: Simplify desktop cognitive.rs**

20 commands → 20 one-liner delegates.

**Step 4: Verify & Commit**

```
refactor(app-core): move cognitive and coaching handlers
```

---

### Task 9: Move settings (MCP) handlers

**Files:**
- Create: `crates/app-core/src/handlers/settings.rs`
- Modify: `crates/desktop/src/commands/settings.rs`

**Step 1: Move MCP settings handlers**

5 commands: `mcp_get_config`, `mcp_add_server`, `mcp_remove_server`, `mcp_toggle_server`, `mcp_update_server`.

Also move helper functions from `desktop/src/commands/mod.rs`:
- `server_to_response`, `build_mcp_response`, `find_server_mut`, `build_transport`

These commands call `state.agent.reconnect_mcp_server()` / `disconnect_mcp_server()` — this is fine since `AppCore` holds `Arc<AgentLoop>`.

The pattern of "acquire config write lock → mutate → save → drop lock → call agent" stays the same but lives in `app-core`.

**Step 2: Simplify desktop settings.rs**

5 commands → 5 one-liner delegates.

**Step 3: Verify & Commit**

```
refactor(app-core): move MCP settings handlers
```

---

### Task 10: Move chat handlers (most complex)

**Files:**
- Create: `crates/app-core/src/handlers/chat.rs`
- Modify: `crates/desktop/src/commands/chat.rs`
- Modify: `crates/app-core/src/events.rs` (add relay_chat_stream)

**Step 1: Move simple chat handlers**

5 simple commands first:
- `chat_threads`, `chat_messages`, `chat_pin_thread`, `chat_rename_thread`, `chat_delete_thread`, `chat_respond_interaction`, `chat_cancel`

These are straightforward repo delegates. `chat_delete_thread` and `chat_cancel` interact with `active_streams` and `pending_interactions` DashMaps — these are already on `AppCore`.

Also move helpers: `format_interaction_summary`, `tool_domain`, `entity_kind_for_tool`, `is_mutating_action`, `entity_kind_for`, `auto_detect_context`, `resolve_ancestry`.

**Step 2: Move chat_send streaming**

This is the most complex handler (~500 lines). Strategy:

Create `chat_send()` on `AppCore` that:
1. Upserts session and context
2. Calls `agent.process_direct_streaming()`
3. Inserts cancel token into `active_streams`
4. Returns `ChatSendResult` containing the user message response

Create `relay_chat_stream()` as a standalone async function in `app-core/src/handlers/chat.rs`:

```rust
pub async fn relay_chat_stream(
    repos: Repos,
    session_key: String,
    active_streams: Arc<DashMap<String, CancellationToken>>,
    pending_interactions: Arc<DashMap<String, (String, oneshot::Sender<FormResponse>)>>,
    mut event_rx: mpsc::UnboundedReceiver<AgentEvent>,
    mut interaction_rx: mpsc::UnboundedReceiver<InteractionBundle>,
    emitter: Arc<dyn AppEventEmitter>,
    has_context: bool,
)
```

This function contains the entire `tokio::select!` loop, `StreamGuard`, `TransparencyData` accumulation, metadata persistence, and `auto_detect_context`. It calls `emitter.emit_event()` instead of `app.emit()`.

**Step 3: Desktop chat_send wrapper**

```rust
#[tauri::command]
pub async fn chat_send(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppCore>>,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
) -> Result<ChatMessageResponse, ApiError> {
    let (result, stream_info) = state.chat_send(content, session_key, context).await?;
    let emitter = Arc::new(TauriEmitter(app));
    tokio::spawn(app_core::handlers::chat::relay_chat_stream(
        state.repos.clone(),
        stream_info.session_key,
        state.active_streams.clone(),
        state.pending_interactions.clone(),
        stream_info.event_rx,
        stream_info.interaction_rx,
        emitter,
        stream_info.has_context,
    ));
    Ok(result)
}
```

**Step 4: Verify**

Run: `cargo build -p app-core -p desktop`
Manual test: `cargo tauri dev` → send a chat message → verify streaming works

**Step 5: Commit**

```
refactor(app-core): move chat handlers including streaming relay
```

---

### Task 11: Simplify dev_server.rs

**Files:**
- Modify: `crates/desktop/src/dev_server.rs`

**Step 1: Replace dispatch body with AppCore method calls**

The ~1,585-line dispatch function becomes ~200 lines. Each match arm becomes:

```rust
"task_list" => {
    let r = core.task_list(get(&body, "area_id"), get(&body, "project_id"), get(&body, "status")).await;
    match r { Ok(v) => ok(v), Err(e) => err(e) }
}
"task_create" => {
    let params = parse_params(&body)?;
    match core.task_create(params).await {
        Ok((v, _updates)) => ok(v),
        Err(e) => err(e),
    }
}
```

For `chat_send`, wire up SSE streaming using `SseEmitter` + `relay_chat_stream`.

**Step 2: Remove unused imports and helpers**

The `get`, `get_str`, `parse_params` helpers can stay (they handle JSON → typed param extraction). Remove all inlined business logic.

**Step 3: Verify**

Run: `cargo build -p desktop`
Manual test: `cargo run -p dev-api` ... wait, dev-api not deleted yet. Test via `cargo tauri dev` + open `localhost:1420` in Chrome.

**Step 4: Commit**

```
refactor(desktop): simplify dev_server dispatch to AppCore delegates
```

---

### Task 12: Delete dev-api crate

**Files:**
- Delete: `crates/dev-api/` (entire directory)
- Modify: `Cargo.toml` (remove from workspace members and dependencies)

**Step 1: Remove from workspace**

In root `Cargo.toml`:
- Remove `"crates/dev-api"` from `members`
- Remove `dev-api` from `[workspace.dependencies]` if present

**Step 2: Delete crate directory**

```bash
rm -rf crates/dev-api
```

**Step 3: Verify**

Run: `cargo build --workspace`
Run: `cargo clippy --workspace --all-targets --all-features`
Expected: zero errors, zero warnings

**Step 4: Commit**

```
chore: delete standalone dev-api crate (superseded by app-core)
```

---

### Task 13: Clean up desktop commands/mod.rs

**Files:**
- Modify: `crates/desktop/src/commands/mod.rs`

**Step 1: Slim down mod.rs**

After all handlers moved to `app-core`, the desktop `commands/mod.rs` should only contain:
- Module declarations
- `emit_updates()` helper (calls `emit_entity_updated` for each `EntityUpdate`)
- `emit_entity_updated()` (still Tauri-specific)

Remove any converter functions that were moved to `app-core`.

**Step 2: Verify & Commit**

```
refactor(desktop): clean up commands/mod.rs after extraction
```

---

### Task 14: Update CLAUDE.md and verify everything

**Files:**
- Modify: `CLAUDE.md`

**Step 1: Update CLAUDE.md**

- Update workspace layer docs to show `app-core` at L7
- Remove `dev-api` references from build commands and architecture
- Add `app-core` to the crate listing
- Update the "Browser-only dev" section to reference desktop dev_server instead of `cargo run -p dev-api`

**Step 2: Full verification**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo test --workspace --doc
cargo clippy --workspace --all-targets --all-features
cargo fmt --all --check
```

**Step 3: Manual smoke test**

1. `cargo tauri dev` → full desktop app works
2. Open `localhost:1420` in Chrome → dev server works
3. Test chat, tasks, notes, productivity, cognitive pages

**Step 4: Commit**

```
docs: update CLAUDE.md for app-core extraction, remove dev-api refs
```

---

## Execution Notes

- **Tasks 4-10 are independent** once Task 3 is complete. They can be parallelized.
- **Task 11 depends on Tasks 4-10** (needs all handlers available to delegate to).
- **Task 12 depends on Task 11** (dev-api can only be deleted after dev_server is updated).
- Each task should compile and pass `cargo clippy` before moving on.
- The `desktop` crate should work at every step — no broken intermediate states.
