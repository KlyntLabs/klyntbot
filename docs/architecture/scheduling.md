# Scheduling System

## Overview

Klyntbot includes an async scheduling system implemented in the `scheduling` crate (layer L3). It supports one-shot reminders, fixed-interval recurring tasks, and cron expression-based schedules. Jobs are persisted to SQLite via `CronRepo`, ensuring crash recovery across restarts.

The system consists of three modules:

- **`types`** -- Data structures (`CronJob`, `CronSchedule`, `CronPayload`, `CronStore`)
- **`service`** -- Core service (`CronService`), split into `executor` and `store` submodules
- **`error`** -- Error types (`CronError`)

## Schedule Types

Three schedule variants are defined in `CronSchedule`, a tagged enum serialized with `{"kind": "..."}`:

### At (One-Shot)

Executes once at a specific UTC timestamp in milliseconds.

```json
{ "kind": "at", "atMs": 1709251200000 }
```

If the timestamp is in the past at evaluation time, `compute_next_run` returns `None` and the job will not fire.

### Every (Fixed Interval)

Executes repeatedly with a fixed interval in milliseconds.

```json
{ "kind": "every", "everyMs": 60000 }
```

After each execution, the next run time is computed as `now + every_ms`.

### Cron (Cron Expression)

Executes according to a standard cron expression with an optional timezone.

```json
{ "kind": "cron", "expr": "0 0 9 * * *", "tz": "America/New_York" }
```

The expression uses 6-field format: `sec min hour day month day-of-week`. Timezone parsing uses `chrono-tz`; if the timezone string is invalid, the system falls back to UTC with a warning. When `tz` is `None`, UTC is used.

## CronService

`CronService` is the central component that manages job lifecycle and execution. It holds:

| Field | Type | Purpose |
|-------|------|---------|
| `store` | `Arc<RwLock<CronStore>>` | In-memory job store |
| `on_job` | `Option<JobCallback>` | Fallback callback for unregistered job names |
| `handlers` | `HashMap<String, JobCallback>` | Named handlers, checked first |
| `running` | `Arc<RwLock<bool>>` | Service running flag |
| `timer_task` | `Arc<RwLock<Option<JoinHandle<()>>>>` | Handle to the timer loop task |
| `repo` | `Option<CronRepo>` | SQL backend (None only in tests) |
| `wake` | `Arc<Notify>` | Signals the timer loop to re-evaluate |

### Timer Loop

The timer loop runs as a spawned `tokio` task and uses `tokio::select!` for efficient sleeping:

```
loop {
    1. Check if still running (break if not)
    2. Compute earliest next_run_at_ms across all enabled jobs
    3. Calculate sleep_duration (or 24h if no jobs are scheduled)
    4. tokio::select! {
         sleep_until(deadline) => {}    // Normal wake at job deadline
         wake.notified() => {}          // Early wake from job add/remove/modify
       }
    5. Check if still running after wake
    6. If any jobs are due (now >= next_wake_ms), process them
}
```

The `Notify`-based wake mechanism ensures the timer loop immediately re-evaluates when jobs are added, removed, or modified, without polling.

### Startup Flow

When `CronService::start()` is called:

1. Set `running = true`
2. Load all jobs from SQL via `load_store()`
3. Recompute `next_run_at_ms` for all enabled jobs
4. Save the updated store back to SQL
5. Start the timer loop

### Shutdown

`CronService::stop()` sets `running = false` and aborts the timer task.

## Job Persistence

### SQL Storage via CronRepo

Jobs are persisted through the `CronRepo` repository (from the `storage` crate). The service maintains a dual representation:

- **In-memory:** `CronStore` holds a `Vec<CronJob>` for fast access
- **On-disk:** SQL rows via `CronJobRow` for crash recovery

Conversion between domain types and SQL rows is handled by `job_to_row()` and `row_to_job()`. Corrupt schedule JSON in a row causes the job to be disabled with a fallback schedule.

### Save Strategy

`save_store()` performs a full sync:

1. **Upsert** all current in-memory jobs to SQL
2. **Delete** any SQL rows that no longer exist in memory (handles `remove_job`)

