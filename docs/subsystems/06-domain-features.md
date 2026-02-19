# 06 — Domain Features: Calendar, Scheduling, Goal, Plan, Heartbeat

> **Analyst**: domain-features-analyst | **Crates**: calendar, scheduling, goal, plan, heartbeat
> **Generated**: 2026-02-19

---

## Table of Contents

1. [Calendar System](#1-calendar-system)
2. [Scheduling System (Cron)](#2-scheduling-system-cron)
3. [Goal System](#3-goal-system)
4. [Planning System](#4-planning-system)
5. [Heartbeat System](#5-heartbeat-system)
6. [Cross-Cutting: JSONL → SQL Migration Status](#6-cross-cutting-jsonl--sql-migration-status)
7. [Gap Analysis & Recommendations](#7-gap-analysis--recommendations)

---

## 1. Calendar System

**Crate**: `calendar` (Layer 2) | **Version**: 0.1.0 | **Files**: 12 source files

### 1.1 Architecture Overview

The calendar system implements a multi-provider CalDAV/REST calendar integration with two-way sync capability. It's built around a provider abstraction pattern with three concrete implementations.

```
                CalendarProvider (trait)
                     /    |    \
                    /     |     \
    AppleCalendarProvider  GoogleCalendarProvider  GenericCalDavProvider
           |                      |                      |
     CalDavClient          REST API v3             CalDavClient
      (RFC 4791)            (OAuth2)               (RFC 4791)
```

### 1.2 CalendarProvider Trait

**File**: `provider.rs` (36 lines)

```rust
#[async_trait]
pub trait CalendarProvider: Send + Sync {
    fn name(&self) -> &str;
    fn provider_id(&self) -> &str;
    async fn get_events(&self, sync_token: Option<&str>) -> Result<(Vec<CalendarEvent>, Option<String>)>;
    async fn put_event(&self, event: &CalendarEvent) -> Result<String>;
    async fn delete_event(&self, uid: &str) -> Result<()>;
    async fn test_connection(&self) -> Result<()>;
}
```

**Key design**: Incremental sync via `sync_token` parameter (RFC 6578). Returns `(events, new_sync_token)` — callers persist the token between syncs.

### 1.3 Provider Implementations

#### AppleCalendarProvider (`providers/apple.rs`, 174 lines)

- **Auth**: HTTP Basic (app-specific passwords)
- **Transport**: CalDAV via `CalDavClient`
- **Lazy discovery**: `ensure_discovered()` performs 3-step CalDAV discovery only on first use:
  1. `PROPFIND /.well-known/caldav` → principal URL
  2. `PROPFIND <principal>` → calendar-home-set
  3. `PROPFIND <calendar-home>` (Depth:1) → find calendar by displayname
- **Concurrency**: `RwLock<CalDavClient>` + `RwLock<bool>` for discovery state
- **Auto-detection**: Skips discovery if URL already contains `/calendars/` or ends with `.ics`

#### GoogleCalendarProvider (`providers/google.rs`, 687 lines) — **Largest provider**

- **Auth**: OAuth2 Bearer with automatic token refresh (5-minute buffer)
- **Transport**: REST API v3 (`https://www.googleapis.com/calendar/v3`)
- **Why REST over CalDAV**: Google restricts CalDAV access (403) unless explicitly enabled in Cloud Console
- **Sync mechanism**: Google's `syncToken` / `nextSyncToken` protocol; handles HTTP 410 (Gone) → full re-sync
- **Event ID mapping**: `iCalUID` ↔ Google `id` translation via `find_event_id_by_uid()`
- **Status mapping**: Google lowercase (`confirmed`) ↔ iCal uppercase (`CONFIRMED`)
- **New event import**: Uses `/import` endpoint to preserve `iCalUID`

#### GenericCalDavProvider (`providers/generic.rs`, 221 lines)

- **Auth**: HTTP Basic
- **Transport**: CalDAV via `CalDavClient`
- **Targets**: Nextcloud, Fastmail, Zoho, Radicale, etc.
- **Provider ID**: Sanitized from label (`"My Fastmail Calendar!"` → `"generic-my-fastmail-calendar-"`)
- **Graceful discovery fallback**: If discovery fails, continues with URL as-is

### 1.4 CalDavClient (`caldav/client.rs`, 901 lines) — **Most complex file**

Core HTTP client implementing RFC 4791 (CalDAV) and RFC 6578 (WebDAV Sync).

| Method | HTTP Method | Purpose |
|--------|------------|---------|
| `discover_calendar_url()` | PROPFIND | 3-step CalDAV discovery sequence |
| `get_events()` | REPORT | Fetch events with optional sync token |
| `put_event()` | PUT | Create/update event (.ics) |
| `delete_event()` | DELETE | Remove event by UID |

**XML parsing**: Uses `quick-xml` with manual event-based (SAX-style) parsing — no DOM tree allocated.

**Sync modes**:
- Full sync: `calendar-query` REPORT with `VEVENT` filter
- Incremental sync: `sync-collection` with token (RFC 6578)

**Response parsing**: `parse_report_response()` extracts `calendar-data` (iCal), `getetag`, and `sync-token` from multistatus XML.

### 1.5 iCalendar Parser/Generator (`caldav/parser.rs`, 621 lines)

Minimal RFC 5545 subset supporting:
- **Parsing**: `parse_vevent()` — extracts UID, SUMMARY, DESCRIPTION, DTSTART, DTEND, STATUS
- **Generation**: `generate_vevent()` — produces VCALENDAR with VTIMEZONE component
- **Timezone handling**: Full `TZID` parameter support via `chrono-tz`
  - Non-UTC: `DTSTART;TZID=Asia/Bangkok:20260301T090000`
  - UTC: `DTSTART:20260301T090000Z`
- **VTIMEZONE generation**: Computes STANDARD/DAYLIGHT subcomponents by comparing Jan 1 and Jul 1 UTC offsets

### 1.6 Sync Engine (`sync_engine.rs`, 151 lines)

Currently minimal:
- `detect_conflict()` — Compares same-UID events on summary, description, start, end, etag, status
- `resolve_conflict()` — **Server-wins strategy only** (returns server version verbatim)

### 1.7 Sync State Persistence (`state.rs`, 153 lines) — **Dual backend**

| Function | Backend | Storage |
|----------|---------|---------|
| `load_provider_sync_state()` | File | `~/.klyntbot/calendar_sync_states/{provider_id}.json` |
| `save_provider_sync_state()` | File | Same |
| `load_provider_sync_state_sql()` | SQL | `CalendarSyncRepo` → `calendar_sync_states` table |
| `save_provider_sync_state_sql()` | SQL | Same |

**Migration status**: Both file and SQL backends exist in parallel. Caller chooses which to use. The SQL variants require a `CalendarSyncRepo` from the `storage` crate.

### 1.8 Domain Types (`types.rs`, 102 lines)

```rust
pub struct CalendarEvent {
    pub uid: String,           // iCalendar UID
    pub summary: String,       // Event title
    pub description: Option<String>,
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
    pub source: EventSource,   // CalDAV | TodoItem
    pub etag: Option<String>,  // CalDAV sync
    pub status: Option<String>, // CONFIRMED | CANCELLED | TENTATIVE | COMPLETED
}

pub enum EventSource { CalDAV, TodoItem }

pub struct SyncState {
    pub sync_token: Option<String>,
    pub last_sync: Option<DateTime<Utc>>,
}
```

### 1.9 Test Coverage

| File | Tests | Notes |
|------|-------|-------|
| types.rs | 4 | Basic construction, variants, sync state |
| state.rs | 5 | Path generation, serialization, roundtrip |
| sync_engine.rs | 4 | Conflict detection/resolution |
| caldav/client.rs | 6 | Client creation, report body, XML parsing |
| caldav/parser.rs | 14 | UTC/TZ parsing, generation, roundtrip, VTIMEZONE, status |
| providers/apple.rs | 2 | Creation, direct URL |
| providers/google.rs | 12 | Event JSON conversion, status mapping, URLs |
| providers/generic.rs | 3 | Creation, ID sanitization, specific URL |
| **Total** | **50** | No integration tests (requires live CalDAV server) |

---

## 2. Scheduling System (Cron)

**Crate**: `scheduling` (Layer 2) | **Version**: workspace-inherited | **Files**: 5 source files

### 2.1 Architecture Overview

```
CronService
├── store (Arc<RwLock<CronStore>>)  ← in-memory state
├── store_path (PathBuf)             ← JSON file backend
├── sql_repo (Option<CronRepo>)      ← SQL backend
├── on_job (Option<JobCallback>)     ← execution callback
├── running (Arc<RwLock<bool>>)      ← lifecycle flag
└── timer_task (Arc<RwLock<Option<JoinHandle<()>>>>)
```

### 2.2 CronSchedule — Three Schedule Types

```rust
pub enum CronSchedule {
    At { at_ms: i64 },           // One-shot at specific timestamp
    Every { every_ms: u64 },     // Fixed interval (ms)
    Cron { expr: String, tz: Option<String> }, // Standard cron expression (6-field)
}
```

**Serde**: Tagged union with `"kind"` discriminator (`"at"`, `"every"`, `"cron"`).

**Cron parsing**: Uses the `cron` crate for `Cron` variant. Supports 6-field expressions (sec min hour day month dow). Invalid expressions return `None` for next run.

### 2.3 CronJob — Job Structure

```rust
pub struct CronJob {
    pub id: String,              // UUID v4 (first 8 chars)
    pub name: String,
    pub enabled: bool,           // Default: true
    pub schedule: CronSchedule,
    pub payload: CronPayload,    // What to execute
    pub state: CronJobState,     // Runtime state
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    pub delete_after_run: bool,  // Auto-delete one-shot jobs
}
```

### 2.4 CronJobState — Runtime State (not a formal state machine)

```rust
pub struct CronJobState {
    pub next_run_at_ms: Option<i64>,
    pub last_run_at_ms: Option<i64>,
    pub last_status: Option<String>,  // "ok" | "error" | "skipped"
    pub last_error: Option<String>,
}
```

**Note**: Unlike Plan/Goal, CronJobState is not a formal state machine — it's a set of nullable runtime counters updated by the executor.

### 2.5 CronPayload — Execution Specification

```rust
pub struct CronPayload {
    pub kind: String,            // Default: "agent_turn"
    pub message: String,         // Prompt to send to agent
    pub deliver: bool,           // Deliver response to channel
    pub channel: Option<String>, // Target channel name
    pub to: Option<String>,      // Target chat ID
}
```

### 2.6 CronService — Service Lifecycle

```
new(store_path) / from_repo(repo) → set_callback() → start() → [running] → stop()
```

**Timer loop** (`start_timer_loop()`):
1. Spawns a `tokio::spawn` task that polls every 100ms
2. On each tick: `next_wake_ms_static()` finds the soonest `next_run_at_ms`
3. When a job is due: `process_due_jobs()` → `execute_job_static()` → callback
4. After execution: state update + save to disk/SQL

**Job execution** (`executor.rs`):
- Calls `on_job` callback with the CronJob reference
- Records status: `"ok"` / `"error"` / `"skipped"` (no callback)
- One-shot jobs (`At` schedule):
  - `delete_after_run=true` → removed from store
  - `delete_after_run=false` → disabled, `next_run_at_ms=None`
- Recurring jobs → `compute_next_run()` recalculates next run

### 2.7 Persistence Layer (`store.rs`) — **Dual backend**

| Mode | Constructor | Storage |
|------|------------|---------|
| File | `CronService::new(path)` | JSON file at `store_path` |
| SQL | `CronService::from_repo(repo)` | `CronRepo` → `cron_jobs` table |

**SQL save strategy**: Full upsert of all jobs + orphan deletion (deletes SQL rows not in memory).

**Row conversion**: `job_to_row()` / `row_to_job()` serialize `schedule` and `payload` as `serde_json::Value` blobs.

### 2.8 CronStore (legacy file format)

```rust
pub struct CronStore {
    pub version: u32,  // Default: 1
    pub jobs: Vec<CronJob>,
}
```

**Note**: This is a flat JSON file (not JSONL journal). The entire store is loaded into memory, mutated, and saved back atomically. Unlike GoalStore/PlanStore, there is no append-only journal or compaction.

### 2.9 Test Coverage

| Module | Tests | Notes |
|--------|-------|-------|
| types.rs | 9 | Schedule serialization, payload, job creation, camelCase |
| service/mod.rs | 20 | CRUD, filtering, execution, error handling, persistence, one-shot, force run |
| **Total** | **29** | All tests use file backend; no SQL backend tests |

---

## 3. Goal System

**Crate**: `goal` (Layer 2) | **Version**: 0.1.0 | **Files**: 3 source files

### 3.1 Architecture Overview

The goal system provides strategic goal tracking with quantifiable metrics and project linkage. Goals sit above projects in the hierarchy: a goal can link to multiple projects.

### 3.2 Goal — Domain Type

```rust
pub struct Goal {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: GoalStatus,
    pub priority: u8,                       // 1-5 (same scale as todo)
    pub target_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metrics: Vec<Metric>,               // Progress indicators
    pub linked_project_ids: Vec<Uuid>,       // N:M relationship
    pub metadata: HashMap<String, String>,   // Extensible KV store
}
```

### 3.3 GoalStatus — State Machine

```
Active (default) ←→ Paused
    |                  |
    ↓                  ↓
Achieved          Abandoned
```

**Implementation**: No formal `validate_transition()` like Plan. Status changes are unconstrained — any status can transition to any other. The state machine above represents the intended semantics, not enforced constraints.

```rust
pub enum GoalStatus {
    Active,     // Default — currently pursuing
    Paused,     // Temporarily suspended
    Achieved,   // Completed successfully
    Abandoned,  // No longer pursuing
}
```

Implements `Display`, `FromStr`, `Default`, `PartialEq`, `Eq`, `Serialize`, `Deserialize`.

### 3.4 Metric — Progress Tracking

```rust
pub struct Metric {
    pub name: String,
    pub current: f64,
    pub target: f64,
    pub unit: String,
}
```

- `progress_percentage()` → `(current / target * 100).min(100.0)` — capped at 100%, returns 0.0 for zero target

### 3.5 GoalProgress — Aggregated Snapshot

```rust
pub struct GoalProgress {
    pub goal_id: Uuid,
    pub completion_percentage: f64,  // Average of all metric percentages
    pub metrics: Vec<Metric>,
    pub summary: String,             // "75% complete across 2 metric(s)"
}
```

### 3.6 GoalStore — Persistence (`store.rs`, 697 lines) — **Dual backend**

**Architecture**: Append-only JSONL journal with in-memory HashMap index. Mirrors PlanStore pattern exactly.

| Feature | Implementation |
|---------|---------------|
| Constructor (file) | `GoalStore::new(file_path)` |
| Constructor (SQL) | `GoalStore::from_repo(repo)` |
| Journal format | Tagged JSONL: `{"_op":"upsert","goal":{...}}` / `{"_op":"delete","id":"..."}` |
| Loading | Lazy — `ensure_loaded()` replays journal on first access |
| Writes | O(1) — append entry to file |
| Compaction | Auto at 100 stale entries — rewrites journal with only live entries |
| Index | `HashMap<Uuid, Goal>` — O(1) lookups |
| Ordering | `Vec<Uuid>` preserves insertion order |

**SQL conversion helpers**:
- `goal_to_row()` / `row_to_goal()` — metrics and metadata serialized as JSON blobs
- Project links stored separately via `GoalRepo::link_project()` / `get_project_links()`
- `update()` performs full sync: clear all existing links, re-link all current ones

**API**:
| Method | Description |
|--------|-------------|
| `add(goal)` | Insert new goal |
| `get(id)` | O(1) lookup |
| `update(goal)` | Replace in-place |
| `delete(id)` | Remove + journal delete entry |
| `list(status?)` | Filtered listing |
| `all()` | All goals in insertion order |
| `calculate_progress(id)` | Average of metric percentages |

### 3.7 Test Coverage

| Module | Tests | Notes |
|--------|-------|-------|
| types.rs | 9 | Construction, status, metrics, serde, progress calculation |
| store.rs | 11 | CRUD, filtering, persistence, compaction, progress, metrics |
| **Total** | **20** | All tests use file backend; no SQL backend tests |

---

## 4. Planning System

**Crate**: `plan` (Layer 2) | **Version**: 0.2.0 | **Files**: 3 source files

### 4.1 Architecture Overview

The planning engine provides structured multi-step execution plans with a formal state machine, backtracking support, and session isolation. Plans are executed by the agent crate's `PlanExecutor` and `AgentLoop::run_plan_execution()`.

### 4.2 Plan — Domain Type

```rust
pub struct Plan {
    pub id: Uuid,
    pub session_key: String,          // Session isolation
    pub goal_id: Option<Uuid>,        // Optional goal linkage
    pub title: String,
    pub description: String,
    pub status: PlanStatus,           // Enforced state machine
    pub steps: Vec<PlanStep>,
    pub current_step_index: usize,
    pub iteration_limit: usize,       // Default: 50
    pub backtrack_history: Vec<BacktrackEntry>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

### 4.3 PlanStatus — **Enforced** State Machine

```
Draft ──→ Approved ──→ Executing ──→ Completed
  |           |            |
  ↓           ↓            ↓
Abandoned  Abandoned    Failed
                          |
                          ↓
                       Abandoned
```

**Key difference from GoalStatus**: Transitions are enforced by `PlanStatus::validate_transition()`.

```rust
impl PlanStatus {
    pub fn validate_transition(from: &PlanStatus, to: &PlanStatus) -> Result<()> {
        // Same-state no-ops allowed
        // Terminal states: Completed, Failed, Abandoned — no outgoing transitions
        // Draft → Approved | Abandoned
        // Approved → Executing | Abandoned
        // Executing → Completed | Failed | Abandoned
    }
}
```

**Comprehensive test coverage**: 5 test functions covering all valid transitions, all invalid transitions from terminal states, and state-skipping prevention.

### 4.4 PlanStep — Step Lifecycle

```rust
pub struct PlanStep {
    pub id: Uuid,
    pub index: usize,
    pub description: String,
    pub reasoning: String,
    pub expected_tools: Vec<String>,
    pub status: StepStatus,           // Pending → Executing → Completed | Failed | Skipped
    pub attempt_count: u8,
    pub max_attempts: u8,             // Default: 3
    pub result: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

**StepStatus**: `Pending` → `Executing` → `Completed | Failed | Skipped`. No formal validation (unlike PlanStatus).

### 4.5 BacktrackEntry — Retry Tracking

```rust
pub struct BacktrackEntry {
    pub step_index: usize,
    pub attempt: u8,
    pub failure_reason: String,
    pub timestamp: DateTime<Utc>,
}
```

Used by the agent crate's `PlanExecutor::regenerate_from()` when a step exceeds `max_attempts`. After `MAX_BACKTRACK_ATTEMPTS` (3) full backtrack events, the plan is marked `Failed`.

### 4.6 PlanStore — Persistence (`store.rs`, 749 lines) — **Dual backend**

**Architecture**: Identical journal pattern to GoalStore (append-only JSONL with compaction).

| Feature | Implementation |
|---------|---------------|
| Constructor (file) | `PlanStore::new(file_path)` |
| Constructor (SQL) | `PlanStore::from_repo(repo)` |
| Journal format | Tagged JSONL: `{"_op":"upsert","plan":{...}}` / `{"_op":"delete","id":"..."}` |
| Compaction threshold | 100 stale entries |
| Backup on compact | Creates `.jsonl.bak` before compaction |
| Borrowing serialization | `JournalEntryRef<'a>` for zero-copy `persist_latest()` |

**Unique features vs GoalStore**:
- **`get_mut()`**: Returns `&mut Plan` for in-place mutation without cloning. Caller calls `persist_latest()` to flush.
- **`persist_latest()`**: Uses `JournalEntryRef<'a>` (borrowing) to serialize without cloning the Plan.
- **`get_active_plan(session_key)`**: Returns most recent Draft/Approved/Executing plan for a session.
- **`list_by_status()`**: Filter plans by status.
- **Backup on compaction**: `plans.jsonl.bak` is created before overwriting (GoalStore does NOT do this).

**SQL conversion**:
- `plan_to_row()` / `row_to_plan()` — status stored as lowercase string, backtrack_history as JSON blob
- `step_to_row()` — steps stored in separate `plan_steps` table with foreign key
- `sql_upsert()` — tries create, falls back to update on duplicate key; step upsert compares existing IDs

### 4.7 Test Coverage

| Module | Tests | Notes |
|--------|-------|-------|
| types.rs | 9 | Defaults, valid/invalid transitions (all combinations), serde roundtrip |
| store.rs | 5 | CRUD, session filtering, persistence, compaction, backup |
| **Total** | **14** | All tests use file backend; no SQL backend tests |

---

## 5. Heartbeat System

**Crate**: `heartbeat` (Layer 4) | **Version**: workspace-inherited | **Files**: 2 source files

### 5.1 Architecture Overview

The simplest domain crate — a periodic wake-up service that reads `HEARTBEAT.md` from the workspace and triggers the agent if actionable content is found.

### 5.2 HeartbeatService

```rust
pub struct HeartbeatService {
    workspace: PathBuf,
    on_heartbeat: Option<HeartbeatCallback>,
    interval_s: u64,           // Default: 30 minutes
    enabled: bool,
    running: Arc<RwLock<bool>>,
    task: Arc<RwLock<Option<JoinHandle<()>>>>,
}
```

**Callback type**: `Arc<dyn Fn(&str) -> Result<String, Box<dyn std::error::Error>> + Send + Sync>`

### 5.3 Lifecycle

```
new(workspace, interval_s, enabled) → set_callback(fn) → start() → [run_loop] → stop()
                                                                        ↓
                                                                   tick() every interval_s
                                                                        ↓
                                                              read HEARTBEAT.md
                                                                        ↓
                                                          is_heartbeat_empty()?
                                                            /           \
                                                          yes            no
                                                           ↓              ↓
                                                         skip      call callback(HEARTBEAT_PROMPT)
                                                                        ↓
                                                              check for HEARTBEAT_OK token
```

### 5.4 HEARTBEAT.md Content Detection

`is_heartbeat_empty()` scans line by line, skipping:
- Empty lines
- Headers (`#`)
- HTML comments (`<!--`)
- Empty/completed checkboxes (`- [ ]`, `- [x]`, `* [ ]`, `* [x]`)

Returns `false` (actionable) if any other content exists, e.g. `- [ ] Do something`.

### 5.5 Constants

| Constant | Value |
|----------|-------|
| `DEFAULT_HEARTBEAT_INTERVAL_S` | `1800` (30 minutes) |
| `HEARTBEAT_PROMPT` | Multi-line prompt to read HEARTBEAT.md |
| `HEARTBEAT_OK_TOKEN` | `"HEARTBEAT_OK"` |

### 5.6 Manual Trigger

`trigger_now()` — Bypasses the timer loop to immediately invoke the callback.

### 5.7 Notable Design Decisions

- **No database dependency**: Heartbeat has no `storage` dependency — it's purely filesystem + callback based
- **`clone_service()`**: Manual `Clone`-like method sharing `Arc` handles instead of deriving `Clone`
- **Case-insensitive OK detection**: Normalizes response and token (removes underscores, uppercases) before comparison

### 5.8 Test Coverage

| Module | Tests | Notes |
|--------|-------|-------|
| service.rs | 1 | `is_heartbeat_empty()` with 11 assertions |
| **Total** | **1** | No async service lifecycle tests |

---

## 6. Cross-Cutting: JSONL → SQL Migration Status

### 6.1 Migration Matrix

| Crate | JSONL Store | SQL Repo | Dual Backend | Notes |
|-------|:-----------:|:--------:|:------------:|-------|
| **calendar** (sync state) | `load/save_provider_sync_state()` | `CalendarSyncRepo` | **Parallel** — caller chooses | Both functions exported, not yet unified |
| **scheduling** | `CronStore` (flat JSON) | `CronRepo` | **Unified** — `from_repo()` | `save_store()` delegates based on `sql_repo` presence |
| **goal** | `GoalStore` (JSONL journal) | `GoalRepo` | **Unified** — `from_repo()` | Full CRUD delegation; project links via separate table |
| **plan** | `PlanStore` (JSONL journal) | `PlanRepo` | **Unified** — `from_repo()` | Steps in separate table; borrowing serialization |
| **heartbeat** | N/A (no persistent state) | N/A | N/A | No migration needed |

### 6.2 Persistence Patterns Comparison

| Feature | CronStore | GoalStore | PlanStore |
|---------|-----------|-----------|-----------|
| **File format** | Flat JSON (full rewrite) | JSONL append-only journal | JSONL append-only journal |
| **Compaction** | N/A | Auto at 100 stale entries | Auto at 100 stale entries |
| **Backup on compact** | N/A | No | Yes (`.jsonl.bak`) |
| **Lazy loading** | No (load on start) | Yes (`ensure_loaded()`) | Yes (`ensure_loaded()`) |
| **In-memory index** | `Vec<CronJob>` in CronStore | `HashMap<Uuid, Goal>` | `HashMap<Uuid, Plan>` |
| **Ordering** | By `next_run_at_ms` (runtime) | `Vec<Uuid>` insertion order | `Vec<Uuid>` insertion order |
| **Zero-copy mutation** | No | No | Yes (`get_mut()` + `persist_latest()`) |
| **SQL save strategy** | Full upsert + orphan delete | Direct CRUD delegation | Try create, fallback update; step diff |

### 6.3 SQL Row Types (from `storage` crate)

| Row Struct | Table | Key Type | Blob Fields |
|------------|-------|----------|-------------|
| `CronJobRow` | `cron_jobs` | `String` (8-char UUID) | `schedule: Value`, `payload: Value` |
| `GoalRow` | `goals` | `Uuid` | `metrics: Value`, `metadata: Value` |
| `GoalProjectLinkRow` | `goal_project_links` | `(Uuid, String)` | None |
| `PlanRow` | `plans` | `Uuid` | `backtrack_history: Value` |
| `PlanStepRow` | `plan_steps` | `Uuid` (FK to plans) | `expected_tools: Vec<String>` |
| `CalendarSyncStateRow` | `calendar_sync_states` | `String` (provider_id) | None |

---

## 7. Gap Analysis & Recommendations

### 7.1 Calendar System Gaps

| Priority | Gap | Impact | Recommendation |
|:--------:|-----|--------|----------------|
| **P1** | No bidirectional sync orchestrator | Events can be fetched and pushed individually, but no automated two-way sync loop exists in this crate | Implement a `SyncOrchestrator` that drives the full fetch→detect→resolve→push cycle |
| **P1** | Server-wins is the only conflict resolution strategy | User changes may be silently overwritten | Add configurable strategies: `ServerWins`, `ClientWins`, `LastWriteWins`, `Manual` |
| **P2** | Calendar sync state uses parallel file/SQL backends (not unified) | Caller must manually choose; easy to use wrong one | Consolidate into a single `CalendarSyncStore` with `from_repo()` pattern (like Goal/Plan) |
| **P2** | No CalDAV event caching | Every `get_events()` call hits the remote server | Add local event cache table in SQL; use ETags for conditional GETs |
| **P3** | Limited iCalendar support | No VALARM, RRULE, VTODO, EXDATE, ATTENDEE support | Extend parser/generator as needed (RRULE for recurring events is high value) |
| **P3** | No tests for CalDAV discovery | Discovery is complex (3-step PROPFIND sequence) with no unit tests | Add mock HTTP server tests using `mockito` or `wiremock` |

### 7.2 Scheduling System Gaps

| Priority | Gap | Impact | Recommendation |
|:--------:|-----|--------|----------------|
| **P1** | CronStore uses flat JSON (full rewrite) while Goal/Plan use JSONL journals | Inconsistent persistence pattern; full rewrite risks data loss on crash | Migrate to JSONL append-only journal pattern, or go SQL-only since `CronRepo` exists |
| **P2** | 100ms polling interval is inefficient for long-wait jobs | Unnecessary CPU wake-ups | Use `tokio::time::sleep_until()` with exact wake time; only poll frequently near deadline |
| **P2** | No timezone support in `CronSchedule::Cron` | `tz` field is parsed but ignored in `compute_next_run()` | Pass `tz` to `cron::Schedule` computation |
| **P2** | SQL backend tests missing | No test coverage for `from_repo()` path | Add integration tests with test database |
| **P3** | `JobCallback` is synchronous (`Fn` not `async Fn`) | Cannot perform async operations (network calls, DB writes) in callbacks | Switch to `AsyncFn` / boxed future callback |

### 7.3 Goal System Gaps

| Priority | Gap | Impact | Recommendation |
|:--------:|-----|--------|----------------|
| **P2** | No state transition validation | Unlike PlanStatus, GoalStatus transitions are unconstrained | Add `GoalStatus::validate_transition()` if business rules require it |
| **P2** | SQL backend tests missing | No test coverage for `from_repo()` path | Add integration tests with test database |
| **P2** | GoalStore does not create backup before compaction | Data loss risk during compaction | Add `.jsonl.bak` backup (PlanStore already does this) |
| **P3** | `linked_project_ids` stored as `Vec<Uuid>` but projects use `String` IDs in link table | Type mismatch: `GoalProjectLinkRow.project_id` is `String`, goal field is `Uuid` | Align types — either both String or both Uuid |
| **P3** | No goal hierarchy (sub-goals) | Can't model goal decomposition | Consider adding `parent_goal_id: Option<Uuid>` |

### 7.4 Planning System Gaps

| Priority | Gap | Impact | Recommendation |
|:--------:|-----|--------|----------------|
| **P1** | `execute_step()` passes `{}` as tool arguments | Tools must work without explicit parameters; most won't | Phase 5: LLM-based parameter generation for each step's tool calls |
| **P2** | No real-time progress streaming | Plan progress only visible between executions | Add `tokio::sync::watch` channel for step-level progress events |
| **P2** | SQL upsert uses try-create/catch-duplicate pattern | Not idempotent; race conditions possible | Use `INSERT ... ON CONFLICT DO UPDATE` (PostgreSQL UPSERT) |
| **P2** | SQL backend tests missing | No test coverage for `from_repo()` path | Add integration tests with test database |
| **P3** | `StepStatus` has no transition validation | Unlike PlanStatus | Add validation if needed to prevent invalid step state changes |

### 7.5 Heartbeat System Gaps

| Priority | Gap | Impact | Recommendation |
|:--------:|-----|--------|----------------|
| **P2** | No async service lifecycle tests | Only `is_heartbeat_empty()` is tested; start/stop/tick untested | Add tests for service lifecycle, callback invocation, HEARTBEAT_OK detection |
| **P3** | No persistence of heartbeat history | No record of past heartbeat executions | Add optional logging to SQL table or at minimum structured log output |
| **P3** | Manual `clone_service()` instead of `Clone` derive | Fragile if new fields are added | Consider wrapping service state in `Arc<Inner>` and deriving Clone |

### 7.6 Cross-Cutting Gaps

| Priority | Gap | Impact | Recommendation |
|:--------:|-----|--------|----------------|
| **P1** | All SQL backend paths lack test coverage | SQL code paths in Goal, Plan, Scheduling are untested | Create a shared test fixture with `PgPool` for integration testing |
| **P1** | No migration from file to SQL | Users with existing JSONL files have no path to SQL | Add `migrate_to_sql()` methods that read JSONL and write to SQL repos |
| **P2** | Inconsistent dual-backend pattern | Calendar uses parallel exports; others use `from_repo()` | Standardize on `from_repo()` pattern everywhere |
| **P2** | `CronStore` vs JSONL journal inconsistency | Flat JSON vs append-only JSONL for similar use cases | Standardize on one persistence strategy |
| **P3** | Goal and Plan crates don't use workspace versions | `goal = "0.1.0"`, `plan = "0.2.0"` — hardcoded versions | Migrate to `version.workspace = true` |
