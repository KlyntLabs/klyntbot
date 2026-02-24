# Web Dashboard Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Build a web dashboard for Klyntbot — Axum REST/WebSocket API + embedded React SPA — integrated into `klyntbot serve`.

**Architecture:** New `dashboard` crate (Layer 4.5) with Axum. REST endpoints for CRUD, single WebSocket for streaming `AgentEvent`s. React frontend (Vite + Tailwind + Radix) embedded in release binary via `include_dir!`. Served from `klyntbot serve --port 18790`.

**Tech Stack:** Rust (Axum, tower-http, serde_json), React 19, React Router 7, Tailwind CSS v4, Radix UI, Motion, Recharts, Lucide icons.

**Design doc:** `docs/plans/2026-02-24-web-dashboard-design.md`

---

## Phase 1: Backend Foundation

### Task 1: Scaffold `dashboard` Crate

**Files:**
- Create: `crates/dashboard/Cargo.toml`
- Create: `crates/dashboard/src/lib.rs`
- Create: `crates/dashboard/src/state.rs`
- Create: `crates/dashboard/src/router.rs`
- Modify: `Cargo.toml` (workspace root — add member + dependency)

**Step 1: Create `Cargo.toml`**

```toml
[package]
name = "dashboard"
version.workspace = true
edition.workspace = true

[dependencies]
common = { path = "../common" }
config = { path = "../config" }
storage = { path = "../storage" }
agent = { path = "../agent" }
scheduling = { path = "../scheduling" }
tools-core = { path = "../tools-core" }

axum = { version = "0.8", features = ["ws", "macros"] }
tower-http = { version = "0.6", features = ["cors", "compression-gzip", "fs"] }
tokio = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
tracing = { workspace = true }
uuid = { workspace = true }
include_dir = { version = "0.7", features = ["glob"] }

[dev-dependencies]
reqwest = { version = "0.12", features = ["json"] }
tokio = { workspace = true, features = ["test-util"] }
```

**Step 2: Create `src/state.rs`**

```rust
//! Shared application state for all Axum handlers.

use agent::AgentLoop;
use config::Config;
use scheduling::CronService;
use std::sync::Arc;
use storage::Repos;
use tokio::sync::Mutex;

/// Shared state injected into every Axum handler via `axum::extract::State`.
#[derive(Clone)]
pub struct AppState {
    pub repos: Repos,
    pub agent_loop: Arc<Mutex<AgentLoop>>,
    pub cron_service: Arc<CronService>,
    pub config: Arc<Config>,
}
```

**Step 3: Create `src/router.rs`**

```rust
//! Axum router with all API routes.

use axum::{routing::get, Json, Router};
use crate::state::AppState;

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .with_state(state)
}

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "ok" }))
}
```

**Step 4: Create `src/lib.rs`**

```rust
//! Klyntbot Web Dashboard — Axum HTTP server.

pub mod router;
pub mod state;

use axum::Router;
use config::GatewayConfig;
use state::AppState;
use tokio::net::TcpListener;
use tracing::info;

pub struct DashboardServer {
    gateway: GatewayConfig,
    state: AppState,
}

impl DashboardServer {
    pub fn new(gateway: GatewayConfig, state: AppState) -> Self {
        Self { gateway, state }
    }

    pub async fn start(self) -> anyhow::Result<()> {
        let app = router::build(self.state);
        let addr = format!("{}:{}", self.gateway.host, self.gateway.port);
        let listener = TcpListener::bind(&addr).await?;
        info!("Dashboard listening on http://{}", addr);
        axum::serve(listener, app).await?;
        Ok(())
    }
}
```

**Step 5: Add to workspace `Cargo.toml`**

Add `"crates/dashboard"` to `[workspace] members` list.
Add `dashboard = { path = "crates/dashboard" }` to `[workspace.dependencies]`.

**Step 6: Write integration test**

Create: `crates/dashboard/tests/health.rs`

```rust
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn health_returns_ok() {
    // Build a minimal router without full state for this test
    let app = Router::new()
        .route("/api/health", axum::routing::get(|| async {
            axum::Json(serde_json::json!({ "status": "ok" }))
        }));

    let response = app
        .oneshot(Request::builder().uri("/api/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}
```

**Step 7: Build and test**

Run: `cargo build -p dashboard && cargo nextest run -p dashboard`
Expected: Build succeeds, health test passes.

**Step 8: Commit**

```bash
git add crates/dashboard/ Cargo.toml
git commit -m "feat(dashboard): scaffold crate with Axum health endpoint"
```

---

### Task 2: Wire Dashboard into `klyntbot serve`

**Files:**
- Modify: `crates/cli/Cargo.toml` (add dashboard dependency)
- Modify: `crates/cli/src/serve.rs`

**Step 1: Add dependency**

In `crates/cli/Cargo.toml`, add `dashboard = { path = "../dashboard" }` to `[dependencies]`.

**Step 2: Import and construct DashboardServer in `serve.rs`**

After the agent loop is built (~L490), before the shutdown signal block:

```rust
// Start dashboard web server
let dashboard_state = dashboard::state::AppState {
    repos: repos.clone(),
    agent_loop: agent_loop.clone(),
    cron_service: cron_service.clone(),
    config: Arc::new(config.clone()),
};
let dashboard = dashboard::DashboardServer::new(
    config.gateway.clone(),
    dashboard_state,
);
let dashboard_handle = tokio::spawn(async move {
    if let Err(e) = dashboard.start().await {
        error!("Dashboard server error: {}", e);
    }
});
```

Update the print block to show the dashboard URL:

```rust
println!("\n  Dashboard: http://{}:{}", config.gateway.host, config.gateway.port);
```

Add `dashboard_handle` to the shutdown join:

```rust
let _ = tokio::join!(agent_loop_handle, channel_manager_handle, dashboard_handle);
```

**Step 3: Build and verify**

Run: `cargo build -p cli`
Expected: Compiles without errors.

**Step 4: Commit**

```bash
git add crates/cli/
git commit -m "feat(serve): wire dashboard server into klyntbot serve"
```

---

### Task 3: Expose SkillManager + ToolRegistry from AgentLoop

The dashboard needs to list skills and tools. Currently `SkillManager` is consumed during build and `tool_registry` is `pub(crate)`. We need public accessors.

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs` (add public methods)
- Modify: `crates/agent/src/agent_loop/builder.rs` (store SkillManager on AgentLoop)
- Modify: `crates/agent/src/skills.rs` (if needed for listing methods)

**Step 1: Add `skill_manager` field to `AgentLoop`**

In `mod.rs`, add to the `AgentLoop` struct:

```rust
pub(crate) skill_manager: Arc<super::SkillManager>,
```

In `builder.rs`, during `build()`, after `skill_manager` is created and filtered (~L159-L171), store it on the `AgentLoop` struct:

```rust
skill_manager: Arc::clone(&skill_manager),
```

**Step 2: Add public accessor methods to `AgentLoop`**

In `mod.rs`, add:

```rust
/// List all registered tool names and their JSON schema definitions.
pub async fn list_tools(&self) -> Vec<serde_json::Value> {
    self.tool_registry.read().await.get_definitions()
}

/// List all tool names.
pub async fn tool_names(&self) -> Vec<String> {
    self.tool_registry.read().await.tool_names()
}