This approach is called on every mutation (add, remove, enable/disable, execution) to ensure the on-disk state stays current.

### Crash Recovery

On startup, `load_store()` reads all rows from SQL and reconstructs the in-memory store. `recompute_next_runs()` then recalculates next run times for all enabled jobs, accounting for any time that passed while the service was down.

### CronJob Fields

| Field | Type | Description |
|-------|------|-------------|
| `id` | `String` | UUID prefix (first 8 chars) |
| `name` | `String` | Human-readable job name |
| `enabled` | `bool` | Whether the job is active |
| `origin` | `CronOrigin` | Who created it: `system`, `user`, `ai`, or `plugin` |
| `schedule` | `CronSchedule` | Schedule definition (At, Every, or Cron) |
| `payload` | `CronPayload` | What to do when the job runs |
| `state` | `CronJobState` | Runtime state (next/last run, status, error) |
| `created_at_ms` | `i64` | Creation timestamp (UTC millis) |
| `updated_at_ms` | `i64` | Last update timestamp (UTC millis) |
| `delete_after_run` | `bool` | Auto-delete after execution (for one-shot jobs) |

### CronPayload

The payload describes what happens when a job fires:

| Field | Type | Description |
|-------|------|-------------|
| `kind` | `String` | Payload type (default: `"agent_turn"`) |
| `message` | `String` | The message or prompt to deliver |
| `deliver` | `bool` | Whether to deliver the response to a channel |
| `channel` | `Option<String>` | Target channel name (e.g., `"telegram"`) |
| `to` | `Option<String>` | Target chat ID |

## Handler Registry

The service supports two levels of job handlers:

### Named Handlers

Registered via `register_handler(name, callback)` before the service starts. When a job executes, the service first looks up a handler matching the job's `name` field. This allows specific jobs (like system health checks or plugin sync tasks) to have dedicated execution logic.

### Fallback Callback

Set via `set_callback(callback)`. Used when no named handler matches the job name. This is typically the agent's general-purpose job executor that processes the job's message payload.

### JobCallback Type

```rust
pub type JobCallback = Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>;
```

Callbacks receive the full `CronJob` and return an optional result string. Errors are captured in the job's `last_error` field.

### Execution Flow

When a job is due (in `execute_job_static`):

1. Look up named handler by job name, fall back to `on_job`
2. Call the handler with the `CronJob` reference
3. Record `last_status` as `"ok"`, `"error"`, or `"skipped"` (no handler)
4. Record `last_run_at_ms` and `last_error` (if any)
5. For `At` jobs: either delete (if `delete_after_run`) or disable
6. For `Every`/`Cron` jobs: compute and set `next_run_at_ms`

## CronTool

The `CronTool` (in `crates/tools/src/cron_tool.rs`) is the user-facing interface for managing scheduled jobs through the agent. It implements the `Tool` trait with three actions:

### Actions

| Action | Required Params | Description |
|--------|----------------|-------------|
| `add` | `message` + (`every_seconds` or `cron_expr`) | Create a new scheduled job |
| `list` | none | List all enabled jobs with next run times |
| `remove` | `job_id` | Delete a job by ID |

### Dependency Inversion

`CronTool` uses the `CronHandler` trait to avoid a direct dependency on the `scheduling` crate (which sits at the same layer). The trait defines three methods: `add_job`, `list_jobs`, and `remove_job`. The actual `CronService` implements this trait and is injected as `Arc<dyn CronHandler>`.

### Entity Cards

When adding a job, the tool emits an `EntityCard` via the routing context's `entity_tx` channel, allowing the UI to display the scheduled job with its next run time.

## One-Shot Auto-Delete

Jobs with `CronSchedule::At` behave differently after execution depending on the `delete_after_run` flag:

| `delete_after_run` | Behavior After Execution |
|-------------------|-------------------------|
| `true` | Job is removed from the store entirely |
| `false` | Job is disabled (`enabled = false`) and `next_run_at_ms` is cleared |

This allows one-shot reminders to clean up automatically while preserving job history when desired.
