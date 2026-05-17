# Subsystem 06 — Scheduling & Automation

> **Status:** 🟡 In Progress (dual-runner mid-migration; stale log message)
> **Status last verified:** 2026-05-16
> **Crates:** `scheduling`
> **Parent overview:** [`00-overview.md`](../00-overview.md)

---

## TL;DR

A single crate that does three jobs at once: (1) **cron** — name-keyed recurring jobs (`CronExecutor`), (2) **wall-clock dispatch** — a unified firing table for cron, alarms, recurrence, and held notifications (`TemporalScheduler` + `FireStore`), and (3) **recurrence engine** — RRULE-based materialization of recurring tasks (`RecurrenceEngine`). Today these run **side-by-side** via `CronBridge`, which keeps the `cron_jobs` definition table in sync with the `scheduled_fires` firing table.

Every wall-clock event in KlyntBot — cron jobs, task alarms, recurrence spawns, held notification releases — flows through `scheduled_fires`. It's the universal timer table; `kind` distinguishes the four types.

---

## Architecture diagram

```mermaid
flowchart TB
    classDef def fill:#f1f8e9,stroke:#558b2f,color:#33691e
    classDef fire fill:#fff9c4,stroke:#f9a825,color:#f57f17
    classDef rec fill:#e3f2fd,stroke:#1976d2,color:#0d47a1
    classDef bus fill:#fff,stroke:#999,stroke-dasharray:5

    subgraph DB ["SQLite tables"]
        CJ[(cron_jobs<br/><i>definition</i>)]:::def
        SF[(scheduled_fires<br/><i>universal firing table</i><br/>kind ∈ {cron_job, alarm, recurrence_spawn, held_release})]:::fire
        NL[(notification_log<br/><i>idempotency</i>)]:::def
        HN[(held_notifications<br/><i>quiet hours buffer</i>)]:::def
    end

    CE[CronExecutor<br/><i>bus subscriber<br/>handler registry<br/>sync closure dispatch via spawn_blocking</i>]:::fire
    TS[TemporalScheduler<br/><i>wall-clock loop<br/>two-phase fire commit<br/>crash recovery</i>]:::fire
    CB[CronBridge<br/><i>reconcile + advance</i><br/>cron_jobs ↔ scheduled_fires]:::fire
    FS[FireStore<br/><i>high-level over ScheduledFiresRepo</i>]:::fire
    RE[RecurrenceEngine<br/><i>on_spawn · materialize_ahead</i>]:::rec
    RR[RRuleSpec / evaluate_next_n<br/><i>chrono boundary</i>]:::rec
    AR[AlarmRule<br/><i>RelativeBefore<br/>CivilTimeOnDayOffset<br/>Absolute</i>]:::rec
    MP[MisfirePolicy<br/><i>Strict / SkipIfStale / Coalesce</i>]:::fire

    BUS[DomainEventBus<br/><i>AlarmFired { kind, ref_id, ... }</i>]:::bus

    CB --> CJ
    CB --> SF
    TS --> SF
    TS --> BUS
    TS --> CB
    TS --> RE
    CE --> BUS
    CE --> CJ
    RE --> FS
    FS --> SF
    RR --> RE
    AR --> FS
    MP --> TS
```

---

## Mental model

Two ways to ask "when should this happen?":

- **`cron_jobs`** stores the *definition* of a recurring schedule: `name`, `schedule` (`At` / `Every` / `Cron(expr)`), `payload`, `enabled`, `intent_window`. There's exactly one row per logical job.
- **`scheduled_fires`** stores the *next concrete firing*: `fire_at_ms`, `kind`, `ref_id`, `payload`, `dedup_prefix`. There can be many rows per logical thing — one per upcoming fire.

`CronBridge` is the glue: on startup it `reconcile_all()`s (one pending `scheduled_fires` row per enabled cron job), and after every fire it `advance(job_id)`s (replace the consumed row with the next one).

