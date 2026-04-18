# Phase 2 — Unified Temporal Scheduler Core Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `TemporalScheduler` — a single SQLite-backed, wall-clock-anchored scheduler that replaces `CronService` and supersedes `DeadlineScheduler`, with RFC 5545 RRULE evaluation and VALARM-style alarm rules.

**Architecture:** One canonical `scheduled_fires` table is the sole source of truth for *when* to fire. A single tokio loop sleeps ≤30s at a time (macOS sleep-safe) and re-checks `Jiff::Timestamp::now()` on every wake. Firing is two-phase committed for crash safety. `AlarmRule` (three variants: RelativeBefore / CivilTimeOnDayOffset / Absolute) computes fire_at using Jiff `Zoned` arithmetic. RRULE recurrence is evaluated via the `rrule` crate at a chrono↔jiff boundary fully hidden inside `temporal/rrule.rs`. Fires emit `DomainEvent::AlarmFired` on the bus — no synchronous handlers, no `block_in_place`.

**Tech stack:** Rust 1.93, tokio, `sqlx` with SQLite, `jiff` (all new datetimes), `jiff::Zoned` + `jiff::Span` for calendar math, `rrule = "0.14"` (chrono-based, isolated), existing `cron` crate retained only for parsing `cron_jobs.schedule` strings. Tests via `cargo nextest`, `StoragePool::connect_in_memory()`. No backward-compat shims: old `CronService` is deleted at the end of this phase.

**Non-goals in Phase 2:** Dispatcher, quiet hours, channel fan-out, idempotency gate, `ReminderEngine` removal, task alarm tool surface, RRULE export. Those are Phase 3/4.

---

## File Structure

**New (scheduling crate):**
- `crates/scheduling/migrations/001_scheduled_fires.sql` — `scheduled_fires` table + indexes.
- `crates/scheduling/src/temporal/mod.rs` — `TemporalScheduler` facade + the loop.
- `crates/scheduling/src/temporal/rules.rs` — `AlarmRule` enum + `compute_fire_at()`.
- `crates/scheduling/src/temporal/fire_store.rs` — `FireStore` service over `ScheduledFiresRepo` (prefix cancel, next-pending, two-phase commit).
- `crates/scheduling/src/temporal/misfire.rs` — `MisfirePolicy` + classification.
- `crates/scheduling/src/temporal/rrule.rs` — RRULE DSL + evaluator (chrono isolated here).
- `crates/scheduling/src/temporal/cron_bridge.rs` — reconcile `cron_jobs` ↔ `scheduled_fires`.
- `crates/scheduling/src/temporal/tests.rs` — unit tests (DST, misfire, two-phase).

**New (storage crate):**
- `crates/storage/src/rows/scheduled_fire.rs` — `ScheduledFireRow`.
- `crates/storage/src/repos/scheduled_fires.rs` — `ScheduledFiresRepo`.

**New (feature-tasks crate):**
- `crates/feature-tasks/src/alarms/mod.rs` — `task_alarms` + `task_recurrence_templates` row/repo re-exports.

**Modified:**
- `crates/scheduling/src/lib.rs` — add `pub mod temporal;`, export `TemporalScheduler`.
- `crates/scheduling/Cargo.toml` — add `rrule = "0.14"`, `bus.workspace = true`, `chrono-tz` already present; update feature-migration registration.
- `crates/scheduling/src/error.rs` — add `SchedulerError` variants for misfire/fire-store.
- `crates/storage/src/rows/mod.rs` — expose `scheduled_fire`.
- `crates/storage/src/repos/mod.rs` — expose `scheduled_fires`.
- `crates/storage/src/repos/mod.rs` `Repos::from_pool` — add `scheduled_fires` repo.
- `crates/bus/src/domain_events.rs` — add `AlarmFired`, `AlarmSnoozed`, `AlarmCancelled`, `MissedAlarms`.
- `crates/feature-tasks/migrations/001_create_tasks.sql` — add `task_alarms` + `task_recurrence_templates` tables (pre-release; edit in place per CLAUDE.md).
- `crates/app-core/src/init/` — replace `CronService::start(...)` wiring with `TemporalScheduler::start(...)`.

**Deleted (end of phase):**
- `crates/scheduling/src/service/` — the whole module (`mod.rs`, `executor.rs`, `intent.rs`, `store.rs`).
- Re-exports of `CronService` / `JobCallback` / `MissedJobClass` / `PresenceSnapshot` from `scheduling/src/lib.rs`.

---

## Task 1: Scheduling crate — migration + row type for `scheduled_fires`

**Files:**
- Create: `crates/scheduling/migrations/001_scheduled_fires.sql`
- Create: `crates/storage/src/rows/scheduled_fire.rs`
- Modify: `crates/storage/src/rows/mod.rs`

- [ ] **Step 1: Write the failing test for row deserialization**

Add to `crates/storage/src/rows/serialization_tests.rs`:

```rust
#[test]
fn scheduled_fire_row_round_trips_payload() {
    use crate::rows::scheduled_fire::ScheduledFireRow;
    let row = ScheduledFireRow {
        id: "fire_abc".into(),
        fire_at_ms: 1_800_000_000_000,
        kind: "cron_job".into(),
        ref_id: Some("job_xyz".into()),
        payload: serde_json::json!({ "message": "hi", "channels": ["tray"] }),
        dedup_prefix: Some("cron:job_xyz:".into()),
        fired: false,
        firing_started_at_ms: None,
        fired_at_ms: None,
        created_at_ms: 1_800_000_000_000 - 1000,
    };
    let json = serde_json::to_string(&row).unwrap();
    let parsed: ScheduledFireRow = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.id, row.id);
    assert_eq!(parsed.payload["message"], "hi");
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage scheduled_fire_row_round_trips_payload`
Expected: FAIL — `crate::rows::scheduled_fire` does not exist.

- [ ] **Step 3: Create the row type**

`crates/storage/src/rows/scheduled_fire.rs`:

```rust
//! Row struct for the `scheduled_fires` table — the canonical "when to fire" store.
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledFireRow {
    pub id: String,
    pub fire_at_ms: i64,
    pub kind: String,
    pub ref_id: Option<String>,
    pub payload: serde_json::Value,
    pub dedup_prefix: Option<String>,
    pub fired: bool,
    pub firing_started_at_ms: Option<i64>,
    pub fired_at_ms: Option<i64>,
    pub created_at_ms: i64,
}
```

Modify `crates/storage/src/rows/mod.rs`:

```rust
pub mod scheduled_fire;
```

- [ ] **Step 4: Create the migration SQL**

`crates/scheduling/migrations/001_scheduled_fires.sql`:

```sql
-- Canonical "when to fire" table. Every scheduled fire lives here.
CREATE TABLE scheduled_fires (
    id TEXT PRIMARY KEY,
    fire_at_ms INTEGER NOT NULL,
    kind TEXT NOT NULL,
    ref_id TEXT,
    payload TEXT NOT NULL DEFAULT '{}',
    dedup_prefix TEXT,
    fired INTEGER NOT NULL DEFAULT 0,
    firing_started_at_ms INTEGER,
    fired_at_ms INTEGER,
    created_at_ms INTEGER NOT NULL
);

CREATE INDEX idx_scheduled_fires_pending
    ON scheduled_fires(fire_at_ms) WHERE fired = 0;

CREATE INDEX idx_scheduled_fires_dedup
    ON scheduled_fires(dedup_prefix) WHERE fired = 0;

CREATE INDEX idx_scheduled_fires_kind_ref
    ON scheduled_fires(kind, ref_id) WHERE fired = 0;
```

- [ ] **Step 5: Run test to verify it passes**

Run: `cargo nextest run -p storage scheduled_fire_row_round_trips_payload`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/src/rows/scheduled_fire.rs crates/storage/src/rows/mod.rs \
        crates/scheduling/migrations/001_scheduled_fires.sql \
        crates/storage/src/rows/serialization_tests.rs
git commit -m "feat(scheduling): add scheduled_fires table + row type"
```

---

## Task 2: `ScheduledFiresRepo` — CRUD + pending query + prefix cancel

**Files:**
- Create: `crates/storage/src/repos/scheduled_fires.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Write failing integration tests**

`crates/storage/src/repos/tests/scheduled_fires_tests.rs` (create file, register under `tests/mod.rs`):

```rust
use crate::pool::StoragePool;
use crate::repos::scheduled_fires::ScheduledFiresRepo;
use crate::rows::scheduled_fire::ScheduledFireRow;
use tools_core::FeatureMigration;

fn sf(id: &str, at: i64, prefix: Option<&str>) -> ScheduledFireRow {
    ScheduledFireRow {
        id: id.into(),
        fire_at_ms: at,
        kind: "task_alarm".into(),
        ref_id: Some("task_1".into()),
        payload: serde_json::json!({}),
        dedup_prefix: prefix.map(String::from),
        fired: false,
        firing_started_at_ms: None,
        fired_at_ms: None,
        created_at_ms: 0,
    }
}

async fn setup() -> ScheduledFiresRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(
        pool.inner(),
        &[FeatureMigration {
            feature_name: "scheduling".into(),
            version: 1,
            description: "scheduled_fires".into(),
            sql: include_str!("../../../../scheduling/migrations/001_scheduled_fires.sql").into(),
        }],
    )
    .await
    .unwrap();
    ScheduledFiresRepo::new(pool.inner().clone())
}

#[tokio::test]
async fn insert_then_next_pending_returns_earliest() {
    let repo = setup().await;
    repo.insert(&sf("a", 2000, None)).await.unwrap();
    repo.insert(&sf("b", 1000, None)).await.unwrap();
    let next = repo.next_pending_fire_at_ms().await.unwrap();
    assert_eq!(next, Some(1000));
}

#[tokio::test]
async fn cancel_by_prefix_deletes_only_matching_pending() {
    let repo = setup().await;
    repo.insert(&sf("a", 1000, Some("task:1:"))).await.unwrap();
    repo.insert(&sf("b", 2000, Some("task:2:"))).await.unwrap();
    let deleted = repo.cancel_by_prefix("task:1:").await.unwrap();
    assert_eq!(deleted, 1);
    assert_eq!(repo.list_pending_up_to_ms(9999).await.unwrap().len(), 1);
}

#[tokio::test]
async fn two_phase_mark_firing_then_fired_is_idempotent() {
    let repo = setup().await;
    repo.insert(&sf("a", 1000, None)).await.unwrap();
    let claimed = repo.begin_firing("a", 1500).await.unwrap();
    assert!(claimed);
    let claimed_again = repo.begin_firing("a", 1600).await.unwrap();
    assert!(!claimed_again, "begin_firing must be idempotent");
    repo.mark_fired("a", 1700).await.unwrap();
    assert_eq!(repo.list_pending_up_to_ms(9999).await.unwrap().len(), 0);
}

#[tokio::test]
async fn list_in_flight_returns_rows_with_firing_started_but_not_fired() {
    let repo = setup().await;
    repo.insert(&sf("a", 1000, None)).await.unwrap();
    repo.begin_firing("a", 1500).await.unwrap();
    let in_flight = repo.list_in_flight().await.unwrap();
    assert_eq!(in_flight.len(), 1);
    assert_eq!(in_flight[0].id, "a");
}
```

