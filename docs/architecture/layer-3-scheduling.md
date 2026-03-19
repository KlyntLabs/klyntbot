# Layer 3: Scheduling Crate

> `crates/scheduling/` -- Cron job scheduling with SQL persistence, named handlers, and deadline-driven timer loop.

## Overview

The `scheduling` crate provides `CronService`, a job scheduler that supports one-shot (`At`), recurring interval (`Every`), and cron expression (`Cron`) schedule types. Jobs are persisted to SQLite via `storage::CronRepo` and executed in a tokio-spawned timer loop that sleeps precisely until the next job deadline. A `Notify`-based wake mechanism ensures the timer recalculates when jobs are added, modified, or removed.

## Dependencies

| Dependency | Purpose |
|---|---|
| `common` | `KlyntbotError`, `Result` |
| `storage` | `CronRepo`, `CronJobRow` |
| `cron` | Cron expression parsing |
| `chrono`, `chrono-tz` | Timestamps and timezone support |
| `uuid` | Job ID generation |
| `tokio` | Async runtime, timers, `RwLock`, `Notify` |
| `thiserror` | Error derivation |
| `serde`, `serde_json` | Serialization |

## Architecture

```mermaid
classDiagram
    class CronService {
        -store: Arc~RwLock~CronStore~~
        -on_job: Option~JobCallback~
        -handlers: HashMap~String, JobCallback~
        -running: Arc~RwLock~bool~~
        -timer_task: Arc~RwLock~Option~JoinHandle~~~
        -repo: Option~CronRepo~
        -wake: Arc~Notify~
        +new(repo) CronService
        +start() Result
        +stop()
        +add_job(...) Result~CronJob~
        +remove_job(id) Result~bool~
        +enable_job(id, enabled) Result~Option~CronJob~~
        +run_job(id, force) Result~bool~
        +list_jobs(include_disabled) Vec~CronJob~
        +status() CronServiceStatus
        +set_callback(callback)
        +register_handler(name, callback)
    }

    class CronJob {
        +id: String
        +name: String
        +enabled: bool
        +origin: CronOrigin
        +schedule: CronSchedule
        +payload: CronPayload
        +state: CronJobState
        +created_at_ms: i64
        +updated_at_ms: i64
        +delete_after_run: bool
    }

    class CronSchedule {
        <<enum>>
        At(at_ms: i64)
        Every(every_ms: u64)
        Cron(expr: String, tz: Option~String~)
    }

    class CronPayload {
        +kind: String
        +message: String
        +deliver: bool
        +channel: Option~String~
        +to: Option~String~
    }

    class CronJobState {
        +next_run_at_ms: Option~i64~
        +last_run_at_ms: Option~i64~
        +last_status: Option~String~
        +last_error: Option~String~
    }

    class CronOrigin {
        <<enum>>
        System
        User
        Ai
        Plugin
    }

    class CronStore {
        +version: u32
        +jobs: Vec~CronJob~
    }

    class CronServiceStatus {
        +enabled: bool
        +jobs: usize
        +next_wake_at_ms: Option~i64~
    }

    class CronError {
        <<enum>>
        InvalidExpression(String)
        JobNotFound(String)
        ExecutionFailed(String)
        Io(io::Error)
        Json(serde_json::Error)
    }

    CronService *-- CronStore : manages
    CronStore *-- CronJob : contains
    CronJob *-- CronSchedule
    CronJob *-- CronPayload
    CronJob *-- CronJobState
    CronJob *-- CronOrigin
```

## Public Types

### `CronSchedule`

Tagged enum (`#[serde(tag = "kind")]`) with three schedule types:

| Variant | Fields | Description |
|---|---|---|
| `At` | `at_ms: i64` | One-time execution at a specific UTC timestamp (ms) |
| `Every` | `every_ms: u64` | Recurring execution at fixed intervals (ms) |
| `Cron` | `expr: String, tz: Option<String>` | Cron expression (6-field: sec min hour day month dow) with optional timezone |

Timezone support uses `chrono-tz` for named timezones (e.g., `America/New_York`, `Asia/Tokyo`). Invalid timezone strings fall back to UTC with a warning.

### `CronPayload`

Defines what happens when a job fires:

| Field | Type | Default | Description |
|---|---|---|---|
| `kind` | `String` | `"agent_turn"` | Payload type |
| `message` | `String` | `""` | Message content to deliver or process |
| `deliver` | `bool` | `false` | Whether to deliver the response to a channel |
| `channel` | `Option<String>` | `None` | Target channel (e.g., `"telegram"`) |
| `to` | `Option<String>` | `None` | Target chat/user ID |

### `CronOrigin`

Identifies who created the job:
- `System` -- built-in system jobs
- `User` -- user-created via UI/command
- `Ai` -- AI-created via tool call
- `Plugin` -- plugin-created

### `CronJobState`

Runtime state tracked per-job:

