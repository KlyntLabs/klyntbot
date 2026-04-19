# Phase 4: Final Integration & Cleanup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close every residual gap from Phases 2, 3, and the chrono→jiff migration against spec `docs/superpowers/specs/2026-04-17-unified-temporal-scheduler-and-notifications-design.md`, retire all legacy subsystems, and ship a single integrated scheduler+dispatcher pipeline with a clippy-enforced chrono-free workspace.

**Architecture:** Five sub-phases executed in dependency order: (4A) Complete `TemporalScheduler` (coalesce + recurrence) and retire `CronService`. (4B) Complete `NotificationDispatcher` (channel_mask, OS Focus critical override, telegram/discord/email adapters). (4C) Route legacy call sites through dispatcher, then delete `reminders.rs`, `deadline.rs`, and the `CronService` module. (4D) Final chrono eviction — clippy.toml, workspace dep removal, CLAUDE.md refresh. (4E) Integration test matrix + tray rewire + final verification gate.

**Tech Stack:** Rust 1.93, `jiff` (calendar/timestamp), `rrule = "0.14"` (RRULE evaluation — confirmed jiff-compatible boundary), `tokio` + `tokio-util::CancellationToken`, `cargo-nextest`, clippy `disallowed-types`.

**Spec cross-reference:** §2.3 (TemporalScheduler), §4.1 (scheduled_fires/notification_log/held_notifications schema), §5.3 (misfire policies), §6 (Dispatcher), §7 (RRULE), §8.4 (MCP alarm tool), §9 (jiff migration), §9.5 (tray rewire), §11 (deletions), §12.2 (e2e tests).

---

## File Structure

**Created:**
- `crates/scheduling/src/temporal/recurrence.rs` — `RecurrenceEngine` (template → instance materialization).
- `crates/notifications/src/channel/telegram.rs`
- `crates/notifications/src/channel/discord.rs`
- `crates/notifications/src/channel/email.rs`
- `clippy.toml` — disallowed-types fence.
- `tests/e2e/alarms.rs` — full alarm lifecycle.
- `tests/e2e/recurrence.rs` — template materialization.
- `tests/e2e/quiet_hours_boundary.rs` — held+release across tz.
- `tests/e2e/cron_bridge_restart.rs` — reconciliation after mid-fire restart.
- `tests/e2e/mcp_alarm_tool.rs` — MCP tool exposure smoke.

**Modified:**
- `crates/scheduling/src/temporal/scheduler.rs:207-225` — real `Coalesce` implementation.
- `crates/scheduling/src/temporal/mod.rs` — export `RecurrenceEngine`.
- `crates/scheduling/src/lib.rs` — remove `CronService` re-export; add `RecurrenceEngine`.
- `crates/scheduling/Cargo.toml` — drop `chrono`/`chrono-tz` deps.
- `crates/notifications/src/dispatcher.rs:270-272` — real `resolve_channels`.
- `crates/notifications/src/channel/os_native.rs:33-42` — critical override.
- `crates/notifications/src/channel/mod.rs` — register new adapters.
- `crates/app-core/src/init/deadline.rs` — DELETE after routing.
- `crates/app-core/src/init/cron.rs:241-345` — route through dispatcher.
- `crates/app-core/src/init/mod.rs` — drop `deadline` module.
- `crates/agent/src/services/reminders.rs` — DELETE.
- `crates/agent/src/services/mod.rs` — drop re-export.
- `crates/scheduling/src/service/mod.rs` — DELETE (CronService module).
- `crates/desktop/src/tray_countdown.rs` — subscribe to bus instead of poll.
- `Cargo.toml` (root) lines 109, 135 — remove chrono, chrono-tz.
- `CLAUDE.md:181` — refresh gotcha with jiff guidance.

**Deleted (in 4C):**
- `crates/agent/src/services/reminders.rs`
- `crates/app-core/src/init/deadline.rs`
- `crates/scheduling/src/service/` (directory — old CronService).

---

## Task Overview

```
Phase 4A: Temporal completion        (Tasks 4.1 – 4.4)    coalesce, recurrence, retire CronService
Phase 4B: Dispatcher completion      (Tasks 4.5 – 4.8)    channel_mask, critical override, new adapters
Phase 4C: Legacy deletion            (Tasks 4.9 – 4.11)   route, then delete reminders/deadline
Phase 4D: Chrono final cleanup       (Tasks 4.12 – 4.14)  clippy.toml, workspace deps, CLAUDE.md
Phase 4E: Integration + tray rewire  (Tasks 4.15 – 4.20)  e2e tests, tray bus wiring, MCP smoke, final gate
```

**Total: 20 tasks.** After every task's commit, run `cargo build --workspace` to confirm no regression. Full workspace verification gate runs after each sub-phase.

---

## Phase 4A: Temporal Scheduler Completion

### Task 4.1: Implement `Coalesce` misfire policy (spec §5.3)

**Files:**
- Modify: `crates/scheduling/src/temporal/scheduler.rs:207-225`
- Modify: `crates/scheduling/src/temporal/fire_store.rs` (add `mark_suppressed`)
- Modify: `crates/scheduling/migrations/001_scheduled_fires.sql` (add `suppressed_by TEXT` column)
- Test: `crates/scheduling/src/temporal/scheduler.rs` (inline)

- [ ] **Step 1: Extend migration with `suppressed_by` column**

In `crates/scheduling/migrations/001_scheduled_fires.sql`, add to the `scheduled_fires` table definition:

```sql
suppressed_by TEXT,  -- id of the scheduled_fires row that absorbed this coalesced fire
```

(Per CLAUDE.md pre-release policy, edit the migration in-place.)

- [ ] **Step 2: Add `mark_suppressed` to `FireStore`**

In `crates/scheduling/src/temporal/fire_store.rs`, add:

```rust
pub async fn mark_suppressed(&self, id: &str, suppressed_by: &str, now: Timestamp) -> Result<()> {
    sqlx::query(
        "UPDATE scheduled_fires
         SET fired = 1, fired_at_ms = ?, suppressed_by = ?
         WHERE id = ? AND fired = 0",
    )
    .bind(now.as_millisecond())
    .bind(suppressed_by)
    .bind(id)
    .execute(self.pool.as_ref())
    .await?;
    Ok(())
}
```

- [ ] **Step 3: Write the failing test**

Add to `crates/scheduling/src/temporal/scheduler.rs` test module:

```rust
#[tokio::test]
async fn coalesce_fires_only_most_recent_per_ref() {
    let (scheduler, store, _bus) = test_scheduler_with_policy(MisfirePolicy::Coalesce).await;
    let now = Timestamp::now();
    // Three stale rows, same ref_id+kind
    let ids = ["c1", "c2", "c3"];
    for (i, id) in ids.iter().enumerate() {
        store.insert_test_row(id, "task:abc:reminder", "task_alarm",
            now - Span::new().minutes((30 - i * 5) as i64)).await.unwrap();
    }
    scheduler.process_due_fires(now).await.unwrap();
    // Most recent (c3) fired; c1+c2 suppressed
    assert_eq!(store.fired_ids().await, vec!["c1", "c2", "c3"]);
    assert_eq!(store.suppressed_by("c1").await, Some("c3".into()));
    assert_eq!(store.suppressed_by("c2").await, Some("c3".into()));
    assert_eq!(store.suppressed_by("c3").await, None);
}
```

