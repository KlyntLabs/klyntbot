# Phase 2 Verification — Agent 3b

**Subsystem doc:** `docs/architecture/subsystems/06-scheduling.md`  
**Crate verified:** `scheduling`  
**Date:** 2026-05-16

---

## Summary

The subsystem doc for Scheduling & Automation is **largely accurate** but contains **non-trivial API signature drift** in the recurrence trait definitions (`TemplateRepo` and `InstanceRepo`). The module tree, public API names, behavioral workflows, constants, and status claims all match the source code. No `TODO`, `FIXME`, `unimplemented!()`, or `todo!()` markers exist inside the `scheduling` crate, but the documented external tech-debt items (stale log line, hardcoded constant) are confirmed in `app-core`.

| Category | Count |
|---|---|
| ✅ Accurate | 12 |
| ⚠️ Drift | 3 |
| ❌ Wrong | 0 |
| 🔍 Missing | 0 |
| 📋 Tech Debt | 4 (all pre-documented) |

---

## Per-Crate Findings

### `scheduling` crate

#### ✅ Existence
All 12 claimed source files exist and are reachable from `src/lib.rs`:
- `src/lib.rs`
- `src/error.rs`
- `src/types.rs`
- `src/service/mod.rs`
- `src/temporal/cron_executor.rs`
- `src/temporal/scheduler.rs`
- `src/temporal/cron_bridge.rs`
- `src/temporal/fire_store.rs`
- `src/temporal/misfire.rs`
- `src/temporal/recurrence.rs`
- `src/temporal/rrule.rs`
- `src/temporal/rules.rs`

The migration `migrations/001_scheduled_fires.sql` also exists.

#### ✅ Module Structure
`src/lib.rs` declares `pub mod error`, `pub mod service`, `pub mod temporal`, `pub mod types`.  
`src/temporal/mod.rs` declares all 8 temporal submodules and re-exports the public types exactly as documented.

#### ✅ Public API Surface — Cron Primitives
All claimed structs, enums, and types are present in `types.rs` with the documented shapes:
- `CronJob`, `CronSchedule` (`At`/`Every`/`Cron`), `CronPayload`, `CronJobState`, `CronOrigin`, `IntentWindow`, `IntentTrigger`, `CatchUpPriority`
- `CronHandler` = `Arc<dyn Fn(&CronJob) -> Result<Option<String>> + Send + Sync>`
- `CronExecutor` exposes `new`, `register`, `set_callback`, `start`, `run_now` — signatures match.

#### ✅ Public API Surface — Temporal Scheduler
- `FireSpec` fields (`fire_at`, `kind`, `ref_id`, `payload`, `dedup_prefix`) match.
- `FireStore` exposes `schedule`, `begin_firing`, `mark_fired`, `mark_suppressed`, `cancel_by_prefix`, `cancel_by_kind_ref`, `pending_with_kind_before` — all present.
- `CronBridge` exposes `new`, `reconcile_all`, `advance` — all present.
- `SchedulerConfig` fields (`max_sleep`, `default_grace_secs`, `default_misfire_policy`) match defaults in `scheduler.rs:40-55`.
- `TemporalScheduler` exposes `new`, `with_cron_bridge`, `with_recurrence_engine`, `start_background`, `wake`, `shutdown` — all present. The doc omits `store()` (minor; not a claimed item).

#### ✅ Public API Surface — Misfire / Rules / RRULE
- `MisfirePolicy` (`Strict`, `SkipIfStale`, `Coalesce`) and `Decision` (`Fire`, `SkipStale`, `CoalesceLater`) with `classify` — present and behaviorally correct.
- `AlarmRule` variants (`RelativeBefore`, `CivilTimeOnDayOffset`, `Absolute`) and `compute_fire_at` — present.
- `RRuleSpec`, `Frequency` (`Daily`, `Weekly`, `Monthly`, `Yearly`), `evaluate_next_n`, `next_n_from_rrule_string` — present.