**Without `CronBridge`, cron jobs would fire exactly once.**

### The "side-by-side" pair (today)

Two runners coexist:
- **`TemporalScheduler`** — watches `scheduled_fires`, emits `DomainEvent::AlarmFired { kind, ref_id, ... }` at the right time, does two-phase commit (`begin_firing` claims, `mark_fired` finalizes), survives crashes via `recover_in_flight`.
- **`CronExecutor`** — bus subscriber that filters `AlarmFired { kind="cron_job" }`, looks up the registered Rust closure by job name, dispatches via `spawn_blocking`.

CLAUDE.md says "TemporalScheduler started (side-by-side with CronService)." **`CronService` was already removed.** The actual side-by-side pair is `TemporalScheduler` + `CronExecutor`. The log line at `app-core/src/init/temporal_scheduler.rs:99` still says `"with CronService"` — that's stale text in production logs.

The end state (Phase 3): retire `CronExecutor`'s callback-registration pattern. Convert each handler into an independent `DomainEvent::AlarmFired` subscriber. Then `CronExecutor` itself can be deleted; `TemporalScheduler` + the bus + per-handler subscribers cover everything.

---

## Reference

### File map

| Path | Purpose |
|---|---|
| `src/lib.rs` | Public re-exports + module declarations |
| `src/error.rs` | `CronError`, `SchedulerError` |
| `src/types.rs` | `CronJob`, `CronSchedule`, `CronPayload`, `CronJobState`, `CronOrigin`, `IntentWindow`, `IntentTrigger`, `CatchUpPriority` |
| `src/service/mod.rs` | `row_to_job()` helper (retained post-`CronService` removal for `CronExecutor`) |
| `src/temporal/cron_executor.rs` | `CronExecutor` — bus subscriber + handler registry + dispatch |
| `src/temporal/scheduler.rs` | `TemporalScheduler` — wall-clock loop, two-phase fire commit |
| `src/temporal/cron_bridge.rs` | `CronBridge` — syncs `cron_jobs` → `scheduled_fires`. **`// CHRONO BOUNDARY`** |
| `src/temporal/fire_store.rs` | `FireStore` — high-level service over `ScheduledFiresRepo` |
| `src/temporal/misfire.rs` | `MisfirePolicy`, `Decision`, `classify()` |
| `src/temporal/recurrence.rs` | `RecurrenceEngine`, `TemplateRepo`/`InstanceRepo` traits, `RecurrenceTemplate` |
| `src/temporal/rrule.rs` | `RRuleSpec`, `Frequency`, `evaluate_next_n`, `next_n_from_rrule_string`. **`// CHRONO BOUNDARY`** |
| `src/temporal/rules.rs` | `AlarmRule` (`RelativeBefore`, `CivilTimeOnDayOffset`, `Absolute`), `RuleError` |

### Cron primitives

```rust
pub struct CronJob {
    id, name, enabled, origin: CronOrigin, schedule: CronSchedule,
    payload: CronPayload, state: CronJobState, delete_after_run: bool,
    intent_window: Option<IntentWindow>, intent_pending_since_ms: Option<i64>, ...
}
pub enum CronSchedule {
    At    { at_ms: i64 },
    Every { every_ms: u64 },
    Cron  { expr: String, tz: Option<String> },
}
pub type CronHandler = Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>;

pub struct CronExecutor { ... }
impl CronExecutor {
    pub fn new(cron_repo: CronRepo, bus: Arc<DomainEventBus>) -> Self;
    pub fn register(&self, name: &str, handler: CronHandler);
    pub fn set_callback(&self, name: &str, handler: CronHandler);   // alias
    pub fn start(&self, shutdown: CancellationToken) -> JoinHandle<()>;
    pub async fn run_now(&self, job_id: &str) -> Result<bool>;
}
```

