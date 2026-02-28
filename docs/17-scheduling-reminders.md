# Scheduling & Reminders

This document covers three cooperating subsystems that drive time-based automation in Klyntbot: the **CronService** (general-purpose job scheduler), the **ReminderEngine** (due-date and calendar alert system), and the **RecurringTaskSpawner** (RRULE-based task instance creator). A **CronHandlerAdapter** bridges the scheduling crate into the tools layer via dependency inversion.

---

## Section 1: Narrative Overview

### CronService design

`CronService` lives in the `scheduling` crate (Layer 2) and provides a general-purpose, SQL-backed cron job scheduler. It holds an in-memory `CronStore` (a versioned `Vec<CronJob>`) guarded by an `Arc<RwLock<_>>`, which is synchronised to SQLite through the `storage::CronRepo` repository on every mutation.

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/mod.rs` (lines 89-99)

The service struct contains:

- `store: Arc<RwLock<CronStore>>` -- the authoritative in-memory job list.
- `on_job: Option<JobCallback>` -- a synchronous callback (`Arc<dyn Fn(&CronJob) -> Result<Option<String>>>`) invoked when a job fires.
- `repo: Option<storage::CronRepo>` -- SQL backend; `None` only in unit tests.
- `wake: Arc<Notify>` -- tokio `Notify` that wakes the timer loop early when jobs are added, removed, or modified.
- `running: Arc<RwLock<bool>>` -- shutdown flag.
- `timer_task: Arc<RwLock<Option<JoinHandle<()>>>>` -- handle to the background timer task.

#### Startup sequence

`CronService::start()` (line 221):

1. Sets `running = true`.
2. Calls `load_store()` -- reads all `CronJobRow` records from SQLite via `CronRepo::list()` and converts them to domain `CronJob` objects.
3. Calls `recompute_next_runs()` -- iterates every enabled job and calls `compute_next_run()` to set `state.next_run_at_ms`.
4. Calls `save_store()` -- upserts every in-memory job back to SQL (and deletes SQL rows that are no longer in memory).
5. Spawns the timer loop via `start_timer_loop()`.

#### Timer loop

`start_timer_loop()` (line 164) spawns a tokio task that:

1. Computes the earliest `next_run_at_ms` across all enabled jobs.
2. Sleeps via `tokio::time::sleep_until(deadline)` with a `tokio::select!` that also listens on `wake.notified()`. This means the loop wakes either when a job is due or when a job mutation signals re-evaluation.
3. When awake, calls `process_due_jobs()` which collects all jobs where `now >= next_run_at_ms`, executes each one, and saves the store.

### How cron jobs are created, stored, and executed

**Creation:** `CronService::add_job()` (line 261) generates a UUID-based 8-char ID, constructs a `CronJob`, computes `next_run_at_ms`, pushes it into the in-memory store, saves to SQL, and signals `wake.notify_one()` so the timer loop re-evaluates.

**Storage:** The in-memory `CronStore.jobs` vector is the source of truth at runtime. On every mutation (add, remove, enable/disable, execution), `save_store()` performs a full sync to SQL:
- Upserts every in-memory job via `CronRepo::upsert()`.
- Deletes any SQL row whose ID is no longer in memory.

This approach is defined in `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/store.rs` (lines 41-68).

**Execution:** When a job fires, `execute_job_static()` in `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/executor.rs` (lines 13-76):

1. Records the start timestamp.
2. Invokes the `JobCallback`. If it returns `Ok`, status is `"ok"`; if `Err`, status is `"error"` with the error message captured.
3. If no callback is configured, status is `"skipped"`.
4. Updates `last_run_at_ms`, `last_status`, `last_error`, `updated_at_ms` on the in-memory job.
5. Handles post-execution behaviour by schedule type:
   - **`At` (one-shot):** If `delete_after_run` is true, removes the job from the store. Otherwise, disables the job and clears `next_run_at_ms`.
   - **`Every` or `Cron` (recurring):** Computes the next run time via `compute_next_run()`.

### Cron expression parsing

Three schedule variants are supported in `CronSchedule` (`/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/types.rs`, lines 7-31):

| Variant | Fields | Behaviour |
|---------|--------|-----------|
| `At` | `at_ms: i64` | One-shot execution at a specific Unix timestamp (ms). Returns `None` from `compute_next_run` if the time is in the past. |
| `Every` | `every_ms: u64` | Fixed-interval recurrence. Next run = `now + every_ms`. |
| `Cron` | `expr: String, tz: Option<String>` | Standard 6-field cron expression (sec min hour day month dow) parsed by the `cron` crate. Optional timezone via `chrono-tz`. Invalid timezone strings fall back to UTC with a warning. |

The `compute_next_run()` function (`/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/mod.rs`, lines 30-83) handles all three variants. For the `Cron` variant, it parses the expression with `cron::Schedule::try_from()`, then calls `.upcoming(tz).next()` to get the next fire time.

### ReminderEngine

`ReminderEngine` lives in `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/reminders.rs` and implements a periodic check loop for todo and calendar event reminders. It follows the same pattern as `RecurringTaskSpawner`: a tokio task with `CancellationToken` for graceful shutdown.

**Architecture:**

- Holds a `storage::TodoRepo` for querying todos.
- Optionally holds an `Arc<dyn CalendarHandler>` for fetching upcoming calendar events.
- Dispatches notifications through a shared `Arc<NotificationDispatcher>`.
- Runs at a configurable `check_interval` (a `std::time::Duration`).

**Reminder rules** (checked every interval):

1. **Due date alerts (Rule #1):** Fires when a todo's `due_date` is within 2 hours and in the future, and `last_reminded_at` is `None`. After firing, sets `last_reminded_at` to now via `TodoRepo::update()`.

2. **Focused deadline alerts (Rule #2):** Fires when a todo has `focused_at` set, `focus_deadline` is within 1 hour and in the future, and `last_reminded_at` is `None`.

3. **Overdue nagging (Rule #3):** Fires when a todo's `due_date` is in the past. Repeats once per 24 hours (checks `last_reminded_at` for debouncing).

4. **Calendar event alerts (Rule #4):** Fires when a calendar event's `start` is within 30 minutes and in the future, and `last_reminded_at` is `None`. Fetches up to 10 upcoming events from the `CalendarHandler`.

**Notification dispatch** goes through `NotificationDispatcher` (`/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/notifications.rs`), which sends to configured targets: `os_native` (macOS/Linux system notifications) and/or chat channels (Telegram, Discord, etc.) via the outbound message bus.

### RecurringTaskSpawner

`RecurringTaskSpawner` lives in `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/recurring_tasks.rs` and handles RRULE-based task recurrence. It periodically scans template todos and spawns concrete instances when they are due.

**How it works:**

1. Queries `TodoRepo::list_templates()` to get all template todos.
2. For each template with a `recurrence_rule`, calls `rrule_utils::should_spawn_instance(next_instance_date, now)` -- returns true when `next_instance_date <= now`.
3. Creates a new `Todo` instance cloned from the template (title, description, priority, tags, project), with:
   - `recurrence_parent_id` set to the template ID.
   - `due_date` set to the template's `next_instance_date`.
   - `is_template = false`.
4. Inserts the instance via `TodoRepo::add()`.
5. Advances the template's `next_instance_date` by calling `rrule_utils::next_occurrence()` and updating via `TodoRepo::update()`.

The RRULE parsing logic lives in `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/feature-todo/src/rrule_utils.rs`. It supports `FREQ` (DAILY, WEEKLY, MONTHLY, YEARLY), `INTERVAL`, `BYDAY`, `BYHOUR`, `BYMINUTE`, `BYMONTHDAY`, `COUNT`, `UNTIL`, and `EXDATE`. Unsupported parameters (`BYSETPOS`, `WKST`, `EXRULE`, `RDATE`) are rejected at parse time. The underlying computation delegates to the `rrule` crate for date generation.

### CronHandlerAdapter

`CronHandlerAdapter` (`/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/cron_handler_adapter.rs`) bridges the `scheduling::CronService` (Layer 2) to the `tools::cron_tool::CronHandler` trait (Layer 3). This follows the dependency inversion pattern used throughout Klyntbot: the trait is defined in the lower-layer `tools` crate, and the implementation lives in the higher-layer `agent` crate.

The adapter wraps an `Arc<CronService>` and:

- Converts `tools::cron_tool::CronSchedule` to `scheduling::CronSchedule` (the two enums have the same structure but live in different crates to avoid a direct dependency from `tools` on `scheduling`).
- Maps `AddCronJobParams` fields to `CronService::add_job()` arguments, inverting `params.internal` to the `deliver` flag.
- Converts returned `CronJob` into `CronJobInfo` for the tools layer.

### How these three systems interact

```
User via chat
    |
    v