- [ ] **Step 4: Run test to verify it fails**

Run: `cargo nextest run -p scheduling coalesce_fires_only_most_recent_per_ref`
Expected: FAIL — coalesce still falls through to `dispatch` for every row.

- [ ] **Step 5: Implement coalescing in `process_due_fires`**

Replace the stub at `scheduler.rs:207-220`:

```rust
// Partition rows into Fire / SkipStale / CoalesceLater
use std::collections::HashMap;
let mut fire: Vec<ScheduledFireRow> = Vec::new();
let mut skip: Vec<ScheduledFireRow> = Vec::new();
let mut coalesce: HashMap<(String, String), Vec<ScheduledFireRow>> = HashMap::new();

for row in due_rows {
    let (policy, grace) = self.extract_misfire_params(&row);
    let Ok(fire_at) = Timestamp::from_millisecond(row.fire_at_ms) else { continue };
    match Decision::classify(policy, grace, fire_at, now) {
        Decision::Fire => fire.push(row),
        Decision::SkipStale => skip.push(row),
        Decision::CoalesceLater => {
            let key = (row.ref_id.clone().unwrap_or_default(), row.kind.clone());
            coalesce.entry(key).or_default().push(row);
        }
    }
}

// Fire + Skip unchanged
for row in fire { self.dispatch(row, now).await?; }
let mut missed = Vec::new();
for row in skip {
    if self.store.begin_firing(&row.id, now).await? {
        self.store.mark_fired(&row.id, now).await?;
        missed.push(row);
    }
}

// Coalesce: most recent fires, others get suppressed_by pointer
for (_key, mut group) in coalesce {
    group.sort_by_key(|r| r.fire_at_ms);
    let winner = group.pop().expect("group non-empty");
    let winner_id = winner.id.clone();
    for loser in group {
        self.store.mark_suppressed(&loser.id, &winner_id, now).await?;
    }
    self.dispatch(winner, now).await?;
}

if !missed.is_empty() { self.emit_missed(missed); }
Ok(())
```

- [ ] **Step 6: Run test to verify it passes**

Run: `cargo nextest run -p scheduling coalesce_fires_only_most_recent_per_ref`
Expected: PASS.

- [ ] **Step 7: Run full scheduling suite + clippy**

```bash
cargo nextest run -p scheduling
cargo clippy -p scheduling --all-targets -- -D warnings
```
Expected: all pass.

- [ ] **Step 8: Commit**

```bash
git add crates/scheduling/
git commit -m "feat(scheduling): implement Coalesce misfire policy with suppressed_by"
```

---

### Task 4.2: Create `RecurrenceEngine` (spec §7)

**Files:**
- Create: `crates/scheduling/src/temporal/recurrence.rs`
- Modify: `crates/scheduling/src/temporal/mod.rs` (export)
- Modify: `crates/scheduling/src/lib.rs` (public re-export)
- Modify: `crates/feature-tasks/src/recurrence.rs` (wire engine — verify existence first)
- Test: `crates/scheduling/src/temporal/recurrence.rs` (inline)

- [ ] **Step 1: Survey existing recurrence scaffolding**

Run: `grep -rn "task_recurrence_templates\|RecurrenceEngine\|RRuleSpec" crates/`
Record: which crate owns the template repo, and whether any engine stub exists.

- [ ] **Step 2: Create `recurrence.rs` scaffold**

File content:

```rust
//! RecurrenceEngine — materializes task instances from RRULE templates.
//! Spec: §7.2 (template + instance model), §7.3 (DST correctness).

use std::sync::Arc;

use jiff::{tz::TimeZone, Timestamp, Zoned};

use crate::temporal::{
    fire_store::{FireStore, ScheduledFireRow},
    rrule::RRuleEvaluator,
};

pub struct RecurrenceEngine {
    store: Arc<FireStore>,
    template_repo: Arc<dyn TemplateRepo>,
    instance_repo: Arc<dyn InstanceRepo>,
    default_materialize_ahead: u32,
}

#[async_trait::async_trait]
pub trait TemplateRepo: Send + Sync {
    async fn get(&self, id: &str) -> anyhow::Result<Option<RecurrenceTemplate>>;
    async fn update_next_instance(&self, id: &str, next_at: Option<Timestamp>) -> anyhow::Result<()>;
    async fn decrement_count(&self, id: &str) -> anyhow::Result<Option<u32>>;
    async fn disable(&self, id: &str) -> anyhow::Result<()>;
}

#[async_trait::async_trait]
pub trait InstanceRepo: Send + Sync {
    async fn create_instance(&self, template_id: &str, due_at: Timestamp) -> anyhow::Result<String>;
    async fn cancel_unfired_instances(&self, template_id: &str) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct RecurrenceTemplate {
    pub id: String,
    pub source_task_id: String,
    pub rrule: String,
    pub iana_tz: String,
    pub materialize_ahead: u32,
    pub next_instance_at: Option<Timestamp>,
    pub until_at: Option<Timestamp>,
    pub count_remaining: Option<u32>,
    pub enabled: bool,
}

impl RecurrenceEngine {
    pub fn new(
        store: Arc<FireStore>,
        template_repo: Arc<dyn TemplateRepo>,
        instance_repo: Arc<dyn InstanceRepo>,
        default_materialize_ahead: u32,
    ) -> Self {
        Self { store, template_repo, instance_repo, default_materialize_ahead }
    }

    /// Fired when a `kind='recurrence_spawn'` scheduled_fire arrives.
    /// Materializes the next `materialize_ahead` instances and reschedules the spawn.
    pub async fn on_spawn(&self, template_id: &str, now: Timestamp) -> anyhow::Result<()> {
        let Some(tmpl) = self.template_repo.get(template_id).await? else { return Ok(()); };
        if !tmpl.enabled { return Ok(()); }

        let tz = TimeZone::get(&tmpl.iana_tz)?;
        let evaluator = RRuleEvaluator::parse(&tmpl.rrule, tz.clone())?;
        let start = tmpl.next_instance_at.unwrap_or(now);

        let mut materialized = 0u32;
        let mut cursor = start;
        let ahead = if tmpl.materialize_ahead == 0 { self.default_materialize_ahead } else { tmpl.materialize_ahead };

        while materialized < ahead {
            let Some(next) = evaluator.next_after(cursor) else { break };
            if let Some(until) = tmpl.until_at { if next > until { break } }
            if let Some(0) = tmpl.count_remaining { break }

            self.instance_repo.create_instance(&tmpl.id, next).await?;
            if tmpl.count_remaining.is_some() {
                if self.template_repo.decrement_count(&tmpl.id).await? == Some(0) {
                    self.template_repo.update_next_instance(&tmpl.id, None).await?;
                    return Ok(());
                }
            }
            cursor = next;
            materialized += 1;
        }

        // Reschedule the next spawn at the N+1th occurrence
        let next_spawn = evaluator.next_after(cursor);
        self.template_repo.update_next_instance(&tmpl.id, next_spawn).await?;
        if let Some(next_at) = next_spawn {
            self.store.insert(ScheduledFireRow::recurrence_spawn(template_id, next_at)).await?;
        }
        Ok(())
    }

    /// Called when a template is disabled or deleted — cascade-cancel unfired instances.
    pub async fn disable_template(&self, template_id: &str) -> anyhow::Result<()> {
        self.template_repo.disable(template_id).await?;
        self.instance_repo.cancel_unfired_instances(template_id).await?;
        self.store.cancel_by_prefix(&format!("template:{template_id}:")).await?;
        Ok(())
    }
}
```