**`CronHandler` is a sync closure** (`Fn`, not `AsyncFn`). Dispatched via `tokio::task::spawn_blocking`. Async work inside uses the `tokio::task::block_in_place + rt.block_on(...)` pattern — visible in every handler in `app-core/src/init/cron.rs`. This is non-obvious; new handlers usually want to be async, but the trait shape forces blocking.

### Temporal scheduler

```rust
pub struct FireSpec {
    fire_at: Timestamp,
    kind: String,                       // "cron_job", "alarm", "recurrence_spawn", "held_release"
    ref_id: Option<String>,
    payload: Value,                     // includes misfire_policy + grace_secs in some kinds
    dedup_prefix: Option<String>,
}

pub struct FireStore { ... }
impl FireStore {
    pub async fn schedule(&self, spec: FireSpec) -> Result<String>;
    pub async fn begin_firing(&self, id: &str, now: Timestamp) -> Result<bool>;
    pub async fn mark_fired(&self, id: &str, now: Timestamp) -> Result<()>;
    pub async fn mark_suppressed(&self, id, suppressed_by, now) -> Result<()>;
    pub async fn cancel_by_prefix(&self, prefix: &str) -> Result<u64>;
    pub async fn cancel_by_kind_ref(&self, kind, ref_id) -> Result<u64>;
    pub async fn pending_with_kind_before(&self, cutoff_ms, kind) -> Result<Vec<ScheduledFireRow>>;
}

pub struct CronBridge { ... }
impl CronBridge {
    pub fn new(cron: CronRepo, fires: FireStore) -> Self;
    pub async fn reconcile_all(&self) -> Result<(), SchedulerError>;
    pub async fn advance(&self, job_id: &str) -> Result<(), SchedulerError>;
}

pub struct SchedulerConfig {
    pub max_sleep: Duration,             // default: 30s (cap on sleep between checks)
    pub default_grace_secs: u64,         // default: 3600 (used by SkipIfStale policy)
    pub default_misfire_policy: MisfirePolicy,  // default: SkipIfStale
}

pub struct TemporalScheduler { ... }   // Clone
impl TemporalScheduler {
    pub fn new(store, bus, config) -> Self;
    pub fn with_cron_bridge(self, bridge: CronBridge) -> Self;
    pub fn with_recurrence_engine(self, engine: Arc<RecurrenceEngine>) -> Self;
    pub fn start_background(self) -> JoinHandle<()>;
    pub fn wake(&self);
    pub fn shutdown(&self);
}
```

### Misfire policies

| Policy | Behavior | Uses `grace_secs`? |
|---|---|---|
| `Strict` | Always fire regardless of age | No |
| `SkipIfStale` (default) | Fire if `age ≤ grace`; otherwise emit `MissedAlarms` event and mark fired | Yes (default 3600s) |
| `Coalesce` | Fire only the most-recent row per `(ref_id, kind)`; suppress older with `suppressed_by` | No |

Configured per-row in the `payload` JSON: `{"misfire_policy": "skip_if_stale", "grace_secs": 3600}`. Falls back to `SchedulerConfig` defaults when absent. **No UI per-job today** — set at fire-row insertion time (by `CronBridge`, `HeldReleaseService`, etc.).

### Recurrence