CronTool (Layer 3, tools crate)
    | uses Arc<dyn CronHandler>
    v
CronHandlerAdapter (Layer 5, agent crate)
    | delegates to
    v
CronService (Layer 2, scheduling crate)
    | persists to
    v
CronRepo (Layer 1.5, storage crate) --> SQLite
```

The three background services run independently during `klyntbot serve`:

- **CronService** manages general-purpose scheduled jobs (reminders, recurring agent prompts). When a job fires, its callback typically sends a message back through the agent loop or dispatches to a channel.
- **ReminderEngine** specifically monitors todo due dates, focus deadlines, overdue tasks, and calendar events. It reads from `TodoRepo` and `CalendarHandler`, and pushes notifications through `NotificationDispatcher`.
- **RecurringTaskSpawner** specifically handles RRULE-based task templates. It reads template todos from `TodoRepo`, spawns instances, and advances the template's `next_instance_date`.

All three follow the same lifecycle pattern: construct -> `start()` -> background tokio task -> `stop()` via `CancellationToken` (or `running` flag for CronService).

---

## Section 2: API Reference

### CronService

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/mod.rs`

```rust
pub struct CronService {
    pub(crate) store: Arc<RwLock<CronStore>>,
    pub(crate) on_job: Option<JobCallback>,
    pub(crate) running: Arc<RwLock<bool>>,
    pub(crate) timer_task: Arc<RwLock<Option<JoinHandle<()>>>>,
    pub(crate) repo: Option<storage::CronRepo>,
    pub(crate) wake: Arc<Notify>,
}
```