/// Get a reference to the skill manager for listing available skills.
pub fn skill_manager(&self) -> &super::SkillManager {
    &self.skill_manager
}
```

**Step 3: Add listing method to `SkillManager` if not present**

Check `skills.rs` for a method that returns all loaded skills with metadata (name, description, enabled, source). If not present, add:

```rust
/// Return metadata for all loaded skills.
pub fn list_skills(&self) -> Vec<SkillInfo> {
    self.skills.iter().map(|(name, skill)| SkillInfo {
        name: name.clone(),
        description: skill.description.clone(),
        source: skill.source.clone(),
        enabled: skill.enabled,
    }).collect()
}
```

**Step 4: Test**

Run: `cargo build --workspace && cargo nextest run -p agent`
Expected: All existing tests still pass, new methods are accessible.

**Step 5: Commit**

```bash
git add crates/agent/
git commit -m "feat(agent): expose SkillManager and ToolRegistry via public API"
```

---

### Task 4: AgentEvent JSON Serialization

The WebSocket protocol sends `AgentEvent` variants as typed JSON frames. Add `Serialize` derive and a tagged JSON format.

**Files:**
- Modify: `crates/agent/src/events.rs`
- Create: `crates/dashboard/src/ws.rs`
- Create: `crates/dashboard/src/api/mod.rs`
- Create: `crates/dashboard/src/api/chat.rs`
- Modify: `crates/dashboard/src/router.rs`

**Step 1: Add `Serialize` to `AgentEvent`**

In `events.rs`, add `serde::Serialize` to derives and add a `#[serde(tag = "type", rename_all = "camelCase")]` attribute. This produces JSON like `{"type": "contentChunk", "data": "..."}`. Define custom serialization names:

```rust
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AgentEvent {
    ContentChunk { data: String },
    ToolStart { name: String, args: serde_json::Value },
    ToolEnd { name: String, success: bool, duration_ms: u64 },
    IterationStart { iteration: usize, max: usize },
    ClassificationComplete { strategy: String, confidence: f32, source: String, duration_ms: u64 },
    ContextAssembled { total_tokens: usize, budget: usize, duration_ms: u64 },
    ExecutionStarted { engine: String, max_iterations: usize },
    Done { content: String },
    ConfidenceAssessed { score: f32, action: String },
    Error { message: String },
    PlanStepCompleted { plan_id: uuid::Uuid, step_index: usize, result: String },
    PlanCompleted { plan_id: uuid::Uuid, summary: String },
}
```

**Important:** This changes `ContentChunk(String)` to `ContentChunk { data: String }` and `Done(String)` to `Done { content: String }` and `Error(String)` to `Error { message: String }`. All sites that construct these variants need updating — grep for `AgentEvent::ContentChunk(`, `AgentEvent::Done(`, `AgentEvent::Error(` and update to the struct form. The CLI's `chat.rs` pattern matches on these — update pattern destructuring too.

**Step 2: Write serialization test**

In `events.rs`, add to `#[cfg(test)] mod tests`:

```rust
#[test]
fn test_agent_event_serializes_to_tagged_json() {
    let event = AgentEvent::ClassificationComplete {
        strategy: "tool_assisted".to_string(),
        confidence: 0.92,
        source: "classifier".to_string(),
        duration_ms: 42,
    };
    let json = serde_json::to_value(&event).unwrap();
    assert_eq!(json["type"], "classificationComplete");
    assert_eq!(json["confidence"], 0.92);
}
```

**Step 3: Run tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass after updating variant constructors.

**Step 4: Commit**

```bash
git add crates/agent/ crates/cli/
git commit -m "feat(events): add Serialize to AgentEvent for WebSocket JSON framing"
```

---

### Task 5: WebSocket Handler

**Files:**
- Create: `crates/dashboard/src/ws.rs`
- Modify: `crates/dashboard/src/router.rs`

**Step 1: Define WebSocket message types**

```rust
//! WebSocket handler for real-time agent streaming.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use common::prompts::FormResponse;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};
use tracing::{debug, error, warn};
use uuid::Uuid;

use crate::state::AppState;

/// Client → Server WebSocket messages.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    #[serde(rename = "chat.send")]
    ChatSend {
        #[serde(default)]
        session_key: Option<String>,
        message: String,
    },
    #[serde(rename = "interaction.respond")]
    InteractionRespond {
        request_id: String,
        response: FormResponse,
    },
    #[serde(rename = "chat.cancel")]
    ChatCancel,
}

/// Server → Client message wrapping interaction requests.
#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ServerMessage {
    #[serde(rename = "interaction.request")]
    InteractionRequest {
        request_id: String,
        #[serde(flatten)]
        request: common::prompts::InteractionRequest,
    },
}
```

**Step 2: Implement WebSocket upgrade handler**

```rust
pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut ws_tx, mut ws_rx) = socket.split();
    // Pending interaction responses: request_id → oneshot sender
    let pending_interactions: Arc<Mutex<HashMap<String, oneshot::Sender<FormResponse>>>> =
        Arc::new(Mutex::new(HashMap::new()));

    // ... main loop: read client messages, dispatch to agent, forward events
    // See design doc Section 3 for the full protocol flow.
}
```

**Step 3: Implement the main WebSocket loop**

Inside `handle_socket`, the core logic:

1. On `ChatSend`: generate a session key (or use provided), call `agent_loop.lock().await.process_direct_streaming()`, spawn two forwarder tasks (event_rx → ws_tx, interaction_rx → ws_tx + store oneshot in pending_interactions).
2. On `InteractionRespond`: pop the matching `request_id` from `pending_interactions`, send the `FormResponse` through the oneshot.
3. On `ChatCancel`: call `cancel_token.cancel()`.