Also add to `crates/storage/src/repos/tests/mod.rs`: `mod scheduled_fires_tests;`.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p storage scheduled_fires_tests`
Expected: FAIL — `ScheduledFiresRepo` does not exist.

- [ ] **Step 3: Implement the repo**

`crates/storage/src/repos/scheduled_fires.rs`:

```rust
//! Repository for `scheduled_fires` — the canonical "when to fire" table.
use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::scheduled_fire::ScheduledFireRow;

#[derive(Debug, Clone)]
pub struct ScheduledFiresRepo {
    pool: SqlitePool,
}

impl ScheduledFiresRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn insert(&self, row: &ScheduledFireRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO scheduled_fires
                 (id, fire_at_ms, kind, ref_id, payload, dedup_prefix,
                  fired, firing_started_at_ms, fired_at_ms, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, NULL, ?7)",
        )
        .bind(&row.id)
        .bind(row.fire_at_ms)
        .bind(&row.kind)
        .bind(&row.ref_id)
        .bind(row.payload.to_string())
        .bind(&row.dedup_prefix)
        .bind(row.created_at_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Returns fire_at_ms of the earliest pending fire, or None.
    pub async fn next_pending_fire_at_ms(&self) -> Result<Option<i64>, StorageError> {
        let result: Option<i64> = sqlx::query_scalar(
            "SELECT fire_at_ms FROM scheduled_fires WHERE fired = 0 ORDER BY fire_at_ms ASC LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(result)
    }

    /// List all pending rows with fire_at_ms <= cutoff, oldest first.
    pub async fn list_pending_up_to_ms(&self, cutoff_ms: i64) -> Result<Vec<ScheduledFireRow>, StorageError> {
        let rows = sqlx::query_as::<_, ScheduledFireRow>(
            "SELECT * FROM scheduled_fires
             WHERE fired = 0 AND fire_at_ms <= ?1
             ORDER BY fire_at_ms ASC",
        )
        .bind(cutoff_ms)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Two-phase commit phase 1: claim a row for firing. Returns true if newly claimed,
    /// false if another worker already claimed it or it's already fired.
    pub async fn begin_firing(&self, id: &str, now_ms: i64) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE scheduled_fires SET firing_started_at_ms = ?2
             WHERE id = ?1 AND fired = 0 AND firing_started_at_ms IS NULL",
        )
        .bind(id)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    /// Two-phase commit phase 2: mark as fired.
    pub async fn mark_fired(&self, id: &str, now_ms: i64) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE scheduled_fires SET fired = 1, fired_at_ms = ?2 WHERE id = ?1",
        )
        .bind(id)
        .bind(now_ms)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Rows claimed but never marked fired — happens when the process crashed mid-dispatch.
    pub async fn list_in_flight(&self) -> Result<Vec<ScheduledFireRow>, StorageError> {
        let rows = sqlx::query_as::<_, ScheduledFireRow>(
            "SELECT * FROM scheduled_fires
             WHERE fired = 0 AND firing_started_at_ms IS NOT NULL",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Delete all pending fires whose dedup_prefix matches the given literal prefix.
    /// Returns count deleted.
    pub async fn cancel_by_prefix(&self, prefix: &str) -> Result<u64, StorageError> {
        let like = format!("{prefix}%");
        let result = sqlx::query(
            "DELETE FROM scheduled_fires WHERE fired = 0 AND dedup_prefix LIKE ?1",
        )
        .bind(like)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    /// Delete all pending fires with the given (kind, ref_id). Used for cron re-sync.
    pub async fn cancel_by_kind_ref(&self, kind: &str, ref_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "DELETE FROM scheduled_fires WHERE fired = 0 AND kind = ?1 AND ref_id = ?2",
        )
        .bind(kind)
        .bind(ref_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }
}
```

Modify `crates/storage/src/repos/mod.rs` to add `pub mod scheduled_fires;` and add `pub scheduled_fires: ScheduledFiresRepo` to the `Repos` struct alongside the existing pattern (match the neighboring `cron` field exactly).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p storage scheduled_fires_tests`
Expected: PASS — 4 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/storage/src/repos/scheduled_fires.rs \
        crates/storage/src/repos/mod.rs \
        crates/storage/src/repos/tests/
git commit -m "feat(storage): add ScheduledFiresRepo with two-phase commit"
```

---

## Task 3: `AlarmRule` + fire-time computation (DST-correct)

**Files:**
- Create: `crates/scheduling/src/temporal/mod.rs` (with `pub mod rules;`)
- Create: `crates/scheduling/src/temporal/rules.rs`
- Modify: `crates/scheduling/src/lib.rs`

- [ ] **Step 1: Write the failing tests**

`crates/scheduling/src/temporal/rules.rs` (include at bottom with `#[cfg(test)] mod tests`):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::{date, time};
    use jiff::tz::TimeZone;
    use jiff::Timestamp;

    fn ts(y: i16, m: i8, d: i8, hh: i8, mm: i8, tz: &str) -> Timestamp {
        date(y, m, d).at(hh, mm, 0, 0).to_zoned(TimeZone::get(tz).unwrap()).unwrap().timestamp()
    }

    #[test]
    fn relative_before_subtracts_offset() {
        let due = ts(2026, 4, 22, 14, 0, "America/New_York");
        let rule = AlarmRule::RelativeBefore { offset: jiff::Span::new().hours(1) };
        let fire = rule.compute_fire_at(Some(due), "UTC").unwrap();
        assert_eq!(fire.as_millisecond(), due.as_millisecond() - 3_600_000);
    }

    #[test]
    fn civil_time_day_minus_one_9am_is_dst_correct_in_ny() {
        // Deadline = 2026-03-09 14:00 NY (day after spring-forward 2026-03-08).
        // day_offset = -1 means 2026-03-08 09:00 NY — but NY spring-forward skips 2am-3am.
        // 9am NY on 2026-03-08 is after the skip, unambiguous.
        let due = ts(2026, 3, 9, 14, 0, "America/New_York");
        let rule = AlarmRule::CivilTimeOnDayOffset {
            day_offset: -1,
            time_of_day: time(9, 0, 0, 0),
            iana_tz: "America/New_York".into(),
        };
        let fire = rule.compute_fire_at(Some(due), "UTC").unwrap();
        // Expected: 2026-03-08 09:00 EDT = 2026-03-08 13:00 UTC (after DST -> EDT)
        let expected = ts(2026, 3, 8, 9, 0, "America/New_York");
        assert_eq!(fire.as_millisecond(), expected.as_millisecond());
    }

    #[test]
    fn civil_time_skipped_hour_resolves_to_post_transition() {
        // 2026-03-08 02:30 NY does not exist (spring-forward).
        // Jiff resolves forward by default — we want the post-transition instant.
        let due = ts(2026, 3, 8, 14, 0, "America/New_York");
        let rule = AlarmRule::CivilTimeOnDayOffset {
            day_offset: 0,
            time_of_day: time(2, 30, 0, 0),
            iana_tz: "America/New_York".into(),
        };
        let fire = rule.compute_fire_at(Some(due), "UTC").unwrap();
        // Post-fold: 03:30 EDT on 2026-03-08 = 07:30 UTC.
        let expected = ts(2026, 3, 8, 3, 30, "America/New_York");
        assert_eq!(fire.as_millisecond(), expected.as_millisecond());
    }

    #[test]
    fn absolute_returns_input_unchanged() {
        let t = Timestamp::from_millisecond(1_800_000_000_000).unwrap();
        let rule = AlarmRule::Absolute { fire_at: t };
        let fire = rule.compute_fire_at(None, "UTC").unwrap();
        assert_eq!(fire, t);
    }

    #[test]
    fn relative_before_without_due_errors() {
        let rule = AlarmRule::RelativeBefore { offset: jiff::Span::new().hours(1) };
        assert!(rule.compute_fire_at(None, "UTC").is_err());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p scheduling rules::tests`
Expected: FAIL — module `rules` does not exist.

- [ ] **Step 3: Implement `AlarmRule`**

`crates/scheduling/src/temporal/mod.rs`:

```rust
//! Unified temporal scheduler: persistent, wall-clock-anchored, VALARM-style rules.
pub mod rules;
```

`crates/scheduling/src/temporal/rules.rs`:

```rust
//! Alarm rule variants and fire-time computation.
//!
//! Three orthogonal variants cover every user utterance:
//! - `RelativeBefore` — "24h before deadline", "5min before"
//! - `CivilTimeOnDayOffset` — "9am the day before", "8am on the deadline day"
//! - `Absolute` — "at 2026-04-20T09:00:00-04:00"
//!
//! All computation is DST-correct via `jiff::Zoned` arithmetic.

use jiff::civil::Time as CivilTime;
use jiff::tz::TimeZone;
use jiff::{Span, Timestamp};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AlarmRule {
    RelativeBefore { offset: Span },
    CivilTimeOnDayOffset {
        day_offset: i32,
        time_of_day: CivilTime,
        iana_tz: String,
    },
    Absolute { fire_at: Timestamp },
}

#[derive(Debug, Error)]
pub enum RuleError {
    #[error("relative_before rule requires a due_date")]
    MissingDueDate,
    #[error("civil_time rule requires a due_date to compute day offset")]
    MissingDueDateForCivil,
    #[error("unknown timezone: {0}")]
    UnknownTimezone(String),
    #[error("jiff arithmetic failed: {0}")]
    Jiff(#[from] jiff::Error),
}

impl AlarmRule {
    /// Compute the UTC Timestamp at which this rule should fire.
    ///
    /// `due_date` is the task's deadline as a UTC instant (required for relative/civil variants).
    /// `default_tz` is used only as a context aid for future variants; current variants store their own tz.
    pub fn compute_fire_at(
        &self,
        due_date: Option<Timestamp>,
        _default_tz: &str,
    ) -> Result<Timestamp, RuleError> {
        match self {
            Self::Absolute { fire_at } => Ok(*fire_at),
            Self::RelativeBefore { offset } => {
                let due = due_date.ok_or(RuleError::MissingDueDate)?;
                Ok(due.checked_sub(*offset)?)
            }
            Self::CivilTimeOnDayOffset { day_offset, time_of_day, iana_tz } => {
                let tz = TimeZone::get(iana_tz)
                    .map_err(|_| RuleError::UnknownTimezone(iana_tz.clone()))?;
                let due = due_date.ok_or(RuleError::MissingDueDateForCivil)?;
                let due_zoned = due.to_zoned(tz.clone());
                let civil_date = due_zoned
                    .date()
                    .checked_add(Span::new().days(*day_offset as i64))?;
                let civil_dt = civil_date.at(
                    time_of_day.hour(),
                    time_of_day.minute(),
                    time_of_day.second(),
                    0,
                );
                let zoned = civil_dt.to_zoned(tz)?;
                Ok(zoned.timestamp())
            }
        }
    }
}
```

Modify `crates/scheduling/src/lib.rs`:

```rust
pub mod temporal;
pub use temporal::rules::{AlarmRule, RuleError};
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p scheduling rules::tests`
Expected: PASS — 5 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/src/temporal/ crates/scheduling/src/lib.rs
git commit -m "feat(scheduling): AlarmRule with DST-correct fire-time computation"
```

---

## Task 4: `FireStore` service layer over `ScheduledFiresRepo`

**Files:**
- Create: `crates/scheduling/src/temporal/fire_store.rs`
- Modify: `crates/scheduling/src/temporal/mod.rs`

- [ ] **Step 1: Write failing test**

In `crates/scheduling/src/temporal/fire_store.rs` at bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;
    use storage::pool::StoragePool;

    async fn setup() -> FireStore {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(),
                version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        )
        .await
        .unwrap();
        FireStore::new(storage::repos::scheduled_fires::ScheduledFiresRepo::new(
            pool.inner().clone(),
        ))
    }

    #[tokio::test]
    async fn schedule_inserts_a_pending_row() {
        let store = setup().await;
        let t = Timestamp::from_millisecond(1_800_000_000_000).unwrap();
        let id = store
            .schedule(FireSpec {
                fire_at: t,
                kind: "task_alarm".into(),
                ref_id: Some("task_1".into()),
                payload: serde_json::json!({ "msg": "hi" }),
                dedup_prefix: Some("task:1:".into()),
            })
            .await
            .unwrap();
        let next = store.next_pending_fire_at().await.unwrap();
        assert_eq!(next, Some(t));
        assert!(!id.is_empty());
    }

    #[tokio::test]
    async fn cancel_by_prefix_removes_only_matching() {
        let store = setup().await;
        store.schedule(FireSpec {
            fire_at: Timestamp::from_millisecond(1000).unwrap(),
            kind: "task_alarm".into(),
            ref_id: None, payload: serde_json::json!({}),
            dedup_prefix: Some("task:1:".into()),
        }).await.unwrap();
        store.schedule(FireSpec {
            fire_at: Timestamp::from_millisecond(2000).unwrap(),
            kind: "task_alarm".into(),
            ref_id: None, payload: serde_json::json!({}),
            dedup_prefix: Some("task:2:".into()),
        }).await.unwrap();
        assert_eq!(store.cancel_by_prefix("task:1:").await.unwrap(), 1);
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p scheduling fire_store::tests`
Expected: FAIL — `FireStore` unknown.

- [ ] **Step 3: Implement `FireStore`**

`crates/scheduling/src/temporal/fire_store.rs`:

```rust
//! High-level service over `ScheduledFiresRepo`. Owns UUID generation, timestamp conversion,
//! and the two-phase commit protocol. All times on the wire are `jiff::Timestamp`; DB stores i64 ms.

use jiff::Timestamp;
use serde_json::Value;
use storage::repos::scheduled_fires::ScheduledFiresRepo;
use storage::rows::scheduled_fire::ScheduledFireRow;
use uuid::Uuid;

use crate::error::SchedulerError;

#[derive(Debug, Clone)]
pub struct FireSpec {
    pub fire_at: Timestamp,
    pub kind: String,
    pub ref_id: Option<String>,
    pub payload: Value,
    pub dedup_prefix: Option<String>,
}

#[derive(Debug, Clone)]
pub struct FireStore {
    repo: ScheduledFiresRepo,
}

impl FireStore {
    pub fn new(repo: ScheduledFiresRepo) -> Self { Self { repo } }

    pub async fn schedule(&self, spec: FireSpec) -> Result<String, SchedulerError> {
        let id = format!("fire_{}", Uuid::new_v4().simple());
        let now_ms = Timestamp::now().as_millisecond();
        let row = ScheduledFireRow {
            id: id.clone(),
            fire_at_ms: spec.fire_at.as_millisecond(),
            kind: spec.kind,
            ref_id: spec.ref_id,
            payload: spec.payload,
            dedup_prefix: spec.dedup_prefix,
            fired: false,
            firing_started_at_ms: None,
            fired_at_ms: None,
            created_at_ms: now_ms,
        };
        self.repo.insert(&row).await?;
        Ok(id)
    }

    pub async fn next_pending_fire_at(&self) -> Result<Option<Timestamp>, SchedulerError> {
        let ms = self.repo.next_pending_fire_at_ms().await?;
        Ok(ms.and_then(|m| Timestamp::from_millisecond(m).ok()))
    }

    pub async fn list_due(&self, now: Timestamp) -> Result<Vec<ScheduledFireRow>, SchedulerError> {
        Ok(self.repo.list_pending_up_to_ms(now.as_millisecond()).await?)
    }

    pub async fn begin_firing(&self, id: &str, now: Timestamp) -> Result<bool, SchedulerError> {
        Ok(self.repo.begin_firing(id, now.as_millisecond()).await?)
    }

    pub async fn mark_fired(&self, id: &str, now: Timestamp) -> Result<(), SchedulerError> {
        self.repo.mark_fired(id, now.as_millisecond()).await?;
        Ok(())
    }

    pub async fn recover_in_flight(&self) -> Result<Vec<ScheduledFireRow>, SchedulerError> {
        Ok(self.repo.list_in_flight().await?)
    }

    pub async fn cancel_by_prefix(&self, prefix: &str) -> Result<u64, SchedulerError> {
        Ok(self.repo.cancel_by_prefix(prefix).await?)
    }

    pub async fn cancel_by_kind_ref(&self, kind: &str, ref_id: &str) -> Result<u64, SchedulerError> {
        Ok(self.repo.cancel_by_kind_ref(kind, ref_id).await?)
    }
}
```

Update `crates/scheduling/src/error.rs` to add a `SchedulerError` that wraps `StorageError`:

```rust
use storage::error::StorageError;

#[derive(Debug, thiserror::Error)]
pub enum SchedulerError {
    #[error("storage error: {0}")]
    Storage(#[from] StorageError),
    #[error("rule error: {0}")]
    Rule(#[from] crate::temporal::rules::RuleError),
    #[error("rrule error: {0}")]
    Rrule(String),
    #[error("invalid state: {0}")]
    InvalidState(String),
}
```

Add to `crates/scheduling/src/temporal/mod.rs`:

```rust
pub mod fire_store;
pub use fire_store::{FireSpec, FireStore};
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p scheduling fire_store::tests`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/src/temporal/fire_store.rs \
        crates/scheduling/src/temporal/mod.rs \
        crates/scheduling/src/error.rs
git commit -m "feat(scheduling): FireStore service with two-phase commit API"
```

---

## Task 5: Misfire policies

**Files:**
- Create: `crates/scheduling/src/temporal/misfire.rs`
- Modify: `crates/scheduling/src/temporal/mod.rs`

- [ ] **Step 1: Write failing tests**

`crates/scheduling/src/temporal/misfire.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jiff::Timestamp;

    fn t(ms: i64) -> Timestamp { Timestamp::from_millisecond(ms).unwrap() }

    #[test]
    fn strict_fires_no_matter_how_stale() {
        let d = Decision::classify(
            MisfirePolicy::Strict, Span60min(), t(0), t(100_000_000),
        );
        assert_eq!(d, Decision::Fire);
    }

    #[test]
    fn skip_if_stale_fires_within_grace() {
        let d = Decision::classify(
            MisfirePolicy::SkipIfStale, Span60min(), t(0), t(30 * 60 * 1000),
        );
        assert_eq!(d, Decision::Fire);
    }

    #[test]
    fn skip_if_stale_skips_past_grace() {
        let d = Decision::classify(
            MisfirePolicy::SkipIfStale, Span60min(), t(0), t(61 * 60 * 1000),
        );
        assert_eq!(d, Decision::SkipStale);
    }

    fn Span60min() -> std::time::Duration { std::time::Duration::from_secs(3600) }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p scheduling misfire::tests`
Expected: FAIL — module not found.

- [ ] **Step 3: Implement**

```rust
//! Misfire policy evaluation. A fire is "misfired" when fire_at <= now by more than epsilon;
//! policy determines whether to fire, skip, or coalesce.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MisfirePolicy {
    Strict,
    SkipIfStale,
    Coalesce,
}

impl Default for MisfirePolicy {
    fn default() -> Self { Self::SkipIfStale }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Decision {
    Fire,
    SkipStale,
    CoalesceLater,
}

impl Decision {
    pub fn classify(
        policy: MisfirePolicy,
        grace: std::time::Duration,
        fire_at: Timestamp,
        now: Timestamp,
    ) -> Self {
        let age_ms = (now.as_millisecond() - fire_at.as_millisecond()).max(0);
        match policy {
            MisfirePolicy::Strict => Self::Fire,
            MisfirePolicy::SkipIfStale => {
                if (age_ms as u128) <= grace.as_millis() { Self::Fire } else { Self::SkipStale }
            }
            MisfirePolicy::Coalesce => Self::CoalesceLater,
        }
    }
}
```

Export from `temporal/mod.rs`:

```rust
pub mod misfire;
pub use misfire::{Decision, MisfirePolicy};
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p scheduling misfire::tests`
Expected: PASS — 3 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/src/temporal/misfire.rs crates/scheduling/src/temporal/mod.rs
git commit -m "feat(scheduling): misfire policies (strict/skip_if_stale/coalesce)"
```

---

## Task 6: Add scheduler DomainEvents

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Write failing test**

Add to `crates/bus/src/domain_events.rs` tests section:

```rust
#[test]
fn alarm_fired_round_trips_json() {
    let event = DomainEvent::AlarmFired {
        fire_id: "fire_abc".into(),
        kind: "task_alarm".into(),
        ref_id: Some("task_1".into()),
        payload_json: "{\"msg\":\"hi\"}".into(),
        fired_at_ms: 1_800_000_000_000,
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
    match parsed {
        DomainEvent::AlarmFired { fire_id, .. } => assert_eq!(fire_id, "fire_abc"),
        _ => panic!("wrong variant"),
    }
}

#[test]
fn missed_alarms_round_trips() {
    let event = DomainEvent::MissedAlarms {
        fire_ids: vec!["a".into(), "b".into(), "c".into()],
        oldest_fire_at_ms: 1_000,
        newest_fire_at_ms: 2_000,
    };
    let json = serde_json::to_string(&event).unwrap();
    let parsed: DomainEvent = serde_json::from_str(&json).unwrap();
    match parsed {
        DomainEvent::MissedAlarms { fire_ids, oldest_fire_at_ms, newest_fire_at_ms } => {
            assert_eq!(fire_ids.len(), 3);
            assert_eq!(oldest_fire_at_ms, 1_000);
            assert_eq!(newest_fire_at_ms, 2_000);
        }
        _ => panic!("wrong variant"),
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p bus alarm`
Expected: FAIL — variants unknown.

- [ ] **Step 3: Add variants**

In `crates/bus/src/domain_events.rs`, inside `pub enum DomainEvent { ... }`, add:

```rust
    AlarmFired {
        fire_id: String,
        kind: String,
        ref_id: Option<String>,
        payload_json: String,
        fired_at_ms: i64,
    },
    AlarmSnoozed {
        fire_id: String,
        new_fire_at_ms: i64,
    },
    AlarmCancelled {
        fire_id: String,
        reason: String,
    },
    MissedAlarms {
        fire_ids: Vec<String>,
        oldest_fire_at_ms: i64,
        newest_fire_at_ms: i64,
    },
```

And in `impl DomainEvent { fn variant_name(...) }`, add arms returning `"AlarmFired"` etc. (match the existing pattern for every other variant in that function).

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p bus alarm`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add AlarmFired/Snoozed/Cancelled/MissedAlarms events"
```

---

## Task 7: `TemporalScheduler` — the wall-clock-anchored loop

**Files:**
- Modify: `crates/scheduling/src/temporal/mod.rs` (add scheduler module + facade)
- Create: `crates/scheduling/src/temporal/scheduler.rs`

- [ ] **Step 1: Write failing test**

`crates/scheduling/src/temporal/scheduler.rs` at bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use bus::domain_events::{DomainEvent, DomainEventBus};
    use jiff::Timestamp;
    use std::sync::Arc;
    use std::time::Duration;
    use storage::pool::StoragePool;

    async fn setup() -> (TemporalScheduler, Arc<DomainEventBus>) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(), version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        ).await.unwrap();
        let store = FireStore::new(storage::repos::scheduled_fires::ScheduledFiresRepo::new(
            pool.inner().clone(),
        ));
        let bus = Arc::new(DomainEventBus::new(32));
        let scheduler = TemporalScheduler::new(store, bus.clone(), SchedulerConfig::default());
        (scheduler, bus)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn fires_due_alarm_and_emits_event() {
        let (scheduler, bus) = setup().await;
        let mut rx = bus.subscribe();
        let _handle = scheduler.clone().start_background();

        // Schedule a fire 50ms in the future.
        let fire_at = Timestamp::now().checked_add(jiff::Span::new().milliseconds(50)).unwrap();
        scheduler.store().schedule(FireSpec {
            fire_at,
            kind: "test".into(),
            ref_id: Some("r1".into()),
            payload: serde_json::json!({}),
            dedup_prefix: None,
        }).await.unwrap();
        scheduler.wake();

        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await.expect("event not received in time").unwrap();
        assert!(matches!(event, DomainEvent::AlarmFired { .. }));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn skip_if_stale_emits_missed_alarms() {
        let (scheduler, bus) = setup().await;
        let mut rx = bus.subscribe();
        let _handle = scheduler.clone().start_background();

        // Fire 2 hours in the past, grace 1 hour, policy skip_if_stale (defaults).
        let fire_at = Timestamp::now().checked_sub(jiff::Span::new().hours(2)).unwrap();
        scheduler.store().schedule(FireSpec {
            fire_at,
            kind: "test".into(),
            ref_id: None,
            payload: serde_json::json!({ "misfire_policy": "skip_if_stale", "grace_secs": 3600 }),
            dedup_prefix: None,
        }).await.unwrap();
        scheduler.wake();

        // Expect MissedAlarms, not AlarmFired.
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv())
            .await.expect("event not received").unwrap();
        assert!(matches!(event, DomainEvent::MissedAlarms { .. }));
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p scheduling scheduler::tests`
Expected: FAIL — `TemporalScheduler` unknown.

- [ ] **Step 3: Implement the scheduler**

`crates/scheduling/src/temporal/scheduler.rs`:

```rust
//! Unified wall-clock-anchored scheduler.
//!
//! Design:
//! - Single tokio loop; sleeps at most 30s at a time. Guarantees re-evaluation of
//!   wall-clock time after macOS sleep, without platform-specific code.
//! - Two-phase fire commit: begin_firing (claim) -> publish event -> mark_fired.
//!   Crash between phases leaves an in-flight row; on restart we re-dispatch.
//! - Wake signal: external mutations call `wake()` to jump out of sleep early.
//! - SystemDidWake: subscribes (Task 12 wires this); no cross-layer dep on app-core.
//!
//! Firing does NOT take a user-provided closure. It publishes `AlarmFired` on the
//! `DomainEventBus`. Dispatchers subscribe (Phase 3 crate).

use std::sync::Arc;
use std::time::Duration;

use bus::domain_events::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use storage::rows::scheduled_fire::ScheduledFireRow;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

use crate::error::SchedulerError;
use crate::temporal::fire_store::FireStore;
use crate::temporal::misfire::{Decision, MisfirePolicy};

/// Max time the loop will sleep without checking wall clock. Keep short enough that
/// macOS system sleep resume leaves us at most this far behind.
const MAX_SLEEP: Duration = Duration::from_secs(30);
/// Default grace window when a payload doesn't specify one.
const DEFAULT_GRACE_SECS: u64 = 3600;

#[derive(Debug, Clone)]
pub struct SchedulerConfig {
    pub max_sleep: Duration,
    pub default_grace_secs: u64,
    pub default_misfire_policy: MisfirePolicy,
}

impl Default for SchedulerConfig {
    fn default() -> Self {
        Self {
            max_sleep: MAX_SLEEP,
            default_grace_secs: DEFAULT_GRACE_SECS,
            default_misfire_policy: MisfirePolicy::SkipIfStale,
        }
    }
}

#[derive(Clone)]
pub struct TemporalScheduler {
    store: FireStore,
    bus: Arc<DomainEventBus>,
    config: SchedulerConfig,
    wake: Arc<Notify>,
    shutdown: CancellationToken,
}

impl TemporalScheduler {
    pub fn new(store: FireStore, bus: Arc<DomainEventBus>, config: SchedulerConfig) -> Self {
        Self {
            store, bus, config,
            wake: Arc::new(Notify::new()),
            shutdown: CancellationToken::new(),
        }
    }

    pub fn store(&self) -> &FireStore { &self.store }

    /// External mutations call this after inserting/cancelling fires.
    pub fn wake(&self) { self.wake.notify_one(); }

    /// Graceful shutdown. Loop exits after current iteration.
    pub fn shutdown(&self) { self.shutdown.cancel(); }

    /// Spawn the loop on the current runtime. Returns a join handle.
    pub fn start_background(self) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            if let Err(e) = self.run().await {
                warn!(error = %e, "TemporalScheduler exited with error");
            }
        })
    }

    /// Main loop. Returns when shutdown is cancelled.
    pub async fn run(self) -> Result<(), SchedulerError> {
        info!("TemporalScheduler starting");
        // Recover: re-dispatch any rows left in-flight from a crash.
        self.recover_in_flight().await?;

        loop {
            let next = self.store.next_pending_fire_at().await?;
            let now = Timestamp::now();
            let sleep = match next {
                None => self.config.max_sleep,
                Some(t) => {
                    let diff_ms = (t.as_millisecond() - now.as_millisecond()).max(0) as u64;
                    Duration::from_millis(diff_ms).min(self.config.max_sleep)
                }
            };

            tokio::select! {
                _ = tokio::time::sleep(sleep) => {}
                _ = self.wake.notified() => {}
                _ = self.shutdown.cancelled() => {
                    info!("TemporalScheduler shutting down");
                    return Ok(());
                }
            }

            self.process_due(Timestamp::now()).await?;
        }
    }

    async fn recover_in_flight(&self) -> Result<(), SchedulerError> {
        let rows = self.store.recover_in_flight().await?;
        if rows.is_empty() { return Ok(()); }
        warn!(count = rows.len(), "recovering in-flight fires after restart");
        for row in rows {
            self.dispatch(row, Timestamp::now()).await?;
        }
        Ok(())
    }

    async fn process_due(&self, now: Timestamp) -> Result<(), SchedulerError> {
        let due = self.store.list_due(now).await?;
        let mut missed: Vec<ScheduledFireRow> = Vec::new();
        for row in due {
            let (policy, grace) = self.extract_misfire_params(&row);
            let fire_at = Timestamp::from_millisecond(row.fire_at_ms)
                .map_err(|_| SchedulerError::InvalidState("bad fire_at_ms".into()))?;
            match Decision::classify(policy, grace, fire_at, now) {
                Decision::Fire => self.dispatch(row, now).await?,
                Decision::SkipStale => {
                    if self.store.begin_firing(&row.id, now).await? {
                        self.store.mark_fired(&row.id, now).await?;
                        missed.push(row);
                    }
                }
                Decision::CoalesceLater => self.dispatch(row, now).await?, // day-1: treat as fire
            }
        }
        if !missed.is_empty() {
            self.emit_missed(missed);
        }
        Ok(())
    }

    fn extract_misfire_params(&self, row: &ScheduledFireRow) -> (MisfirePolicy, Duration) {
        let policy = row.payload.get("misfire_policy")
            .and_then(|v| v.as_str())
            .and_then(|s| serde_json::from_str::<MisfirePolicy>(&format!("\"{s}\"")).ok())
            .unwrap_or(self.config.default_misfire_policy);
        let grace_secs = row.payload.get("grace_secs")
            .and_then(|v| v.as_u64())
            .unwrap_or(self.config.default_grace_secs);
        (policy, Duration::from_secs(grace_secs))
    }

    async fn dispatch(&self, row: ScheduledFireRow, now: Timestamp) -> Result<(), SchedulerError> {
        if !self.store.begin_firing(&row.id, now).await? {
            return Ok(()); // another worker or already fired
        }
        self.bus.publish(DomainEvent::AlarmFired {
            fire_id: row.id.clone(),
            kind: row.kind.clone(),
            ref_id: row.ref_id.clone(),
            payload_json: row.payload.to_string(),
            fired_at_ms: now.as_millisecond(),
        });
        self.store.mark_fired(&row.id, now).await?;
        Ok(())
    }

    fn emit_missed(&self, rows: Vec<ScheduledFireRow>) {
        let fire_ids: Vec<String> = rows.iter().map(|r| r.id.clone()).collect();
        let oldest = rows.iter().map(|r| r.fire_at_ms).min().unwrap_or(0);
        let newest = rows.iter().map(|r| r.fire_at_ms).max().unwrap_or(0);
        self.bus.publish(DomainEvent::MissedAlarms {
            fire_ids,
            oldest_fire_at_ms: oldest,
            newest_fire_at_ms: newest,
        });
    }
}
```

Register in `temporal/mod.rs`:

```rust
pub mod scheduler;
pub use scheduler::{SchedulerConfig, TemporalScheduler};
```

Add `bus.workspace = true` to `crates/scheduling/Cargo.toml` `[dependencies]`.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p scheduling scheduler::tests`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/src/temporal/scheduler.rs crates/scheduling/src/temporal/mod.rs crates/scheduling/Cargo.toml
git commit -m "feat(scheduling): TemporalScheduler wall-clock-anchored loop with misfire handling"
```

---

## Task 8: Restart recovery test — in-flight rows re-dispatch

**Files:**
- Modify: `crates/scheduling/src/temporal/scheduler.rs` (add test)

- [ ] **Step 1: Write failing test**

Add to the scheduler tests module:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn recovers_in_flight_rows_on_restart() {
    let pool = storage::pool::StoragePool::connect_in_memory().await.unwrap();
    storage::pool::StoragePool::run_feature_migrations(
        pool.inner(),
        &[tools_core::FeatureMigration {
            feature_name: "scheduling".into(), version: 1,
            description: "scheduled_fires".into(),
            sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
        }],
    ).await.unwrap();
    let store = FireStore::new(storage::repos::scheduled_fires::ScheduledFiresRepo::new(
        pool.inner().clone(),
    ));

    // Simulate a crash: insert a row, mark it as firing, do NOT mark fired.
    let fire_at = Timestamp::now().checked_sub(jiff::Span::new().seconds(1)).unwrap();
    let id = store.schedule(FireSpec {
        fire_at, kind: "test".into(), ref_id: None,
        payload: serde_json::json!({}), dedup_prefix: None,
    }).await.unwrap();
    assert!(store.begin_firing(&id, Timestamp::now()).await.unwrap());
    // (no mark_fired — simulates crash)

    // "Restart": new scheduler, fresh bus.
    let bus = Arc::new(DomainEventBus::new(32));
    let mut rx = bus.subscribe();
    let scheduler = TemporalScheduler::new(store, bus.clone(), SchedulerConfig::default());
    let _h = scheduler.start_background();

    let ev = tokio::time::timeout(Duration::from_secs(2), rx.recv())
        .await.expect("no recovery event").unwrap();
    match ev {
        DomainEvent::AlarmFired { fire_id, .. } => assert_eq!(fire_id, id),
        other => panic!("expected AlarmFired, got {other:?}"),
    }
}
```

- [ ] **Step 2: Run test**

Run: `cargo nextest run -p scheduling recovers_in_flight`
Expected: PASS (recovery is already implemented in Task 7's `recover_in_flight`). If it fails, fix the `recover_in_flight` method.

- [ ] **Step 3: Commit**

```bash
git add crates/scheduling/src/temporal/scheduler.rs
git commit -m "test(scheduling): in-flight recovery after simulated crash"
```

---

## Task 9: RRULE DSL compiler + evaluator (chrono isolated)

**Files:**
- Create: `crates/scheduling/src/temporal/rrule.rs`
- Modify: `crates/scheduling/src/temporal/mod.rs`
- Modify: `crates/scheduling/Cargo.toml`

- [ ] **Step 1: Add dependency**

In `crates/scheduling/Cargo.toml`:

```toml
rrule = "0.14"
```

- [ ] **Step 2: Write failing tests**

`crates/scheduling/src/temporal/rrule.rs` at bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use jiff::civil::time;

    #[test]
    fn compiles_weekly_mwf_at_9am() {
        let dsl = RRuleSpec {
            frequency: Frequency::Weekly,
            interval: Some(1),
            by_day: Some(vec!["MO".into(), "WE".into(), "FR".into()]),
            at: Some(time(9, 0, 0, 0)),
            timezone: "America/New_York".into(),
            by_month_day: None, until: None, count: None,
        };
        let rule = dsl.compile().unwrap();
        assert!(rule.contains("FREQ=WEEKLY"));
        assert!(rule.contains("BYDAY=MO,WE,FR"));
        assert!(rule.contains("BYHOUR=9"));
    }

    #[test]
    fn evaluator_returns_next_three_instances_daily() {
        let dsl = RRuleSpec {
            frequency: Frequency::Daily,
            interval: Some(1),
            at: Some(time(9, 0, 0, 0)),
            timezone: "America/New_York".into(),
            by_day: None, by_month_day: None, until: None, count: None,
        };
        let start = jiff::Timestamp::from_millisecond(1_800_000_000_000).unwrap();
        let next = evaluate_next_n(&dsl, start, 3).unwrap();
        assert_eq!(next.len(), 3);
        // Adjacent instances are ~24h apart (modulo DST).
        let delta = next[1].as_millisecond() - next[0].as_millisecond();
        assert!(delta >= 23 * 3600 * 1000 && delta <= 25 * 3600 * 1000);
    }
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p scheduling rrule::tests`
Expected: FAIL — module not found.

- [ ] **Step 4: Implement**

`crates/scheduling/src/temporal/rrule.rs`:

```rust
//! RRULE DSL → RFC 5545 compiler + evaluator.
//!
//! ⚠️ CHRONO BOUNDARY: the upstream `rrule` crate uses `chrono::DateTime<Tz>` at its API
//! boundary. We keep chrono *only* in this module; downstream code sees only `jiff::Timestamp`.
//!
//! Conversion strategy (lossless, epoch-ms):
//!   jiff::Timestamp  <--ms-->  chrono::DateTime<Utc>  <--with_timezone-->  chrono::DateTime<Tz>

use chrono::{DateTime, TimeZone, Utc};
use jiff::civil::Time as CivilTime;
use jiff::Timestamp;
use rrule::{RRuleSet, Tz as RruleTz};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

use crate::error::SchedulerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Frequency { Daily, Weekly, Monthly, Yearly }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct RRuleSpec {
    pub frequency: Frequency,
    pub interval: Option<u32>,
    pub by_day: Option<Vec<String>>,      // "MO","TU",...
    pub by_month_day: Option<Vec<i32>>,   // 1..=31 or -1..=-31
    pub at: Option<CivilTime>,
    pub timezone: String,                 // IANA
    pub until: Option<Timestamp>,
    pub count: Option<u32>,
}

impl RRuleSpec {
    /// Compile to RFC 5545 RRULE text. Does NOT include DTSTART (caller provides).
    pub fn compile(&self) -> Result<String, SchedulerError> {
        let freq = match self.frequency {
            Frequency::Daily => "DAILY", Frequency::Weekly => "WEEKLY",
            Frequency::Monthly => "MONTHLY", Frequency::Yearly => "YEARLY",
        };
        let mut parts = vec![format!("FREQ={freq}")];
        if let Some(i) = self.interval { parts.push(format!("INTERVAL={i}")); }
        if let Some(bd) = &self.by_day { parts.push(format!("BYDAY={}", bd.join(","))); }
        if let Some(bmd) = &self.by_month_day {
            parts.push(format!("BYMONTHDAY={}", bmd.iter().map(i32::to_string)
                .collect::<Vec<_>>().join(",")));
        }
        if let Some(t) = self.at {
            parts.push(format!("BYHOUR={}", t.hour()));
            parts.push(format!("BYMINUTE={}", t.minute()));
        }
        if let Some(c) = self.count { parts.push(format!("COUNT={c}")); }
        if let Some(u) = self.until {
            let dt = timestamp_to_chrono_utc(u);
            parts.push(format!("UNTIL={}", dt.format("%Y%m%dT%H%M%SZ")));
        }
        Ok(parts.join(";"))
    }
}

pub fn evaluate_next_n(
    spec: &RRuleSpec,
    after: Timestamp,
    n: usize,
) -> Result<Vec<Timestamp>, SchedulerError> {
    let rrule_text = spec.compile()?;
    let tz = chrono_tz::Tz::from_str(&spec.timezone)
        .map_err(|e| SchedulerError::Rrule(format!("bad timezone {}: {e}", spec.timezone)))?;
    let rrule_tz: RruleTz = tz.into();

    let dtstart_utc = timestamp_to_chrono_utc(after);
    let dtstart: DateTime<RruleTz> = dtstart_utc.with_timezone(&rrule_tz);
    let full = format!(
        "DTSTART;TZID={}:{}\nRRULE:{}",
        spec.timezone,
        dtstart.format("%Y%m%dT%H%M%S"),
        rrule_text
    );

    let set: RRuleSet = full.parse()
        .map_err(|e| SchedulerError::Rrule(format!("rrule parse: {e}")))?;
    let iter = set.into_iter().take(n);
    let out: Vec<Timestamp> = iter
        .filter_map(|dt| Timestamp::from_millisecond(dt.timestamp_millis()).ok())
        .collect();
    Ok(out)
}

fn timestamp_to_chrono_utc(t: Timestamp) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(t.as_millisecond()).single().expect("valid ms")
}
```

Register in `temporal/mod.rs`:

```rust
pub mod rrule;
pub use rrule::{evaluate_next_n, Frequency, RRuleSpec};
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p scheduling rrule::tests`
Expected: PASS — 2 tests.

- [ ] **Step 6: DST test for RRULE**

Add to `rrule.rs` tests:

```rust
#[test]
fn daily_at_9am_ny_skips_evenly_across_dst_spring_forward() {
    // March 8 2026 is spring-forward in NY (02:00 -> 03:00 EDT).
    // Daily @ 9am NY should fire 9am local each day (wall-clock anchor), so adjacent
    // UTC instants across 2026-03-07 and 2026-03-08 differ by 23h (not 24h).
    let dsl = RRuleSpec {
        frequency: Frequency::Daily, interval: Some(1),
        at: Some(jiff::civil::time(9, 0, 0, 0)),
        timezone: "America/New_York".into(),
        by_day: None, by_month_day: None, until: None, count: None,
    };
    // Anchor: March 7 2026 08:00 NY = 2026-03-07T13:00Z
    let anchor = jiff::civil::date(2026, 3, 7).at(8, 0, 0, 0)
        .to_zoned(jiff::tz::TimeZone::get("America/New_York").unwrap()).unwrap()
        .timestamp();
    let next = evaluate_next_n(&dsl, anchor, 3).unwrap();
    let diff_0_1 = next[1].as_millisecond() - next[0].as_millisecond();
    // 23h because we cross into EDT.
    assert_eq!(diff_0_1, 23 * 3600 * 1000);
}
```

Run: `cargo nextest run -p scheduling rrule::tests`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/scheduling/src/temporal/rrule.rs \
        crates/scheduling/src/temporal/mod.rs \
        crates/scheduling/Cargo.toml
git commit -m "feat(scheduling): RRULE DSL compiler + evaluator with DST-aware boundary"
```

---

## Task 10: `task_alarms` + `task_recurrence_templates` — schema + row types

**Files:**
- Modify: `crates/feature-tasks/migrations/001_create_tasks.sql` (edit in place — pre-release policy)
- Create: `crates/storage/src/rows/task_alarm.rs`
- Create: `crates/storage/src/rows/task_recurrence.rs`
- Create: `crates/storage/src/repos/task_alarms.rs`
- Create: `crates/storage/src/repos/task_recurrence.rs`
- Modify: `crates/storage/src/rows/mod.rs`, `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Write failing repo tests**

`crates/storage/src/repos/tests/task_alarms_tests.rs`:

```rust
use crate::pool::StoragePool;
use crate::repos::task_alarms::TaskAlarmsRepo;
use crate::rows::task_alarm::TaskAlarmRow;
use tools_core::FeatureMigration;

async fn setup() -> TaskAlarmsRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    StoragePool::run_feature_migrations(
        pool.inner(),
        &[FeatureMigration {
            feature_name: "tasks".into(), version: 1,
            description: "tasks + alarms".into(),
            sql: include_str!("../../../../feature-tasks/migrations/001_create_tasks.sql").into(),
        }],
    ).await.unwrap();
    // Insert a task first (FK requirement):
    sqlx::query("INSERT INTO tasks (id, title, created_at_ms) VALUES ('task_1','T',0)")
        .execute(pool.inner()).await.unwrap();
    TaskAlarmsRepo::new(pool.inner().clone())
}

#[tokio::test]
async fn insert_and_list_by_task() {
    let repo = setup().await;
    let row = TaskAlarmRow {
        id: "a1".into(), task_id: "task_1".into(),
        rule_type: "relative_before".into(),
        offset_secs: Some(3600), day_offset: None, time_of_day: None, iana_tz: None,
        absolute_fire_at_ms: None, channel_mask: 0,
        priority_override: None, misfire_policy: None, grace_window_secs: None,
        created_at_ms: 0,
    };
    repo.insert(&row).await.unwrap();
    let listed = repo.list_by_task("task_1").await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, "a1");
}

#[tokio::test]
async fn delete_by_task_cascade() {
    let repo = setup().await;
    repo.insert(&TaskAlarmRow {
        id: "a1".into(), task_id: "task_1".into(),
        rule_type: "relative_before".into(), offset_secs: Some(3600),
        day_offset: None, time_of_day: None, iana_tz: None,
        absolute_fire_at_ms: None, channel_mask: 0,
        priority_override: None, misfire_policy: None, grace_window_secs: None,
        created_at_ms: 0,
    }).await.unwrap();
    repo.delete_by_task("task_1").await.unwrap();
    assert!(repo.list_by_task("task_1").await.unwrap().is_empty());
}
```

Register `mod task_alarms_tests;` in `tests/mod.rs`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p storage task_alarms_tests`
Expected: FAIL — tables don't exist, types don't exist.

- [ ] **Step 3: Edit the feature-tasks migration (pre-release, edit in place)**

Append to `crates/feature-tasks/migrations/001_create_tasks.sql`:

```sql
-- ---------- Task alarms (Phase 2) ----------
CREATE TABLE task_alarms (
    id TEXT PRIMARY KEY,
    task_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
    rule_type TEXT NOT NULL,                        -- 'relative_before'|'civil_time'|'absolute'
    offset_secs INTEGER,
    day_offset INTEGER,
    time_of_day TEXT,                               -- 'HH:MM'
    iana_tz TEXT,
    absolute_fire_at_ms INTEGER,
    channel_mask INTEGER NOT NULL DEFAULT 0,
    priority_override TEXT,
    misfire_policy TEXT,
    grace_window_secs INTEGER,
    created_at_ms INTEGER NOT NULL,
    UNIQUE (task_id, rule_type, offset_secs, day_offset, time_of_day, absolute_fire_at_ms)
);
CREATE INDEX idx_task_alarms_task ON task_alarms(task_id);

-- ---------- Task recurrence templates (Phase 2) ----------
CREATE TABLE task_recurrence_templates (
    id TEXT PRIMARY KEY,
    source_task_id TEXT NOT NULL,
    rrule TEXT NOT NULL,
    iana_tz TEXT NOT NULL,
    materialize_ahead INTEGER NOT NULL DEFAULT 3,
    next_instance_at_ms INTEGER,
    last_instance_at_ms INTEGER,
    until_at_ms INTEGER,
    count_remaining INTEGER,
    enabled INTEGER NOT NULL DEFAULT 1,
    created_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_task_recurrence_enabled ON task_recurrence_templates(enabled) WHERE enabled = 1;

-- tasks table gains a template_id column (nullable). Edit the existing CREATE TABLE tasks above
-- to add:  template_id TEXT REFERENCES task_recurrence_templates(id)
-- (Do this by adding the column inside the existing CREATE TABLE tasks block, not with ALTER.)
```

**Engineer note:** Locate the `CREATE TABLE tasks (...)` block in that same file and insert `template_id TEXT REFERENCES task_recurrence_templates(id),` as a new column. Do not add via `ALTER TABLE`.

- [ ] **Step 4: Create row + repo files**

`crates/storage/src/rows/task_alarm.rs`:

```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskAlarmRow {
    pub id: String,
    pub task_id: String,
    pub rule_type: String,
    pub offset_secs: Option<i64>,
    pub day_offset: Option<i64>,
    pub time_of_day: Option<String>,
    pub iana_tz: Option<String>,
    pub absolute_fire_at_ms: Option<i64>,
    pub channel_mask: i64,
    pub priority_override: Option<String>,
    pub misfire_policy: Option<String>,
    pub grace_window_secs: Option<i64>,
    pub created_at_ms: i64,
}
```

`crates/storage/src/rows/task_recurrence.rs`:

```rust
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecurrenceTemplateRow {
    pub id: String,
    pub source_task_id: String,
    pub rrule: String,
    pub iana_tz: String,
    pub materialize_ahead: i64,
    pub next_instance_at_ms: Option<i64>,
    pub last_instance_at_ms: Option<i64>,
    pub until_at_ms: Option<i64>,
    pub count_remaining: Option<i64>,
    pub enabled: bool,
    pub created_at_ms: i64,
}
```

`crates/storage/src/repos/task_alarms.rs`:

```rust
use sqlx::SqlitePool;
use crate::error::StorageError;
use crate::rows::task_alarm::TaskAlarmRow;

#[derive(Debug, Clone)]
pub struct TaskAlarmsRepo { pool: SqlitePool }

impl TaskAlarmsRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn insert(&self, row: &TaskAlarmRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO task_alarms
             (id, task_id, rule_type, offset_secs, day_offset, time_of_day, iana_tz,
              absolute_fire_at_ms, channel_mask, priority_override, misfire_policy,
              grace_window_secs, created_at_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        )
        .bind(&row.id).bind(&row.task_id).bind(&row.rule_type)
        .bind(row.offset_secs).bind(row.day_offset)
        .bind(&row.time_of_day).bind(&row.iana_tz)
        .bind(row.absolute_fire_at_ms).bind(row.channel_mask)
        .bind(&row.priority_override).bind(&row.misfire_policy)
        .bind(row.grace_window_secs).bind(row.created_at_ms)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn list_by_task(&self, task_id: &str) -> Result<Vec<TaskAlarmRow>, StorageError> {
        Ok(sqlx::query_as::<_, TaskAlarmRow>(
            "SELECT * FROM task_alarms WHERE task_id = ?1 ORDER BY created_at_ms ASC",
        ).bind(task_id).fetch_all(&self.pool).await?)
    }

    pub async fn delete_by_task(&self, task_id: &str) -> Result<u64, StorageError> {
        Ok(sqlx::query("DELETE FROM task_alarms WHERE task_id = ?1")
            .bind(task_id).execute(&self.pool).await?.rows_affected())
    }

    pub async fn delete_by_id(&self, id: &str) -> Result<u64, StorageError> {
        Ok(sqlx::query("DELETE FROM task_alarms WHERE id = ?1")
            .bind(id).execute(&self.pool).await?.rows_affected())
    }
}
```

`crates/storage/src/repos/task_recurrence.rs`:

```rust
use sqlx::SqlitePool;
use crate::error::StorageError;
use crate::rows::task_recurrence::TaskRecurrenceTemplateRow;

#[derive(Debug, Clone)]
pub struct TaskRecurrenceRepo { pool: SqlitePool }

impl TaskRecurrenceRepo {
    pub fn new(pool: SqlitePool) -> Self { Self { pool } }

    pub async fn upsert(&self, row: &TaskRecurrenceTemplateRow) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO task_recurrence_templates
             (id, source_task_id, rrule, iana_tz, materialize_ahead,
              next_instance_at_ms, last_instance_at_ms, until_at_ms,
              count_remaining, enabled, created_at_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)
             ON CONFLICT(id) DO UPDATE SET
                rrule = EXCLUDED.rrule,
                iana_tz = EXCLUDED.iana_tz,
                materialize_ahead = EXCLUDED.materialize_ahead,
                next_instance_at_ms = EXCLUDED.next_instance_at_ms,
                last_instance_at_ms = EXCLUDED.last_instance_at_ms,
                until_at_ms = EXCLUDED.until_at_ms,
                count_remaining = EXCLUDED.count_remaining,
                enabled = EXCLUDED.enabled",
        )
        .bind(&row.id).bind(&row.source_task_id).bind(&row.rrule)
        .bind(&row.iana_tz).bind(row.materialize_ahead)
        .bind(row.next_instance_at_ms).bind(row.last_instance_at_ms)
        .bind(row.until_at_ms).bind(row.count_remaining)
        .bind(row.enabled).bind(row.created_at_ms)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn get(&self, id: &str) -> Result<Option<TaskRecurrenceTemplateRow>, StorageError> {
        Ok(sqlx::query_as::<_, TaskRecurrenceTemplateRow>(
            "SELECT * FROM task_recurrence_templates WHERE id = ?1",
        ).bind(id).fetch_optional(&self.pool).await?)
    }

    pub async fn list_enabled(&self) -> Result<Vec<TaskRecurrenceTemplateRow>, StorageError> {
        Ok(sqlx::query_as::<_, TaskRecurrenceTemplateRow>(
            "SELECT * FROM task_recurrence_templates WHERE enabled = 1",
        ).fetch_all(&self.pool).await?)
    }
}
```

Register in `rows/mod.rs`:

```rust
pub mod task_alarm;
pub mod task_recurrence;
```

Register in `repos/mod.rs`:

```rust
pub mod task_alarms;
pub mod task_recurrence;
```

And add to the `Repos` struct `from_pool` construction — follow the exact pattern of existing `pub task_groups: TaskGroupRepo,` field adjacency.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p storage task_alarms_tests`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/feature-tasks/migrations/001_create_tasks.sql \
        crates/storage/src/rows/task_alarm.rs \
        crates/storage/src/rows/task_recurrence.rs \
        crates/storage/src/rows/mod.rs \
        crates/storage/src/repos/task_alarms.rs \
        crates/storage/src/repos/task_recurrence.rs \
        crates/storage/src/repos/mod.rs \
        crates/storage/src/repos/tests/
git commit -m "feat(storage): task_alarms + task_recurrence_templates schema + repos"
```

---

## Task 11: Cron bridge — reconcile `cron_jobs` ↔ `scheduled_fires`

**Files:**
- Create: `crates/scheduling/src/temporal/cron_bridge.rs`
- Modify: `crates/scheduling/src/temporal/mod.rs`

- [ ] **Step 1: Write failing test**

`crates/scheduling/src/temporal/cron_bridge.rs` at bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::pool::StoragePool;
    use storage::repos::cron::CronRepo;
    use storage::repos::scheduled_fires::ScheduledFiresRepo;
    use storage::rows::cron::CronJobRow;

    async fn setup() -> (CronBridge, CronRepo, ScheduledFiresRepo) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        // Need the initial storage migrations for cron_jobs + scheduling for scheduled_fires.
        storage::run_initial_migrations(pool.inner()).await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[tools_core::FeatureMigration {
                feature_name: "scheduling".into(), version: 1,
                description: "scheduled_fires".into(),
                sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
            }],
        ).await.unwrap();
        let cron = CronRepo::new(pool.inner().clone());
        let sf = ScheduledFiresRepo::new(pool.inner().clone());
        let bridge = CronBridge::new(
            cron.clone(),
            crate::temporal::fire_store::FireStore::new(sf.clone()),
        );
        (bridge, cron, sf)
    }

    #[tokio::test]
    async fn reconcile_creates_pending_fire_for_enabled_job() {
        let (bridge, cron, sf) = setup().await;
        cron.upsert(&CronJobRow {
            id: "j1".into(), name: "daily".into(), enabled: true,
            origin: "user".into(),
            schedule: serde_json::json!({ "cron": "0 9 * * *", "tz": "UTC" }),
            payload: serde_json::json!({}),
            next_run_at_ms: None, last_run_at_ms: None, last_status: None, last_error: None,
            created_at_ms: 0, updated_at_ms: 0, delete_after_run: false,
            intent_window: None, intent_pending_since_ms: None,
        }).await.unwrap();
        bridge.reconcile_all().await.unwrap();
        let pending = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].kind, "cron_job");
        assert_eq!(pending[0].ref_id.as_deref(), Some("j1"));
    }

    #[tokio::test]
    async fn reconcile_does_not_duplicate_when_called_twice() {
        let (bridge, cron, sf) = setup().await;
        cron.upsert(&CronJobRow {
            id: "j1".into(), name: "daily".into(), enabled: true,
            origin: "user".into(),
            schedule: serde_json::json!({ "cron": "0 9 * * *", "tz": "UTC" }),
            payload: serde_json::json!({}),
            next_run_at_ms: None, last_run_at_ms: None, last_status: None, last_error: None,
            created_at_ms: 0, updated_at_ms: 0, delete_after_run: false,
            intent_window: None, intent_pending_since_ms: None,
        }).await.unwrap();
        bridge.reconcile_all().await.unwrap();
        bridge.reconcile_all().await.unwrap();
        let pending = sf.list_pending_up_to_ms(i64::MAX).await.unwrap();
        assert_eq!(pending.len(), 1);
    }
}
```

**Note:** `storage::run_initial_migrations` may not exist by that name. If not, use the pattern shown in `crates/storage/src/lib.rs` tests — adapt to whatever helper already exists for applying `001_initial.sql`.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p scheduling cron_bridge::tests`
Expected: FAIL — `CronBridge` unknown.

- [ ] **Step 3: Implement**

`crates/scheduling/src/temporal/cron_bridge.rs`:

```rust
//! Bridge between legacy `cron_jobs` table and the unified `scheduled_fires`.
//!
//! `cron_jobs` remains the *definition* table (user- and agent-created recurring jobs).
//! `scheduled_fires` is the *firing* table. For each enabled cron job, we maintain
//! exactly one pending `scheduled_fires` row (kind='cron_job', ref_id=cron_jobs.id).
//!
//! On `AlarmFired` for a cron_job row, call `advance()` to compute the next run and
//! insert a fresh scheduled_fires row.

use cron::Schedule;
use jiff::{Timestamp, tz::TimeZone};
use std::str::FromStr;
use storage::repos::cron::CronRepo;
use storage::rows::cron::CronJobRow;

use crate::error::SchedulerError;
use crate::temporal::fire_store::{FireSpec, FireStore};

#[derive(Debug, Clone)]
pub struct CronBridge {
    cron: CronRepo,
    fires: FireStore,
}

impl CronBridge {
    pub fn new(cron: CronRepo, fires: FireStore) -> Self { Self { cron, fires } }

    /// Ensure every enabled cron_jobs row has exactly one pending scheduled_fires row.
    /// Called on startup and whenever cron_jobs changes.
    pub async fn reconcile_all(&self) -> Result<(), SchedulerError> {
        let jobs = self.cron.list().await?;
        for job in jobs {
            if job.enabled {
                self.ensure_scheduled(&job).await?;
            } else {
                self.fires.cancel_by_kind_ref("cron_job", &job.id).await?;
            }
        }
        Ok(())
    }

    /// Insert a pending fire for this job only if one does not already exist.
    async fn ensure_scheduled(&self, job: &CronJobRow) -> Result<(), SchedulerError> {
        // Clear any stale pending (handles schedule change + re-enable).
        self.fires.cancel_by_kind_ref("cron_job", &job.id).await?;
        let next = self.next_fire_for(job)?;
        self.fires.schedule(FireSpec {
            fire_at: next,
            kind: "cron_job".into(),
            ref_id: Some(job.id.clone()),
            payload: serde_json::json!({ "job": job.id, "name": job.name }),
            dedup_prefix: Some(format!("cron:{}:", job.id)),
        }).await?;
        Ok(())
    }

    /// Call after the cron fire fires: compute the next run, insert new pending row.
    pub async fn advance(&self, job_id: &str) -> Result<(), SchedulerError> {
        let job = self.cron.get(job_id).await?;
        if job.enabled {
            self.ensure_scheduled(&job).await?;
        }
        Ok(())
    }

    fn next_fire_for(&self, job: &CronJobRow) -> Result<Timestamp, SchedulerError> {
        let cron_expr = job.schedule.get("cron").and_then(|v| v.as_str())
            .ok_or_else(|| SchedulerError::InvalidState(format!("cron job {} has no 'cron' field", job.id)))?;
        let tz_name = job.schedule.get("tz").and_then(|v| v.as_str()).unwrap_or("UTC");
        let schedule = Schedule::from_str(cron_expr)
            .map_err(|e| SchedulerError::InvalidState(format!("invalid cron expr: {e}")))?;

        // Use jiff for current time, compute next via chrono at boundary.
        // The cron crate requires chrono::TimeZone; we convert at the API boundary only.
        let tz: chrono_tz::Tz = tz_name.parse()
            .map_err(|e| SchedulerError::InvalidState(format!("bad tz {tz_name}: {e}")))?;
        let now_utc = chrono::Utc::now();
        let now_tz = now_utc.with_timezone(&tz);
        let next_tz = schedule.after(&now_tz).next()
            .ok_or_else(|| SchedulerError::InvalidState("cron schedule yielded no next".into()))?;
        Ok(Timestamp::from_millisecond(next_tz.timestamp_millis())
            .map_err(|_| SchedulerError::InvalidState("cron next out of range".into()))?)
    }
}
```

Register in `temporal/mod.rs`:

```rust
pub mod cron_bridge;
pub use cron_bridge::CronBridge;
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p scheduling cron_bridge::tests`
Expected: PASS — 2 tests.

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/src/temporal/cron_bridge.rs \
        crates/scheduling/src/temporal/mod.rs
git commit -m "feat(scheduling): CronBridge reconciles cron_jobs ↔ scheduled_fires"
```

---

## Task 12: Wire scheduler to advance cron jobs after AlarmFired

**Files:**
- Modify: `crates/scheduling/src/temporal/scheduler.rs`
- Modify: `crates/scheduling/src/lib.rs`

- [ ] **Step 1: Write failing test**

Add to `scheduler.rs` tests:

```rust
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cron_job_fire_triggers_next_schedule() {
    // Setup: storage with cron_jobs row, CronBridge wired, scheduler observes AlarmFired
    // for kind=cron_job and calls bridge.advance.
    let pool = storage::pool::StoragePool::connect_in_memory().await.unwrap();
    storage::run_initial_migrations(pool.inner()).await.unwrap();
    storage::pool::StoragePool::run_feature_migrations(
        pool.inner(),
        &[tools_core::FeatureMigration {
            feature_name: "scheduling".into(), version: 1,
            description: "scheduled_fires".into(),
            sql: include_str!("../../migrations/001_scheduled_fires.sql").into(),
        }],
    ).await.unwrap();
    let cron = storage::repos::cron::CronRepo::new(pool.inner().clone());
    let sf_repo = storage::repos::scheduled_fires::ScheduledFiresRepo::new(pool.inner().clone());
    let store = FireStore::new(sf_repo.clone());
    let bridge = super::cron_bridge::CronBridge::new(cron.clone(), store.clone());

    cron.upsert(&storage::rows::cron::CronJobRow {
        id: "j1".into(), name: "every-sec".into(), enabled: true, origin: "user".into(),
        schedule: serde_json::json!({ "cron": "* * * * * * *", "tz": "UTC" }),
        payload: serde_json::json!({}),
        next_run_at_ms: None, last_run_at_ms: None, last_status: None, last_error: None,
        created_at_ms: 0, updated_at_ms: 0, delete_after_run: false,
        intent_window: None, intent_pending_since_ms: None,
    }).await.unwrap();
    bridge.reconcile_all().await.unwrap();

    let bus = Arc::new(DomainEventBus::new(32));
    let scheduler = TemporalScheduler::new(store.clone(), bus.clone(), SchedulerConfig::default())
        .with_cron_bridge(bridge);
    let _h = scheduler.start_background();

    // Wait long enough for at least 2 fires (every-second cron).
    tokio::time::sleep(Duration::from_secs(3)).await;
    let fired_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM scheduled_fires WHERE fired = 1 AND kind = 'cron_job'"
    ).fetch_one(pool.inner()).await.unwrap();
    assert!(fired_count >= 2, "expected ≥2 cron fires, got {fired_count}");
}
```

- [ ] **Step 2: Run test**

Run: `cargo nextest run -p scheduling cron_job_fire_triggers_next_schedule`
Expected: FAIL — `with_cron_bridge` unknown.

- [ ] **Step 3: Implement `with_cron_bridge` and wire advance**

In `scheduler.rs`, add field and method:

```rust
// In struct TemporalScheduler:
    cron_bridge: Option<Arc<crate::temporal::cron_bridge::CronBridge>>,

// In impl:
    pub fn with_cron_bridge(mut self, bridge: crate::temporal::cron_bridge::CronBridge) -> Self {
        self.cron_bridge = Some(Arc::new(bridge));
        self
    }
```

Update `new()` to initialize `cron_bridge: None`. In `dispatch()`, after `mark_fired`, call advance for cron_job kind:

```rust
        self.store.mark_fired(&row.id, now).await?;
        if row.kind == "cron_job" {
            if let (Some(bridge), Some(ref_id)) = (&self.cron_bridge, &row.ref_id) {
                if let Err(e) = bridge.advance(ref_id).await {
                    warn!(error = %e, job = %ref_id, "cron bridge advance failed");
                }
            }
        }
        Ok(())
```

- [ ] **Step 4: Run test**

Run: `cargo nextest run -p scheduling cron_job_fire_triggers_next_schedule`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/src/temporal/scheduler.rs
git commit -m "feat(scheduling): scheduler advances cron jobs via bridge on fire"
```

---

## Task 13: Wire `TemporalScheduler` into app-core; subscribe to SystemDidWake

**Files:**
- Modify: `crates/app-core/src/init/` (replace CronService init)
- Modify: `crates/app-core/Cargo.toml` if scheduling dep not already present

- [ ] **Step 1: Find the current CronService init site**

Run: `grep -rn "CronService::start\|CronService::new" crates/app-core/`
Expected: one or two sites in `crates/app-core/src/init/*.rs`.

- [ ] **Step 2: Replace the init**

Replace the `CronService` construction with:

```rust
use scheduling::temporal::{TemporalScheduler, SchedulerConfig};
use scheduling::temporal::fire_store::FireStore;
use scheduling::temporal::cron_bridge::CronBridge;

let fire_store = FireStore::new(repos.scheduled_fires.clone());
let cron_bridge = CronBridge::new(repos.cron.clone(), fire_store.clone());
cron_bridge.reconcile_all().await?;
let scheduler = TemporalScheduler::new(fire_store, bus.clone(), SchedulerConfig::default())
    .with_cron_bridge(cron_bridge);

// Subscribe to SystemDidWake to wake the loop immediately.
{
    let sched = scheduler.clone();
    let mut rx = bus.subscribe();
    tokio::spawn(async move {
        while let Ok(event) = rx.recv().await {
            if matches!(event, bus::domain_events::DomainEvent::SystemDidWake { .. }) {
                sched.wake();
            }
        }
    });
}

let scheduler_handle = scheduler.start_background();
```

Store `scheduler_handle` somewhere that outlives `AppCore` (same pattern as `MirrorEngine` handles per CLAUDE.md).

- [ ] **Step 3: Verify compilation**

Run: `cargo build --workspace`
Expected: clean build.

- [ ] **Step 4: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/ crates/app-core/Cargo.toml
git commit -m "feat(app-core): wire TemporalScheduler, subscribe to SystemDidWake"
```

---

## Task 14: Delete old `CronService`

**Files:**
- Delete: `crates/scheduling/src/service/` (entire directory)
- Modify: `crates/scheduling/src/lib.rs`
- Modify: any remaining caller — should now be none after Task 13

- [ ] **Step 1: Verify no remaining callers**

Run: `grep -rn "CronService\|JobCallback\|MissedJobClass\|PresenceSnapshot\|crate::service" --include="*.rs" crates/`
Expected: only matches are inside `crates/scheduling/src/service/` itself plus the `pub use` in `lib.rs`.

If other crates still import `CronService`, stop and migrate those call sites before proceeding.

- [ ] **Step 2: Delete the service module**

```bash
rm -r crates/scheduling/src/service/
```

Edit `crates/scheduling/src/lib.rs` to remove these lines:

```rust
pub mod service;
pub use service::{
    classify_missed_job, evaluate_trigger, CronService, JobCallback, MissedJobClass,
    PresenceSnapshot,
};
```

Also remove `pub mod deadline;`, `pub use deadline::...`, `pub mod deadline_actions;`, and `pub use deadline_actions::...` ONLY if no callers remain (run the grep again to check). Deadline is consumed by `feature-tasks`; if still consumed, leave it for Phase 3.

- [ ] **Step 3: Build & test**

Run: `cargo build --workspace && cargo nextest run --workspace && cargo clippy --workspace --all-targets -- -D warnings`
Expected: clean.

- [ ] **Step 4: Commit**

```bash
git add -u crates/scheduling/src/
git commit -m "refactor(scheduling): delete legacy CronService (superseded by TemporalScheduler)"
```

---

## Task 15: End-to-end integration test through facade

**Files:**
- Create: `tests/integration/temporal_scheduler.rs`
- Modify: `tests/integration/main.rs` (if module list exists there)

- [ ] **Step 1: Write test**

```rust
//! E2E: schedule a fire via the facade-exposed TemporalScheduler, observe AlarmFired.

use klyntbot::bus::domain_events::{DomainEvent, DomainEventBus};
use klyntbot::scheduling::temporal::{FireSpec, FireStore, SchedulerConfig, TemporalScheduler};
use std::sync::Arc;
use std::time::Duration;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn end_to_end_fires_and_emits_event() {
    let pool = klyntbot::storage::pool::StoragePool::connect_in_memory().await.unwrap();
    klyntbot::storage::run_initial_migrations(pool.inner()).await.unwrap();
    klyntbot::storage::pool::StoragePool::run_feature_migrations(
        pool.inner(),
        &[tools_core::FeatureMigration {
            feature_name: "scheduling".into(), version: 1,
            description: "scheduled_fires".into(),
            sql: include_str!(
                "../../crates/scheduling/migrations/001_scheduled_fires.sql"
            ).into(),
        }],
    ).await.unwrap();

    let store = FireStore::new(
        klyntbot::storage::repos::scheduled_fires::ScheduledFiresRepo::new(pool.inner().clone())
    );
    let bus = Arc::new(DomainEventBus::new(32));
    let mut rx = bus.subscribe();
    let scheduler = TemporalScheduler::new(store.clone(), bus.clone(), SchedulerConfig::default());
    let _h = scheduler.clone().start_background();

    let fire_at = jiff::Timestamp::now()
        .checked_add(jiff::Span::new().milliseconds(100)).unwrap();
    store.schedule(FireSpec {
        fire_at, kind: "test".into(), ref_id: Some("x".into()),
        payload: serde_json::json!({ "m": "hi" }),
        dedup_prefix: None,
    }).await.unwrap();
    scheduler.wake();

    let ev = tokio::time::timeout(Duration::from_secs(3), rx.recv()).await.unwrap().unwrap();
    assert!(matches!(ev, DomainEvent::AlarmFired { .. }));
}
```

- [ ] **Step 2: Add to integration test module list**

Modify `tests/integration/main.rs` if it registers modules explicitly; otherwise ensure the file is picked up by the facade's test config.

- [ ] **Step 3: Run**

Run: `cargo nextest run --test integration temporal`
Expected: PASS.

- [ ] **Step 4: Full workspace verification**

Run sequentially:
```bash
cargo build --workspace
cargo nextest run --workspace
cargo test --workspace --doc
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```
All must be clean.

- [ ] **Step 5: Final commit**

```bash
git add tests/integration/temporal_scheduler.rs tests/integration/main.rs
git commit -m "test(integration): end-to-end TemporalScheduler via facade"
```

---

## Self-Review Summary

**Spec coverage (vs §2, §4, §5 of design):**
- §4.1 `scheduled_fires` table → Task 1 ✓
- §4.1 `task_alarms` + `task_recurrence_templates` → Task 10 ✓
- §4.2 cron_jobs bridge → Task 11, 12 ✓
- §5.1 wall-clock anchor loop → Task 7 ✓
- §5.2 two-phase fire commit → Tasks 2, 7 ✓
- §5.3 misfire policies → Tasks 5, 7 ✓
- §5.4 prefix cancellation → Task 2 ✓
- §3.1–§3.4 AlarmRule types & fire computation → Task 3 ✓
- §7 RRULE → Task 9 ✓ (RecurrenceEngine materialization deferred to Phase 4 — it needs `TaskTool` integration)
- §10 DomainEvents → Task 6 ✓ (`HeldNotificationReleased`, `NotificationDeliveryFailed` are Phase 3)
- §9.4 chrono removal from `scheduling` crate → mostly done, chrono now *contained* in `rrule.rs` + `cron_bridge.rs` by necessity of the `rrule` and `cron` crate APIs.

**Deferred to Phase 3:** notifications crate, dispatcher, quiet hours, idempotency gate, held-release, ReminderEngine removal, DeadlineScheduler removal.
**Deferred to Phase 4:** TaskTool extensions, AlarmTool, RecurrenceEngine instance materialization.

**No placeholders; type names consistent across tasks** (`FireSpec`, `FireStore`, `TemporalScheduler`, `SchedulerConfig`, `CronBridge`, `AlarmRule`, `RRuleSpec`).