#### Constructor

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(repo: storage::CronRepo) -> Self` | Creates a service backed by SQL persistence. (line 103) |

#### Public methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `set_callback` | `fn set_callback(&mut self, callback: JobCallback)` | Sets the function invoked when a job fires. Must be called before `start()`. (line 128) |
| `start` | `async fn start(&self) -> Result<()>` | Loads store from SQL, recomputes next-run times, saves, and starts the timer loop. (line 221) |
| `stop` | `async fn stop(&self)` | Sets `running = false` and aborts the timer task. (line 236) |
| `list_jobs` | `async fn list_jobs(&self, include_disabled: bool) -> Vec<CronJob>` | Returns jobs sorted by `next_run_at_ms`. If `include_disabled` is false, only returns enabled jobs. (line 246) |
| `add_job` | `async fn add_job(&self, name, schedule, message, deliver, channel, to, delete_after_run) -> Result<CronJob>` | Creates a job with a UUID-based 8-char ID, saves to SQL, wakes the timer loop. (line 261) |
| `remove_job` | `async fn remove_job(&self, job_id: impl AsRef<str>) -> Result<bool>` | Removes a job by ID. Returns `true` if found and removed. (line 296) |
| `enable_job` | `async fn enable_job(&self, job_id, enabled: bool) -> Result<Option<CronJob>>` | Enables or disables a job. Disabling clears `next_run_at_ms`; enabling recomputes it. (line 316) |
| `run_job` | `async fn run_job(&self, job_id, force: bool) -> Result<bool>` | Manually executes a job. If `force` is false, skips disabled jobs. (line 349) |
| `status` | `async fn status(&self) -> serde_json::Value` | Returns `{ enabled, jobs, nextWakeAtMs }`. (line 414) |

#### JobCallback type

```rust
pub type JobCallback = Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>;
```

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/mod.rs` (line 86)

### CronJob

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/types.rs` (lines 88-113)

```rust
pub struct CronJob {
    pub id: String,               // UUID-based 8-char identifier
    pub name: String,             // Human-readable job name
    pub enabled: bool,            // Whether the job is active (default: true)
    pub schedule: CronSchedule,   // When to run (At, Every, or Cron)
    pub payload: CronPayload,     // What to do when the job fires
    pub state: CronJobState,      // Runtime execution state
    pub created_at_ms: i64,       // Creation timestamp (Unix ms)
    pub updated_at_ms: i64,       // Last update timestamp (Unix ms)
    pub delete_after_run: bool,   // If true, removes the job after execution
}
```

#### CronSchedule

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/types.rs` (lines 7-31)