```rust
pub struct RRuleSpec {
    frequency: Frequency,
    interval, by_day, by_month_day, at, timezone, until, count,
}
pub enum Frequency { Daily, Weekly, Monthly, Yearly }

pub fn evaluate_next_n(spec: &RRuleSpec, after: Timestamp, n: usize) -> Result<Vec<Timestamp>>;
pub fn next_n_from_rrule_string(rrule: &str, iana_tz: &str, after: Timestamp, n: usize) -> Result<Vec<Timestamp>>;

pub struct RecurrenceTemplate {
    id, source_task_id, rrule: String, iana_tz,
    materialize_ahead: u32,
    next_instance_at, until_at, count_remaining, enabled,
}
pub trait TemplateRepo: Send + Sync {
    async fn get(&self, id: &str) -> Result<Option<RecurrenceTemplate>>;
    async fn update_next_instance(&self, id: &str, next: Option<Timestamp>) -> Result<()>;
    async fn decrement_count(&self, id: &str) -> Result<Option<u32>>;
    async fn disable(&self, id: &str) -> Result<()>;
}
pub trait InstanceRepo: Send + Sync {
    async fn create_instance(&self, template_id: &str, due_at: Timestamp) -> Result<CreateInstanceOutcome>;
    async fn cancel_unfired_instances(&self, template_id: &str) -> Result<()>;
}
pub struct RecurrenceEngine { ... }
impl RecurrenceEngine {
    pub fn new(store: Arc<FireStore>, template_repo, instance_repo, default_materialize_ahead: u32) -> Self;
    pub async fn on_spawn(&self, template_id: &str, now: Timestamp) -> Result<(), SchedulerError>;
    pub async fn disable_template(&self, template_id: &str) -> Result<(), SchedulerError>;
}
```

### Alarm rules

```rust
pub enum AlarmRule {
    RelativeBefore { offset: Span },                                // "1h before due"
    CivilTimeOnDayOffset { day_offset: i32, time_of_day, iana_tz },  // "9am day before"
    Absolute { fire_at: Timestamp },
}
impl AlarmRule {
    pub fn compute_fire_at(&self, due_date: Option<Timestamp>, default_tz: &str) -> Result<Timestamp, RuleError>;
}
```

### Storage tables touched

| Table | Owner crate | Purpose |
|---|---|---|
| `cron_jobs` | `storage` (`001_initial.sql`) | Cron definitions — one row per logical job |
| `scheduled_fires` | `scheduling` (`001_scheduled_fires.sql`) | Universal firing table — pending + historical fire events for all kinds |
| `notification_log` | `notifications` (`001_notification_tables.sql`) | Idempotency gate: one row per `(alarm_id, channel)` delivery |
| `held_notifications` | `notifications` (`001_notification_tables.sql`) | Quiet-hours buffer; companion `scheduled_fires(kind="held_release")` row releases it |

### `scheduled_fires` schema (key columns)

- `id`, `fire_at_ms`, `kind`, `ref_id`, `payload` (JSON), `dedup_prefix`
- `fired` (bool), `firing_started_at_ms` (two-phase claim), `fired_at_ms`
- `suppressed_by`, `created_at_ms`
- 3 indexes: pending by time, pending by dedup prefix, pending by `(kind, ref_id)`

### `DEFAULT_MATERIALIZE_AHEAD`

`3`. Defined in `app-core/src/init/temporal_scheduler.rs:19-21`. Comment notes it should be promoted to `config.notifications.default_materialize_ahead` — **not done yet**.

---

## Workflows

### A cron fires (TemporalScheduler path)

```
1. TemporalScheduler::run() sleeps until next_pending_fire_at (capped at 30s).
2. Woken by:
   - Timer expiry
   - scheduler.wake() signal
   - SystemDidWake event (macOS resume)
3. process_due(now): SELECT all scheduled_fires WHERE fire_at_ms ≤ now AND fired=0.
4. Per due row: Decision::classify(policy, grace, fire_at, now)
   → Fire | SkipStale | CoalesceLater.
5. Fire path:
   a. begin_firing(id, now)
        → CAS sets firing_started_at_ms; returns false if already claimed.
   b. publish DomainEvent::AlarmFired { fire_id, kind, ref_id, payload, ... }.
   c. mark_fired(id, now) → sets fired=1, fired_at_ms.
   d. If kind="cron_job": cron_bridge.advance(job_id) cancels old row,
      inserts next scheduled_fires row.
6. CronExecutor (separate tokio task) receives the AlarmFired event:
   - Filters kind == "cron_job".
   - Fetches CronJobRow from DB; calls row_to_job().
   - Evaluates intent_window.
   - Looks up handler by job name.
   - Dispatches: tokio::task::spawn_blocking(|| handler(&job)).
```

