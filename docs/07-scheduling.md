# Scheduling

## Purpose

The `scheduling` crate (Layer 2) provides cron-style job scheduling for Klyntbot. It manages recurring and one-shot jobs that trigger agent actions on a timer -- daily digest summaries, focus-check reminders, calendar sync, and user-defined scheduled messages. Jobs are persisted in SQLite via the `storage` crate's `CronRepo` and survive restarts. The service runs a single timer loop on a Tokio task that sleeps until the next job deadline, waking early via `Notify` when jobs are added or removed.

## Key Types

### Enums

**`CronSchedule`** -- tagged enum defining when a job runs. Three variants:

| Variant | Fields | Behavior |
|---------|--------|----------|
| `At` | `at_ms: i64` | One-time execution at a specific UTC timestamp (milliseconds). If the timestamp is in the past, the job will not fire. |
| `Every` | `every_ms: u64` | Recurring execution at a fixed interval (e.g., every 60000ms = every minute). Next run is always `now + every_ms`. |
| `Cron` | `expr: String, tz: Option<String>` | Standard cron expression (6-field: sec min hour day month dow) evaluated via the `cron` crate. Optional IANA timezone string (e.g., `"America/New_York"`) for timezone-aware scheduling; defaults to UTC. Invalid timezone strings fall back to UTC with a warning. |

**`CronError`** -- domain error type with variants `InvalidExpression`, `JobNotFound`, `ExecutionFailed`, `Io`, `Json`. Converts into `KlyntbotError::Cron(String)` via `From`.

### Structs

**`CronJob`** -- a scheduled job. Fields:

| Field | Type | Purpose |
|-------|------|---------|
| `id` | `String` | UUID-based 8-character identifier, generated on creation. |
| `name` | `String` | Human-readable label (e.g., `"todo_daily_digest"`). |
| `enabled` | `bool` | Whether the job is active. Disabled jobs are skipped by the timer loop. |
| `schedule` | `CronSchedule` | When the job should run. |
| `payload` | `CronPayload` | What happens when the job fires. |
| `state` | `CronJobState` | Runtime bookkeeping (next run, last run, last status/error). |
| `created_at_ms` | `i64` | Creation timestamp. |
| `updated_at_ms` | `i64` | Last modification timestamp. |
| `delete_after_run` | `bool` | If true, the job is removed from the store after execution (one-shot cleanup). |

**`CronPayload`** -- what a job does when it fires:

| Field | Type | Purpose |
|-------|------|---------|
| `kind` | `String` | Payload type, defaults to `"agent_turn"`. The callback inspects this to decide how to process the message. |
| `message` | `String` | The message text sent to the agent (e.g., `"Generate my daily task digest"`). |
| `deliver` | `bool` | If true, the agent's response is delivered to a specific channel/chat instead of being processed silently. |
| `channel` | `Option<String>` | Target channel name (e.g., `"telegram"`, `"discord"`). |
| `to` | `Option<String>` | Target chat ID within the channel. |

**`CronJobState`** -- mutable runtime state:

| Field | Type | Purpose |
|-------|------|---------|
| `next_run_at_ms` | `Option<i64>` | Next scheduled execution time. `None` for disabled or completed one-shot jobs. |
| `last_run_at_ms` | `Option<i64>` | When the job last ran. |
| `last_status` | `Option<String>` | `"ok"`, `"error"`, or `"skipped"`. |
| `last_error` | `Option<String>` | Error message from the last failed execution. |

**`CronStore`** -- in-memory collection of all jobs, with a `version` field (currently always 1) and a `jobs: Vec<CronJob>`. Serializable for debugging but not used for persistence directly -- SQL is the source of truth.

**`CronService`** -- the service that owns the timer loop and exposes the public API.

**`JobCallback`** -- `Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>`. The function called when a job fires. Set once via `set_callback()` before `start()`.

## How It Works

### Lifecycle

1. **Construction** -- `CronService::new(repo)` creates the service with a `CronRepo` for SQL persistence. The in-memory `CronStore` starts empty.

2. **Callback registration** -- `set_callback(callback)` installs the `JobCallback`. In production, the agent layer injects a closure that publishes the job's payload as a bus message, triggering an agent turn. The callback receives the full `CronJob` and can return an optional response string.

3. **Start** -- `start()` performs four steps:
   - Loads all jobs from SQLite via `CronRepo::list()` into the in-memory `CronStore`.
   - Recomputes `next_run_at_ms` for all enabled jobs based on their schedules and the current time.
   - Saves the updated state back to SQL.
   - Spawns the timer loop on a Tokio task.

