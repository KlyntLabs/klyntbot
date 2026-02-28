# Dashboard Crate

## Section 1: Narrative Overview

### What the Dashboard Is

The `dashboard` crate (`crates/dashboard/`) provides Klyntbot's web-facing interface: a REST API for managing all agent resources, a WebSocket endpoint for real-time streaming chat, and an embedded React single-page application. It sits at Layer 4.5 in the workspace dependency graph, depending on `agent`, `storage`, `config`, `common`, `plan`, and `scheduling`, and is wired into `cli::serve` (Layer 6) as the HTTP server that starts when the user runs `klyntbot serve`.

The crate is built on [Axum](https://github.com/tokio-rs/axum), using `tower-http` for CORS and response compression, and `include_dir` to bake the frontend build artifacts directly into the binary for single-binary deployment.

### Axum Server Setup

The entry point is `DashboardServer`, defined in `src/lib.rs` (lines 32-68). Construction requires two arguments: a `GatewayConfig` (host + port) and an `AppState` (shared application state). The `start()` method:

1. Parses `host:port` from `GatewayConfig` into a `SocketAddr`.
2. Builds the full Axum router via `router::build()`.
3. Binds a `TcpListener` to the address.
4. Serves requests with graceful shutdown, accepting a `Future<Output = ()>` as the shutdown signal.

```rust
let dashboard = DashboardServer::new(config.gateway.clone(), state);
dashboard.start(shutdown_signal).await?;
```

### API Endpoint Design and Routing

All routes are assembled in `src/router.rs` (lines 96-213). The function `router::build(state)` produces a single `Router<()>` with three layers:

1. **API routes** -- nested under `/api`, covering all REST resources.
2. **WebSocket** -- `GET /ws` at the top level.
3. **SPA fallback** -- a catch-all `fallback` handler that serves the embedded frontend.

Two middleware layers wrap the entire router:

- **CorsLayer** (permissive: `allow_origin(Any)`, `allow_methods(Any)`, `allow_headers(Any)`) -- suitable for local/dev use.
- **CompressionLayer** -- gzip/brotli response compression for all responses.

The state is attached via `with_state(state)`, making it available to every handler as `State<AppState>`.

### Endpoint Groups

#### Health (`src/api/health.rs`)

A minimal liveness probe returning `{"status": "ok"}` with no database access. Used by orchestrators and monitoring to confirm the server process is alive.

#### Status (`src/api/status.rs`)

Returns an aggregate overview of the running agent: version, active model, configured provider, permission level, which providers have API keys configured, uptime in seconds, and storage statistics (task count, session count). The handler runs the task-count and session-count queries concurrently via `tokio::join!`, then reads the in-memory config under a `RwLock` to extract provider/model information.

#### Tasks (`src/api/tasks.rs`)

Full CRUD for tasks (todos) plus sub-resources:

- **List/Create** at `/api/tasks` with query-param filtering (status, project, priority, tags, template-only mode, limit).
- **Get/Patch/Delete** individual tasks at `/api/tasks/{id}`.
- **Subtasks** at `/api/tasks/{id}/subtasks` -- children of a parent task.
- **Attachments** at `/api/tasks/{id}/attachments`.
- **Time entries** at `/api/tasks/{id}/time-entries` -- manual time tracking with source, start time, duration, and note.
- **Focus slots** at `/api/tasks/{id}/focus` (POST to set, DELETE to clear) -- limits concurrent focus items with optional deadline.
- **Dependencies** at `/api/tasks/{id}/dependencies` -- tracks "blocked by" and "blocks" relationships between tasks. Both directions are fetched concurrently via `tokio::join!`.

Validation enforces non-empty titles and priority range 1-5. New IDs are generated as UUID v4 strings. Patch requests use `Option<Option<T>>` to distinguish "not provided" from "set to null" for nullable fields.

#### Projects (`src/api/projects.rs`)

CRUD for projects with optional stats aggregation:

- **List** supports filtering by status, tags, and limit.
- **Get** accepts a `?withStats=true` query param that returns a `ProjectWithStats` envelope (includes task counts) instead of a bare `ProjectRow`.
- **Create** defaults color to `#4f46e5` and status to `active`.
- **Patch/Delete** follow the same patterns as tasks.

#### Plans (`src/api/plans.rs`)

Plan lifecycle management tied to the planning engine:

- **List** with filters for status, session key, goal ID, and visibility.
- **Create** initializes a plan in `draft` status with configurable iteration limit (default 20). Visibility is read from `config.orchestrator.default_plan_visibility`.
- **Get** returns a `PlanWithSteps` envelope (plan row + all step rows), fetched concurrently.
- **Patch** allows updating title, description, and iteration limit on an existing plan.
- **Status transitions** via `POST /api/plans/{id}/status` -- validates the transition using `plan::PlanStatus::validate_transition()` (e.g., Draft -> Approved -> Executing -> Completed). Invalid transitions return 409 Conflict.
- **Delete** cascades to plan steps.

#### Sessions (`src/api/sessions.rs`)

Read-only listing plus delete for conversation sessions:

- **List** returns summary rows (key, message count, timestamps).
- **Get** returns the session row plus all messages, fetched concurrently.
- **Delete** removes a session and returns 204 or 404.

#### Cron (`src/api/cron.rs`)

CRUD for scheduled cron jobs with manual trigger support:

- **List** returns all cron job rows.
- **Create** requires a `schedule` (JSON value) and optional `payload` and `deleteAfterRun` flag.
- **Toggle** via `PATCH /api/cron/{id}/toggle` -- enables or disables the job.
- **Run** via `POST /api/cron/{id}/run` -- verifies the job exists, then spawns execution in a background task so the HTTP response returns immediately as 202 Accepted.
- **Delete** removes the job.

#### Calendar (`src/api/calendar.rs`)

Calendar event cache and sync status:

- **List events** supports filtering by `providerId` or returns upcoming events (default limit 50).
- **Sync status** returns per-provider sync state rows.
- **Trigger sync** returns 202 Accepted with `{"status": "sync_queued"}` (the actual sync is handled asynchronously by the calendar sync engine).
- **Create event** inserts a new event into the local cache with provider set to `"local"` and source set to `"dashboard"`. Validates RFC3339 datetime parsing for start/end.

#### Finance (`src/api/finance.rs`)

Comprehensive financial tracking across five sub-resources, each with full CRUD:

- **Accounts** -- banking/brokerage accounts with type, currency, balance, institution, and archive flag.
- **Transactions** -- individual transactions linked to accounts, with category/subcategory, counterparty, and recurring-rule support.
- **Budgets** -- budget periods with envelope/jar method, category targeting, alert thresholds, plus a dedicated `/budgets/usage` endpoint for aggregated spending vs. budget.
- **Investments** -- portfolio holdings with asset type, symbol, quantity, cost basis, and current valuation.
- **Goals** -- financial goals with target/current amounts, monthly contribution, expected return, and inflation rate projections.
- **Liabilities** -- debts/loans with principal, remaining balance, interest rate, and monthly payment tracking.

All monetary amounts are stored as `i64` (cents/minor units) to avoid floating-point precision issues.

#### Skills (`src/api/skills.rs`)

Manages both built-in and workspace skills:

- **List** returns all skills from the `SkillManager`, annotating each with its enabled state (built-in skills check `config.packs.enabled_skills`; workspace skills are always enabled) and source (`"built-in"` or `"workspace"`).
- **Get** returns full skill detail including content.
- **Create** writes a new `SKILL.md` file to the workspace skills directory (`~/.klyntbot/skills/<name>/SKILL.md`). Validates the name has no path separators or existing conflicts.
- **Update** (PATCH) behaves differently by source:
  - Built-in skills: only the `enabled` toggle is respected, persisted by modifying `config.packs.enabledSkills` in the config file.
  - Workspace skills: `description`, `content`, `triggers`, and `always` fields update the `SKILL.md` on disk.
- **Delete** removes the workspace skill directory. Built-in skills cannot be deleted (returns 422).

#### Settings (`src/api/settings.rs`)

Live configuration management:

- **Get full config** serializes the in-memory `Config` to JSON, then recursively redacts secret fields (API keys, tokens, passwords) with `"••••••"`.
- **Get section** extracts a single top-level key (e.g., `providers`, `agents`, `tools`) and redacts it.
- **Patch section** applies RFC 7396 JSON Merge Patch semantics to a config section:
  1. Strips redacted placeholder values (`"••••••"`) to prevent writing them to disk.
  2. Rejects patches containing `dataDir` (immutable at runtime).
  3. Loads the current config from disk, applies the merge patch, re-deserializes into `Config` for validation, saves to disk, and updates the in-memory `Arc<RwLock<Config>>`.

The list of redacted field names is defined in `SECRET_FIELDS` at `src/api/settings.rs` line 10.

### Static Asset Embedding (`src/embed.rs`)

The `include_dir!` macro at `src/embed.rs` line 17 embeds `frontend/dist/` into the binary at compile time. The `spa_handler` function:

1. Tries to match the request URI to an exact file in the embedded directory, serving it with the correct MIME type (via `mime_guess`).
2. Falls back to `index.html` for SPA client-side routing.
3. If `frontend/dist/` was not built (empty directory), returns a development stub HTML page instructing the developer to run `npm run build`.

### WebSocket Streaming (`src/ws.rs`)

The `GET /ws` endpoint upgrades to a WebSocket connection for real-time agent interaction. The protocol supports three client message types:

- **`chat.send`** -- starts a streaming chat turn with a session key and message text. The handler calls `agent_loop.process_direct_streaming()` and spawns two background tasks: one to forward `AgentEvent` frames (tool starts, tool ends, entity cards, text deltas) and persist metadata after the stream ends, and another to forward `InteractionBundle` requests (ask-user forms).
- **`chat.cancel`** -- cancels the in-flight streaming request via a `CancellationToken`.
- **`interaction.respond`** -- delivers the user's form response to a pending `ask_user` request, matched by `requestId`.

Constraints enforced: one streaming operation per connection at a time (duplicate `chat.send` returns an error); client disconnect cancels in-flight processing; pending `ask_user` senders are dropped on disconnect to unblock the agent loop.

### Error Handling Strategy

All handlers return `Result<T, ApiError>`, where `ApiError` (`src/error.rs`) serializes to a consistent JSON shape:

```json
{"status": 404, "message": "task 'abc' not found"}
```

The `ApiError` struct provides factory methods for common status codes (`not_found`, `unprocessable`, `internal`, `bad_request`, `conflict`) and implements `IntoResponse` to set the matching HTTP status code.

Two `From` implementations handle automatic conversion:

- **`From<StorageError>`** -- routes through `KlyntbotError` conversion.
- **`From<KlyntbotError>`** -- maps domain errors to HTTP status codes:
  - `StorageNotFound` -> 404
  - `StorageConflict` -> 409
  - `Storage(_)` -> 500 (generic, details not leaked)
  - `Config(_)` -> 422
  - Everything else -> 500 with the error's `Display` output.

A shared `deleted_or_not_found()` helper in `src/api/mod.rs` (line 37) standardizes delete responses: returns 204 No Content if the row was removed, or 404 if nothing matched.

### How the Dashboard Connects to Agent + Storage

The `AppState` struct (`src/state.rs`) carries all shared dependencies:

| Field | Type | Purpose |
|-------|------|---------|
| `repos` | `storage::Repos` | Aggregate of all repository structs (SQLite pool-based, Clone+Send+Sync) |
| `agent_loop` | `Arc<AgentLoop>` | Agent loop for streaming chat and skill manager access |
| `cron_service` | `Arc<CronService>` | Cron service for manual job triggering |
| `config` | `Arc<RwLock<config::Config>>` | Live configuration (readable by all handlers, writable by settings handlers) |
| `started_at` | `std::time::Instant` | Server start time for uptime calculation |

Handlers access storage through `state.repos.<resource>`, the agent through `state.agent_loop`, and config through `state.config`. No locking is needed for repository access because `SqlitePool` handles connection pooling internally. The `RwLock` on config allows concurrent reads with exclusive writes when the settings API patches configuration.

---

## Section 2: API Reference

### DashboardServer

**File:** `src/lib.rs` (lines 32-68)

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(gateway: GatewayConfig, state: AppState) -> Self` | Construct the server with gateway config (host + port) and shared state |
| `start` | `async fn start(self, shutdown: impl Future<Output = ()>) -> Result<()>` | Bind, serve, and run until the shutdown signal resolves |

### AppState

**File:** `src/state.rs` (lines 14-20)

```rust
#[derive(Clone)]
pub struct AppState {
    pub repos: storage::Repos,
    pub agent_loop: Arc<AgentLoop>,
    pub cron_service: Arc<CronService>,
    pub config: Arc<RwLock<config::Config>>,
    pub started_at: std::time::Instant,
}
```

### ApiError

**File:** `src/error.rs` (lines 11-81)

| Factory Method | HTTP Status | Usage |
|----------------|-------------|-------|
| `not_found(msg)` | 404 | Resource does not exist |
| `unprocessable(msg)` | 422 | Validation failure or invalid input |
| `internal(msg)` | 500 | Unexpected server error |
| `bad_request(msg)` | 400 | Malformed request |
| `conflict(msg)` | 409 | State conflict (e.g., invalid plan status transition) |

Response body: `{"status": <u16>, "message": "<string>"}`

### Shared Utilities

**File:** `src/api/mod.rs` (lines 21-43)

| Function | Signature | Description |
|----------|-----------|-------------|
| `new_id()` | `fn new_id() -> String` | Generate UUID v4 string for row IDs |
| `parse_comma_tags(s)` | `fn parse_comma_tags(s: &str) -> Vec<String>` | Split comma-separated tag string, trim whitespace, drop empties |
| `deleted_or_not_found(deleted, entity, id)` | `fn ... -> Result<StatusCode, ApiError>` | Return 204 if deleted, 404 if not found |

### REST Endpoints

#### Health

**File:** `src/api/health.rs` (lines 1-13)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/health` | `health` | -- | `{"status": "ok"}` | 200 |

#### Status

**File:** `src/api/status.rs` (lines 1-100)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/status` | `get_status` | -- | `StatusResponse` | 200 |

`StatusResponse` fields: `version`, `model`, `provider`, `permissionLevel`, `configuredProviders`, `uptimeSeconds`, `storage: { taskCount, sessionCount }`.

#### Tasks

**File:** `src/api/tasks.rs` (lines 1-442)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/tasks` | `list_tasks` | -- | `Vec<TodoRow>` | 200 |
| POST | `/api/tasks` | `create_task` | `CreateTaskRequest` | `TodoRow` | 201 |
| GET | `/api/tasks/summary` | `get_summary` | -- | `TodoSummary` | 200 |
| GET | `/api/tasks/{id}` | `get_task` | -- | `TodoRow` | 200 |
| PATCH | `/api/tasks/{id}` | `patch_task` | `PatchTaskRequest` | `TodoRow` | 200 |
| DELETE | `/api/tasks/{id}` | `delete_task` | -- | -- | 204 |
| GET | `/api/tasks/{id}/subtasks` | `get_subtasks` | -- | `Vec<TodoRow>` | 200 |
| GET | `/api/tasks/{id}/attachments` | `get_attachments` | -- | `Vec<TodoAttachmentRow>` | 200 |
| GET | `/api/tasks/{id}/time-entries` | `get_time_entries` | -- | `Vec<TodoTimeEntryRow>` | 200 |
| POST | `/api/tasks/{id}/time-entries` | `add_time_entry` | `AddTimeEntryRequest` | `TodoTimeEntryRow` | 201 |
| POST | `/api/tasks/{id}/focus` | `set_focus` | -- | `{"focused": <bool>}` | 200 |
| DELETE | `/api/tasks/{id}/focus` | `delete_focus` | -- | -- | 204 |
| GET | `/api/tasks/{id}/dependencies` | `get_task_dependencies` | -- | `DependenciesResponse` | 200 |
| POST | `/api/tasks/{id}/dependencies` | `add_task_dependency` | `AddDependencyRequest` | -- | 201 |
| DELETE | `/api/tasks/{id}/dependencies/{blocker_id}` | `remove_task_dependency` | -- | -- | 204 |

**Query params for `GET /api/tasks`:** `status`, `projectId`, `priorityMin`, `limit`, `tags` (comma-separated), `templatesOnly`.

**Query params for `POST /api/tasks/{id}/focus`:** `maxSlots` (default 3), `deadline` (RFC3339).

**`CreateTaskRequest`:** `title` (required), `description`, `priority` (1-5), `dueDate`, `tags`, `status` (default "todo"), `parentId`, `projectId`, `estimatedMinutes`, `isTemplate`, `recurrenceRule`.

**`PatchTaskRequest`:** all fields optional; `description`, `priority`, `dueDate`, `estimatedMinutes`, `recurrenceRule` use `Option<Option<T>>` to support explicit null.

**`AddDependencyRequest`:** `blockerId` (required).

**`DependenciesResponse`:** `blockedBy: Vec<TodoRow>`, `blocks: Vec<TodoRow>`.

#### Projects

**File:** `src/api/projects.rs` (lines 1-202)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/projects` | `list_projects` | -- | `Vec<ProjectRow>` | 200 |
| POST | `/api/projects` | `create_project` | `CreateProjectRequest` | `ProjectRow` | 201 |
| GET | `/api/projects/{id}` | `get_project` | -- | `ProjectRow` or `ProjectWithStats` | 200 |
| PATCH | `/api/projects/{id}` | `patch_project` | `PatchProjectRequest` | `ProjectRow` | 200 |
| DELETE | `/api/projects/{id}` | `delete_project` | -- | -- | 204 |

**Query params for `GET /api/projects`:** `status`, `tags` (comma-separated), `limit`.

**Query params for `GET /api/projects/{id}`:** `withStats` (bool, returns `ProjectWithStats` envelope when true).

**`CreateProjectRequest`:** `name` (required), `description`, `color` (default "#4f46e5"), `tags`, `status` (default "active").

#### Plans

**File:** `src/api/plans.rs` (lines 1-235)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/plans` | `list_plans` | -- | `Vec<PlanRow>` | 200 |
| POST | `/api/plans` | `create_plan` | `CreatePlanRequest` | `PlanRow` | 201 |
| GET | `/api/plans/{id}` | `get_plan` | -- | `PlanWithSteps` | 200 |
| PATCH | `/api/plans/{id}` | `patch_plan` | `PatchPlanRequest` | `PlanRow` | 200 |
| DELETE | `/api/plans/{id}` | `delete_plan` | -- | -- | 204 |
| GET | `/api/plans/{id}/steps` | `get_plan_steps` | -- | `Vec<PlanStepRow>` | 200 |
| POST | `/api/plans/{id}/status` | `update_plan_status` | `UpdateStatusRequest` | `{"status": "<new>"}` | 200 |

**Query params for `GET /api/plans`:** `status`, `sessionKey`, `goalId` (UUID), `visibility`.

**`CreatePlanRequest`:** `title` (required), `description`, `sessionKey`, `goalId`, `iterationLimit` (default 20).

**`PatchPlanRequest`:** `title`, `description`, `iterationLimit` -- all optional.

**`UpdateStatusRequest`:** `status` (required). Validates transition via `PlanStatus::validate_transition`; returns 409 on invalid transition.

**`PlanWithSteps`:** `plan: PlanRow`, `steps: Vec<PlanStepRow>`.

#### Sessions

**File:** `src/api/sessions.rs` (lines 1-67)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/sessions` | `list_sessions` | -- | `Vec<SessionListRow>` | 200 |
| GET | `/api/sessions/{id}` | `get_session` | -- | `SessionWithMessages` | 200 |
| DELETE | `/api/sessions/{id}` | `delete_session` | -- | -- | 204 |

**`SessionWithMessages`:** `session: SessionRow`, `messages: Vec<SessionMessageRow>`.

#### Cron

**File:** `src/api/cron.rs` (lines 1-116)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/cron` | `list_cron` | -- | `Vec<CronJobRow>` | 200 |
| POST | `/api/cron` | `create_cron` | `CreateCronRequest` | `CronJobRow` | 200 |
| PATCH | `/api/cron/{id}/toggle` | `toggle_cron` | `ToggleRequest` | `CronJobRow` | 200 |
| POST | `/api/cron/{id}/run` | `run_cron` | -- | -- | 202 |
| DELETE | `/api/cron/{id}` | `delete_cron` | -- | -- | 204 |

**`CreateCronRequest`:** `name`, `schedule` (JSON value, required), `payload` (JSON value, optional), `deleteAfterRun` (bool, default false).

**`ToggleRequest`:** `enabled` (bool).

#### Calendar

**File:** `src/api/calendar.rs` (lines 1-116)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/calendar/events` | `list_events` | -- | `Vec<CalendarEventCacheRow>` | 200 |
| POST | `/api/calendar/events` | `create_event` | `CreateEventRequest` | `CalendarEventCacheRow` | 201 |
| GET | `/api/calendar/sync-status` | `get_sync_status` | -- | `Vec<CalendarSyncStateRow>` | 200 |
| POST | `/api/calendar/sync` | `trigger_sync` | -- | `{"status": "sync_queued"}` | 202 |

**Query params for `GET /api/calendar/events`:** `providerId` (filters by provider), `limit` (default 50; ignored when `providerId` is set).

**`CreateEventRequest`:** `summary` (required), `description`, `start` (RFC3339), `end` (RFC3339).

#### Finance

**File:** `src/api/finance.rs` (lines 1-719)

**Accounts:**

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/finance/accounts` | `list_accounts` | -- | `Vec<FinanceAccountRow>` | 200 |
| POST | `/api/finance/accounts` | `create_account` | `CreateAccountRequest` | `FinanceAccountRow` | 200 |
| GET | `/api/finance/accounts/{id}` | `get_account` | -- | `FinanceAccountRow` | 200 |
| PATCH | `/api/finance/accounts/{id}` | `patch_account` | `PatchAccountRequest` | `FinanceAccountRow` | 200 |
| DELETE | `/api/finance/accounts/{id}` | `delete_account` | -- | -- | 204 |

**`CreateAccountRequest`:** `name`, `accountType`, `currency`, `balance` (default 0), `institution`, `notes`.

**Transactions:**

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/finance/transactions` | `list_transactions` | -- | `Vec<FinanceTransactionRow>` | 200 |
| POST | `/api/finance/transactions` | `create_transaction` | `CreateTransactionRequest` | `FinanceTransactionRow` | 200 |
| GET | `/api/finance/transactions/{id}` | `get_transaction` | -- | `FinanceTransactionRow` | 200 |
| PATCH | `/api/finance/transactions/{id}` | `patch_transaction` | `PatchTransactionRequest` | `FinanceTransactionRow` | 200 |
| DELETE | `/api/finance/transactions/{id}` | `delete_transaction` | -- | -- | 204 |

**`CreateTransactionRequest`:** `accountId`, `txType`, `amount`, `currency`, `txDate` (NaiveDate), `category`, `subcategory`, `counterparty`, `notes`, `transferId`, `isRecurring` (default false), `recurringRule`.

**Budgets:**

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/finance/budgets` | `list_budgets` | -- | `Vec<FinanceBudgetRow>` | 200 |
| POST | `/api/finance/budgets` | `create_budget` | `CreateBudgetRequest` | `FinanceBudgetRow` | 200 |
| PATCH | `/api/finance/budgets/{id}` | `patch_budget` | `PatchBudgetRequest` | `FinanceBudgetRow` | 200 |
| DELETE | `/api/finance/budgets/{id}` | `delete_budget` | -- | -- | 204 |
| GET | `/api/finance/budgets/usage` | `get_budget_usage` | -- | `Vec<BudgetUsageRow>` | 200 |

**`CreateBudgetRequest`:** `name`, `amount`, `currency`, `period`, `category`, `method` (default "envelope"), `jarType`, `startDate` (NaiveDate), `endDate`, `alertThreshold` (default 80).

**Investments:**

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/finance/investments` | `list_investments` | -- | `Vec<FinanceInvestmentRow>` | 200 |
| POST | `/api/finance/investments` | `create_investment` | `CreateInvestmentRequest` | `FinanceInvestmentRow` | 200 |
| PATCH | `/api/finance/investments/{id}` | `patch_investment` | `PatchInvestmentRequest` | `FinanceInvestmentRow` | 200 |
| DELETE | `/api/finance/investments/{id}` | `delete_investment` | -- | -- | 204 |

**`CreateInvestmentRequest`:** `portfolioId`, `assetType`, `symbol`, `name`, `quantity`, `costBasis`, `currency`, `currentPrice`, `currentValue`, `purchaseDate`, `notes`.

**Goals:**

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/finance/goals` | `list_goals` | -- | `Vec<FinanceGoalRow>` | 200 |
| POST | `/api/finance/goals` | `create_goal` | `CreateGoalRequest` | `FinanceGoalRow` | 200 |
| PATCH | `/api/finance/goals/{id}` | `patch_goal` | `PatchGoalRequest` | `FinanceGoalRow` | 200 |
| DELETE | `/api/finance/goals/{id}` | `delete_goal` | -- | -- | 204 |

**`CreateGoalRequest`:** `name`, `goalType`, `targetAmount`, `currentAmount` (default 0), `currency`, `deadline`, `monthlyContribution`, `expectedReturnRate`, `inflationRate`, `notes`.

**Liabilities:**

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/finance/liabilities` | `list_liabilities` | -- | `Vec<FinanceLiabilityRow>` | 200 |
| POST | `/api/finance/liabilities` | `create_liability` | `CreateLiabilityRequest` | `FinanceLiabilityRow` | 200 |
| PATCH | `/api/finance/liabilities/{id}` | `patch_liability` | `PatchLiabilityRequest` | `FinanceLiabilityRow` | 200 |
| DELETE | `/api/finance/liabilities/{id}` | `delete_liability` | -- | -- | 204 |

**`CreateLiabilityRequest`:** `name`, `liabilityType`, `principal`, `remaining` (default = principal), `currency`, `interestRate`, `monthlyPayment`, `dueDate`, `notes`.

#### Skills

**File:** `src/api/skills.rs` (lines 1-405)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/skills` | `list_skills` | -- | `Vec<SkillResponse>` | 200 |
| POST | `/api/skills` | `create_skill` | `CreateSkillRequest` | `SkillResponse` | 201 |
| GET | `/api/skills/{name}` | `get_skill` | -- | `SkillResponse` | 200 |
| PATCH | `/api/skills/{name}` | `update_skill` | `UpdateSkillRequest` | `SkillResponse` | 200 |
| DELETE | `/api/skills/{name}` | `delete_skill` | -- | -- | 204 |

**`SkillResponse`:** `name`, `description`, `version`, `available`, `always`, `source` ("built-in" or "workspace"), `triggers`, `requiresBins`, `requiresEnv`, `content`, `enabled`.

**`CreateSkillRequest`:** `name` (required), `description`, `version` (default "1.0"), `content`, `triggers`, `always` (default false).

**`UpdateSkillRequest`:** `enabled`, `description`, `content`, `triggers`, `always` -- all optional. Behavior differs by skill source (see Section 1).

#### Settings

**File:** `src/api/settings.rs` (lines 1-173)

| Method | Path | Handler | Request Body | Response Body | Status |
|--------|------|---------|-------------|--------------|--------|
| GET | `/api/settings` | `get_settings` | -- | Full config JSON (secrets redacted) | 200 |
| GET | `/api/settings/{section}` | `get_settings_section` | -- | Section JSON (secrets redacted) | 200 |
| PATCH | `/api/settings/{section}` | `patch_settings_section` | JSON merge patch | Updated section JSON (secrets redacted) | 200 |

Redacted fields: `apiKey`, `token`, `botToken`, `appToken`, `imapPassword`, `smtpPassword`, `secret`, `appSecret`, `encryptKey`, `clientSecret`.

PATCH rejects: `dataDir` in the patch body (returns 422). Placeholder values (`"••••••"`) are stripped before applying the patch.

### WebSocket Endpoint

**File:** `src/ws.rs` (lines 1-361)

| Direction | Path | Handler |
|-----------|------|---------|
| Upgrade | `GET /ws` | `ws_handler` |

**Client -> Server messages (tagged union on `type`):**

| Type | Fields | Description |
|------|--------|-------------|
| `chat.send` | `sessionKey`, `message` | Start streaming chat turn |
| `chat.cancel` | -- | Cancel in-flight request |
| `interaction.respond` | `requestId` (UUID), `response` (FormResponse) | Answer an ask-user prompt |

**Server -> Client messages:**

- Any `AgentEvent` variant (JSON-tagged, camelCase).
- `interaction.request` with fields `requestId`, `title`, `questions`.
- `error` with field `message`.

### Static Asset Serving

**File:** `src/embed.rs` (lines 1-61)

| Path | Handler | Behavior |
|------|---------|----------|
| `/*` (fallback) | `spa_handler` | Serves exact file from embedded `frontend/dist/` or falls back to `index.html` for SPA routing. Returns dev stub if frontend not built. |

The embedded directory is compiled in at `CARGO_MANIFEST_DIR/frontend/dist`. MIME types are auto-detected via `mime_guess`.
