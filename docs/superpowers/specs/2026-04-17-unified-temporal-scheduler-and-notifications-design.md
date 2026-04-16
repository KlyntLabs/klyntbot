# Unified Temporal Scheduler and Notifications System — Design Spec

**Date:** 2026-04-17
**Status:** Approved design, pending implementation plan
**Scope:** Comprehensive (A–E): rule model + unified scheduler + persistence + dispatch policy + workspace-wide Chrono→Jiff migration

---

## 1. Problem Statement

Klyntbot's current notification/reminder/scheduling stack has four overlapping subsystems with known correctness gaps:

- **`ReminderEngine`** (`crates/agent/src/services/reminders.rs`) — legacy polling loop with hard-coded 2h-before-deadline and 24h-overdue rules.
- **`DeadlineScheduler`** (`crates/scheduling/src/deadline.rs`) — event-driven, exact-fire, but **in-memory only** (no persistence across restarts).
- **`CronService`** (`crates/scheduling/src/service/`) — persistent, exact-fire, handles recurring/one-shot/cron jobs, but not wired to task reminders.
- **`NotificationDispatcher`** (`crates/agent/src/services/notifications.rs`) — flat target list, no quiet hours, no held notifications, no idempotency gate.

Additional gaps:
1. No per-task user-configurable reminder rules (e.g., "24h before, 9am the day before, 5min before").
2. No task recurrence model ("every Monday at 9am") — `next_instance_date` column exists but pattern language doesn't.
3. macOS `Instant` pauses during system sleep → scheduled fires are **late by the sleep duration** (broken on every laptop-close).
4. Chrono cannot serialize IANA timezone identifiers → silent DST bugs across the codebase.
5. No emitted `AlarmFired` / `MissedAlarms` events → UI can't observe the scheduler.

### Goals

Build one comprehensive subsystem, covering:

- **(A)** User-configurable reminder rules per task with global defaults (multiple rules per task).
- **(B)** A single unified scheduler engine replacing `DeadlineScheduler` + `CronService` + `ReminderEngine`.
- **(C)** Full persistence — every scheduled fire survives restarts, crashes, long laptop-sleeps.
- **(D)** Dispatcher with quiet hours, multi-channel fan-out, idempotency gate, held-notification release.
- **(E)** Workspace-wide Chrono → Jiff migration for correct DST/IANA time handling on a foundation for years to come.
- **Plus:** Task recurrence via RFC 5545 RRULE (template + instance materialization).

### Non-Goals

- Per-user timezones (single-user app; `config.timezone` remains global).
- Push notifications to mobile (OS native + tray + channels only).
- Named notification profiles (schema shaped to accept them later; not built day-1).
- Location-based (proximity) triggers.
- Interop import/export of iCalendar files (RRULE internal only).

---

## 2. Architecture Overview

### 2.1 Crate Topology

New crate `crates/notifications/` (L4) carves dispatch concerns out of `agent`. The existing `scheduling` crate (L3) absorbs reminder rule types and the RRULE evaluator. Dependency flow remains strictly upward (L0 → L8).

```
L0 common         → NotificationSender trait (existing); Jiff re-exports
L1 config         → notifications.* subtree (quiet hours, default rules, channel defaults)
L1 bus            → new DomainEvents (AlarmFired, AlarmSnoozed, AlarmCancelled,
                    MissedAlarms, HeldNotificationReleased)
L2 storage        → 5 new tables (see §4)
L3 scheduling     → TemporalScheduler (unified engine), AlarmRule types,
                    RRULE evaluator (rrule crate), wall-clock anchor loop
L4 notifications  → NEW. Dispatcher, channel adapters, quiet-hours policy,
                    held-notification release, idempotency gate, misfire handling
L4 feature-tasks  → task_alarms CRUD, recurrence template, instance materialization,
                    extends TaskTool with alarms + recurrence subfields
L4 tools          → new AlarmTool (standalone / free-floating reminders)
L5 agent          → removes ReminderEngine (delete); removes old
                    services/notifications.rs (migrated to new crate)
L7 app-core       → removes init/deadline.rs; wires new notifications crate
L8 klyntbot       → re-export facade updated
```