4. **Timer loop** -- runs in a `tokio::spawn` task. On each iteration:
   - Finds the earliest `next_run_at_ms` across all enabled jobs.
   - Sleeps until that deadline using `tokio::time::sleep_until`, or wakes early if `Notify` is signaled (by `add_job`, `remove_job`, or `enable_job`).
   - After waking, collects all due jobs (where `now >= next_run_at_ms`) and executes them sequentially.
   - Saves the updated store to SQL after processing.

5. **Stop** -- `stop()` sets the `running` flag to false and aborts the timer task.

### Job Execution

When a job fires (via the timer loop or manual `run_job()`):

1. The `JobCallback` is invoked with the job. If no callback is set, the status is recorded as `"skipped"`.
2. On success, `last_status` is set to `"ok"`. On error, `last_status` is `"error"` and `last_error` captures the message.
3. `last_run_at_ms` is updated to the current time.
4. Post-execution behavior depends on the schedule type:
   - **`At` (one-shot)**: If `delete_after_run` is true, the job is removed from the store entirely. Otherwise it is disabled (`enabled = false`, `next_run_at_ms = None`).
   - **`Every` / `Cron` (recurring)**: `next_run_at_ms` is recomputed from the schedule for the next cycle.

### Persistence

All persistence goes through `CronRepo` in the `storage` crate. Two operations happen:

- **`save_store()`** -- iterates the in-memory `CronStore`, upserts each job as a `CronJobRow` in SQLite, and deletes any SQL rows whose IDs no longer exist in memory (handling `remove_job` cleanup).
- **`load_store()`** -- reads all `CronJobRow` records from SQL and converts them back to `CronJob` structs via `row_to_job()`.

The conversion between domain types and SQL rows involves serializing `CronSchedule` and `CronPayload` as JSON values in the `schedule` and `payload` columns.

### System Jobs

At agent startup, the agent layer registers several built-in system jobs if they do not already exist in the store. These include:

- **`todo_focus_check`** -- periodic check for tasks that need attention.
- **`todo_daily_digest`** -- daily summary of tasks and priorities.
- **`calendar_sync`** -- periodic calendar synchronization (triggers `CalendarHandler::sync_now()`).

System jobs use the same `CronSchedule` and `CronPayload` mechanism as user-created jobs. They are registered with fixed IDs so they are not duplicated on restart.

### Next-Run Computation

The `compute_next_run()` function handles all three schedule variants:

- **`At`** -- returns `Some(at_ms)` if the timestamp is in the future, `None` otherwise.
- **`Every`** -- returns `Some(now + every_ms)`, or `None` if the interval is zero.
- **`Cron`** -- parses the expression with the `cron` crate, then computes the next occurrence using `schedule.upcoming()`. If a timezone is specified, the `chrono_tz` crate is used to evaluate in that timezone. Invalid timezone strings produce a warning and fall back to UTC.

### Public API

| Method | Purpose |
|--------|---------|
| `add_job(name, schedule, message, deliver, channel, to, delete_after_run)` | Create a new job, compute its next run, save to SQL, and wake the timer loop. Returns the new `CronJob`. |
| `remove_job(job_id)` | Remove by ID, save, and wake the timer. Returns whether a job was found. |
| `enable_job(job_id, enabled)` | Toggle enabled state. Recomputes `next_run_at_ms` when enabling. |
| `run_job(job_id, force)` | Manually trigger execution. If `force` is false, disabled jobs are skipped. |
| `list_jobs(include_disabled)` | List jobs sorted by next run time. Optionally includes disabled jobs. |
| `status()` | Returns a JSON value with `enabled` (bool), `jobs` (count), and `nextWakeAtMs`. |

## Connections

**Depends on:**
- `common` (Layer 0) -- `Result`, `KlyntbotError`
- `storage` (Layer 1.5) -- `CronRepo` for SQL persistence, `CronJobRow` for the row type
- External crates: `cron` (expression parsing), `chrono` / `chrono_tz` (timezone-aware scheduling), `uuid` (job ID generation), `tokio` (async runtime, timers, notify)

**Depended on by:**
- `agent` (Layer 5) -- creates the `CronService`, registers the callback, starts/stops the service, and registers system jobs at startup
- `tools` (Layer 3) -- `CronHandler` trait (defined in tools, implemented in agent) provides the dependency-inversion bridge so the `CronTool` can manage jobs without directly depending on the agent