Note: `process_direct_streaming` takes `self: &Arc<Self>`, but we have `Arc<Mutex<AgentLoop>>`. We need to either: (a) hold the lock briefly to get an `Arc<AgentLoop>` (which doesn't work because Mutex gives `&AgentLoop`), or (b) rethink the state to hold `Arc<AgentLoop>` directly.

**Key decision:** Change `AppState.agent_loop` from `Arc<Mutex<AgentLoop>>` to `Arc<AgentLoop>`. The `process_direct_streaming` method takes `self: &Arc<Self>` and spawns work internally — it doesn't need exclusive access. The `run()` method in serve mode is the only one that needs `&mut self`, but that's called once at startup. We may need to refactor `serve.rs` to call `run()` before wrapping in `Arc`, or change `run()` to take `&self` with internal mutability.

Investigate the `AgentLoop::run()` signature and determine the minimal change needed. If `run()` takes `&mut self`, the serve command can call `run()` on the owned value before sharing, or we keep the `Arc<Mutex<AgentLoop>>` in serve.rs but pass an `Arc<AgentLoop>` clone to the dashboard (since the dashboard only calls streaming methods).

Alternative: Create a `DashboardAgentHandle` that wraps just what the dashboard needs:

```rust
pub struct AgentHandle {
    inner: Arc<AgentLoopInner>, // or just expose process_direct_streaming differently
}
```

**Pragmatic approach:** Keep `Arc<Mutex<AgentLoop>>` in AppState. For `process_direct_streaming`, briefly lock → clone the inner Arc (if AgentLoop stores an Arc to itself) → release lock → call method. Or, refactor `process_direct_streaming` to not require `&Arc<Self>` — it can take `&self` and internally wrap in Arc for the spawn. Investigate the actual constraint and pick the simplest fix.

**Step 4: Wire WebSocket route**

In `router.rs`:

```rust
use crate::ws;

pub fn build(state: AppState) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .route("/ws", get(ws::ws_handler))
        .with_state(state)
}
```

**Step 5: Test**

Write a test that connects via WebSocket, sends a `chat.send` message, and receives at least one `AgentEvent` frame. This requires a running provider (or mock). Start with a connection test:

```rust
#[tokio::test]
async fn ws_upgrade_succeeds() {
    // Start server on random port, connect WebSocket, verify upgrade
}
```

**Step 6: Commit**

```bash
git add crates/dashboard/
git commit -m "feat(dashboard): WebSocket handler with AgentEvent streaming"
```

---

### Task 6: REST API — Tasks

**Files:**
- Create: `crates/dashboard/src/api/mod.rs`
- Create: `crates/dashboard/src/api/tasks.rs`
- Modify: `crates/dashboard/src/router.rs`

**Step 1: Define request/response types**

In `api/tasks.rs`:

```rust
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use storage::rows::todo::{TodoRow, TodoAttachmentRow, TodoTimeEntryRow};
use storage::repos::todo_repo::{TodoFilter, TodoPatch, TodoSummary};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskListQuery {
    pub status: Option<String>,
    pub project_id: Option<String>,
    pub priority_min: Option<i16>,
    pub limit: Option<i64>,
    pub tags: Option<String>,  // comma-separated
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i16>,
    pub due_date: Option<String>,  // ISO 8601
    pub tags: Option<Vec<String>>,
    pub project_id: Option<String>,
    pub parent_id: Option<String>,
    pub estimated_minutes: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<String>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub estimated_minutes: Option<Option<i32>>,
}
```

**Step 2: Implement handlers**

```rust
pub async fn list_tasks(
    State(state): State<AppState>,
    Query(query): Query<TaskListQuery>,
) -> Result<Json<Vec<TodoRow>>, StatusCode> {
    let filter = TodoFilter {
        status: query.status,
        tags: query.tags.map(|t| t.split(',').map(String::from).collect()),
        project_id: query.project_id,
        priority_min: query.priority_min,
        limit: query.limit,
        templates_only: false,
    };
    state.repos.todos.list(&filter).await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TodoRow>, StatusCode> {
    state.repos.todos.get(&id).await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
        .map(Json)
        .ok_or(StatusCode::NOT_FOUND)
}

pub async fn create_task(
    State(state): State<AppState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<(StatusCode, Json<TodoRow>), StatusCode> {
    // Build TodoRow from request, generate ID, set defaults
    // Call state.repos.todos.add(&row).await
    // Return (StatusCode::CREATED, Json(row))
    todo!()
}

pub async fn update_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(req): Json<UpdateTaskRequest>,
) -> Result<Json<TodoRow>, StatusCode> {
    // Build TodoPatch from request
    // Call state.repos.todos.update(&patch).await
    todo!()
}

pub async fn delete_task(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, StatusCode> {
    state.repos.todos.delete(&id).await
        .map(|deleted| if deleted { StatusCode::NO_CONTENT } else { StatusCode::NOT_FOUND })
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

pub async fn get_task_summary(
    State(state): State<AppState>,
) -> Result<Json<TodoSummary>, StatusCode> {
    state.repos.todos.summary().await
        .map(Json)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

// Sub-resource endpoints:
pub async fn list_subtasks(State(state): State<AppState>, Path(id): Path<String>) -> ... { ... }
pub async fn list_attachments(State(state): State<AppState>, Path(id): Path<String>) -> ... { ... }
pub async fn list_time_entries(State(state): State<AppState>, Path(id): Path<String>) -> ... { ... }
pub async fn add_time_entry(State(state): State<AppState>, Path(id): Path<String>, Json(req): ...) -> ... { ... }
pub async fn focus_task(State(state): State<AppState>, Path(id): Path<String>) -> ... { ... }
pub async fn unfocus_task(State(state): State<AppState>, Path(id): Path<String>) -> ... { ... }
```

**Step 3: Add routes**

In `router.rs`:

```rust
use crate::api::tasks;

// Inside build():
.route("/api/tasks", get(tasks::list_tasks).post(tasks::create_task))
.route("/api/tasks/summary", get(tasks::get_task_summary))
.route("/api/tasks/:id", get(tasks::get_task).patch(tasks::update_task).delete(tasks::delete_task))
.route("/api/tasks/:id/subtasks", get(tasks::list_subtasks))
.route("/api/tasks/:id/attachments", get(tasks::list_attachments))
.route("/api/tasks/:id/time-entries", get(tasks::list_time_entries).post(tasks::add_time_entry))
.route("/api/tasks/:id/focus", post(tasks::focus_task).delete(tasks::unfocus_task))
```

**Step 4: Add Serialize derives to storage row types**

Check if `TodoRow`, `TodoSummary`, `TodoAttachmentRow`, `TodoTimeEntryRow` derive `Serialize`. If not, add it. These live in `crates/storage/src/rows/todo.rs`.

**Step 5: Write integration test**

```rust
// crates/dashboard/tests/tasks_api.rs
#[tokio::test]
async fn crud_tasks() {
    let (app, _pool) = setup_test_app().await;  // helper that creates in-memory pool + AppState

    // POST /api/tasks
    let res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/api/tasks")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"title":"Test task"}"#))
            .unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);

    // GET /api/tasks
    let res = app.clone().oneshot(
        Request::builder().uri("/api/tasks").body(Body::empty()).unwrap()
    ).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    // Parse body, assert contains the task
}
```

**Step 6: Commit**

```bash
git add crates/dashboard/ crates/storage/
git commit -m "feat(dashboard): REST API for tasks CRUD"
```

---

### Task 7: REST API — Plans

**Files:**
- Create: `crates/dashboard/src/api/plans.rs`
- Modify: `crates/dashboard/src/router.rs`

**Step 1: Implement handlers**

Follow the same pattern as Task 6. Key endpoints:

```rust
// GET /api/plans?status=&session_key=
pub async fn list_plans(State(state): State<AppState>, Query(q): ...) -> ...
// GET /api/plans/:id (includes steps)
pub async fn get_plan(State(state): State<AppState>, Path(id): Path<Uuid>) -> ...
// POST /api/plans
pub async fn create_plan(State(state): State<AppState>, Json(req): ...) -> ...
// PATCH /api/plans/:id
pub async fn update_plan(State(state): State<AppState>, Path(id): Path<Uuid>, Json(req): ...) -> ...
// PATCH /api/plans/:id/status  (approve, abandon, etc.)
pub async fn update_plan_status(State(state): State<AppState>, Path(id): Path<Uuid>, Json(req): ...) -> ...
// GET /api/plans/:id/steps
pub async fn list_plan_steps(State(state): State<AppState>, Path(id): Path<Uuid>) -> ...
```

Use `PlanRepo.get(id)`, `PlanRepo.list(status, session_key, goal_id)`, `PlanRepo.get_steps(plan_id)`.

**Step 2: Add Serialize to `PlanRow` and `PlanStepRow`**

In `crates/storage/src/rows/plan.rs`, ensure both structs derive `Serialize`.

**Step 3: Add routes, write test, commit**

```bash
git commit -m "feat(dashboard): REST API for plans CRUD"
```

---

### Task 8: REST API — Sessions & Status

**Files:**
- Create: `crates/dashboard/src/api/sessions.rs`
- Create: `crates/dashboard/src/api/status.rs`
- Modify: `crates/dashboard/src/router.rs`

**Step 1: Sessions endpoints**

```rust
// GET /api/sessions — list all sessions
pub async fn list_sessions(State(state): State<AppState>) -> Result<Json<Vec<SessionListRow>>, _> {
    state.repos.sessions.list_sessions().await.map(Json).map_err(...)
}

// GET /api/sessions/:key — get session with recent messages
pub async fn get_session(State(state): State<AppState>, Path(key): Path<String>) -> ... {
    let session = state.repos.sessions.get_session(&key).await?;
    let messages = state.repos.sessions.get_messages(&key).await?;
    // Return combined response
}

// DELETE /api/sessions/:key
pub async fn delete_session(State(state): State<AppState>, Path(key): Path<String>) -> ...
```

**Step 2: Status endpoint**

```rust
// GET /api/status
pub async fn get_status(State(state): State<AppState>) -> Json<StatusResponse> {
    Json(StatusResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        model: state.config.agents.defaults.model.clone(),
        // Add more: uptime, storage stats, provider name, etc.
    })
}
```

**Step 3: Add routes, test, commit**

```bash
git commit -m "feat(dashboard): REST API for sessions and status"
```

---

### Task 9: REST API — Cron Jobs

**Files:**
- Create: `crates/dashboard/src/api/cron.rs`
- Modify: `crates/dashboard/src/router.rs`

**Step 1: Implement handlers**

```rust
// GET /api/cron — list all cron jobs
pub async fn list_cron_jobs(State(state): State<AppState>) -> ... {
    let jobs = state.cron_service.list_jobs(true).await; // include disabled
    Json(jobs)
}

// POST /api/cron — create a new cron job
pub async fn create_cron_job(State(state): State<AppState>, Json(req): ...) -> ...

// PATCH /api/cron/:id/enable — toggle enabled/disabled
pub async fn toggle_cron_job(State(state): State<AppState>, Path(id): Path<String>, Json(req): ...) -> ... {
    state.cron_service.enable_job(&id, req.enabled).await?;
}

// DELETE /api/cron/:id
pub async fn delete_cron_job(State(state): State<AppState>, Path(id): Path<String>) -> ... {
    state.cron_service.remove_job(&id).await?;
}
```

**Step 2: Ensure `CronJob` derives `Serialize`**

Check `scheduling/src/types.rs`. `CronJob`, `CronSchedule`, `CronJobState` need `Serialize`.

**Step 3: Routes, test, commit**

```bash
git commit -m "feat(dashboard): REST API for cron job management"
```

---

### Task 10: REST API — Calendar

**Files:**
- Create: `crates/dashboard/src/api/calendar.rs`
- Modify: `crates/dashboard/src/router.rs`

**Step 1: Implement handlers**

```rust
// GET /api/calendar/events?provider_id=&limit=
pub async fn list_events(State(state): State<AppState>, Query(q): ...) -> ... {
    if let Some(provider_id) = q.provider_id {
        state.repos.calendar_event_cache.list_by_provider(&provider_id).await
    } else {
        state.repos.calendar_event_cache.list_upcoming(q.limit.unwrap_or(50)).await
    }
}

// GET /api/calendar/sync-status
pub async fn sync_status(State(state): State<AppState>) -> ... {
    state.repos.calendar_sync.list().await
}

// POST /api/calendar/sync — trigger immediate sync
pub async fn trigger_sync(State(state): State<AppState>) -> ... {
    // Publish a calendar_sync message through the bus, or call sync directly
    // This needs access to the calendar sync adapter — may need to add to AppState
}
```

**Step 2: Ensure `CalendarEventCacheRow` and `CalendarSyncStateRow` derive `Serialize`**

**Step 3: Routes, test, commit**

```bash
git commit -m "feat(dashboard): REST API for calendar events and sync"
```

---

### Task 11: REST API — Skills

**Files:**
- Create: `crates/dashboard/src/api/skills.rs`
- Modify: `crates/dashboard/src/router.rs`
- Modify: `crates/dashboard/src/state.rs` (add SkillManager to AppState)

**Step 1: Add SkillManager to AppState**

```rust
pub struct AppState {
    // ... existing fields
    pub skill_manager: Arc<agent::SkillManager>,
}
```

Update `serve.rs` to pass the skill manager. Since `SkillManager` is now stored on `AgentLoop` (from Task 3), extract it:

```rust
let skill_manager = agent_loop.lock().await.skill_manager().clone(); // Arc clone
```

**Step 2: Implement handlers**

```rust
// GET /api/skills
pub async fn list_skills(State(state): State<AppState>) -> Json<Vec<SkillInfo>> {
    Json(state.skill_manager.list_skills())
}

// PATCH /api/skills/:name — enable/disable
pub async fn toggle_skill(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ToggleSkillRequest>,
) -> ... {
    // Update skill enabled state
    // Persist to config.packs.enabled_skills
}
```

**Step 3: Routes, test, commit**

```bash
git commit -m "feat(dashboard): REST API for skills listing"
```

---

### Task 12: REST API — Finance

**Files:**
- Create: `crates/dashboard/src/api/finance.rs`
- Modify: `crates/dashboard/src/router.rs`

**Step 1: Implement handlers for all finance sub-resources**

This is the largest API surface. Follow the pattern from Task 6 for each:

```rust
// Accounts
// GET /api/finance/accounts
// POST /api/finance/accounts
// GET /api/finance/accounts/:id
// PATCH /api/finance/accounts/:id
// DELETE /api/finance/accounts/:id

// Transactions
// GET /api/finance/transactions?from=&to=&category=&type=&account_id=
// POST /api/finance/transactions
// GET /api/finance/transactions/:id
// PATCH /api/finance/transactions/:id
// DELETE /api/finance/transactions/:id
// GET /api/finance/transactions/summary?from=&to=  (category sums)

// Budgets
// GET /api/finance/budgets
// POST /api/finance/budgets
// PATCH /api/finance/budgets/:id
// DELETE /api/finance/budgets/:id
// GET /api/finance/budgets/usage  (all budget usage in one call)

// Investments
// GET /api/finance/portfolios
// POST /api/finance/portfolios
// GET /api/finance/portfolios/:id/summary
// GET /api/finance/investments?portfolio_id=
// POST /api/finance/investments
// PATCH /api/finance/investments/:id
// DELETE /api/finance/investments/:id

// Goals
// GET /api/finance/goals
// POST /api/finance/goals
// PATCH /api/finance/goals/:id
// DELETE /api/finance/goals/:id

// Liabilities
// GET /api/finance/liabilities
// POST /api/finance/liabilities
// PATCH /api/finance/liabilities/:id
// DELETE /api/finance/liabilities/:id
```

**Step 2: Ensure all finance row types derive `Serialize`**

Check `crates/storage/src/rows/finance.rs`. Add `Serialize` to: `FinanceAccountRow`, `FinanceTransactionRow`, `FinanceBudgetRow`, `BudgetUsageRow`, `FinanceInvestmentRow`, `FinancePortfolioRow`, `PortfolioSummaryRow`, `FinanceGoalRow`, `FinanceLiabilityRow`.

**Step 3: Routes — prefix all under `/api/finance/`**

**Step 4: Write integration tests for at least accounts CRUD**

**Step 5: Commit**

```bash
git commit -m "feat(dashboard): REST API for finance (accounts, transactions, budgets, investments, goals, liabilities)"
```

---

### Task 13: REST API — Settings (with Secret Redaction)

**Files:**
- Create: `crates/dashboard/src/api/settings.rs`
- Modify: `crates/dashboard/src/router.rs`

**CRITICAL:** `Secret<String>` is `#[serde(transparent)]` — serializing `Config` directly exposes all API keys. We MUST redact before returning.

**Step 1: Define a redaction layer**

```rust
use serde_json::Value;

/// Redact known secret fields in a JSON value.
/// Replaces secret values with "••••••" if they are non-empty, or null if empty.
fn redact_secrets(value: &mut Value) {
    // Known secret field names (camelCase as serialized)
    const SECRET_FIELDS: &[&str] = &[
        "apiKey", "token", "botToken", "appToken",
        "imapPassword", "smtpPassword", "secret",
        "appSecret", "encryptKey", "clientSecret",
        "clawToken", "braveApiKey", "password",
        "accessToken", "refreshToken",
    ];

    match value {
        Value::Object(map) => {
            for (key, val) in map.iter_mut() {
                if SECRET_FIELDS.contains(&key.as_str()) {
                    if val.is_string() && !val.as_str().unwrap_or("").is_empty() {
                        *val = Value::String("••••••".to_string());
                    }
                } else {
                    redact_secrets(val);
                }
            }
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                redact_secrets(item);
            }
        }
        _ => {}
    }
}
```

**Step 2: Implement handlers**

```rust
// GET /api/settings — returns full config with secrets redacted
pub async fn get_settings(State(state): State<AppState>) -> Json<Value> {
    let mut value = serde_json::to_value(state.config.as_ref()).unwrap();
    redact_secrets(&mut value);
    Json(value)
}

// GET /api/settings/:section — returns one section (e.g., "agents", "todo", "finance")
pub async fn get_settings_section(
    State(state): State<AppState>,
    Path(section): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let mut value = serde_json::to_value(state.config.as_ref()).unwrap();
    redact_secrets(&mut value);
    value.get(&section).cloned().map(Json).ok_or(StatusCode::NOT_FOUND)
}

// PATCH /api/settings/:section — merge-patch a section
pub async fn update_settings_section(
    State(state): State<AppState>,
    Path(section): Path<String>,
    Json(patch): Json<Value>,
) -> Result<Json<Value>, StatusCode> {
    // 1. Load current config from disk (not from memory — it may have changed)
    let mut config = config::load().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 2. Serialize, merge the section, deserialize back
    let mut full = serde_json::to_value(&config).unwrap();
    if let Some(section_val) = full.get_mut(&section) {
        merge_json(section_val, &patch);
    } else {
        return Err(StatusCode::NOT_FOUND);
    }
    config = serde_json::from_value(full).map_err(|_| StatusCode::BAD_REQUEST)?;

    // 3. Save to disk
    config::save(&config).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    // 4. Return the updated section (redacted)
    let mut value = serde_json::to_value(&config).unwrap();
    redact_secrets(&mut value);
    Ok(Json(value.get(&section).cloned().unwrap()))
}

/// JSON merge-patch (RFC 7396).
fn merge_json(target: &mut Value, patch: &Value) {
    if let (Value::Object(target_map), Value::Object(patch_map)) = (target, patch) {
        for (key, value) in patch_map {
            if value.is_null() {
                target_map.remove(key);
            } else if let Some(existing) = target_map.get_mut(key) {
                merge_json(existing, value);
            } else {
                target_map.insert(key.clone(), value.clone());
            }
        }
    } else {
        *target = patch.clone();
    }
}
```

**Step 3: Write test for secret redaction**

```rust
#[test]
fn test_redact_secrets() {
    let mut value = serde_json::json!({
        "providers": {
            "anthropic": { "apiKey": "sk-secret-123", "apiBase": "https://api.anthropic.com" }
        }
    });
    redact_secrets(&mut value);
    assert_eq!(value["providers"]["anthropic"]["apiKey"], "••••••");
    assert_eq!(value["providers"]["anthropic"]["apiBase"], "https://api.anthropic.com");
}
```

**Step 4: Routes**

```rust
.route("/api/settings", get(settings::get_settings))
.route("/api/settings/:section", get(settings::get_settings_section).patch(settings::update_settings_section))
```

**Step 5: Commit**

```bash
git commit -m "feat(dashboard): settings API with secret redaction"
```

---

### Task 14: CORS Middleware + Error Handling

**Files:**
- Modify: `crates/dashboard/src/router.rs`
- Create: `crates/dashboard/src/error.rs`

**Step 1: Add CORS for development**

```rust
use tower_http::cors::{CorsLayer, Any};

pub fn build(state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        // ... all routes
        .layer(cors)
        .with_state(state)
}
```

**Step 2: Create consistent error response type**

```rust
// error.rs
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;

pub struct ApiError {
    pub status: StatusCode,
    pub message: String,
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = serde_json::json!({ "error": self.message });
        (self.status, Json(body)).into_response()
    }
}

impl From<common::KlyntbotError> for ApiError {
    fn from(err: common::KlyntbotError) -> Self {
        match &err {
            common::KlyntbotError::NotFound(_) => ApiError {
                status: StatusCode::NOT_FOUND,
                message: err.to_string(),
            },
            _ => ApiError {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                message: err.to_string(),
            },
        }
    }
}
```

**Step 3: Commit**

```bash
git commit -m "feat(dashboard): CORS middleware and consistent error handling"
```

---

## Phase 2: Frontend Foundation

### Task 15: Scaffold React + Vite + Tailwind

**Files:**
- Create: `crates/dashboard/frontend/package.json`
- Create: `crates/dashboard/frontend/vite.config.ts`
- Create: `crates/dashboard/frontend/tsconfig.json`
- Create: `crates/dashboard/frontend/index.html`
- Create: `crates/dashboard/frontend/src/main.tsx`
- Create: `crates/dashboard/frontend/src/styles/theme.css`
- Create: `crates/dashboard/frontend/.gitignore`

**Step 1: Initialize project**

```bash
cd crates/dashboard/frontend
npm init -y
npm install react react-dom react-router@7
npm install -D vite @vitejs/plugin-react typescript @types/react @types/react-dom
npm install tailwindcss@4 @tailwindcss/vite
npm install @radix-ui/react-dialog @radix-ui/react-dropdown-menu @radix-ui/react-tooltip @radix-ui/react-popover @radix-ui/react-tabs @radix-ui/react-toggle @radix-ui/react-select
npm install lucide-react motion recharts
npm install clsx
```

**Step 2: Create `vite.config.ts`**

```typescript
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import tailwindcss from '@tailwindcss/vite';

export default defineConfig({
  plugins: [react(), tailwindcss()],
  server: {
    port: 5173,
    proxy: {
      '/api': 'http://localhost:18790',
      '/ws': { target: 'ws://localhost:18790', ws: true },
    },
  },
  build: {
    outDir: 'dist',
  },
});
```

**Step 3: Create `index.html`**

```html
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Klyntbot</title>
  <link rel="preconnect" href="https://fonts.googleapis.com" />
  <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin />
  <link href="https://fonts.googleapis.com/css2?family=Inter:wght@300;400;500;600;700&family=JetBrains+Mono:wght@400;500&display=swap" rel="stylesheet" />
</head>
<body>
  <div id="root"></div>
  <script type="module" src="/src/main.tsx"></script>
</body>
</html>
```

**Step 4: Create `src/styles/theme.css`**

Adapted from the Figma Make `theme.css`:

```css
@import "tailwindcss";

@theme {
  --color-codex-bg: #0d0d0d;
  --color-codex-surface: #1a1a1a;
  --color-codex-surface-hover: #242424;
  --color-codex-border: #2a2a2a;
  --color-codex-border-subtle: #1f1f1f;
  --color-codex-text: #e5e5e5;
  --color-codex-text-secondary: #999999;
  --color-codex-text-tertiary: #666666;
  --color-codex-accent: #10a37f;
  --color-codex-accent-hover: #0d8c6d;
  --color-codex-accent-subtle: rgba(16, 163, 127, 0.1);
  --color-codex-danger: #ef4444;
  --color-codex-warning: #f59e0b;
  --color-codex-info: #3b82f6;

  --font-sans: 'Inter', system-ui, sans-serif;
  --font-mono: 'JetBrains Mono', monospace;
}
```

**Step 5: Create `src/main.tsx`**

```tsx
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './styles/theme.css';
import App from './app/App';

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>
);
```

**Step 6: Create minimal `src/app/App.tsx`**

```tsx
import { createBrowserRouter, RouterProvider } from 'react-router';
import { routes } from './routes';

const router = createBrowserRouter(routes);

export default function App() {
  return <RouterProvider router={router} />;
}
```

**Step 7: Verify**

```bash
cd crates/dashboard/frontend && npm run dev
```
Expected: Vite dev server starts on `:5173`, blank page loads without errors.

**Step 8: Commit**

```bash
git add crates/dashboard/frontend/
git commit -m "feat(frontend): scaffold React + Vite + Tailwind with Codex theme"
```

---

### Task 16: Layout Shell + Routing

**Files:**
- Create: `crates/dashboard/frontend/src/app/routes.tsx`
- Create: `crates/dashboard/frontend/src/app/components/Layout.tsx`
- Create: `crates/dashboard/frontend/src/app/pages/Chat.tsx` (placeholder)
- Create all other page placeholders

**Step 1: Create `Layout.tsx`**

48px left nav rail, content area, bottom status bar. Adapted from Figma (without macOS traffic lights):

```tsx
import { NavLink, Outlet } from 'react-router';
import { MessageSquare, CheckSquare, Map, Calendar, Clock, Zap, BarChart3, Settings } from 'lucide-react';

const navItems = [
  { to: '/', icon: MessageSquare, label: 'Chat' },
  { to: '/tasks', icon: CheckSquare, label: 'Tasks' },
  { to: '/plans', icon: Map, label: 'Plans' },
  { to: '/calendar', icon: Calendar, label: 'Calendar' },
  { to: '/cron', icon: Clock, label: 'Cron' },
  { to: '/skills', icon: Zap, label: 'Skills' },
  { to: '/finance', icon: BarChart3, label: 'Finance' },
];

export default function Layout() {
  return (
    <div className="flex h-screen bg-codex-bg text-codex-text font-sans">
      {/* Left nav rail */}
      <nav className="w-12 flex flex-col items-center py-3 gap-1 border-r border-codex-border">
        {navItems.map(({ to, icon: Icon, label }) => (
          <NavLink
            key={to}
            to={to}
            className={({ isActive }) =>
              `w-9 h-9 flex items-center justify-center rounded-lg transition-colors
               ${isActive ? 'bg-codex-accent-subtle text-codex-accent' : 'text-codex-text-tertiary hover:text-codex-text-secondary hover:bg-codex-surface-hover'}`
            }
            title={label}
          >
            <Icon size={18} />
          </NavLink>
        ))}
        <div className="flex-1" />
        <NavLink
          to="/settings"
          className={({ isActive }) =>
            `w-9 h-9 flex items-center justify-center rounded-lg transition-colors
             ${isActive ? 'bg-codex-accent-subtle text-codex-accent' : 'text-codex-text-tertiary hover:text-codex-text-secondary hover:bg-codex-surface-hover'}`
          }
          title="Settings"
        >
          <Settings size={18} />
        </NavLink>
      </nav>

      {/* Main content */}
      <main className="flex-1 flex flex-col overflow-hidden">
        <Outlet />
      </main>
    </div>
  );
}
```

**Step 2: Create placeholder pages**

For each page (Chat, Tasks, TaskDetail, Plans, Calendar, Cron, Skills, Finance, Settings, Setup), create a minimal component:

```tsx
// src/app/pages/Chat.tsx
export default function Chat() {
  return <div className="flex-1 flex items-center justify-center text-codex-text-secondary">Chat — coming soon</div>;
}
```

**Step 3: Create `routes.tsx`**

```tsx
import type { RouteObject } from 'react-router';
import Layout from './components/Layout';
import Chat from './pages/Chat';
import Tasks from './pages/Tasks';
import TaskDetail from './pages/TaskDetail';
import Plans from './pages/Plans';
import CalendarPage from './pages/Calendar';
import Cron from './pages/Cron';
import Skills from './pages/Skills';
import Finance from './pages/Finance';
import SettingsPage from './pages/Settings';
import Setup from './pages/Setup';

export const routes: RouteObject[] = [
  { path: '/setup', element: <Setup /> },
  {
    element: <Layout />,
    children: [
      { index: true, element: <Chat /> },
      { path: 'tasks', element: <Tasks /> },
      { path: 'tasks/:id', element: <TaskDetail /> },
      { path: 'plans', element: <Plans /> },
      { path: 'calendar', element: <CalendarPage /> },
      { path: 'cron', element: <Cron /> },
      { path: 'skills', element: <Skills /> },
      { path: 'finance', element: <Finance /> },
      { path: 'settings', element: <SettingsPage /> },
    ],
  },
];
```

**Step 4: Verify navigation works**

```bash
cd crates/dashboard/frontend && npm run dev
```
Click through all nav items — each shows the placeholder text.

**Step 5: Commit**

```bash
git commit -m "feat(frontend): layout shell, nav rail, routing, and page placeholders"
```

---

### Task 17: API Client + WebSocket Hook

**Files:**
- Create: `crates/dashboard/frontend/src/lib/api.ts`
- Create: `crates/dashboard/frontend/src/lib/ws.ts`
- Create: `crates/dashboard/frontend/src/lib/hooks/useApi.ts`
- Create: `crates/dashboard/frontend/src/lib/hooks/useAgent.ts`
- Create: `crates/dashboard/frontend/src/lib/types.ts`

**Step 1: Create `api.ts` — REST client**

```typescript
const BASE = '';  // Vite proxy handles /api → backend

export async function apiFetch<T>(path: string, init?: RequestInit): Promise<T> {
  const res = await fetch(`${BASE}${path}`, {
    headers: { 'Content-Type': 'application/json', ...init?.headers },
    ...init,
  });
  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new ApiError(res.status, body.error || res.statusText);
  }
  return res.json();
}

export class ApiError extends Error {
  constructor(public status: number, message: string) {
    super(message);
  }
}
```

**Step 2: Create `ws.ts` — WebSocket client with reconnection**

```typescript
type MessageHandler = (msg: any) => void;

export class AgentSocket {
  private ws: WebSocket | null = null;
  private handlers = new Map<string, Set<MessageHandler>>();
  private reconnectTimer: ReturnType<typeof setTimeout> | null = null;

  connect() {
    const protocol = location.protocol === 'https:' ? 'wss:' : 'ws:';
    this.ws = new WebSocket(`${protocol}//${location.host}/ws`);
    this.ws.onmessage = (e) => {
      const msg = JSON.parse(e.data);
      this.handlers.get(msg.type)?.forEach(fn => fn(msg));
      this.handlers.get('*')?.forEach(fn => fn(msg));
    };
    this.ws.onclose = () => {
      this.reconnectTimer = setTimeout(() => this.connect(), 2000);
    };
  }

  send(msg: object) { this.ws?.send(JSON.stringify(msg)); }
  on(type: string, handler: MessageHandler) {
    if (!this.handlers.has(type)) this.handlers.set(type, new Set());
    this.handlers.get(type)!.add(handler);
    return () => this.handlers.get(type)?.delete(handler);
  }
  disconnect() {
    if (this.reconnectTimer) clearTimeout(this.reconnectTimer);
    this.ws?.close();
  }
}
```

**Step 3: Create `useAgent.ts` hook**

```typescript
import { useCallback, useEffect, useRef, useState } from 'react';
import { AgentSocket } from '../ws';