### 2.2 High-Level Dataflow

```
User / Agent                      Scheduler (L3)                Dispatcher (L4)               Channels
─────────                         ──────────────                 ──────────────                ────────
                                                                                             
TaskTool.create ──────► writes ──► scheduled_fires table ──► sleep loop fires ──► AlarmFired ──► OS native
  { alarms: [...],       task_alarms ├── fire_at epoch         (wall-clock anchor,  event (bus)       ├── Tauri tray
    recurrence: {...} }  task_recur  ├── rule_type              cap 30s sleep)                        ├── Telegram
                                    └── misfire_policy                                                ├── Discord
                                                                     │                                └── Email
AlarmTool.create ───────► writes ──► scheduled_fires               subscribes
                                                                     ▼
                                                             NotificationDispatcher
                                                             ├── check quiet hours
                                                             ├── INSERT OR IGNORE notification_log (idempotency)
                                                             ├── held_notifications (if suppressed)
                                                             └── fan-out to channels from channel_mask
```

### 2.3 Core Abstractions

**`TemporalScheduler`** (new, unifies `CronService` + `DeadlineScheduler`):
- Single timer loop with wall-clock anchor (max 30s sleep, re-check `Jiff::Timestamp::now()` on every wake).
- One canonical table `scheduled_fires` with `fire_at INTEGER` (Unix epoch ms), indexed.
- Handles all prior variants: task reminders, focus warnings, focus expiry, recurring spawn, cron jobs, one-shot alarms, held-notification release.
- Emits `AlarmFired` on the `DomainEventBus` — no synchronous handler closure, no `block_in_place`.
- Subscribes to `SystemDidWake` for immediate catch-up after sleep (in addition to the 30s poll).

**`AlarmRule`** (new, three variants — matches RFC 5545 VALARM semantics):
```rust
pub enum AlarmRule {
    RelativeBefore { offset: jiff::Span },
    CivilTimeOnDayOffset { day_offset: i32, time_of_day: jiff::civil::Time, iana_tz: String },
    Absolute { fire_at: jiff::Timestamp },
}
```