**Crash recovery:** `recover_in_flight()` runs at startup; finds rows with `firing_started_at_ms IS NOT NULL` AND `fired=0`, re-publishes the event, marks them fired. Eliminates the at-least-once gap from a crash between steps 5a and 5c.

### Creating a recurring task

```
1. User creates a task with RRULE (e.g. FREQ=WEEKLY;BYDAY=MO).
   feature-tasks writes a RecurrenceTemplate row via SqliteTemplateRepo:
   { rrule, iana_tz, materialize_ahead, next_instance_at (first occurrence),
     optional until_at, count_remaining }.
2. Insert scheduled_fires row:
   { kind="recurrence_spawn", ref_id=template_id,
     fire_at=next_instance_at, dedup_prefix=format!("template:{}:", id) }.
3. At fire_at: TemporalScheduler dispatches → RecurrenceEngine::on_spawn(template_id, now).
4. Engine fetches template, calls next_n_from_rrule_string(rrule, tz, cursor, materialize_ahead+1).
5. For each of the first materialize_ahead occurrences:
     instance_repo.create_instance(template_id, due_at) → inserts task row.
     Decrement count_remaining if finite.
6. Update template.next_instance_at to the (materialize_ahead+1)th occurrence.
7. Insert new scheduled_fires(kind="recurrence_spawn") row for the new next_instance_at.
   Cycle repeats.
8. If SourceTaskMissing returned (source task deleted):
   - Template disabled
   - Unfired instances cancelled
   - All pending scheduled_fires with template prefix cancelled
```

### Held notification release (quiet-hours integration)

```
1. AlarmFired during quiet hours → NotificationDispatcher.
2. Dispatcher computes release_at via QuietHoursPolicy (from WakeDeliveryConfig).
3. HeldReleaseService::hold(payload, release_at):
   - Insert held_notifications row.
   - Insert scheduled_fires(kind="held_release", fire_at=release_at) row.
4. At release_at: TemporalScheduler dispatches.
5. NotificationDispatcher::handle_held_release reads the held_notifications row
   and delivers the deferred notification.
```

---

## Internals

### Why `CronHandler` is sync

The trait shape (`Fn`, not `AsyncFn`) predates Rust's stable async-in-trait support. Migration would require renaming `CronHandler` to `AsyncCronHandler` and converting all consumers — touchable but invasive. The current pattern (`spawn_blocking` + `block_in_place`/`block_on`) works and is well-understood.

### Two-phase fire commit