export interface AgentMessage {
  role: 'user' | 'assistant' | 'tool' | 'system';
  content: string;
  meta?: Record<string, any>;
}

export interface ThinkingState {
  phase: 'idle' | 'classifying' | 'assembling' | 'executing';
  strategy?: string;
  confidence?: number;
  engine?: string;
  iteration?: number;
  maxIterations?: number;
}

export function useAgent() {
  const socketRef = useRef<AgentSocket | null>(null);
  const [messages, setMessages] = useState<AgentMessage[]>([]);
  const [thinking, setThinking] = useState<ThinkingState>({ phase: 'idle' });
  const [isStreaming, setIsStreaming] = useState(false);

  useEffect(() => {
    const socket = new AgentSocket();
    socket.connect();
    socketRef.current = socket;

    // Wire up event handlers
    socket.on('classificationComplete', (e) => setThinking(t => ({ ...t, phase: 'classifying', strategy: e.strategy, confidence: e.confidence })));
    socket.on('contextAssembled', () => setThinking(t => ({ ...t, phase: 'assembling' })));
    socket.on('executionStarted', (e) => setThinking(t => ({ ...t, phase: 'executing', engine: e.engine, maxIterations: e.maxIterations })));
    socket.on('iterationStart', (e) => setThinking(t => ({ ...t, iteration: e.iteration })));
    socket.on('contentChunk', (e) => {
      // Accumulate content
    });
    socket.on('done', (e) => {
      setMessages(prev => [...prev, { role: 'assistant', content: e.content }]);
      setThinking({ phase: 'idle' });
      setIsStreaming(false);
    });
    socket.on('error', (e) => {
      setMessages(prev => [...prev, { role: 'system', content: `Error: ${e.message}` }]);
      setThinking({ phase: 'idle' });
      setIsStreaming(false);
    });

    return () => socket.disconnect();
  }, []);

  const sendMessage = useCallback((text: string, sessionKey?: string) => {
    setMessages(prev => [...prev, { role: 'user', content: text }]);
    setIsStreaming(true);
    setThinking({ phase: 'classifying' });
    socketRef.current?.send({ type: 'chat.send', message: text, sessionKey });
  }, []);

  const cancel = useCallback(() => {
    socketRef.current?.send({ type: 'chat.cancel' });
  }, []);

  return { messages, thinking, isStreaming, sendMessage, cancel };
}
```

**Step 4: Create `useApi.ts` hook**

```typescript
import { useCallback, useEffect, useState } from 'react';
import { apiFetch } from '../api';