**`NotificationDispatcher`** (new, in `notifications` crate):
- Subscribes to `AlarmFired`.
- Resolves channel set (alarm's `channel_mask`, else global `defaultChannels`).
- Evaluates quiet hours against user timezone (Jiff `Zoned`).
- Idempotency gate: `INSERT OR IGNORE INTO notification_log (alarm_id, channel, sent_at)`.
- If suppressed by quiet hours (and not priority-overridden): writes to `held_notifications`, schedules a release-alarm at end-of-quiet-window.

**`RecurrenceEngine`** (part of scheduler, uses `rrule` crate):
- Evaluates RRULE expressions in user's IANA timezone.
- Computes `next_fire` for template rows.
- Materializes instance tasks N-ahead (default 3, configurable).

---

## 3. Reminder Rule Model

### 3.1 Rule Types

Three orthogonal rule types cover every user utterance:

| Type | Example | Stored as |
|---|---|---|
| `RelativeBefore` | "24h before deadline", "5 min before" | `offset_secs INTEGER` |
| `CivilTimeOnDayOffset` | "9am the day before", "8am on the deadline day" | `day_offset INTEGER, time_of_day TEXT, iana_tz TEXT` |
| `Absolute` | "at 2026-04-20T09:00:00-04:00" | `fire_at INTEGER` (epoch ms) |

Each rule row additionally stores: `channel_mask INTEGER`, `priority_override TEXT` (NULL or "urgent"), `misfire_policy TEXT` ("skip_if_stale" | "strict" | "coalesce"), `grace_window_secs INTEGER` (default NULL → inherits global).

### 3.2 Default Rules (config.json)

```json
"notifications": {
  "defaultRules": [
    { "type": "relativeBefore", "offsetSecs": 86400 },
    { "type": "civilTime", "dayOffset": -1, "timeOfDay": "09:00" },
    { "type": "relativeBefore", "offsetSecs": 300 }
  ],
  "defaultChannels": ["os_native", "tray"],
  "defaultMisfirePolicy": "skip_if_stale",
  "defaultGraceWindowSecs": 3600,
  "quietHours": {
    "enabled": true,
    "timezone": "follow_config_timezone",
    "start": "22:00",
    "end": "07:00",
    "overrideForUrgentTasks": true
  },
  "dndRespectsFocusMode": true
}
```

### 3.3 Precedence

Per-task rules *replace* defaults (not merge). `task_alarms` rows mean "this task has these exact rules." Tasks without any `task_alarms` rows inherit `defaultRules` at scheduling time (materialized into `scheduled_fires` on creation).

### 3.4 Rule-to-Fire Computation

When a task is created/updated, or a recurring instance is materialized:

1. Resolve the rule set (per-task if any, else global defaults).
2. For each rule + task's `due_date`:
   - `RelativeBefore` → `fire_at = due_date - offset`
   - `CivilTimeOnDayOffset` → compute civil date = `(due_date local in iana_tz) + day_offset days`, combine with `time_of_day`, convert to UTC epoch via Jiff `Zoned`.
   - `Absolute` → `fire_at = fire_at` (no computation).
3. Insert one `scheduled_fires` row per rule (with `alarm_id`, `task_id`, `fire_at`, `rule_kind`, channel_mask, etc.).
4. Recomputation is idempotent: re-running against unchanged inputs produces identical rows (dedup via `(task_id, rule_id)` unique key).

---

## 4. Data Model (SQLite Schema)

### 4.1 New Tables

```sql
-- The one canonical "when to fire" table. Every scheduled fire lives here.
CREATE TABLE scheduled_fires (
    id TEXT PRIMARY KEY,                   -- UUID
    fire_at INTEGER NOT NULL,              -- Unix epoch ms, UTC
    kind TEXT NOT NULL,                    -- 'task_alarm' | 'cron_job' | 'held_release' | 'recurrence_spawn' | 'focus_warning' | 'focus_expire' | 'standalone_alarm'
    ref_id TEXT,                           -- points to task_alarms.id / cron_jobs.id / task_templates.id / etc.
    payload TEXT NOT NULL,                 -- JSON: {message, channel_mask, priority_override, misfire_policy, grace_window_secs, ...}
    dedup_prefix TEXT,                     -- for prefix cancellation (e.g., 'task:abc123:')
    fired INTEGER NOT NULL DEFAULT 0,
    firing_started_at INTEGER,             -- two-phase commit: set before dispatch, fired=1 after
    fired_at INTEGER,
    created_at INTEGER NOT NULL
);
CREATE INDEX idx_scheduled_fires_pending ON scheduled_fires(fire_at) WHERE fired = 0;
CREATE INDEX idx_scheduled_fires_dedup ON scheduled_fires(dedup_prefix) WHERE fired = 0;

-- Per-task rule definitions. One row = one rule.
CREATE TABLE task_alarms (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    rule_type TEXT NOT NULL,               -- 'relative_before' | 'civil_time' | 'absolute'
    offset_secs INTEGER,                   -- for relative_before
    day_offset INTEGER,                    -- for civil_time
    time_of_day TEXT,                      -- 'HH:MM' for civil_time
    iana_tz TEXT,                          -- for civil_time
    absolute_fire_at INTEGER,              -- only for rule_type='absolute'; NULL otherwise. Scheduled_fires.fire_at is the authoritative computed fire time.
    channel_mask INTEGER NOT NULL DEFAULT 0, -- 0 = inherit defaultChannels
    priority_override TEXT,                -- NULL | 'urgent'
    misfire_policy TEXT,                   -- NULL = inherit
    grace_window_secs INTEGER,             -- NULL = inherit
    created_at INTEGER NOT NULL,
    UNIQUE (task_id, rule_type, offset_secs, day_offset, time_of_day, absolute_fire_at)
);
CREATE INDEX idx_task_alarms_task ON task_alarms(task_id);

-- Recurrence template. Task rows carry FK to template; template holds the RRULE.
CREATE TABLE task_recurrence_templates (
    id TEXT PRIMARY KEY,
    source_task_id TEXT NOT NULL,          -- the "master" task definition (title, priority, etc.)
    rrule TEXT NOT NULL,                   -- RFC 5545 RRULE string
    iana_tz TEXT NOT NULL,                 -- timezone for the RRULE
    materialize_ahead INTEGER NOT NULL DEFAULT 3,
    next_instance_at INTEGER,              -- next fire_at to materialize
    last_instance_at INTEGER,              -- last materialized
    until_at INTEGER,                      -- RRULE UNTIL (optional)
    count_remaining INTEGER,               -- RRULE COUNT (optional, decremented per materialize)
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at INTEGER NOT NULL
);
-- Existing tasks table gains: template_id TEXT REFERENCES task_recurrence_templates(id)

-- Idempotency gate. One row per (alarm, channel) delivery attempt.
CREATE TABLE notification_log (
    alarm_id TEXT NOT NULL,
    channel TEXT NOT NULL,
    sent_at INTEGER NOT NULL,
    ack_at INTEGER,
    error TEXT,
    PRIMARY KEY (alarm_id, channel)
);
CREATE INDEX idx_notification_log_sent ON notification_log(sent_at);

-- Quiet-hours-held notifications awaiting release.
CREATE TABLE held_notifications (
    id TEXT PRIMARY KEY,
    alarm_id TEXT NOT NULL,
    channels TEXT NOT NULL,                -- JSON array of channel names
    payload TEXT NOT NULL,                 -- JSON: {title, body, priority, ...}
    release_at INTEGER NOT NULL,
    released INTEGER NOT NULL DEFAULT 0,
    held_at INTEGER NOT NULL
);
CREATE INDEX idx_held_notifications_pending ON held_notifications(release_at) WHERE released = 0;
```

### 4.2 Relationship to Existing `cron_jobs` Table

`cron_jobs` **is kept** and becomes a *definition* table (user- and agent-created recurring/one-shot jobs via `CronTool`). The `TemporalScheduler` owns the *firing* via `scheduled_fires`. Bridge semantics:

- For each enabled row in `cron_jobs`, the scheduler maintains exactly one pending row in `scheduled_fires` (kind = `cron_job`, `ref_id = cron_jobs.id`).
- When that scheduled_fire fires, the scheduler (a) emits `AlarmFired`, (b) computes the next run time from the cron schedule, (c) inserts a fresh `scheduled_fires` row.
- On `cron_jobs` enable/disable/delete, prefix cancellation clears its pending `scheduled_fires` row.
- On startup, the scheduler reconciles: every enabled `cron_jobs` row must have exactly one pending `scheduled_fires` row; mismatches are repaired.

The old `CronService` type is deleted; its `next_run_at_ms` column on `cron_jobs` is retained as a cache for UI listing but is authoritative-nowhere — `scheduled_fires.fire_at` is the truth.

### 4.3 Migration Strategy

Per CLAUDE.md's pre-release schema policy (no user data to migrate): schema changes are made directly in the existing `feature-tasks`/`scheduling`/`notifications` migration files rather than incremental migration files. No backward-compatible shims.

---

## 5. Scheduler Correctness Guarantees

### 5.1 Wall-Clock Anchor Loop

```rust
loop {
    let next_fire = repo.next_pending_fire_at().await?;
    let now = Timestamp::now();
    let sleep_duration = next_fire
        .map(|t| (t - now).max(Duration::ZERO))
        .unwrap_or(Duration::from_secs(60))
        .min(Duration::from_secs(30));   // never sleep > 30s

    tokio::select! {
        _ = tokio::time::sleep(sleep_duration) => {}
        _ = self.wake_signal.notified() => {}
        _ = self.shutdown.cancelled() => break,
    }

    self.process_due_fires(Timestamp::now()).await?;
}
```

**Why 30s cap:** Tokio's `sleep` on macOS uses `CLOCK_UPTIME_RAW`, which pauses during system sleep. A scheduler that `sleep_until(tomorrow_9am)` while the laptop is closed will fire late by the sleep duration. Capping at 30s guarantees the scheduler re-evaluates wall-clock time at least every 30 seconds after wake — no platform-specific code, no `tokio-timerfd` dependency.

Additionally, subscribe to `DomainEvent::SystemDidWake` (already emitted by `app-core`) → immediately `wake_signal.notify()` for sub-second catch-up after resume.

### 5.2 Two-Phase Fire Commit

```sql
-- Phase 1: mark as firing
UPDATE scheduled_fires SET firing_started_at = ? WHERE id = ? AND fired = 0 AND firing_started_at IS NULL;
-- Dispatch via DomainEventBus
-- Phase 2: mark fired
UPDATE scheduled_fires SET fired = 1, fired_at = ? WHERE id = ?;
```

On restart, rows with `firing_started_at IS NOT NULL AND fired = 0` are considered in-flight; the scheduler re-dispatches them. Idempotency at the dispatcher (`notification_log`) absorbs the resulting double-call.

### 5.3 Misfire Semantics

On `process_due_fires(now)`:

```
For each row where fire_at <= now AND fired = 0:
  policy = row.misfire_policy ?? config.defaultMisfirePolicy
  grace = row.grace_window_secs ?? config.defaultGraceWindowSecs
  age = now - fire_at

  if policy == "strict": fire
  elif policy == "skip_if_stale":
      if age <= grace: fire
      else: mark fired=1, emit MissedAlarms event, do not dispatch
  elif policy == "coalesce":
      group by (task_id, kind); fire only most recent per group;
      mark others as fired=1 with suppressed_by pointing to the fired one
```

### 5.4 Prefix Cancellation

`cancel_by_prefix("task:abc123:")` → `DELETE FROM scheduled_fires WHERE dedup_prefix LIKE ? AND fired = 0`. Used when a task is deleted or its rule set changes — clear all existing pending fires, then re-materialize.

---

## 6. Dispatcher (L4 notifications crate)

### 6.1 AlarmFired Handler

```rust
async fn on_alarm_fired(&self, event: AlarmFired) -> Result<()> {
    let channels = self.resolve_channels(&event);
    let now_zoned = self.now_zoned();

    if self.in_quiet_hours(now_zoned, &event) {
        self.hold(&event, channels).await?;
        return Ok(());
    }

    for channel in channels {
        // Idempotency gate
        let inserted = self.log.try_insert(event.alarm_id, channel).await?;
        if !inserted { continue; }

        match self.send_to(channel, &event).await {
            Ok(()) => {},
            Err(e) => {
                self.log.record_error(event.alarm_id, channel, e).await?;
                self.retry_queue.push(event, channel);
            }
        }
    }
    Ok(())
}
```

### 6.2 Channel Routing Behavior

| Channel | During Quiet Hours | During Quiet Hours + Urgent Override |
|---|---|---|
| `os_native` | Respect OS Focus state (macOS handles) | Fire anyway (critical interruption level) |
| `tray` | Always update badge count | Always update badge count |
| `telegram` | Hold for release | Fire |
| `discord` | Hold for release | Fire |
| `email` | Hold for release | Fire |

### 6.3 Held Notification Release

When quiet hours end, the held-release alarm fires → dispatcher reads all unreleased `held_notifications` where `release_at <= now` → dispatches each to its original channel set → marks `released = 1`.

### 6.4 Retry Policy

Channel send failures (network error, rate limit, transient) retry up to 3 times with exponential backoff (1s, 4s, 16s). After 3 failures, the row stays in `notification_log` with `error` populated; a daily cleanup sweep emits `DomainEvent::NotificationDeliveryFailed` for observability but does not retry further.

---

## 7. Recurrence (RRULE)

### 7.1 Expression Format

RFC 5545 RRULE stored as canonical text. Agent and UI input a structured DSL that compiles to RRULE:

```json
{
  "frequency": "weekly",
  "interval": 1,
  "byDay": ["MO", "WE", "FR"],
  "at": "07:00",
  "timezone": "America/New_York"
}
→ "FREQ=WEEKLY;INTERVAL=1;BYDAY=MO,WE,FR;BYHOUR=7;BYMINUTE=0"
```

Supported features (subset of RFC 5545 that covers every user example):

| Feature | Example |
|---|---|
| `FREQ` | `DAILY`, `WEEKLY`, `MONTHLY`, `YEARLY` |
| `INTERVAL` | every 2 days = `FREQ=DAILY;INTERVAL=2` |
| `BYDAY` | every Mon/Wed/Fri = `FREQ=WEEKLY;BYDAY=MO,WE,FR` |
| `BYMONTHDAY` | first of month = `BYMONTHDAY=1`; last = `BYMONTHDAY=-1` |
| `BYHOUR` / `BYMINUTE` | at 9am = `BYHOUR=9;BYMINUTE=0` |
| `UNTIL` | stop on a date |
| `COUNT` | fire N times total |

### 7.2 Template + Instance Model

- `task_recurrence_templates` row holds the RRULE + source task definition.
- `RecurrenceEngine` materializes the next N (default 3) instance rows in `tasks` table ahead of time.
- Each instance is a normal task with its own `due_date`, eligible for normal `task_alarms` (inherits the template's alarm rules at materialization time).
- A single `scheduled_fires` row (kind = `recurrence_spawn`) fires when it's time to materialize the next instance; it re-schedules itself after each materialization.
- Completing an instance does not affect siblings; completing the template disables it (cascades to unfired instances via prefix cancellation).

### 7.3 DST Correctness

RRULE evaluated in the `iana_tz` timezone via the `rrule` crate's `Tz` type. `FREQ=DAILY;BYHOUR=9` during DST transitions: on spring-forward day (2am→3am skipped), 9am is unambiguous → fires normally. On fall-back day (1am-2am repeats), 9am is unambiguous → fires once. Ambiguous times (e.g., `BYHOUR=1` during fall-back) fire on the first occurrence (pre-transition) and mark as fired to suppress the repeat.

---

## 8. LLM Agent Tool Surface

### 8.1 Extended `TaskTool`

New subfields on `create` and `update`:

```typescript
TaskTool.create({
  title: "dentist",
  due_date: "2026-04-22T14:00:00",
  recurrence?: RRuleSpec,
  alarms?: AlarmSpec[],
  ...
})

TaskTool.update({
  id: "task_123",
  add_alarms?: AlarmSpec[],
  remove_alarm_ids?: string[],
  recurrence?: RRuleSpec | null,   // null disables recurrence
  ...
})

TaskTool.snooze({
  id: "task_123",
  alarm_id?: string,               // null = snooze all alarms for this task
  duration: "10m" | "1h" | ISO-8601-duration
})
```

### 8.2 New `AlarmTool` (standalone / free-floating)

```typescript
AlarmTool.create({
  fire_at?: string,                // ISO-8601; mutex with relative_duration
  relative_duration?: string,      // "10m", "1h", "1d"
  message: string,
  channels?: string[],
  priority?: "normal" | "urgent",
  recurrence?: RRuleSpec
})

AlarmTool.list({
  window?: "today" | "week" | { from: ISO, to: ISO },
  include_history?: boolean        // default false → pending only
})

AlarmTool.snooze({ id, duration })
AlarmTool.cancel({ id })
```

### 8.3 Shared Types

```typescript
AlarmSpec =
  | { type: "relative_before", offset_secs: number }
  | { type: "civil_time", day_offset: number, time_of_day: "HH:MM", timezone?: string }
  | { type: "absolute", fire_at: string }
  & { channels?: string[], priority?: "normal" | "urgent" }

RRuleSpec = {
  frequency: "daily" | "weekly" | "monthly" | "yearly",
  interval?: number,
  by_day?: string[],              // ["MO", "TU", ...]
  by_month_day?: number[],        // [1] | [-1] | ...
  at?: string,                    // "HH:MM"
  timezone?: string,              // IANA; default config.timezone
  until?: string,                 // ISO-8601
  count?: number
}
```

### 8.4 MCP Exposure

`default_exposed_tools()` in `crates/config/src/schema/mcp.rs` gains `"alarm"`. Users can override via `config.mcp.server.exposedTools`. Debug: `klyntbot-mcp tools --list` shows `alarm` alongside existing tools.

---

## 9. Chrono → Jiff Migration (Workspace-Wide)

### 9.1 Why Jiff

- Round-trips IANA timezone identifiers losslessly (RFC 9557 IXDTF: `2024-07-21T17:11-04[America/New_York]`).
- Calendar-aware arithmetic: `span("1 day").add_to(zoned)` correctly produces 23h or 25h on DST days.
- Separate `Timestamp` (UTC instant) vs `Zoned` (instant + tz) vs `civil::DateTime` (floating) — makes floating-time reminders structurally type-safe.
- Maintained by BurntSushi; strong API stability commitments.

### 9.2 Type Mapping

| Chrono | Jiff |
|---|---|
| `chrono::DateTime<Utc>` | `jiff::Timestamp` |
| `chrono::DateTime<Local>` | `jiff::Zoned` with user's IANA tz |
| `chrono::DateTime<Tz>` | `jiff::Zoned` |
| `chrono::NaiveDateTime` | `jiff::civil::DateTime` |
| `chrono::NaiveDate` | `jiff::civil::Date` |
| `chrono::NaiveTime` | `jiff::civil::Time` |
| `chrono::Duration` | `jiff::Span` (calendar) or `std::time::Duration` (wall) |

### 9.3 Storage Wire Format

All persisted times → `INTEGER` (Unix epoch milliseconds, UTC). Both Jiff and Chrono read/write `i64` losslessly — enables zero-schema-change migration. Existing string-based RFC 3339 columns migrate to epoch-ms columns as part of the layer-by-layer rollout (pre-release, no user data).

### 9.4 Rollout Sequence

Layer-by-layer, bottom-up (L0 → L8). Each PR is independently shippable. Workspace compiles at every step.

1. **L0 `common`** — Re-export `jiff` types; add `TimeConvert` helpers; keep `chrono` re-exports temporarily for downstream crates.
2. **L0 `platform-macos`** — Migrate any datetime usage; leaf crate, no deps to care about.
3. **L1 `config`, `bus`, `tools-core`, `analytics`** — In parallel; independent.
4. **L2 `storage`** — Migrate all `*Row` types + repo helpers; wire format = epoch ms. Touches `migrations/*.sql` for column-type changes.
5. **L3 `providers`, `session`, `scheduling`, `context_engine`, `skill-system`** — In parallel. `scheduling` gets the new `TemporalScheduler` as part of this step (two concerns addressed together since they share files).
6. **L4 `tools`, `feature-*`, `plugin-runtime`, `autotuner`, `voice-engine`, `simulator`** — In parallel across feature crates; `notifications` crate created here.
7. **L5 `channels`, `agent`, `cognitive`** — Agent's `reminders.rs` and `notifications.rs` deleted in this PR; agent retains only runtime concerns.
8. **L6 `mcp`** — MCP server updated for `alarm` tool exposure.
9. **L7 `app-core`, `desktop-shared`, `desktop`** — `init/deadline.rs` deleted (scheduler self-initializes). Tray countdown rewired to subscribe to scheduler events instead of polling.
10. **L8 `klyntbot`, `klyntbot-server`** — Facade updates; final `chrono` removal from workspace `Cargo.toml`.

Each layer PR includes: migration + tests + clippy clean + no new warnings. Rollback unit = one layer PR.

### 9.5 Tray Countdown Rewire

Currently polls every 30s. New: subscribe to `AlarmFired` + task events on the bus; maintain an in-memory "next upcoming" cache invalidated on bus events. Removes the independent polling loop.

---

## 10. Domain Events

New events on `DomainEventBus`:

```rust
DomainEvent::AlarmFired {
    alarm_id: String,
    task_id: Option<String>,
    kind: String,
    payload_json: String,
}

DomainEvent::AlarmSnoozed {
    alarm_id: String,
    new_fire_at: Timestamp,
    duration: Span,
}

DomainEvent::AlarmCancelled {
    alarm_id: String,
    reason: String,
}

DomainEvent::MissedAlarms {
    count: usize,
    task_ids: Vec<String>,
    oldest_fire_at: Timestamp,
    newest_fire_at: Timestamp,
}

DomainEvent::HeldNotificationReleased {
    held_id: String,
    alarm_id: String,
    channels: Vec<String>,
}

DomainEvent::NotificationDeliveryFailed {
    alarm_id: String,
    channel: String,
    error: String,
    attempts: u32,
}
```

Existing `TaskDueDateChanged`, `TaskFocusChanged`, `RecurringTemplateAdvanced`, `SystemDidWake`, etc. remain unchanged — the scheduler subscribes to them as inputs.

---

## 11. Deletions

Files deleted in the final rollout PR (after everything else is green):

- `crates/agent/src/services/reminders.rs` — legacy `ReminderEngine` (polling, superseded).
- `crates/agent/src/services/notifications.rs` — moved to `crates/notifications/`.
- `crates/app-core/src/init/deadline.rs` — scheduler self-initializes from `scheduled_fires` on boot.
- `tests/e2e/reminders.rs` — replaced by new e2e suite (`tests/e2e/alarms.rs`) against `TemporalScheduler`.

---

## 12. Testing Strategy

### 12.1 Unit Tests (per crate)

- `scheduling`: RRULE evaluation across DST transitions (spring-forward gap, fall-back fold), misfire policy correctness at boundary conditions, wall-clock anchor behavior with mocked `Timestamp::now()`.
- `notifications`: quiet-hours evaluation across timezones, idempotency gate races (concurrent fires for same alarm_id + channel), held-notification release ordering.
- `feature-tasks`: rule→fire_at computation for all three rule types, instance materialization idempotency.

### 12.2 Integration Tests (facade `tests/`)

- `tests/e2e/alarms.rs` (new) — full lifecycle: create task with alarms → sleep → verify fires → snooze → cancel.
- `tests/e2e/recurrence.rs` (new) — template creation → 3 instances materialize → complete one → next spawns.
- `tests/e2e/quiet_hours.rs` (new) — fire during quiet window → held → released at end of window.
- `tests/e2e/sleep_wake.rs` (new) — simulate `Timestamp::now()` jumping forward 8h → verify missed alarms classified per policy.

### 12.3 Simulation Tests

Extend `tests/simulation/` with scenarios exercising the scheduler under agent load: 100 tasks with varied alarm rules, concurrent modifications, process restart mid-fire.

### 12.4 Migration Validation

Per-layer PR runs: `cargo build --workspace`, `cargo nextest run --workspace`, `cargo clippy --workspace --all-targets --all-features -- -D warnings`, `cargo fmt --all --check`. No PR merges without all four clean.

---

## 13. Observability

Existing `tracing` logs + `PipelineEvent` SSE stream are sufficient per CLAUDE.md's "no structured observability" non-goal. Scheduler emits `tracing::info!` on every fire (alarm_id, kind, delay_ms) and `tracing::warn!` on every misfire skip or delivery failure. UI reads `AlarmFired` + `MissedAlarms` events from the SSE stream for live display.

---

## 14. Open Questions Deferred to Implementation

- **Snooze duration presets** — UI affordance question. Default presets: 5min / 15min / 1h / "tomorrow 9am" — finalize during UI design.
- **RRULE field coverage** — Start with fields listed in §7.1. `BYSETPOS`, `BYWEEKNO`, `BYYEARDAY` deferred unless a concrete user need surfaces.
- **Cross-device sync** — Out of scope (single-user local app). Schema is sync-friendly (UUIDs, epoch timestamps, no local-only references) in case multi-device arrives later.
- **Profile system** — Schema is shaped so that `alarm.profile_id` FK can be added later without data migration. Not built day-1.

---

## 15. Summary

One unified `TemporalScheduler` at L3 with SQLite-backed exact-fire and wall-clock-anchored loop (macOS sleep-safe). One `notifications` crate at L4 with quiet hours, multi-channel dispatch, idempotency, held-release. Per-task alarm rules (relative / civil-time / absolute) + global defaults, all VALARM-inspired. RFC 5545 RRULE for recurring tasks with template + instance materialization. Workspace-wide Chrono → Jiff migration for correct DST handling and RFC 9557 serialization. `TaskTool` gains `alarms` + `recurrence` subfields; new `AlarmTool` for free-floating reminders. Three legacy files deleted. Five new SQLite tables. Six new domain events. Ten-step layer-by-layer rollout.