- [ ] **Step 3: Register module and re-export**

In `crates/scheduling/src/temporal/mod.rs`, add:

```rust
pub mod recurrence;
pub use recurrence::{RecurrenceEngine, RecurrenceTemplate, TemplateRepo, InstanceRepo};
```

In `crates/scheduling/src/lib.rs`, add:

```rust
pub use temporal::{RecurrenceEngine, RecurrenceTemplate};
```

- [ ] **Step 4: Write unit test: UNTIL stops materialization**

```rust
#[tokio::test]
async fn until_stops_materialization() {
    let (engine, tmpl_repo, inst_repo, _store) = test_engine().await;
    let until = Timestamp::from_str("2026-05-01T00:00:00Z").unwrap();
    tmpl_repo.insert(RecurrenceTemplate {
        id: "t1".into(), source_task_id: "s1".into(),
        rrule: "FREQ=DAILY".into(), iana_tz: "UTC".into(),
        materialize_ahead: 10, next_instance_at: Some(Timestamp::from_str("2026-04-28T00:00:00Z").unwrap()),
        until_at: Some(until), count_remaining: None, enabled: true,
    }).await;
    engine.on_spawn("t1", Timestamp::now()).await.unwrap();
    assert_eq!(inst_repo.count_for("t1").await, 3); // 4/28, 4/29, 4/30
}
```

- [ ] **Step 5: Write unit test: COUNT decrements to zero**

```rust
#[tokio::test]
async fn count_decrements_to_zero_then_halts() {
    let (engine, tmpl_repo, inst_repo, _store) = test_engine().await;
    tmpl_repo.insert(RecurrenceTemplate {
        rrule: "FREQ=DAILY".into(), count_remaining: Some(2),
        ..minimal_template("t2")
    }).await;
    engine.on_spawn("t2", Timestamp::now()).await.unwrap();
    assert_eq!(inst_repo.count_for("t2").await, 2);
    assert_eq!(tmpl_repo.count_remaining("t2").await, Some(0));
}
```

- [ ] **Step 6: Write unit test: disable cascades**

```rust
#[tokio::test]
async fn disable_cancels_unfired_instances_and_fires() {
    let (engine, tmpl_repo, inst_repo, store) = test_engine().await;
    tmpl_repo.insert(minimal_template("t3")).await;
    engine.on_spawn("t3", Timestamp::now()).await.unwrap();
    engine.disable_template("t3").await.unwrap();
    assert_eq!(inst_repo.unfired_count("t3").await, 0);
    assert_eq!(store.pending_with_prefix("template:t3:").await, 0);
}
```

- [ ] **Step 7: Run tests**

```bash
cargo nextest run -p scheduling recurrence
cargo clippy -p scheduling --all-targets -- -D warnings
```
Expected: PASS.

- [ ] **Step 8: Wire engine into scheduler fire dispatch**

In `crates/scheduling/src/temporal/scheduler.rs`, when dispatching a row with `kind == "recurrence_spawn"`, call `self.recurrence_engine.on_spawn(&row.ref_id, now)`. Add `recurrence_engine: Option<Arc<RecurrenceEngine>>` to scheduler (optional to keep tests minimal).

- [ ] **Step 9: Commit**

```bash
git add crates/scheduling/
git commit -m "feat(scheduling): add RecurrenceEngine for RRULE template materialization"
```

---

### Task 4.3: Wire `RecurrenceEngine` into feature-tasks template repo

**Files:**
- Modify: `crates/feature-tasks/src/recurrence.rs` (or create if absent — verify with grep in Step 1)
- Modify: `crates/app-core/src/init/temporal_scheduler.rs` (construct engine + inject)

- [ ] **Step 1: Verify template repo location**

Run: `grep -rln "task_recurrence_templates" crates/feature-tasks/`
If no repo exists: create `crates/feature-tasks/src/recurrence_repo.rs` implementing `scheduling::TemplateRepo` + `InstanceRepo` over `StoragePool`.

- [ ] **Step 2: Implement `TemplateRepo` trait over SQLite**

Standard pattern: read `task_recurrence_templates` rows, map to `RecurrenceTemplate`. `update_next_instance`, `decrement_count`, `disable` as single UPDATE statements. `create_instance` inserts into `tasks` with `template_id` FK.

- [ ] **Step 3: Construct engine in `app-core/init/temporal_scheduler.rs`**

```rust
let template_repo = Arc::new(feature_tasks::recurrence_repo::SqliteTemplateRepo::new(pool.clone()));
let instance_repo = Arc::new(feature_tasks::recurrence_repo::SqliteInstanceRepo::new(pool.clone()));
let recurrence = Arc::new(RecurrenceEngine::new(
    fire_store.clone(), template_repo, instance_repo,
    config.notifications.default_materialize_ahead.unwrap_or(3),
));
let scheduler = TemporalScheduler::builder()
    .recurrence_engine(recurrence.clone())
    .build(...);
```

- [ ] **Step 4: Run tests + build**

```bash
cargo build --workspace
cargo nextest run -p feature-tasks -p scheduling -p app-core
cargo clippy --workspace --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add crates/feature-tasks/ crates/app-core/
git commit -m "feat(feature-tasks): wire SqliteTemplateRepo to RecurrenceEngine"
```

---

### Task 4.4: Retire `CronService` — use `CronBridge` exclusively

> **Addendum (2026-04-19, post-survey):** This task was split into 4.4a / 4.4b / 4.4c after a subagent survey revealed that `CronBridge` only handles scheduling — the *execution dispatch* layer (20+ handler callbacks like reforge nightly, session cleanup, atom decay) lives entirely in `CronService` and has no replacement. The original single-task scope would have required deleting CronService without first building the executor, breaking all recurring-job execution.
>
> **4.4a — Build `CronExecutor`:** New L3 type in `crates/scheduling/src/temporal/cron_executor.rs` that subscribes to `AlarmFired` events where `kind == "cron_job"` and dispatches to a handler map with the same signature CronService used. Pure addition; no deletions. ~200 LOC.
>
> **4.4b — Migrate CRUD/IPC to `CronRepo` + `CronBridge`:** Tauri handlers (`handlers/cron.rs`) and `ensure_cron_jobs()` call `CronRepo` directly for DB ops + `CronBridge::reconcile_all()` after mutations. `AppState.cron_service` is preserved for now (handler registration only). ~300 LOC across handlers/cron.rs, state.rs, init/cron.rs.
>
> **4.4c — Delete `CronService`; wire `CronExecutor`:** `register_cron_callbacks` registers into `CronExecutor` instead of `CronService`. Delete `crates/scheduling/src/service/`, remove `AppState.cron_service`, update `agent::builder::with_cron_service` to take the executor. ~400 LOC deleted, ~100 LOC rewired.
>
> The original checklist below is superseded by the three sub-tasks.