```rust
pub enum CronSchedule {
    At { at_ms: i64 },                              // One-shot at a timestamp
    Every { every_ms: u64 },                        // Fixed interval in milliseconds
    Cron { expr: String, tz: Option<String> },      // 6-field cron expression + optional timezone
}
```

Serialized as tagged JSON with `"kind"` discriminator: `"at"`, `"every"`, `"cron"`.

#### CronPayload

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/types.rs` (lines 34-52)

```rust
pub struct CronPayload {
    pub kind: String,              // Payload type (default: "agent_turn")
    pub message: String,           // Message content for the callback
    pub deliver: bool,             // Whether to send the response to a channel
    pub channel: Option<String>,   // Target channel name (e.g. "telegram")
    pub to: Option<String>,        // Target chat ID
}
```

#### CronJobState

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/types.rs` (lines 71-85)

```rust
pub struct CronJobState {
    pub next_run_at_ms: Option<i64>,    // Next scheduled execution (Unix ms)
    pub last_run_at_ms: Option<i64>,    // Last execution timestamp (Unix ms)
    pub last_status: Option<String>,    // "ok", "error", or "skipped"
    pub last_error: Option<String>,     // Error message if last run failed
}
```

#### CronStore

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/types.rs` (lines 120-127)

```rust
pub struct CronStore {
    pub version: u32,       // Schema version (always 1)
    pub jobs: Vec<CronJob>, // All jobs
}
```

### Job executor internals

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/executor.rs`

```rust
pub(crate) async fn execute_job_static(
    store: &Arc<RwLock<CronStore>>,
    on_job: &Option<JobCallback>,
    job: &CronJob,
)
```

This is the core execution function (line 13). It:

1. Records `start_ms = now_ms()`.
2. Invokes the callback, producing a status (`"ok"`, `"error"`, or `"skipped"`) and optional error message.
3. Acquires a write lock on the store and updates the job's `state` fields.
4. For `At` schedule jobs: either deletes the job (if `delete_after_run`) or disables it.
5. For recurring schedules (`Every`, `Cron`): calls `compute_next_run()` to set the next fire time.

**`process_due_jobs()` (store module):**

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/store.rs` (lines 71-105)

Collects all jobs where `enabled && now >= next_run_at_ms`, executes each via `execute_job_static()`, then saves the store to SQL.

**`compute_next_run()`:**

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/service/mod.rs` (lines 30-83)

| Schedule | Logic |
|----------|-------|
| `At { at_ms }` | Returns `Some(at_ms)` if in the future, else `None`. |
| `Every { every_ms }` | Returns `Some(now_ms + every_ms)` if `every_ms > 0`. |
| `Cron { expr, tz }` | Parses `expr` via `cron::Schedule::try_from()`. Computes next occurrence using `chrono-tz` if `tz` is provided. Falls back to UTC on invalid timezone. |

### ReminderEngine

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/reminders.rs`

```rust
pub struct ReminderEngine {
    todo_repo: storage::TodoRepo,
    calendar_handler: Option<Arc<dyn CalendarHandler>>,
    dispatcher: Arc<NotificationDispatcher>,
    check_interval: StdDuration,
    task_handle: Option<JoinHandle<()>>,
    cancel_token: CancellationToken,
}
```

#### Constructor

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(todo_repo, calendar_handler, dispatcher, check_interval) -> Self` | Creates the engine. Does not start the background task. (line 39) |

#### Lifecycle methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `start` | `fn start(&mut self)` | Spawns a tokio task that loops: sleep for `check_interval`, then call `check_and_send_reminders()`. Cancellable via `CancellationToken`. (line 56) |
| `stop` | `async fn stop(&mut self)` | Cancels the token and awaits the task handle. (line 87) |

#### Reminder rule methods (all `pub`)