`begin_firing` does an atomic `UPDATE scheduled_fires SET firing_started_at_ms=? WHERE id=? AND firing_started_at_ms IS NULL`. Returns `true` only if the CAS succeeded. Two workers competing for the same fire row (which shouldn't happen, but could on a startup race) won't both fire it.

`mark_fired` then sets `fired=1` and clears the firing claim. If we crash between `begin_firing` and `mark_fired`, the row is "in flight" — `recover_in_flight` finds it on next startup and re-publishes.

### Corrupt-schedule defensive behavior

`row_to_job()` forces `enabled=false` on any job whose `schedule` JSON fails to deserialize. Prevents silent infinite loops from bad-data inserts.

### Chrono ↔ Jiff boundary

The whole crate uses `jiff::Timestamp` *except* in two files marked with `// CHRONO BOUNDARY`:

- `cron_bridge.rs` — `cron::Schedule::after(&now_tz)` requires `chrono::DateTime<Tz>`.
- `rrule.rs` — `rrule::RRuleSet` iterator yields `chrono::DateTime<RruleTz>`.

Both files carry `#![allow(clippy::disallowed_types, clippy::disallowed_methods)]`. Conversion goes through epoch-ms: `Timestamp::as_millisecond()` → `chrono::Utc.timestamp_millis_opt()`.

### The two `AlarmFired` kinds

Two `DomainEvent::AlarmFired` paths with confusingly similar `kind` strings:

- `kind="cron_job"` — emitted by `TemporalScheduler` → routed to `CronExecutor`. Internal dispatch.
- `kind="cron"` — emitted by `app-core/init/cron.rs::publish_cron_alarm` directly from CronHandler callbacks → routed to `NotificationDispatcher` for user-facing notifications.

Same enum variant, different consumers, almost identical kind strings. **Easy to mix up.** Adding to TECH_DEBT.

### `SystemDidWake` integration

`scheduler.wake()` is called when the OS resumes from sleep (macOS `NSWorkspaceDidWakeNotification` → `SystemDidWake` bus event). Eliminates the lag where a 30s `max_sleep` would let the scheduler oversleep after a sleep/resume.

---

## Dependencies & extension points

### Upstream deps

- `chrono` + `chrono-tz` (forced by `cron` crate and `rrule` crate)
- `jiff` (everything else)
- `cron` (cron expression parser)
- `rrule` (RFC 5545 RRULE parser)
- `tokio` (runtime + broadcast)
- `storage` (`CronRepo`, `ScheduledFiresRepo`)
- `bus` (`DomainEventBus`, `DomainEvent`)
- `common`, `config`

### Adding a new `kind` to `scheduled_fires`

1. Pick a stable string (e.g., `"my_event"`).
2. Producer: insert a `scheduled_fires` row with your kind via `FireStore::schedule`.
3. Consumer: subscribe to `DomainEvent::AlarmFired` and filter `kind == "my_event"`.
4. Decide misfire policy and `grace_secs`; embed in `payload` JSON or rely on defaults.
5. Document the new kind here.

### Adding a new `CronHandler`

1. In `app-core/src/init/cron.rs::register_cron_callbacks`:
   ```rust
   cron_executor.register("my_job", Arc::new(|job| { /* sync work */ Ok(None) }));
   ```
2. Insert a `cron_jobs` row with `name="my_job"`. `CronBridge` will pick it up on next `reconcile_all`.
3. For async work inside the closure: `tokio::task::block_in_place(|| { rt.block_on(async { ... }) })`.

### Adding a new misfire policy

⚠️ Requires changes to `Decision::classify` and likely to consumers that need new semantics. Coordinate carefully.

---

## Open questions & debt

- **Stale "CronService" log message.** `temporal_scheduler.rs:99` still says `"side-by-side with CronService"` — CronService was removed; the actual pair is `TemporalScheduler` + `CronExecutor`.
- **`DEFAULT_MATERIALIZE_AHEAD = 3` hardcoded.** Should be `config.notifications.default_materialize_ahead`.
- **Two `AlarmFired` `kind` strings confusingly close** (`cron_job` vs `cron`). Rename one (e.g., `cron_user_facing`) to disambiguate.
- **`CronHandler` is sync.** Migration to `AsyncCronHandler` would clean up every handler in `init/cron.rs` (no more `block_in_place`).
- **Dual chrono/jiff dependency.** Acceptable while the upstream crates require chrono; revisit when alternatives mature.
- **Phase 3 migration unfinished.** End state is each cron job becomes an independent bus subscriber, so `CronExecutor` can be removed.

See [`TECH_DEBT.md`](../TECH_DEBT.md) categories #3 (legacy), #6 (hardcoded config), #8 (naming) for specifics.

---

## Cross-references

- [`01-foundations.md`](./01-foundations.md) — `DomainEventBus` carries `AlarmFired`
- [`02-storage.md`](./02-storage.md) — `CronRepo` + `ScheduledFiresRepo`
- [`05-cognitive-memory.md`](./05-cognitive-memory.md) — Reforge nightly cron registered here
- [`08-assistant-features.md`](./08-assistant-features.md) — `feature-tasks` uses `RecurrenceEngine`
- [`11-channels-mcp.md`](./11-channels-mcp.md) — `notifications` consumes `AlarmFired` and `held_release`