**Files:**
- Delete: `crates/scheduling/src/service/` (entire directory)
- Modify: `crates/scheduling/src/lib.rs` (drop `pub mod service`, drop `pub use service::CronService`)
- Modify: `crates/app-core/src/init/cron.rs` — replace `CronService::start` with `CronBridge::reconcile_all` at startup
- Modify: `crates/scheduling/Cargo.toml` (drop chrono deps once last usage removed)

- [ ] **Step 1: Find all `CronService` call sites**

Run: `grep -rn "CronService\|scheduling::service" crates/`
Record the list. Expect: `app-core/init/cron.rs`, possibly `lib.rs` re-exports in `klyntbot`.

- [ ] **Step 2: Replace `CronService::start` with bridge reconcile**

In `crates/app-core/src/init/cron.rs`, replace the `CronService` construction with:

```rust
// CronBridge now owns the firing lifecycle; reconcile on startup is sufficient.
let bridge = CronBridge::new(repos.cron_jobs.clone(), fire_store.clone());
bridge.reconcile_all().await?;
```

The bridge's `advance()` is already called by `TemporalScheduler` after each fire (verified in Phase 2 audit).

- [ ] **Step 3: Delete `crates/scheduling/src/service/`**

Run: `rm -rf crates/scheduling/src/service/`

- [ ] **Step 4: Remove module declaration + re-export**

In `crates/scheduling/src/lib.rs`, delete `pub mod service;` and any `pub use service::...` lines.

- [ ] **Step 5: Verify no chrono usage remains in scheduling (except documented shims)**

Run: `grep -rn "^use chrono\|chrono::" crates/scheduling/src/`
Expected: matches only in `cron_bridge.rs` and `rrule.rs` (documented boundary shims). If matches elsewhere, migrate per the jiff migration cookbook.

- [ ] **Step 6: Build + test + clippy**

```bash
cargo build -p scheduling -p app-core
cargo nextest run -p scheduling -p app-core
cargo clippy -p scheduling -p app-core --all-targets -- -D warnings
```

- [ ] **Step 7: Commit**

```bash
git add crates/scheduling/ crates/app-core/
git commit -m "refactor(scheduling): retire CronService; CronBridge is the sole cron firing path"
```

---

### Phase 4A Checkpoint

- [ ] **Run full workspace verification**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: all pass. Note: the `scheduled_fires` column-name divergence (`_ms` suffix) is **accepted as-is** — we update the spec in Task 4.20 rather than rename columns, because the `_ms` suffix is a project-wide convention (see `cron_jobs.next_run_at_ms`).

---

## Phase 4B: Dispatcher Completion

### Task 4.5: Implement real `resolve_channels` with channel_mask (spec §6.1)

**Files:**
- Modify: `crates/notifications/src/dispatcher.rs:270-272`
- Modify: `crates/notifications/src/dispatcher.rs` (NotificationPayload struct — add `channel_mask: u32`, `priority_override: Option<String>`)

- [ ] **Step 1: Extend `NotificationPayload` to carry channel_mask + priority_override**

In `crates/notifications/src/dispatcher.rs`, locate `struct NotificationPayload` and add:

```rust
pub channel_mask: u32,              // 0 = inherit defaults
pub priority_override: Option<String>, // "urgent" | None
```

Update `parse_payload` to read these from the JSON:

```rust
let channel_mask = v.get("channel_mask").and_then(|x| x.as_u64()).unwrap_or(0) as u32;
let priority_override = v.get("priority_override").and_then(|x| x.as_str()).map(String::from);
```

- [ ] **Step 2: Define channel bit constants**

Add to `crates/notifications/src/channel/mod.rs`:

```rust
pub const CHANNEL_OS_NATIVE: u32 = 1 << 0;
pub const CHANNEL_TRAY: u32      = 1 << 1;
pub const CHANNEL_TELEGRAM: u32  = 1 << 2;
pub const CHANNEL_DISCORD: u32   = 1 << 3;
pub const CHANNEL_EMAIL: u32     = 1 << 4;

pub fn mask_to_names(mask: u32) -> Vec<String> {
    let mut v = Vec::new();
    if mask & CHANNEL_OS_NATIVE != 0 { v.push("os_native".into()); }
    if mask & CHANNEL_TRAY      != 0 { v.push("tray".into()); }
    if mask & CHANNEL_TELEGRAM  != 0 { v.push("telegram".into()); }
    if mask & CHANNEL_DISCORD   != 0 { v.push("discord".into()); }
    if mask & CHANNEL_EMAIL     != 0 { v.push("email".into()); }
    v
}
```

- [ ] **Step 3: Write the failing test**

Add to `crates/notifications/src/dispatcher.rs` test module:

```rust
#[test]
fn resolve_channels_uses_mask_when_nonzero() {
    let dispatcher = test_dispatcher(vec!["os_native".into(), "tray".into()]);
    let payload = NotificationPayload {
        channel_mask: CHANNEL_TELEGRAM | CHANNEL_EMAIL,
        ..test_payload()
    };
    assert_eq!(dispatcher.resolve_channels(&payload), vec!["telegram", "email"]);
}

#[test]
fn resolve_channels_falls_back_to_defaults_when_mask_zero() {
    let dispatcher = test_dispatcher(vec!["tray".into()]);
    let payload = NotificationPayload { channel_mask: 0, ..test_payload() };
    assert_eq!(dispatcher.resolve_channels(&payload), vec!["tray"]);
}
```

- [ ] **Step 4: Run test to verify failure**

Run: `cargo nextest run -p notifications resolve_channels`
Expected: FAIL — mask ignored.

- [ ] **Step 5: Replace stub at lines 270-272**

```rust
fn resolve_channels(&self, payload: &NotificationPayload) -> Vec<String> {
    if payload.channel_mask == 0 {
        self.default_channels.clone()
    } else {
        crate::channel::mask_to_names(payload.channel_mask)
    }
}
```

- [ ] **Step 6: Run tests pass**