| Method | Signature | Rule | Description |
|--------|-----------|------|-------------|
| `should_remind_due_date` | `fn(todo: &Todo) -> bool` | #1 | True if `due_date` is within 2 hours, in the future, and `last_reminded_at` is `None`. (line 223) |
| `should_remind_focused_deadline` | `fn(todo: &Todo) -> bool` | #2 | True if `focused_at` is set, `focus_deadline` is within 1 hour and in the future, and `last_reminded_at` is `None`. (line 240) |
| `should_remind_overdue` | `fn(todo: &Todo) -> bool` | #3 | True if `due_date` is in the past and either never reminded or last reminded over 24 hours ago. (line 261) |
| `should_remind_calendar_event` | `fn(event: &CalendarEvent) -> bool` | #4 | True if `start` is within 30 minutes, in the future, and `last_reminded_at` is `None`. (line 283) |

#### CalendarEvent

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/reminders.rs` (lines 18-25)

```rust
pub struct CalendarEvent {
    pub uid: String,
    pub summary: String,
    pub start: DateTime<Utc>,
    pub last_reminded_at: Option<DateTime<Utc>>,
}
```

### RecurringTaskSpawner

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/recurring_tasks.rs`

```rust
pub struct RecurringTaskSpawner {
    todo_repo: storage::TodoRepo,
    timezone: String,
    check_interval: StdDuration,
    task_handle: Option<JoinHandle<()>>,
    cancel_token: CancellationToken,
}
```

#### Constructor

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(todo_repo, timezone: String, check_interval) -> Self` | Creates the spawner. `timezone` is stored but currently unused (reserved for future TZ-aware spawning). (line 27) |

#### Lifecycle methods

| Method | Signature | Description |
|--------|-----------|-------------|
| `start` | `fn start(&mut self)` | Spawns a tokio task that loops: sleep for `check_interval`, then call `check_and_spawn()`. (line 42) |
| `stop` | `async fn stop(&mut self)` | Cancels the token and awaits the task handle. (line 72) |

#### Internal method

| Method | Signature | Description |
|--------|-----------|-------------|
| `check_and_spawn` | `async fn check_and_spawn(repo, timezone) -> Result<()>` | Queries template todos, spawns instances for those where `next_instance_date <= now`, and advances `next_instance_date` via `rrule_utils::next_occurrence()`. (line 80) |

### RRULE utilities

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/feature-todo/src/rrule_utils.rs`

| Function | Signature | Description |
|----------|-----------|-------------|
| `validate_rrule` | `fn(rule: &str) -> Result<()>` | Parses and validates an RRULE string. Rejects unsupported parameters (BYSETPOS, WKST, EXRULE, RDATE). (line 221) |
| `next_occurrence` | `fn(rule: &str, after: DateTime<Utc>) -> Result<Option<DateTime<Utc>>>` | Computes the next occurrence of an RRULE after the given datetime. (line 232) |
| `should_spawn_instance` | `fn(next_instance_date: Option<DateTime<Utc>>, now: DateTime<Utc>) -> bool` | Returns true if `next_instance_date` is present and `<= now`. (line 239) |
| `humanize_rrule` | `fn(rule: &str) -> String` | Converts an RRULE string to human-readable text (e.g. "Every week on Monday, Wednesday, Friday"). (line 247) |