export function useApi<T>(path: string, deps: any[] = []) {
  const [data, setData] = useState<T | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refetch = useCallback(async () => {
    setLoading(true);
    try {
      const result = await apiFetch<T>(path);
      setData(result);
      setError(null);
    } catch (e: any) {
      setError(e.message);
    } finally {
      setLoading(false);
    }
  }, [path, ...deps]);

  useEffect(() => { refetch(); }, [refetch]);

  return { data, loading, error, refetch };
}
```

**Step 5: Create `types.ts`**

Define TypeScript interfaces matching the Rust structs that the API returns. These will be populated incrementally as pages are built:

```typescript
// Core types matching storage row structs

export interface Task {
  id: string;
  title: string;
  description: string | null;
  priority: number | null;
  dueDate: string | null;
  tags: string[];
  status: string;
  focusedAt: string | null;
  focusDeadline: string | null;
  createdAt: string;
  updatedAt: string;
  completedAt: string | null;
  parentId: string | null;
  projectId: string | null;
  totalTrackedSecs: number;
  estimatedMinutes: number | null;
}

export interface TaskSummary {
  todo: number;
  doing: number;
  done: number;
  total: number;
}

export interface Plan {
  id: string;
  sessionKey: string;
  title: string;
  description: string;
  status: string;
  currentStepIndex: number;
  createdAt: string;
  completedAt: string | null;
}