| Field | Type | Description |
|---|---|---|
| `next_run_at_ms` | `Option<i64>` | Next scheduled execution time (UTC ms) |
| `last_run_at_ms` | `Option<i64>` | Last execution time |
| `last_status` | `Option<String>` | `"ok"`, `"error"`, or `"skipped"` |
| `last_error` | `Option<String>` | Error message from last failed execution |

### `CronJob`

Complete job definition. All serialization uses `#[serde(rename_all = "camelCase")]`.

### `CronServiceStatus`

Snapshot of service state: `{ enabled, jobs, next_wake_at_ms }`.

### `CronError`

Cron-specific errors. Converts to `KlyntbotError::Cron(String)` via `From`.

| Variant | Description |
|---|---|
| `InvalidExpression` | Invalid cron expression syntax |
| `JobNotFound` | Job ID not found |
| `ExecutionFailed` | Callback returned an error |
| `Io` | File I/O error |
| `Json` | JSON serialization error |

## `JobCallback`

```rust
pub type JobCallback = Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>;
```

Callback type for job execution. Returns `Ok(Some(response))` on success or `Err` on failure.

## CronService

### Construction and Setup

```rust
let mut service = CronService::new(repo);
service.set_callback(fallback_callback);           // generic fallback handler
service.register_handler("daily-review", handler); // named handler for specific job name
service.start().await?;
```

### Timer Loop

```mermaid
flowchart TD
    A[Start Timer Loop] --> B{Running?}
    B -->|No| Z[Exit]
    B -->|Yes| C[Compute next_wake_ms]
    C --> D{Has scheduled job?}
    D -->|Yes| E["sleep_duration = next_wake - now"]
    D -->|No| F["sleep_duration = 24 hours"]
    E --> G["select! { sleep_until(deadline), wake.notified() }"]
    F --> G
    G --> H{Still running?}
    H -->|No| Z
    H -->|Yes| I{Any jobs due?}
    I -->|Yes| J[Process due jobs]
    I -->|No| B
    J --> K[Save store to SQL]
    K --> B
```

Key design points:
- Uses `tokio::time::sleep_until()` for precise deadline-driven scheduling (no polling)
- `Notify` wakes the loop immediately when jobs are added/modified/removed
- `tokio::select!` allows early wake from both timer and notify

### Job Execution Flow

```mermaid
flowchart TD
    A[Execute Job] --> B{Named handler registered?}
    B -->|Yes| C[Call named handler]
    B -->|No| D{Fallback callback set?}
    D -->|Yes| E[Call fallback callback]
    D -->|No| F["Status: skipped"]
    C --> G{Success?}
    E --> G
    G -->|Yes| H["Status: ok"]
    G -->|No| I["Status: error, record error message"]
    H --> J{Schedule type?}
    I --> J
    F --> J
    J -->|At + delete_after_run| K[Delete job from store]
    J -->|At + !delete_after_run| L[Disable job, clear next_run]
    J -->|Every / Cron| M[Compute next run time]
```

### Public API

| Method | Description |
|---|---|
| `new(repo)` | Create service backed by SQL `CronRepo` |
| `start()` | Load from SQL, recompute schedules, start timer loop |
| `stop()` | Set running=false, abort timer task |
| `set_callback(callback)` | Set fallback execution callback |
| `register_handler(name, callback)` | Register named handler (checked first during execution) |
| `add_job(name, schedule, message, deliver, channel, to, delete_after_run, origin)` | Add a new job, auto-generates UUID-based ID |
| `remove_job(job_id)` | Remove by ID, returns whether job existed |
| `enable_job(job_id, enabled)` | Enable/disable; recomputes next_run on enable, clears on disable |
| `run_job(job_id, force)` | Manually execute; `force=true` runs even if disabled |
| `list_jobs(include_disabled)` | List jobs sorted by next_run_at_ms |
| `status()` | Returns `CronServiceStatus` snapshot |

## Persistence (SQL)

### Store Module (`service/store.rs`)

- `load_store()`: Reads all rows from `CronRepo`, converts to domain `CronJob` objects
- `save_store()`: Upserts all in-memory jobs to SQL, deletes orphaned SQL rows not in memory
- Corrupt schedule JSON disables the job with an error log (never panics)
- Unknown `origin` values default to `User` with a warning

### Domain <-> SQL Conversion

`job_to_row()` and `row_to_job()` handle bidirectional conversion between `CronJob` and `CronJobRow`. Schedule and payload are serialized as JSON values.

## Schedule Computation

`compute_next_run(schedule, now_ms)` computes the next execution time:

| Schedule | Logic |
|---|---|
| `At { at_ms }` | Returns `Some(at_ms)` if in the future, `None` if past |
| `Every { every_ms }` | Returns `Some(now + every_ms)` |
| `Cron { expr, tz }` | Parses cron expression, computes next occurrence in the given timezone (UTC if unspecified or invalid) |

## Thread Safety

- `CronService` uses `Arc<RwLock<_>>` for shared mutable state
- Timer loop runs as a detached `tokio::spawn` task
- `Notify` provides a lock-free wake mechanism
- All public methods use async locking (no blocking)
