# klyntbot-cron

**Cron job scheduling service.**

## Overview

`klyntbot-cron` provides scheduled task execution for klyntbot:
- Three schedule types: `at` (one-time), `every` (interval), `cron` (expression)
- Job persistence across restarts
- Async execution with Tokio timers
- Natural language interval parsing

## Contents

### Schedule Types

```rust
use klyntbot_cron::{CronSchedule, CronJob, CronPayload};
use chrono::Utc;

// One-time execution at specific timestamp
let at_schedule = CronSchedule::At(Utc::now() + Duration::hours(1));

// Interval execution (every N seconds)
let every_schedule = CronSchedule::Every(3600);  // Every hour

// Cron expression (standard cron format)
let cron_schedule = CronSchedule::Cron("0 9 * * *".into());  // Daily at 9am
```

### Creating Jobs

```rust
use klyntbot_cron::{CronService, CronJob, CronPayload};

let cron_service = CronService::new(jobs_path).await?;

// Create job
let job = CronJob {
    id: Uuid::new_v4().to_string(),
    name: "daily-reminder".into(),
    schedule: CronSchedule::Cron("0 9 * * *".into()),
    payload: CronPayload::SendMessage {
        channel: ChannelName::Telegram,
        chat_id: ChatId("user123".into()),
        message: "Good morning!".into(),
    },
    state: CronJobState::Active,
    created_at: Utc::now(),
    last_run: None,
    next_run: None,
};

cron_service.add_job(job).await?;
```

### Job Execution

```rust
use klyntbot_cron::CronService;

// Start cron service (runs in background)
let cron_service = CronService::new(jobs_path).await?;
cron_service.start().await;

// Jobs execute automatically at scheduled times
// Results are sent via callback
```

### Job Management

```rust
// List all jobs
let jobs = cron_service.list_jobs().await?;
for job in jobs {
    println!("{}: {} ({})", job.id, job.name, job.state);
}

// Delete job
cron_service.delete_job("job-id-123").await?;

// Update job
cron_service.update_job(updated_job).await?;
```

## Usage

Add to your `Cargo.toml`:

```toml
[dependencies]
klyntbot-cron.workspace = true
```

Example:

```rust
use klyntbot_cron::{CronService, CronJob, CronSchedule, CronPayload};
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    let jobs_path = PathBuf::from("/tmp/cron_jobs.json");
    let cron_service = CronService::new(jobs_path).await?;

    // Schedule daily reminder
    let job = CronJob {
        id: Uuid::new_v4().to_string(),
        name: "daily-standup".into(),
        schedule: CronSchedule::Cron("0 10 * * 1-5".into()),  // Weekdays at 10am
        payload: CronPayload::SendMessage {
            channel: ChannelName::Slack,
            chat_id: ChatId("team-channel".into()),
            message: "Time for daily standup!".into(),
        },
        state: CronJobState::Active,
        created_at: Utc::now(),
        last_run: None,
        next_run: None,
    };

    cron_service.add_job(job).await?;
    cron_service.start().await;

    Ok(())
}
```

## Schedule Formats

### Cron Expressions

Standard cron format (5 fields):

```
 ┌───────────── minute (0-59)
 │ ┌───────────── hour (0-23)
 │ │ ┌───────────── day of month (1-31)
 │ │ │ ┌───────────── month (1-12)
 │ │ │ │ ┌───────────── day of week (0-6, 0=Sunday)
 │ │ │ │ │
 │ │ │ │ │
 * * * * *
```

**Examples:**
```
"0 9 * * *"       → Daily at 9:00 AM
"0 */4 * * *"     → Every 4 hours
"0 9 * * 1-5"     → Weekdays at 9:00 AM
"30 14 1 * *"     → 1st of month at 2:30 PM
"0 0 * * 0"       → Sundays at midnight
```

### Interval (Seconds)

```rust
CronSchedule::Every(60)      // Every minute
CronSchedule::Every(3600)    // Every hour
CronSchedule::Every(86400)   // Every day
```

### One-time Execution

```rust
use chrono::{Utc, Duration};

let run_at = Utc::now() + Duration::hours(2);
CronSchedule::At(run_at)
```

## Payload Types

### SendMessage

```rust
CronPayload::SendMessage {
    channel: ChannelName::Telegram,
    chat_id: ChatId("user123".into()),
    message: "Scheduled message".into(),
}
```

### RunAgent

```rust
CronPayload::RunAgent {
    task: "Summarize daily metrics".into(),
    channel: ChannelName::Cli,
    chat_id: ChatId("system".into()),
}
```

## Persistence

Jobs are stored in JSON format:

```json
[
  {
    "id": "abc-123",
    "name": "daily-reminder",
    "schedule": { "Cron": "0 9 * * *" },
    "payload": {
      "SendMessage": {
        "channel": "Telegram",
        "chat_id": "user123",
        "message": "Good morning!"
      }
    },
    "state": "Active",
    "created_at": "2026-02-12T08:00:00Z",
    "last_run": "2026-02-12T09:00:00Z",
    "next_run": "2026-02-13T09:00:00Z"
  }
]
```

Jobs survive restarts by reloading from this file.

## Job States

```rust
pub enum CronJobState {
    Active,      // Job will execute
    Paused,      // Job skipped
    Completed,   // One-time job finished
}
```

## Design Principles

1. **Three schedule types** — Cover all common use cases
2. **Persistent storage** — Jobs survive restarts
3. **Async execution** — Non-blocking with Tokio timers
4. **Standard cron format** — Familiar syntax for scheduling
5. **Callback pattern** — Job execution triggers callbacks

## Dependencies

- `klyntbot-core` — Error types, shared types
- `tokio` — Async runtime and timers
- `chrono` — Date/time handling
- `serde`, `serde_json` — Serialization
- `uuid` — Job ID generation
- `cron` — Cron expression parsing
- `tracing` — Logging

## See Also

- [klyntbot Architecture](../../docs/ARCHITECTURE.md)
- [Cron Tool](../klyntbot-tools/README.md)