export interface PlanStep {
  id: string;
  planId: string;
  stepIndex: number;
  description: string;
  status: string;
  attemptCount: number;
  result: string | null;
}

// ... more types added per page
```

**Step 6: Commit**

```bash
git commit -m "feat(frontend): API client, WebSocket hook, and TypeScript types"
```

---

## Phase 3: Frontend Pages

Each page follows the same pattern: fetch data with `useApi`, render with Tailwind + Radix components, handle mutations via `apiFetch`. The Figma Make source provides the visual reference.

### Task 18: Chat Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Chat.tsx`

**Step 1: Build the chat UI**

Key components:
- Message list (scrollable, auto-scroll on new messages)
- Input bar at bottom with send button
- Thinking state indicator (phase dots: classifying → assembling → executing)
- Strategy badge with confidence percentage
- Tool execution cards (name, args, duration, success/fail)
- `ask_user` form rendering (InteractionRequest → form UI → send response)
- Empty state with suggestion cards

Wire to `useAgent()` hook. Messages render differently by role (user/assistant/tool/system).

**Step 2: Implement tool call rendering**

When `toolStart` event arrives, add a collapsible card. When `toolEnd` arrives, update it with duration and status.

**Step 3: Implement interaction form**

When `interaction.request` arrives via WebSocket, render the questions (SingleSelect, MultiSelect, YesNo, FreeText) as a form. On submit, send `interaction.respond` via WebSocket.