#### RRule struct

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/feature-todo/src/rrule_utils.rs` (lines 46-56)

```rust
pub struct RRule {
    pub freq: Frequency,              // DAILY, WEEKLY, MONTHLY, YEARLY
    pub interval: u32,                // INTERVAL (default: 1)
    pub byday: Vec<String>,           // BYDAY (e.g. ["MO", "WE", "FR"])
    pub byhour: Vec<u32>,             // BYHOUR
    pub byminute: Vec<u32>,           // BYMINUTE
    pub bymonthday: Vec<u32>,         // BYMONTHDAY
    pub count: Option<u32>,           // COUNT
    pub until: Option<DateTime<Utc>>, // UNTIL
    pub exdate: Vec<DateTime<Utc>>,   // EXDATE (excluded dates)
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `parse` | `fn parse(rule: &str) -> Result<Self>` | Parses an RRULE string into the struct. Rejects unsupported parameters. (line 70) |
| `next_occurrences` | `fn next_occurrences(&self, from, max) -> Result<Vec<DateTime<Utc>>>` | Computes up to `max` occurrences after `from` using the `rrule` crate. (line 192) |

### CronHandler trait and CronHandlerAdapter

#### CronHandler trait

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/tools/src/cron_tool.rs` (lines 54-64)

```rust
#[async_trait]
pub trait CronHandler: Send + Sync {
    async fn add_job(&self, params: AddCronJobParams) -> Result<CronJobInfo>;
    async fn list_jobs(&self, include_internal: bool) -> Vec<CronJobInfo>;
    async fn remove_job(&self, job_id: &str) -> Result<bool>;
}
```

#### Supporting types (tools crate)

**AddCronJobParams** (line 42):

```rust
pub struct AddCronJobParams {
    pub name: String,
    pub schedule: CronSchedule,  // tools::cron_tool::CronSchedule (Every | Cron)
    pub message: String,
    pub enabled: bool,
    pub channel: Option<String>,
    pub to: Option<String>,
    pub internal: bool,
}
```

**CronJobInfo** (line 33):

```rust
pub struct CronJobInfo {
    pub id: String,
    pub name: String,
    pub next_run_at_ms: Option<u64>,
    pub last_status: Option<String>,
}
```

**CronSchedule (tools)** (line 24):

```rust
pub enum CronSchedule {
    Every { every_ms: u64 },
    Cron { expr: String, tz: Option<String> },
}
```

Note: The tools-layer `CronSchedule` has only `Every` and `Cron` variants (no `At`), since one-shot scheduling is handled internally by the service layer.

#### CronHandlerAdapter

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/cron_handler_adapter.rs`

```rust
pub struct CronHandlerAdapter {
    service: Arc<CronService>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(service: Arc<CronService>) -> Self` | Wraps a CronService. (line 18) |

The adapter implements `CronHandler` (line 32) by:

- **`add_job`:** Converts `tools::CronSchedule` to `scheduling::CronSchedule` via `convert_schedule()`, maps `!params.internal` to `deliver`, passes `delete_after_run = false`, then delegates to `CronService::add_job()`. Returns a `CronJobInfo`.
- **`list_jobs`:** Calls `CronService::list_jobs(!include_internal)` and maps each `CronJob` to `CronJobInfo`.
- **`remove_job`:** Delegates directly to `CronService::remove_job()`.

### NotificationDispatcher

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/agent/src/notifications.rs`

```rust
pub struct NotificationDispatcher {
    outbound_tx: mpsc::Sender<OutboundMessage>,
    config: TodoNotificationConfig,
    last_active_channel: Arc<RwLock<Option<(ChannelName, ChatId)>>>,
}
```

| Method | Signature | Description |
|--------|-----------|-------------|
| `new` | `fn new(outbound_tx, config) -> Self` | Creates a dispatcher with configured notification targets. (line 20) |
| `last_active_handle` | `fn last_active_handle(&self) -> Arc<RwLock<Option<(ChannelName, ChatId)>>>` | Returns a cloneable handle for tracking the last active channel. (line 29) |
| `notify` | `async fn notify(&self, title: &str, body: &str) -> Result<()>` | Sends notifications to all configured targets. Supports `"os_native"` (system notifications) and channel names (sends via outbound message bus if the channel matches the last active one). (line 34) |

### Error types

#### CronError

**File:** `/Users/jayden/Projects/Klynt/nanobot/klyntbot/crates/scheduling/src/error.rs`

```rust
pub enum CronError {
    InvalidExpression(String),   // Invalid cron expression
    JobNotFound(String),         // Job ID not found
    ExecutionFailed(String),     // Job callback returned an error
    Io(std::io::Error),          // I/O error (from trait)
    Json(serde_json::Error),     // JSON serialization error (from trait)
}
```

Converts to `common::KlyntbotError::Cron(String)` via a `From` impl (line 24).

#### StorageError (from storage crate)

Used by `CronRepo` methods. Propagated through `CronError::ExecutionFailed` in the store module when SQL operations fail.

#### Common error flow

```
CronRepo (StorageError) --> CronError::ExecutionFailed --> KlyntbotError::Cron
```

For `ReminderEngine` and `RecurringTaskSpawner`, errors from `TodoRepo` operations propagate as `common::KlyntbotError::Storage` and are logged but do not crash the background task (the loop continues after logging the error).