Run: `cargo nextest run -p notifications`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/notifications/
git commit -m "feat(notifications): respect alarm channel_mask with fallback to defaultChannels"
```

---

### Task 4.6: Add OS Focus critical override (spec §6.2)

**Files:**
- Modify: `crates/notifications/src/channel/os_native.rs:33-42`
- Modify: `crates/notifications/src/channel/mod.rs` (Channel trait — pass priority)
- Modify: `crates/platform-macos/src/notifications.rs` (expose `send_critical`)

- [ ] **Step 1: Extend Channel trait with priority**

In `crates/notifications/src/channel/mod.rs`, update the `Channel::deliver` signature:

```rust
#[async_trait::async_trait]
pub trait Channel: Send + Sync {
    async fn deliver(&self, payload: &NotificationPayload) -> Result<(), ChannelError>;
}
```

`NotificationPayload` already has `priority_override` from Task 4.5.

- [ ] **Step 2: Add `send_critical` on macOS platform**

In `crates/platform-macos/src/notifications.rs` (or wherever `NotificationSender` impl lives), add:

```rust
pub fn send_critical(&self, title: &str, body: &str) -> Result<()> {
    // UNNotificationInterruptionLevelCritical — bypasses Focus / DND.
    // Requires entitlement com.apple.developer.usernotifications.critical-alerts.
    unsafe {
        let request = build_request(title, body, InterruptionLevel::Critical);
        UNUserNotificationCenter::current().add(request)?;
    }
    Ok(())
}
```

On non-macOS: `send_critical` is a thin alias over `send` (no-op override).

- [ ] **Step 3: Branch in `OsNativeChannel::deliver`**

Replace `crates/notifications/src/channel/os_native.rs:33-42`:

```rust
async fn deliver(&self, payload: &NotificationPayload) -> Result<(), ChannelError> {
    let is_urgent = payload.priority_override.as_deref() == Some("urgent");
    if is_urgent {
        self.sender.send_critical(&payload.title, &payload.body)
            .map_err(ChannelError::from)?;
    } else {
        self.sender.send(&payload.title, &payload.body)
            .map_err(ChannelError::from)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Write unit test with mock sender**

```rust
#[tokio::test]
async fn urgent_priority_invokes_critical_path() {
    let mock = MockSender::default();
    let channel = OsNativeChannel::new(Arc::new(mock.clone()));
    channel.deliver(&payload_with_priority("urgent")).await.unwrap();
    assert_eq!(mock.critical_calls(), 1);
    assert_eq!(mock.normal_calls(), 0);
}
```

- [ ] **Step 5: Run tests + clippy**

```bash
cargo nextest run -p notifications -p platform-macos
cargo clippy -p notifications -p platform-macos --all-targets -- -D warnings
```

- [ ] **Step 6: Commit**

```bash
git add crates/notifications/ crates/platform-macos/
git commit -m "feat(notifications): route urgent priority to OS critical interruption level"
```

---

### Task 4.7: Add telegram / discord / email channel adapters (spec §6.2)

**Files:**
- Create: `crates/notifications/src/channel/telegram.rs`
- Create: `crates/notifications/src/channel/discord.rs`
- Create: `crates/notifications/src/channel/email.rs`
- Modify: `crates/notifications/src/channel/mod.rs` (register modules)
- Modify: `crates/notifications/src/dispatcher.rs` (construct adapters from config)
- Modify: `crates/notifications/Cargo.toml` (depend on `channels` crate)

- [ ] **Step 1: Inspect existing `channels` crate API**

Run: `ls crates/channels/src/ && grep -l "send_message" crates/channels/src/`
Record the sender traits for each platform (likely `TelegramSender`, `DiscordSender`, `EmailSender`).

- [ ] **Step 2: Create `telegram.rs`**

```rust
use async_trait::async_trait;
use channels::telegram::TelegramSender;
use std::sync::Arc;

use super::{Channel, ChannelError, NotificationPayload};

pub struct TelegramChannel {
    sender: Arc<TelegramSender>,
    default_chat_id: String,
}

impl TelegramChannel {
    pub fn new(sender: Arc<TelegramSender>, default_chat_id: String) -> Self {
        Self { sender, default_chat_id }
    }
}

#[async_trait]
impl Channel for TelegramChannel {
    async fn deliver(&self, payload: &NotificationPayload) -> Result<(), ChannelError> {
        let text = format!("*{}*\n{}", payload.title, payload.body);
        self.sender.send_message(&self.default_chat_id, &text).await
            .map_err(|e| ChannelError::Delivery(e.to_string()))?;
        Ok(())
    }
}
```

- [ ] **Step 3: Create `discord.rs`**

Same pattern with `DiscordSender` and the configured default channel id.

- [ ] **Step 4: Create `email.rs`**

Same pattern with `EmailSender`; subject = payload.title, body = payload.body.

- [ ] **Step 5: Register modules**

In `crates/notifications/src/channel/mod.rs`:

```rust
pub mod discord;
pub mod email;
pub mod telegram;
pub use discord::DiscordChannel;
pub use email::EmailChannel;
pub use telegram::TelegramChannel;
```

- [ ] **Step 6: Construct adapters in dispatcher init**

In `crates/notifications/src/dispatcher.rs` builder / constructor, register adapters when their respective config sections are present:

```rust
if let Some(tg_cfg) = &config.channels.telegram {
    dispatcher.register("telegram", Arc::new(TelegramChannel::new(
        tg_sender.clone(), tg_cfg.default_chat_id.clone(),
    )));
}
// similar for discord, email
```

- [ ] **Step 7: Unit test — adapter invokes sender**

One test per adapter with a mock sender, verifying `deliver` calls `send_message` with the payload text.

- [ ] **Step 8: Build + test + clippy**

```bash
cargo build -p notifications
cargo nextest run -p notifications
cargo clippy -p notifications --all-targets -- -D warnings
```

- [ ] **Step 9: Commit**

```bash
git add crates/notifications/
git commit -m "feat(notifications): add telegram, discord, email channel adapters"
```

---

### Task 4.8: Phase 4B Checkpoint

- [ ] **Full workspace verification**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: all pass.

---

## Phase 4C: Legacy Deletion

### Task 4.9: Route `init/deadline.rs` logic through `NotificationDispatcher`

**Files:**
- Modify: `crates/app-core/src/init/deadline.rs:20-30`
- Modify: `crates/bus/src/domain_events.rs` (confirm `AlarmFired` carries all needed fields)

- [ ] **Step 1: Locate all deadline notification call sites**

Run: `grep -n "TrayNotificationRequested\|notify_tray" crates/app-core/src/init/deadline.rs`

- [ ] **Step 2: Replace `notify_tray` with `AlarmFired` emission**

In `crates/app-core/src/init/deadline.rs:20-30`, replace:

```rust
fn notify_alarm(bus: &DomainEventBus, alarm_id: &str, title: String, body: String) {
    let payload = serde_json::json!({
        "title": title,
        "body": body,
        "channel_mask": 0,              // inherit defaults
        "priority_override": null,
    }).to_string();
    bus.publish(DomainEvent::AlarmFired {
        alarm_id: alarm_id.into(),
        task_id: None,
        kind: "deadline_legacy".into(),
        payload_json: payload,
    });
}
```

Update every call site from `notify_tray(&bus, title, body)` to `notify_alarm(&bus, &uuid::Uuid::new_v4().to_string(), title, body)`.

- [ ] **Step 3: Remove the `TODO(phase-4)` comment**

Delete lines 22-24 of `crates/app-core/src/init/deadline.rs`.

- [ ] **Step 4: Test via scheduler integration test**

```bash
cargo nextest run -p app-core -E 'test(deadline)'
cargo clippy -p app-core --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/deadline.rs
git commit -m "refactor(app-core): route deadline notifications through NotificationDispatcher"
```

---

### Task 4.10: Route `init/cron.rs` TODOs through dispatcher

**Files:**
- Modify: `crates/app-core/src/init/cron.rs:241-242, 302-303, 345-346`

- [ ] **Step 1: Read the three TODO sites**

Run: `grep -n "TODO(phase-4)" crates/app-core/src/init/cron.rs`
Expected: three matches at the line numbers above.

- [ ] **Step 2: Replace each direct tray publish with `AlarmFired`**

At each site, swap `DomainEvent::TrayNotificationRequested { ... }` for `DomainEvent::AlarmFired { ... }` with the same `payload_json` shape used in Task 4.9. Include `kind: "cron"` and `ref_id: cron_job_id` via the payload.

- [ ] **Step 3: Delete the three `TODO(phase-4)` comments**

- [ ] **Step 4: Test + clippy**

```bash
cargo nextest run -p app-core -E 'test(cron)'
cargo clippy -p app-core --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/cron.rs
git commit -m "refactor(app-core): route cron notifications through NotificationDispatcher"
```

---

### Task 4.11: Delete legacy files (spec §11)

**Files:**
- Delete: `crates/agent/src/services/reminders.rs`
- Delete: `crates/app-core/src/init/deadline.rs`
- Modify: `crates/agent/src/services/mod.rs` (drop `pub mod reminders`)
- Modify: `crates/app-core/src/init/mod.rs` (drop `pub mod deadline`)
- Modify: `crates/app-core/src/lib.rs` / `AppCore` struct (remove `DeadlineScheduler` field if present)

- [ ] **Step 1: Find remaining references**

Run: `grep -rn "ReminderEngine\|services::reminders\|init::deadline\|DeadlineScheduler" crates/`
Record every hit.

- [ ] **Step 2: Remove each reference**

For each hit: delete the line (imports) or refactor callers to use `TemporalScheduler` / `NotificationDispatcher`. The scheduler self-initializes from `scheduled_fires` on boot, so no replacement startup logic is needed (spec §11).

- [ ] **Step 3: Delete the files**

```bash
rm crates/agent/src/services/reminders.rs
rm crates/app-core/src/init/deadline.rs
```

- [ ] **Step 4: Remove module declarations**

- `crates/agent/src/services/mod.rs`: delete `pub mod reminders;`
- `crates/app-core/src/init/mod.rs`: delete `pub mod deadline;` and the `init_deadline_scheduler` call

- [ ] **Step 5: Build + test full workspace**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: all pass. Any failure indicates a missed reference from Step 1.

- [ ] **Step 6: Commit**

```bash
git add -u   # captures deletions
git commit -m "chore: delete legacy ReminderEngine, DeadlineScheduler init module (spec §11)"
```

---

## Phase 4D: Chrono Final Cleanup

### Task 4.12: Create `clippy.toml` fence

**Files:**
- Create: `clippy.toml`

- [ ] **Step 1: Create the file**

File content at repo root `clippy.toml`:

```toml
disallowed-types = [
    { path = "chrono::DateTime",     reason = "use jiff::Timestamp or jiff::Zoned" },
    { path = "chrono::NaiveDateTime", reason = "use jiff::civil::DateTime" },
    { path = "chrono::NaiveDate",     reason = "use jiff::civil::Date" },
    { path = "chrono::NaiveTime",     reason = "use jiff::civil::Time" },
    { path = "chrono::Utc",           reason = "use jiff::Timestamp or jiff::tz::TimeZone::UTC" },
    { path = "chrono::Local",         reason = "use jiff::Zoned::now() with system tz" },
    { path = "chrono::Duration",      reason = "use jiff::Span or std::time::Duration" },
    { path = "chrono_tz::Tz",         reason = "use jiff::tz::TimeZone" },
]
disallowed-methods = [
    { path = "chrono::Utc::now",   reason = "use jiff::Timestamp::now" },
    { path = "chrono::Local::now", reason = "use jiff::Zoned::now" },
]
```

- [ ] **Step 2: Add crate-level allow to boundary shims**

In `crates/scheduling/src/temporal/cron_bridge.rs` and `crates/scheduling/src/temporal/rrule.rs`, at the top of each file add:

```rust
#![allow(clippy::disallowed_types, clippy::disallowed_methods)]
// Documented chrono boundary shim — transitively required by `cron` / `rrule` crates.
```

- [ ] **Step 3: Run workspace clippy to confirm zero violations**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: PASS. If a chrono call site surfaces, migrate it or (if genuinely transitive-boundary) add the file-level allow with a one-line justification.

- [ ] **Step 4: Commit**

```bash
git add clippy.toml crates/scheduling/src/temporal/cron_bridge.rs crates/scheduling/src/temporal/rrule.rs
git commit -m "chore(lint): add clippy disallowed-types fence for chrono"
```

---

### Task 4.13: Remove workspace-level chrono deps

**Files:**
- Modify: `Cargo.toml` (lines 109, 135)
- Modify: `crates/scheduling/Cargo.toml` (lines 14-15)

- [ ] **Step 1: Inspect current deps**

Run: `grep -n "^chrono\|^chrono-tz" Cargo.toml crates/*/Cargo.toml`
Expected: root `Cargo.toml:109,135` and `crates/scheduling/Cargo.toml:14-15`.

- [ ] **Step 2: Move chrono to `scheduling` crate-local dep**

In `crates/scheduling/Cargo.toml`, replace `chrono = { workspace = true }` with:

```toml
chrono = { version = "0.4", default-features = false, features = ["std", "serde"] }
chrono-tz = "0.10"
```

These remain because `cron` and `rrule` crates require chrono types at their API boundary (verified: `rrule = "0.14"` still exposes chrono — only `>=0.12` roadmaps jiff support, which has not landed as of 2026-04).

- [ ] **Step 3: Delete workspace-root chrono lines**

In root `Cargo.toml`, delete lines 109 and 135 (the `chrono` and `chrono-tz` entries under `[workspace.dependencies]`).

- [ ] **Step 4: Verify no other crate uses `chrono = { workspace = true }`**

Run: `grep -rn "chrono = { workspace" crates/*/Cargo.toml`
Expected: zero matches.

- [ ] **Step 5: Full workspace build + clippy**

```bash
cargo build --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo tree | grep chrono | head -20
```
Expected: build passes. `cargo tree` shows chrono only as transitive of `cron`, `rrule`, and `tauri` — not direct dep of any Klyntbot crate except scheduling (via boundary shims).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/scheduling/Cargo.toml
git commit -m "build: remove chrono from workspace; scoped to scheduling boundary shims"
```

---

### Task 4.14: Refresh CLAUDE.md gotcha

**Files:**
- Modify: `CLAUDE.md:181`

- [ ] **Step 1: Read the current gotcha**

Run: `sed -n '178,185p' CLAUDE.md`  → (you can also Read the file)

- [ ] **Step 2: Replace with jiff guidance**

Replace the paragraph with:

```markdown
- **Timestamps are UTC, display in local time** — Rust stores `jiff::Timestamp::now()` which serialises as RFC 3339 (`2026-04-19T14:30:00Z`) by default via serde. For user-facing display strings formatted in Rust, convert with `ts.to_zoned(jiff::tz::TimeZone::system())` and format via `.strftime("%-I:%M %p")`. In the frontend, parse with `new Date(iso)` and use `toLocaleTimeString()`. Shared helper: `formatTime()` in `desktop-ui/src/shared/lib/dates.ts`. Never `.slice()` ISO strings.
```

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md
git commit -m "docs(CLAUDE): refresh timestamp gotcha for jiff"
```

---

## Phase 4E: Integration Tests + Tray Rewire + Final Gate

### Task 4.15: E2E alarm lifecycle test (spec §12.2)

**Files:**
- Create: `tests/e2e/alarms.rs`

- [ ] **Step 1: Scaffold test with in-memory StoragePool**

```rust
use klyntbot::{AppCore, Config};
use scheduling::temporal::TemporalScheduler;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread")]
async fn alarm_lifecycle_create_fire_snooze_cancel() {
    let core = AppCore::test_fixture().await;
    let task_id = core.task_tool().create_with_alarms(
        "dentist",
        "2026-04-22T14:00:00Z",
        vec![AlarmSpec::RelativeBefore { offset_secs: 60 }],
    ).await.unwrap();
    // Fast-forward mock clock to 60s before due
    core.mock_clock().advance(Duration::from_secs(24*60*60)).await;
    // Observe AlarmFired on bus
    let evt = core.bus_recv::<DomainEvent::AlarmFired>().await;
    assert_eq!(evt.task_id.as_deref(), Some(task_id.as_str()));
    // Snooze 10min
    core.alarm_tool().snooze(&evt.alarm_id, "10m").await.unwrap();
    core.mock_clock().advance(Duration::from_secs(10*60)).await;
    let _evt2 = core.bus_recv::<DomainEvent::AlarmFired>().await;
    // Cancel
    core.alarm_tool().cancel(&evt.alarm_id).await.unwrap();
    assert_eq!(core.bus_try_recv::<DomainEvent::AlarmFired>().await, None);
}
```

- [ ] **Step 2: Run the test**

```bash
cargo nextest run -E 'test(alarm_lifecycle)'
```
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add tests/e2e/alarms.rs
git commit -m "test(e2e): alarm lifecycle create/fire/snooze/cancel"
```

---

### Task 4.16: E2E recurrence test (spec §7.2)

**Files:**
- Create: `tests/e2e/recurrence.rs`

- [ ] **Step 1: Scaffold the test**

```rust
#[tokio::test(flavor = "multi_thread")]
async fn recurrence_materializes_three_instances_then_advances() {
    let core = AppCore::test_fixture().await;
    let tmpl_id = core.task_tool().create_template(
        "standup",
        RRuleSpec::daily_at("09:00", "America/New_York"),
    ).await.unwrap();
    // On template creation, 3 instances should materialize
    assert_eq!(core.tasks_for_template(&tmpl_id).await.len(), 3);
    // Complete one
    let first = core.tasks_for_template(&tmpl_id).await[0].id.clone();
    core.task_tool().complete(&first).await.unwrap();
    // Fire the recurrence_spawn alarm for the next occurrence
    core.mock_clock().advance(Duration::from_secs(24*60*60)).await;
    assert_eq!(core.tasks_for_template(&tmpl_id).await.len(), 4);
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -E 'test(recurrence_materializes)'
git add tests/e2e/recurrence.rs
git commit -m "test(e2e): recurrence materializes N-ahead and advances"
```

---

### Task 4.17: E2E quiet-hours boundary test (spec §6.3)

**Files:**
- Create: `tests/e2e/quiet_hours_boundary.rs`

- [ ] **Step 1: Scaffold the test**

```rust
#[tokio::test(flavor = "multi_thread")]
async fn quiet_hours_holds_and_releases_across_tz_boundary() {
    let core = AppCore::test_fixture_with_quiet_hours(
        "America/New_York", "22:00", "07:00",
    ).await;
    // Fire at 23:30 EDT → should hold
    core.mock_clock_set("2026-04-20T03:30:00Z").await; // 23:30 EDT
    core.fire_test_alarm("alarm_1", "telegram").await;
    assert_eq!(core.notification_log_count("alarm_1", "telegram").await, 0);
    // Advance past 07:00 EDT → 11:01 UTC
    core.mock_clock_set("2026-04-20T11:01:00Z").await;
    // Release alarm should fire
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(core.notification_log_count("alarm_1", "telegram").await, 1);
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -E 'test(quiet_hours_holds)'
git add tests/e2e/quiet_hours_boundary.rs
git commit -m "test(e2e): quiet-hours held+release across tz boundary"
```

---

### Task 4.18: E2E cron bridge restart test

**Files:**
- Create: `tests/e2e/cron_bridge_restart.rs`

- [ ] **Step 1: Scaffold**

```rust
#[tokio::test(flavor = "multi_thread")]
async fn cron_bridge_reconciles_after_mid_fire_restart() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Write a cron_jobs row + a half-fired scheduled_fires row
    // (simulate crash between begin_firing and mark_fired)
    seed_in_flight_fire(&pool, "cj_1").await;
    let core = AppCore::from_pool(pool).await;
    // On startup, scheduler should recover and re-dispatch
    let evt = core.bus_recv_timeout::<DomainEvent::AlarmFired>(Duration::from_secs(2)).await;
    assert_eq!(evt.kind, "cron_job");
    // And bridge reconcile should have created exactly one pending row for the enabled cron_jobs row
    assert_eq!(core.pending_fires_for_ref("cj_1").await, 1);
}
```

- [ ] **Step 2: Run + commit**

```bash
cargo nextest run -E 'test(cron_bridge_reconciles)'
git add tests/e2e/cron_bridge_restart.rs
git commit -m "test(e2e): cron bridge reconciles after mid-fire restart"
```

---

### Task 4.19: Tray countdown rewire + MCP smoke (spec §8.4, §9.5)

**Files:**
- Modify: `crates/desktop/src/tray_countdown.rs`
- Create: `tests/e2e/mcp_alarm_tool.rs`

- [ ] **Step 1: Read current poll loop**

Run: `grep -n "tokio::time::sleep\|interval" crates/desktop/src/tray_countdown.rs | head`
Record the polling cadence. Replace with bus subscription.

- [ ] **Step 2: Replace poll with bus subscription**

```rust
pub async fn start(app: AppHandle, bus: Arc<DomainEventBus>) {
    let mut rx = bus.subscribe();
    let mut cache: Option<UpcomingAlarm> = initial_load(&app).await;
    render(&app, &cache);
    while let Ok(evt) = rx.recv().await {
        match evt {
            DomainEvent::AlarmFired { .. }
            | DomainEvent::AlarmSnoozed { .. }
            | DomainEvent::AlarmCancelled { .. }
            | DomainEvent::TaskDueDateChanged { .. } => {
                cache = recompute_next(&app).await;
                render(&app, &cache);
            }
            _ => {}
        }
    }
}
```

Remove the `tokio::time::sleep(...)` poll interval entirely.

- [ ] **Step 3: MCP smoke test**

Create `tests/e2e/mcp_alarm_tool.rs`:

```rust
#[tokio::test]
async fn mcp_exposes_alarm_tool() {
    let out = std::process::Command::new(env!("CARGO_BIN_EXE_klyntbot-mcp"))
        .args(["tools", "--list"]).output().unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("alarm"), "alarm tool missing: {stdout}");
}
```

- [ ] **Step 4: Run + commit**

```bash
cargo nextest run -E 'test(mcp_exposes_alarm) or test(tray)'
git add crates/desktop/src/tray_countdown.rs tests/e2e/mcp_alarm_tool.rs
git commit -m "refactor(desktop): rewire tray countdown to bus; test MCP alarm tool exposure"
```

---

### Task 4.20: Final workspace verification gate + spec addendum

**Files:**
- Modify: `docs/superpowers/specs/2026-04-17-unified-temporal-scheduler-and-notifications-design.md` (§4.1 column-name addendum)

- [ ] **Step 1: Full build**

```bash
cargo build --workspace
```
Expected: exit 0.

- [ ] **Step 2: Full nextest**

```bash
cargo nextest run --workspace
```
Expected: all pass.

- [ ] **Step 3: Doctests**

```bash
cargo test --workspace --doc
```
Expected: all pass.

- [ ] **Step 4: Clippy with -D warnings + all features**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```
Expected: exit 0, no chrono disallowed-types violations.

- [ ] **Step 5: Format check**

```bash
cargo fmt --all --check
```
Expected: exit 0.

- [ ] **Step 6: Chrono eviction check**

```bash
grep -rn "^use chrono" crates/ | grep -v "target/" | grep -v "^[^:]*:[[:space:]]*//"
```
Expected: only `crates/scheduling/src/temporal/cron_bridge.rs` and `crates/scheduling/src/temporal/rrule.rs` (documented boundary shims). Any other match fails the gate.

- [ ] **Step 7: Cargo tree check**

```bash
cargo tree -i chrono 2>&1 | head -30
```
Expected: chrono appears only as transitive of `cron`, `rrule`, `tauri`. No direct klyntbot crate dep other than `scheduling`.

- [ ] **Step 8: Desktop UI smoke**

```bash
cd desktop-ui && bun run build && cd -
```
Expected: build succeeds. Then manually: `cargo tauri dev`, create a task with a due date, confirm it renders in the task list and the tray countdown updates on task creation (bus-driven).

- [ ] **Step 9: Update spec addendum for `_ms` column naming**

In `docs/superpowers/specs/2026-04-17-unified-temporal-scheduler-and-notifications-design.md`, under §4.1, add an addendum paragraph:

```markdown
> **Addendum (2026-04-19):** Implementation uses `_ms` column suffix (`fire_at_ms`, `firing_started_at_ms`, `fired_at_ms`, `created_at_ms`) to match the project-wide convention established by `cron_jobs.next_run_at_ms`. The spec's bare names above are aspirational; the `_ms` suffix is canonical in code.
```

- [ ] **Step 10: Commit**

```bash
git add docs/superpowers/specs/2026-04-17-unified-temporal-scheduler-and-notifications-design.md
git commit -m "docs(spec): addendum — scheduled_fires uses _ms suffix per project convention"
```

---

## Final Verification Gate

After Task 4.20 all commits landed, run once more end-to-end:

- [ ] `cargo build --workspace` → exit 0
- [ ] `cargo nextest run --workspace` → all pass
- [ ] `cargo test --workspace --doc` → all pass
- [ ] `cargo clippy --workspace --all-targets --all-features -- -D warnings` → exit 0
- [ ] `cargo fmt --all --check` → exit 0
- [ ] `grep -rn "^use chrono" crates/` → only `cron_bridge.rs` + `rrule.rs`
- [ ] `cargo tree -i chrono` → transitive only (cron / rrule / tauri)
- [ ] `ls crates/agent/src/services/reminders.rs` → No such file
- [ ] `ls crates/app-core/src/init/deadline.rs` → No such file
- [ ] `ls crates/scheduling/src/service/` → No such directory
- [ ] `test ! -f` asserted via shell OK for all three deletions
- [ ] Desktop UI: create task + due date → tray countdown updates on `AlarmFired` (no 30s poll)
- [ ] `klyntbot-mcp tools --list` → contains `alarm`

---

## Rollback Strategy

Each Task's final commit is a rollback unit. Most dangerous commits:

- **Task 4.11 (legacy deletion)** — hardest to revert if a downstream reference was missed. Verify with a full workspace build before merging.
- **Task 4.13 (chrono dep removal)** — transitive breakage possible. `cargo tree -i chrono` before/after snapshot is the discriminator.

Phase-level rollback: revert commits in reverse Task order within the affected sub-phase. Cross-sub-phase rollback requires reverting 4C before 4A (deletion depends on retirement).

---

## Self-Review Findings

**Spec coverage check** (each spec section → task):
- §2.3 TemporalScheduler completeness → Tasks 4.1 (coalesce), 4.2 (recurrence), 4.4 (retire CronService)
- §4.1 scheduled_fires schema divergence → Task 4.20 Step 9 (spec addendum)
- §5.3 misfire policies → Task 4.1
- §6.1 resolve channels → Task 4.5
- §6.2 channel routing matrix → Tasks 4.6 (os_native critical), 4.7 (telegram/discord/email)
- §6.3 held release → Task 4.17 (e2e verification; dispatcher already implements)
- §7 RRULE recurrence → Tasks 4.2, 4.3, 4.16
- §8.4 MCP alarm tool → Task 4.19
- §9 Jiff migration residuals → Tasks 4.12, 4.13, 4.14
- §9.5 Tray rewire → Task 4.19
- §11 Deletions → Tasks 4.9, 4.10, 4.11 (reminders + deadline + CronService)
- §12.2 Integration tests → Tasks 4.15, 4.16, 4.17, 4.18

**Placeholder scan:** No TBD / TODO / "add validation" / "similar to Task N" patterns. Each step contains either exact code, exact commands with expected output, or exact file paths with line numbers.

**Type consistency:** `NotificationPayload` gains `channel_mask: u32` and `priority_override: Option<String>` in Task 4.5 and is consumed with those exact types in Tasks 4.6, 4.7. `RecurrenceEngine::on_spawn` signature matches invocation in Task 4.3 Step 3. `Channel::deliver` signature unchanged across Tasks 4.6–4.7 (payload passes priority).

**Ambiguity check:** Task 4.13 Step 2 is the most load-bearing — it relies on `cron` and `rrule` still requiring chrono at their API boundary. Step 1 makes this observable via `cargo tree`. If `rrule >= 0.15` ships jiff support during execution, that's an opportunity to fully evict chrono — but this plan does not require it.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-19-phase-4-final-integration-and-cleanup.md`.

Two execution options:

**1. Subagent-Driven (recommended)** — 20 tasks, one fresh subagent per task with review between each. Best for this plan because several tasks (4.2 RecurrenceEngine, 4.7 channel adapters, 4.19 tray rewire) touch code with many call sites and benefit from focused contexts.

**2. Inline Execution** — Batch within sub-phases (4A, 4B, 4C, 4D, 4E), review at sub-phase checkpoints. Faster but less granular review.

Which approach?