**Step 4: Commit**

```bash
git commit -m "feat(frontend): chat page with WebSocket streaming and interaction forms"
```

---

### Task 19: Tasks Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Tasks.tsx`

**Step 1: Build task list**

Fetch from `GET /api/tasks`. Render as a list with:
- Status icons (todo/doing/done)
- Priority color indicators (1=red, 2=orange, 3=default, 4=blue)
- Due dates (relative: "in 2 days", "overdue")
- Estimated time badges
- Project color dots
- Tags
- Focus indicator (star icon)
- Summary bar at top (todo/doing/done counts from `GET /api/tasks/summary`)

**Step 2: Add filter panel**

Status filter (all/todo/doing/done), project filter, priority filter, search input (keyword search via query param).

**Step 3: Add create task dialog**

Radix Dialog with form: title, description, priority, due date, tags, project. POST to `/api/tasks`.

**Step 4: Inline status toggle**

Click status icon → cycles todo → doing → done via `PATCH /api/tasks/:id`.

**Step 5: Commit**

```bash
git commit -m "feat(frontend): tasks page with list, filters, and create dialog"
```

---

### Task 20: Task Detail Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/TaskDetail.tsx`

**Step 1: Build detail view**

Fetch from `GET /api/tasks/:id`. Two-column layout:
- Left: title (editable), rich description (textarea), subtask list (from `GET /api/tasks/:id/subtasks`)
- Right panel: properties (status, priority, due date, project, tags — all editable via PATCH), time tracking section, focus toggle

**Step 2: Time tracking**

Client-side timer (start/stop button). On stop, POST to `/api/tasks/:id/time-entries` with duration. Display list of past entries from `GET /api/tasks/:id/time-entries`.

**Step 3: Commit**

```bash
git commit -m "feat(frontend): task detail page with editable fields and time tracking"
```

---

### Task 21: Plans Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Plans.tsx`

**Step 1: Build plan list**

Fetch from `GET /api/plans`. Render as expandable cards:
- Plan title, status badge (draft/approved/executing/completed/failed/abandoned)
- Progress bar (completed steps / total steps)
- Expandable step list with status icons per step
- Created date, completion date

**Step 2: Plan detail expansion**

On click, fetch steps from `GET /api/plans/:id/steps`. Show step descriptions, reasoning, tool expectations, results.

**Step 3: Subscribe to plan progress via WebSocket**

Listen for `planStepCompleted` and `planCompleted` events to update the UI in real time during plan execution.

**Step 4: Commit**

```bash
git commit -m "feat(frontend): plans page with progress tracking and step details"
```

---

### Task 22: Calendar Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Calendar.tsx`

**Step 1: Build calendar grid**

Fetch from `GET /api/calendar/events`. Build a month view calendar grid (no external lib — CSS grid is sufficient for a month view):
- Days as grid cells
- Events as colored pills within each day
- Click event → detail sidebar

**Step 2: Day/week toggle**

Add view mode toggle (month/week). Week view shows time slots.

**Step 3: Sync status**

Show sync status from `GET /api/calendar/sync-status`. Manual sync button triggers `POST /api/calendar/sync`.

**Step 4: Commit**

```bash
git commit -m "feat(frontend): calendar page with month/week views and sync status"
```