#### ⚠️ Drift — Recurrence Trait Signatures
The documented trait signatures in `06-scheduling.md` do not match the actual code in `src/temporal/recurrence.rs`.

| Item | Doc Claim | Actual Source |
|---|---|---|
| `TemplateRepo::update_next_instance` | `async fn update_next_instance(&self, id: &str, next: Timestamp) -> Result<()>` | `async fn update_next_instance(&self, id: &str, next_at: Option<Timestamp>) -> anyhow::Result<()>` |
| `TemplateRepo::decrement_count` | `async fn decrement_count(&self, id: &str) -> Result<()>` | `async fn decrement_count(&self, id: &str) -> anyhow::Result<Option<u32>>` |
| `InstanceRepo::create_instance` | `async fn create_instance(&self, template_id: &str, due_at: Timestamp) -> Result<String>` | `async fn create_instance(&self, template_id: &str, due_at: Timestamp) -> anyhow::Result<CreateInstanceOutcome>` |
| `InstanceRepo::cancel_unfired_instances` | `async fn cancel_unfired_instances(&self, template_id: &str) -> Result<u64>` | `async fn cancel_unfired_instances(&self, template_id: &str) -> anyhow::Result<()>` |

**Impact:** The doc describes an older, simplified contract. The `Option<Timestamp>` on `update_next_instance` is load-bearing (used to clear `next_instance_at` when recurrence ends). The `CreateInstanceOutcome` enum (with `SourceTaskMissing`) is load-bearing for the disable-and-cleanup workflow described in the doc. The return-type drifts are not merely naming differences.

#### ✅ Behavioral Claims — Cron Fire Workflow
The step-by-step workflow "A cron fires (TemporalScheduler path)" matches `TemporalScheduler::run()` and `process_due()` in `scheduler.rs:132-285`:
1. Sleep capped at `MAX_SLEEP` (30s) — `scheduler.rs:143-148`.
2. Wake on timer, `wake.notify_one()`, or shutdown — `scheduler.rs:151-158`.
3. `list_due(now)` selects pending rows — `scheduler.rs:215`.
4. `Decision::classify(policy, grace, fire_at, now)` — `scheduler.rs:237`.
5. `begin_firing` → publish `AlarmFired` → `mark_fired` → `cron_bridge.advance` for `kind="cron_job"` — `scheduler.rs:305-325`.
6. Crash recovery via `recover_in_flight()` re-publishes and marks fired — `scheduler.rs:164-212`.

#### ✅ Behavioral Claims — Two-Phase Commit
Doc: `begin_firing` does an atomic `UPDATE ... WHERE id=? AND firing_started_at_ms IS NULL`.  
Source (`storage/src/repos/scheduled_fires.rs:68-78`): exact query shape; returns `rows_affected() == 1`. Confirmed.

#### ✅ Behavioral Claims — Recurring Task Workflow
Doc steps 1-8 match `RecurrenceEngine::on_spawn()` in `recurrence.rs:105-225`:
- Fetches template, checks `enabled` and `count_remaining`.
- Calls `next_n_from_rrule_string` with `ahead+1`.
- Creates instances via `instance_repo.create_instance`.
- Decrements count if finite.
- Sets `next_instance_at` to the candidate after the last materialized one.
- Schedules new `recurrence_spawn` row.
- On `SourceTaskMissing`, disables template, cancels unfired instances, cancels pending fires by prefix.

#### ✅ Behavioral Claims — Held Notification Release
The workflow is an integration claim with the `notifications` crate. The `scheduling` crate provides the `FireStore` API used by `HeldReleaseService` to insert `scheduled_fires(kind="held_release")`. The doc description is consistent with the `FireStore::schedule` API surface; no contradiction found.