---

### Task 23: Cron Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Cron.tsx`

**Step 1: Build cron job list**

Fetch from `GET /api/cron`. Render as cards:
- Job name, schedule (human-readable), enabled toggle
- Last run status, last run time, next run time
- Play/pause toggle (PATCH enable/disable)
- Delete button

**Step 2: Commit**

```bash
git commit -m "feat(frontend): cron jobs page with enable/disable toggle"
```

---

### Task 24: Skills Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Skills.tsx`

**Step 1: Build skill list**

Fetch from `GET /api/skills`. Render as cards:
- Skill name, description, source (built-in/workspace)
- Enable/disable toggle (PATCH)

**Step 2: Commit**

```bash
git commit -m "feat(frontend): skills page with listing and toggle"
```

---

### Task 25: Finance Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Finance.tsx`

This is the most complex frontend page — 6 tabs. Build incrementally.

**Step 1: Tab structure**

Use Radix Tabs with 6 tabs: Dashboard, Transactions, Budgets, Investments, Goals, Reports.

**Step 2: Dashboard tab**

Summary cards: total balance by currency, budget usage overview, portfolio value, goal progress. Fetch from respective API endpoints.

**Step 3: Transactions tab**

Transaction list with quick-add form. Fetch from `GET /api/finance/transactions`. Create via POST. Filters: date range, category, type (income/expense).

**Step 4: Budgets tab**

Budget cards with progress bars (spent / limit). Fetch from `GET /api/finance/budgets/usage`. Support standard and six-jar modes (toggle based on config).

**Step 5: Investments tab**

Portfolio selector, holdings table with current value, gain/loss. Fetch from investment endpoints.

**Step 6: Goals tab**

Goal cards with progress bars. FIRE calculator as a client-side form (inputs: current savings, monthly contribution, target amount, expected return rate → compute years to goal).

**Step 7: Reports tab**

Bar chart (spending by category) and line chart (spending over time) using Recharts. Fetch from `GET /api/finance/transactions/summary`.

**Step 8: Commit**

```bash
git commit -m "feat(frontend): finance page with 6 tabs (dashboard, transactions, budgets, investments, goals, reports)"
```

---

### Task 26: Settings Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Settings.tsx`

**Step 1: Build settings editor**

Fetch from `GET /api/settings`. Render as collapsible sections (one per config section). Each field renders as an appropriate input:
- String → text input
- Number → number input
- Boolean → toggle switch
- Secret → password input with "Change" button (shows `••••••` by default)
- Array → tag-like input
- Enum → select dropdown

**Step 2: Save handler**

On change, debounce 500ms, then PATCH the specific section via `PATCH /api/settings/:section`.

**Step 3: Organize 14 sections**

General, Providers, Channels, Agent Defaults, Tools, Tasks & Todo, Calendar, Conversation, Learning, Confidence, Finance, Projects, Packs & Skills, Plugins.

**Step 4: Commit**

```bash
git commit -m "feat(frontend): settings page with section-based config editor"
```

---

### Task 27: Setup Wizard Page

**Files:**
- Modify: `crates/dashboard/frontend/src/app/pages/Setup.tsx`

**Step 1: Build multi-step wizard**

Matches `klyntbot init` flow:
1. Welcome + data directory
2. Provider selection + API key
3. Default model selection
4. Channel configuration
5. Pack selection
6. Semantic search toggle

Each step saves to settings API. On completion, redirect to `/`.

**Step 2: First-run detection**

On app load, check `GET /api/status`. If no provider is configured, redirect to `/setup`.

**Step 3: Commit**

```bash
git commit -m "feat(frontend): setup wizard matching klyntbot init flow"
```

---

## Phase 4: Production Integration

### Task 28: Embed Frontend in Release Binary

**Files:**
- Modify: `crates/dashboard/src/embed.rs`
- Modify: `crates/dashboard/src/router.rs`
- Modify: `crates/dashboard/Cargo.toml`
- Create: `crates/dashboard/build.rs` (optional — or use cfg flags)

**Step 1: Create `embed.rs`**

```rust
use axum::response::{Html, IntoResponse, Response};
use axum::http::{header, StatusCode};
use include_dir::{include_dir, Dir};

static FRONTEND_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/frontend/dist");

pub async fn serve_frontend(uri: axum::http::Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };

    match FRONTEND_DIR.get_file(path) {
        Some(file) => {
            let mime = mime_guess::from_path(path).first_or_text_plain();
            (
                [(header::CONTENT_TYPE, mime.as_ref())],
                file.contents(),
            ).into_response()
        }
        None => {
            // SPA fallback: serve index.html for client-side routing
            match FRONTEND_DIR.get_file("index.html") {
                Some(file) => Html(std::str::from_utf8(file.contents()).unwrap_or("")).into_response(),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        }
    }
}
```

**Step 2: Add fallback route in `router.rs`**

```rust
// After all /api and /ws routes:
.fallback(embed::serve_frontend)
```

This serves the React app for any path not matched by the API — enabling client-side routing.

**Step 3: Add `mime_guess` dependency**

```toml
mime_guess = "2"
```

**Step 4: Build frontend before cargo build**

Document the build sequence:

```bash
cd crates/dashboard/frontend && npm run build
cargo build --release
```

For development, the Vite dev server handles serving — `embed.rs` only kicks in for release builds when `frontend/dist/` exists.

**Step 5: Add conditional compilation**

If `frontend/dist/` doesn't exist (dev mode), the `include_dir!` macro would fail. Use a cfg flag or make the embed optional:

```rust
#[cfg(feature = "embed-frontend")]
static FRONTEND_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/frontend/dist");
```

Or simpler: add a `.gitkeep` in `frontend/dist/` and document that `npm run build` must run before release builds.

**Step 6: Commit**

```bash
git commit -m "feat(dashboard): embed frontend in release binary via include_dir"
```

---

### Task 29: Status Bar Data + Final Polish

**Files:**
- Modify: `crates/dashboard/frontend/src/app/components/Layout.tsx`

**Step 1: Add bottom status bar**

```tsx
// In Layout.tsx, after <Outlet />:
<footer className="h-7 flex items-center px-3 gap-4 border-t border-codex-border text-xs text-codex-text-tertiary font-mono">
  <span>Model: {status?.model}</span>
  <span>Session: {sessionKey}</span>
  <span>v{status?.version}</span>
</footer>
```

Fetch from `GET /api/status` on mount.

**Step 2: Verify full application flow**

1. `cargo build --workspace` — all crates compile
2. `klyntbot serve --port 18790` — dashboard starts
3. `cd crates/dashboard/frontend && npm run dev` — Vite connects to backend
4. Navigate all pages — each loads data from API
5. Send a chat message — WebSocket streams events
6. Create/edit/delete a task — REST CRUD works
7. View settings — secrets are redacted

**Step 3: Commit**

```bash
git commit -m "feat(frontend): status bar and final integration polish"
```

---

## Summary

| Phase | Tasks | What it delivers |
|-------|-------|-----------------|
| Phase 1: Backend | Tasks 1-14 | Dashboard crate, Axum server, WebSocket streaming, full REST API, settings with secret redaction, CORS |
| Phase 2: Frontend Foundation | Tasks 15-17 | React scaffold, theme, layout, routing, API client, WebSocket hook |
| Phase 3: Frontend Pages | Tasks 18-27 | All 10 pages: Chat, Tasks, TaskDetail, Plans, Calendar, Cron, Skills, Finance, Settings, Setup |
| Phase 4: Production | Tasks 28-29 | Embedded frontend, status bar, full integration |

**Total: 29 tasks.** Each is independently testable and committable. Dependencies flow strictly forward — no task requires backtracking.