#### ✅ Constants & Magic Numbers
| Constant | Doc Claim | Source Location | Verified |
|---|---|---|---|
| `MAX_SLEEP` | 30s | `scheduler.rs:33` | ✅ |
| `DEFAULT_GRACE_SECS` | 3600 | `scheduler.rs:34` | ✅ |
| `DEFAULT_MATERIALIZE_AHEAD` | 3 | `app-core/src/init/temporal_scheduler.rs:21` | ✅ |
| `RECURRENCE_SPAWN_KIND` | `"recurrence_spawn"` | `scheduler.rs:38` | ✅ |

#### ✅ Corrupt-Schedule Defense
`row_to_job()` in `service/mod.rs:18-71` forces `enabled=false` when schedule JSON fails to deserialize, exactly as claimed.

#### ✅ Chrono Boundary Files
Both `cron_bridge.rs` and `rrule.rs` carry `#![allow(clippy::disallowed_types, clippy::disallowed_methods)]` and convert via epoch-ms. Confirmed.

#### ✅ The Two `AlarmFired` Kinds
- `kind="cron_job"` — emitted by `TemporalScheduler::dispatch` → consumed by `CronExecutor`.
- `kind="cron"` — emitted by `app-core/src/init/cron.rs::publish_cron_alarm` (line 27) → consumed by notification dispatcher.
Both strings exist in the codebase; the doc's warning about confusion is accurate.

#### 📋 Tech Debt Catalog (pre-documented, confirmed)
1. **Stale "CronService" log message** — `app-core/src/init/temporal_scheduler.rs:99` still logs `"TemporalScheduler started (side-by-side with CronService)"`. `CronService` was removed; actual pair is `TemporalScheduler` + `CronExecutor`.
2. **`DEFAULT_MATERIALIZE_AHEAD = 3` hardcoded** — confirmed at `app-core/src/init/temporal_scheduler.rs:19-21`. Comment explicitly states promotion to config is deferred.
3. **Dual chrono/jiff dependency** — `Cargo.toml` lists `chrono`, `chrono-tz`, and `jiff`. The two CHRONO BOUNDARY files justify this.
4. **Phase 3 migration unfinished** — `CronExecutor` still exists; callbacks are registered in `app-core/src/init/cron.rs`. End-state (independent bus subscribers) is not yet achieved.

No `TODO`, `FIXME`, `unimplemented!()`, or `todo!()` markers exist inside the `scheduling` crate (`grep` returned zero matches).

---

## Cross-Reference Check

| Link in Doc | Target Path | Resolves? |
|---|---|---|
| `../00-overview.md` | `docs/architecture/00-overview.md` | ✅ Exists |
| `./01-foundations.md` | `docs/architecture/subsystems/01-foundations.md` | ✅ Exists |
| `./02-storage.md` | `docs/architecture/subsystems/02-storage.md` | ✅ Exists |
| `./05-cognitive-memory.md` | `docs/architecture/subsystems/05-cognitive-memory.md` | ✅ Exists |
| `./08-assistant-features.md` | `docs/architecture/subsystems/08-assistant-features.md` | ✅ Exists |
| `./11-channels-mcp.md` | `docs/architecture/subsystems/11-channels-mcp.md` | ✅ Exists |
| `../TECH_DEBT.md` | `docs/architecture/TECH_DEBT.md` | ✅ Exists |

All cross-referenced subsystem docs and the parent overview exist on disk. The relative paths from `subsystems/06-scheduling.md` resolve correctly.

---

## Recommendations

1. **Update `TemplateRepo` and `InstanceRepo` signatures** in `06-scheduling.md` to match the actual `Option`-bearing and `CreateInstanceOutcome`-returning shapes.
2. **Add `CreateInstanceOutcome`** to the doc's public API reference; it is a first-class type with semantics (`SourceTaskMissing`).
3. **Fix stale log message** in `app-core/src/init/temporal_scheduler.rs:99` — change `"with CronService"` → `"with CronExecutor"`.
4. **Promote `DEFAULT_MATERIALIZE_AHEAD`** to a config field as already noted in the source comment.
