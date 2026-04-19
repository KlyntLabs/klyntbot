# Phase 3 — Notifications Crate & Dispatcher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Carve a new L4 `notifications` crate that subscribes to `DomainEvent::AlarmFired` from Phase 2's `TemporalScheduler` and delivers notifications through multi-channel fan-out with quiet hours, an idempotency gate, held-notification release, and bounded retry.

**Architecture:** A single `NotificationDispatcher` service owns a `JoinHandle` that consumes `DomainEvent::AlarmFired` from the bus. For each event it (1) checks `QuietHoursPolicy` against the user's IANA timezone via `jiff::Zoned`, (2) if held, writes a `held_notifications` row and schedules a `scheduled_fires(kind='held_release')` row via Phase 2's `FireStore` to re-fire at the end of the window, (3) otherwise iterates the resolved channel set, gates each delivery via `INSERT OR IGNORE INTO notification_log (alarm_id, channel)`, and dispatches through a `Channel` trait. Channel adapters include `OsNativeChannel` (migrated from `common::notify::OsNotificationSender`), `TrayChannel` (emits bus event), and `OutboundChannel` (bridge to existing `OutboundMessage` mpsc for Telegram/Discord/Slack/Email). Retry is in-process with exponential backoff (1s/4s/16s); permanent failures emit `DomainEvent::NotificationDeliveryFailed`.

**Tech stack:** Rust 1.93, tokio, sqlx+SQLite, `jiff` (`Zoned`, `civil::Time`, `tz::TimeZone`), `bus::DomainEventBus`, Phase 2's `FireStore` + `ScheduledFiresRepo`. Tests via `cargo nextest`, `StoragePool::connect_in_memory()`, mock bus subscribers. No backward-compat shims — `crates/agent/src/services/notifications.rs` is deleted at the end.

**Non-goals in Phase 3:** TaskTool.alarms/recurrence subfields, standalone `AlarmTool`, instance materialization for recurrence templates, deletion of `ReminderEngine` (`crates/agent/src/services/reminders.rs`). Those are Phase 4.

**Spec reference:** `docs/superpowers/specs/2026-04-17-unified-temporal-scheduler-and-notifications-design.md` §3.2, §4.1 (notification_log, held_notifications), §6 (dispatcher), §10 (new events).

---

## File Structure

**New (notifications crate — L4):**
- `crates/notifications/Cargo.toml`
- `crates/notifications/src/lib.rs` — crate facade, re-exports.
- `crates/notifications/src/dispatcher.rs` — `NotificationDispatcher`, event-loop task.
- `crates/notifications/src/quiet_hours.rs` — `QuietHoursPolicy` + `is_in_quiet_hours()`.
- `crates/notifications/src/held.rs` — `HeldReleaseService`: write held rows, schedule/fire release.
- `crates/notifications/src/retry.rs` — exponential backoff helper (1s/4s/16s).
- `crates/notifications/src/channel/mod.rs` — `Channel` trait + `ChannelKind` enum + registry.
- `crates/notifications/src/channel/os_native.rs` — adapter around existing `NotificationSender`.
- `crates/notifications/src/channel/tray.rs` — emits `DomainEvent::TrayNotificationRequested` (new, narrow).
- `crates/notifications/src/channel/outbound.rs` — bridge to `mpsc::Sender<OutboundMessage>`.
- `crates/notifications/src/error.rs` — `NotificationError`.
- `crates/notifications/migrations/001_notification_tables.sql` — `notification_log` + `held_notifications`.
- `crates/notifications/src/migrations.rs` — `FeatureMigration` registration.

**New (storage crate):**
- `crates/storage/src/rows/notification_log.rs` — `NotificationLogRow`.
- `crates/storage/src/rows/held_notification.rs` — `HeldNotificationRow`.
- `crates/storage/src/repos/notification_log.rs` — `NotificationLogRepo` (INSERT OR IGNORE gate).
- `crates/storage/src/repos/held_notifications.rs` — `HeldNotificationsRepo`.

**Modified:**
- `Cargo.toml` — add `notifications = { path = "crates/notifications" }` to workspace members.
- `crates/storage/src/rows/mod.rs` — expose new row modules.
- `crates/storage/src/repos/mod.rs` — expose new repos; `Repos::from_pool` adds them.
- `crates/bus/src/domain_events.rs` — add `HeldNotificationReleased`, `NotificationDeliveryFailed`, `TrayNotificationRequested`.
- `crates/config/src/schema/mod.rs` — add `notifications` subtree: `quiet_hours`, `default_channels`, `default_misfire_policy`, `default_grace_window_secs`, `retry` config.
- `crates/app-core/src/init/mod.rs` — wire `NotificationDispatcher::start(...)` alongside `TemporalScheduler`.
- `crates/app-core/Cargo.toml` — add `notifications.workspace = true`.
- `crates/klyntbot/src/lib.rs` — re-export `notifications::NotificationDispatcher`.
- `crates/klyntbot/Cargo.toml` — add `notifications.workspace = true`.

**Deleted (end of phase):**
- `crates/agent/src/services/notifications.rs` — superseded. Call sites in `agent` that constructed the old dispatcher are moved to consume the new dispatcher via injection from `app-core`.

---

## Task 1: Workspace + crate scaffold

**Files:**
- Create: `crates/notifications/Cargo.toml`
- Create: `crates/notifications/src/lib.rs`
- Create: `crates/notifications/src/error.rs`
- Modify: `Cargo.toml` (workspace members + `[workspace.dependencies]`)

- [ ] **Step 1: Add crate to workspace members**

In root `Cargo.toml`, under `[workspace] members = [...]`, add `"crates/notifications"` in alphabetical position (after `"crates/mcp"`).

Under `[workspace.dependencies]`, add:

```toml
notifications = { path = "crates/notifications" }
```

- [ ] **Step 2: Create `crates/notifications/Cargo.toml`**

```toml
[package]
name = "notifications"
version = "0.1.0"
edition = "2021"

[dependencies]
common = { workspace = true }
config = { workspace = true }
bus = { workspace = true }
storage = { workspace = true }
scheduling = { workspace = true }
tools-core = { workspace = true }
tokio = { workspace = true, features = ["sync", "rt", "macros", "time"] }
tracing = { workspace = true }
thiserror = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
jiff = { workspace = true }
uuid = { workspace = true, features = ["v4"] }

[dev-dependencies]
tokio = { workspace = true, features = ["test-util"] }
```

- [ ] **Step 3: Create `src/error.rs`**

```rust
//! Error type for the notifications crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("storage error: {0}")]
    Storage(#[from] common::KlyntbotError),
    #[error("scheduler error: {0}")]
    Scheduler(#[from] scheduling::SchedulerError),
    #[error("channel delivery failed: channel={channel} reason={reason}")]
    Delivery { channel: String, reason: String },
    #[error("invalid quiet hours configuration: {0}")]
    InvalidConfig(String),
    #[error("jiff error: {0}")]
    Jiff(String),
}

impl From<jiff::Error> for NotificationError {
    fn from(e: jiff::Error) -> Self {
        NotificationError::Jiff(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, NotificationError>;
```

- [ ] **Step 4: Create `src/lib.rs`**

```rust
//! L4 notifications crate — quiet-hours-aware, multi-channel dispatcher
//! subscribing to `DomainEvent::AlarmFired` from the `TemporalScheduler`.

pub mod channel;
pub mod dispatcher;
pub mod error;
pub mod held;
pub mod migrations;
pub mod quiet_hours;
pub mod retry;

pub use dispatcher::{NotificationDispatcher, NotificationDispatcherHandle};
pub use error::{NotificationError, Result};
```

Create placeholder files so compilation succeeds:

`src/channel/mod.rs`:
```rust
//! Channel trait + adapters. Filled in Task 7.
```

`src/dispatcher.rs`:
```rust
//! Filled in Task 8.
pub struct NotificationDispatcher;
pub struct NotificationDispatcherHandle;
```

`src/held.rs`:
```rust
//! Filled in Task 9.
```

`src/migrations.rs`:
```rust
//! Filled in Task 5.
```

`src/quiet_hours.rs`:
```rust
//! Filled in Task 6.
```

`src/retry.rs`:
```rust
//! Filled in Task 10.
```

- [ ] **Step 5: Verify workspace builds**

Run: `cargo build -p notifications`
Expected: PASS (empty crate compiles).

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml crates/notifications/
git commit -m "feat(notifications): scaffold L4 crate"
```

---

## Task 2: `notification_log` row + repo (idempotency gate)

**Files:**
- Create: `crates/storage/src/rows/notification_log.rs`
- Create: `crates/storage/src/repos/notification_log.rs`
- Modify: `crates/storage/src/rows/mod.rs`
- Modify: `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Write failing row serialization test**

Append to `crates/storage/src/rows/serialization_tests.rs`:

```rust
#[test]
fn notification_log_row_round_trips() {
    use crate::rows::notification_log::NotificationLogRow;
    let row = NotificationLogRow {
        alarm_id: "fire_abc".into(),
        channel: "os_native".into(),
        sent_at_ms: 1_800_000_000_000,
        ack_at_ms: None,
        error: None,
    };
    let s = serde_json::to_string(&row).unwrap();
    let parsed: NotificationLogRow = serde_json::from_str(&s).unwrap();
    assert_eq!(parsed.alarm_id, "fire_abc");
    assert_eq!(parsed.channel, "os_native");
}
```

- [ ] **Step 2: Run test — expect fail**

Run: `cargo nextest run -p storage notification_log_row_round_trips`
Expected: FAIL (module doesn't exist).

- [ ] **Step 3: Create row type**

`crates/storage/src/rows/notification_log.rs`:

```rust
//! Row for the `notification_log` idempotency-gate table.
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationLogRow {
    pub alarm_id: String,
    pub channel: String,
    pub sent_at_ms: i64,
    pub ack_at_ms: Option<i64>,
    pub error: Option<String>,
}
```

Add to `crates/storage/src/rows/mod.rs`:

```rust
pub mod notification_log;
```

- [ ] **Step 4: Run test — expect pass**

Run: `cargo nextest run -p storage notification_log_row_round_trips`
Expected: PASS.

- [ ] **Step 5: Write failing repo test**

Create `crates/storage/src/repos/tests/notification_log_tests.rs`:

```rust
use crate::pool::StoragePool;
use crate::repos::notification_log::NotificationLogRepo;

async fn setup() -> NotificationLogRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    // Apply notifications migrations via FeatureMigration registration.
    crate::test_util::run_notifications_migrations(pool.inner()).await;
    NotificationLogRepo::new(pool.clone())
}

#[tokio::test]
async fn insert_or_ignore_gates_duplicate_deliveries() {
    let repo = setup().await;
    let inserted1 = repo.try_insert("fire_1", "os_native", 100).await.unwrap();
    let inserted2 = repo.try_insert("fire_1", "os_native", 200).await.unwrap();
    assert!(inserted1, "first insert must succeed");
    assert!(!inserted2, "duplicate must be ignored");
}

#[tokio::test]
async fn per_channel_rows_independent() {
    let repo = setup().await;
    assert!(repo.try_insert("fire_1", "os_native", 100).await.unwrap());
    assert!(repo.try_insert("fire_1", "tray",      100).await.unwrap());
    assert!(repo.try_insert("fire_1", "telegram",  100).await.unwrap());
}

#[tokio::test]
async fn record_error_updates_existing_row() {
    let repo = setup().await;
    repo.try_insert("fire_1", "telegram", 100).await.unwrap();
    repo.record_error("fire_1", "telegram", "rate limited").await.unwrap();
    let row = repo.get("fire_1", "telegram").await.unwrap().unwrap();
    assert_eq!(row.error.as_deref(), Some("rate limited"));
}
```

Register in `crates/storage/src/repos/tests/mod.rs`:

```rust
pub mod notification_log_tests;
```

Add a tiny test helper `crates/storage/src/test_util.rs` (create if missing):

```rust
//! Test-only helpers for migrations.
#[cfg(test)]
pub async fn run_notifications_migrations(pool: &sqlx::SqlitePool) {
    // Inline the SQL; avoids cyclic dep on notifications crate.
    sqlx::query(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../notifications/migrations/001_notification_tables.sql"
    )))
    .execute(pool)
    .await
    .unwrap();
}
```

Register in `crates/storage/src/lib.rs`:

```rust
#[cfg(test)]
pub mod test_util;
```

- [ ] **Step 6: Run tests — expect fail**

Run: `cargo nextest run -p storage notification_log`
Expected: FAIL (`NotificationLogRepo` missing, migration file missing).

- [ ] **Step 7: Create the migration**

`crates/notifications/migrations/001_notification_tables.sql`:

```sql
-- Idempotency gate: one row per (alarm, channel) delivery.
CREATE TABLE notification_log (
    alarm_id TEXT NOT NULL,
    channel TEXT NOT NULL,
    sent_at_ms INTEGER NOT NULL,
    ack_at_ms INTEGER,
    error TEXT,
    PRIMARY KEY (alarm_id, channel)
);
CREATE INDEX idx_notification_log_sent ON notification_log(sent_at_ms);

-- Quiet-hours-held notifications awaiting release.
CREATE TABLE held_notifications (
    id TEXT PRIMARY KEY,
    alarm_id TEXT NOT NULL,
    channels TEXT NOT NULL,       -- JSON array of channel names
    payload TEXT NOT NULL,        -- JSON {title, body, priority, ...}
    release_at_ms INTEGER NOT NULL,
    released INTEGER NOT NULL DEFAULT 0,
    held_at_ms INTEGER NOT NULL
);
CREATE INDEX idx_held_notifications_pending
    ON held_notifications(release_at_ms) WHERE released = 0;
```

- [ ] **Step 8: Create the repo**

`crates/storage/src/repos/notification_log.rs`:

```rust
//! Idempotency gate for notification deliveries.
use crate::pool::StoragePool;
use crate::rows::notification_log::NotificationLogRow;
use common::{KlyntbotError, Result};

#[derive(Clone)]
pub struct NotificationLogRepo {
    pool: StoragePool,
}

impl NotificationLogRepo {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    /// Returns `true` if the row was newly inserted, `false` if a duplicate
    /// (alarm_id, channel) pair already existed.
    pub async fn try_insert(&self, alarm_id: &str, channel: &str, sent_at_ms: i64)
        -> Result<bool>
    {
        let res = sqlx::query(
            "INSERT OR IGNORE INTO notification_log (alarm_id, channel, sent_at_ms) \
             VALUES (?, ?, ?)"
        )
        .bind(alarm_id).bind(channel).bind(sent_at_ms)
        .execute(self.pool.inner()).await
        .map_err(|e| KlyntbotError::storage(format!("notification_log insert: {e}")))?;
        Ok(res.rows_affected() == 1)
    }

    pub async fn record_error(&self, alarm_id: &str, channel: &str, error: &str) -> Result<()> {
        sqlx::query(
            "UPDATE notification_log SET error = ? WHERE alarm_id = ? AND channel = ?"
        )
        .bind(error).bind(alarm_id).bind(channel)
        .execute(self.pool.inner()).await
        .map_err(|e| KlyntbotError::storage(format!("notification_log update: {e}")))?;
        Ok(())
    }

    pub async fn record_ack(&self, alarm_id: &str, channel: &str, ack_at_ms: i64) -> Result<()> {
        sqlx::query(
            "UPDATE notification_log SET ack_at_ms = ? WHERE alarm_id = ? AND channel = ?"
        )
        .bind(ack_at_ms).bind(alarm_id).bind(channel)
        .execute(self.pool.inner()).await
        .map_err(|e| KlyntbotError::storage(format!("notification_log ack: {e}")))?;
        Ok(())
    }

    pub async fn get(&self, alarm_id: &str, channel: &str) -> Result<Option<NotificationLogRow>> {
        let row = sqlx::query_as::<_, NotificationLogRow>(
            "SELECT alarm_id, channel, sent_at_ms, ack_at_ms, error \
             FROM notification_log WHERE alarm_id = ? AND channel = ?"
        )
        .bind(alarm_id).bind(channel)
        .fetch_optional(self.pool.inner()).await
        .map_err(|e| KlyntbotError::storage(format!("notification_log get: {e}")))?;
        Ok(row)
    }
}
```

Add to `crates/storage/src/repos/mod.rs`:

```rust
pub mod notification_log;
```

And in `Repos::from_pool` add:

```rust
notification_log: notification_log::NotificationLogRepo::new(pool.clone()),
```

(with corresponding field on the `Repos` struct).

- [ ] **Step 9: Run repo tests — expect pass**

Run: `cargo nextest run -p storage notification_log`
Expected: 3 tests PASS.

- [ ] **Step 10: Commit**

```bash
git add crates/storage/ crates/notifications/migrations/
git commit -m "feat(notifications): notification_log table + idempotency-gate repo"
```

---

## Task 3: `held_notifications` row + repo

**Files:**
- Create: `crates/storage/src/rows/held_notification.rs`
- Create: `crates/storage/src/repos/held_notifications.rs`
- Modify: `crates/storage/src/rows/mod.rs`, `crates/storage/src/repos/mod.rs`

- [ ] **Step 1: Write failing repo tests**

`crates/storage/src/repos/tests/held_notifications_tests.rs`:

```rust
use crate::pool::StoragePool;
use crate::repos::held_notifications::HeldNotificationsRepo;
use crate::rows::held_notification::HeldNotificationRow;

async fn setup() -> HeldNotificationsRepo {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    crate::test_util::run_notifications_migrations(pool.inner()).await;
    HeldNotificationsRepo::new(pool.clone())
}

fn sample(id: &str, release: i64) -> HeldNotificationRow {
    HeldNotificationRow {
        id: id.into(),
        alarm_id: "fire_1".into(),
        channels: serde_json::json!(["telegram", "discord"]),
        payload: serde_json::json!({"title": "t", "body": "b"}),
        release_at_ms: release,
        released: false,
        held_at_ms: release - 1000,
    }
}

#[tokio::test]
async fn insert_and_list_pending_before_time() {
    let repo = setup().await;
    repo.insert(&sample("h1", 100)).await.unwrap();
    repo.insert(&sample("h2", 300)).await.unwrap();
    let pending = repo.list_pending_before(200).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].id, "h1");
}

#[tokio::test]
async fn mark_released_hides_from_pending() {
    let repo = setup().await;
    repo.insert(&sample("h1", 100)).await.unwrap();
    repo.mark_released("h1").await.unwrap();
    assert!(repo.list_pending_before(999).await.unwrap().is_empty());
}
```

Register in `tests/mod.rs`.

- [ ] **Step 2: Run — expect fail**

Run: `cargo nextest run -p storage held_notifications`
Expected: FAIL.

- [ ] **Step 3: Create row type**

`crates/storage/src/rows/held_notification.rs`:

```rust
//! Row for `held_notifications` (quiet-hours-suppressed deliveries).
use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HeldNotificationRow {
    pub id: String,
    pub alarm_id: String,
    pub channels: serde_json::Value,
    pub payload: serde_json::Value,
    pub release_at_ms: i64,
    pub released: bool,
    pub held_at_ms: i64,
}
```

Register in `rows/mod.rs`.

- [ ] **Step 4: Create repo**

`crates/storage/src/repos/held_notifications.rs`:

```rust
use crate::pool::StoragePool;
use crate::rows::held_notification::HeldNotificationRow;
use common::{KlyntbotError, Result};

#[derive(Clone)]
pub struct HeldNotificationsRepo {
    pool: StoragePool,
}

impl HeldNotificationsRepo {
    pub fn new(pool: StoragePool) -> Self { Self { pool } }

    pub async fn insert(&self, row: &HeldNotificationRow) -> Result<()> {
        let channels = row.channels.to_string();
        let payload = row.payload.to_string();
        sqlx::query(
            "INSERT INTO held_notifications \
              (id, alarm_id, channels, payload, release_at_ms, released, held_at_ms) \
             VALUES (?, ?, ?, ?, ?, ?, ?)"
        )
        .bind(&row.id).bind(&row.alarm_id).bind(channels).bind(payload)
        .bind(row.release_at_ms).bind(row.released as i64).bind(row.held_at_ms)
        .execute(self.pool.inner()).await
        .map_err(|e| KlyntbotError::storage(format!("held insert: {e}")))?;
        Ok(())
    }

    pub async fn list_pending_before(&self, ts_ms: i64) -> Result<Vec<HeldNotificationRow>> {
        let rows = sqlx::query_as::<_, HeldNotificationRow>(
            "SELECT id, alarm_id, channels, payload, release_at_ms, \
                    released, held_at_ms \
             FROM held_notifications \
             WHERE released = 0 AND release_at_ms <= ? \
             ORDER BY release_at_ms ASC"
        )
        .bind(ts_ms)
        .fetch_all(self.pool.inner()).await
        .map_err(|e| KlyntbotError::storage(format!("held list: {e}")))?;
        Ok(rows)
    }

    pub async fn mark_released(&self, id: &str) -> Result<()> {
        sqlx::query("UPDATE held_notifications SET released = 1 WHERE id = ?")
            .bind(id)
            .execute(self.pool.inner()).await
            .map_err(|e| KlyntbotError::storage(format!("held mark: {e}")))?;
        Ok(())
    }

    pub async fn next_release_at_ms(&self) -> Result<Option<i64>> {
        let row: Option<(i64,)> = sqlx::query_as(
            "SELECT release_at_ms FROM held_notifications \
             WHERE released = 0 ORDER BY release_at_ms ASC LIMIT 1"
        )
        .fetch_optional(self.pool.inner()).await
        .map_err(|e| KlyntbotError::storage(format!("held next: {e}")))?;
        Ok(row.map(|(v,)| v))
    }
}
```

Register in `repos/mod.rs` and add to `Repos::from_pool`.

- [ ] **Step 5: Run tests — expect pass**

Run: `cargo nextest run -p storage held_notifications`
Expected: 2 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/storage/
git commit -m "feat(notifications): held_notifications row + repo"
```

---

## Task 4: Domain events

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

- [ ] **Step 1: Write failing test**

Append to `crates/bus/src/domain_events.rs` tests:

```rust
#[test]
fn held_notification_released_event_round_trips() {
    use super::DomainEvent;
    let e = DomainEvent::HeldNotificationReleased {
        held_id: "h1".into(),
        alarm_id: "fire_1".into(),
        channels: vec!["telegram".into()],
    };
    let s = serde_json::to_string(&e).unwrap();
    let parsed: DomainEvent = serde_json::from_str(&s).unwrap();
    assert!(matches!(parsed, DomainEvent::HeldNotificationReleased { .. }));
}

#[test]
fn notification_delivery_failed_event_round_trips() {
    use super::DomainEvent;
    let e = DomainEvent::NotificationDeliveryFailed {
        alarm_id: "fire_1".into(),
        channel: "discord".into(),
        error: "500".into(),
        attempts: 3,
    };
    let s = serde_json::to_string(&e).unwrap();
    let parsed: DomainEvent = serde_json::from_str(&s).unwrap();
    assert!(matches!(parsed, DomainEvent::NotificationDeliveryFailed { .. }));
}

#[test]
fn tray_notification_requested_event_round_trips() {
    use super::DomainEvent;
    let e = DomainEvent::TrayNotificationRequested {
        title: "ping".into(),
        body: "hello".into(),
        alarm_id: Some("fire_1".into()),
    };
    let s = serde_json::to_string(&e).unwrap();
    assert!(matches!(serde_json::from_str::<DomainEvent>(&s).unwrap(),
                      DomainEvent::TrayNotificationRequested { .. }));
}
```

- [ ] **Step 2: Run — expect fail**

Run: `cargo nextest run -p bus held_notification`
Expected: FAIL.

- [ ] **Step 3: Add variants**

In `crates/bus/src/domain_events.rs`, add to the `DomainEvent` enum:

```rust
HeldNotificationReleased {
    held_id: String,
    alarm_id: String,
    channels: Vec<String>,
},

NotificationDeliveryFailed {
    alarm_id: String,
    channel: String,
    error: String,
    attempts: u32,
},

TrayNotificationRequested {
    title: String,
    body: String,
    alarm_id: Option<String>,
},
```

- [ ] **Step 4: Run — expect pass**

Run: `cargo nextest run -p bus`
Expected: all tests PASS including 3 new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/bus/
git commit -m "feat(bus): add held/delivery-failed/tray notification events"
```

---

## Task 5: Notifications feature migration registration

**Files:**
- Create: `crates/notifications/src/migrations.rs`
- Modify: `crates/notifications/src/lib.rs` (already registered in Task 1)

- [ ] **Step 1: Implement FeatureMigration**

`crates/notifications/src/migrations.rs`:

```rust
//! Registers the notifications-crate SQL migrations with the storage pool.
use tools_core::FeatureMigration;

pub fn migration() -> FeatureMigration {
    FeatureMigration::new(
        "notifications",
        1,
        include_str!("../migrations/001_notification_tables.sql"),
    )
}
```

- [ ] **Step 2: Expose from lib.rs**

Add to `crates/notifications/src/lib.rs`:

```rust
pub use migrations::migration;
```

- [ ] **Step 3: Write registration test**

Append to `crates/notifications/src/migrations.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use storage::StoragePool;

    #[tokio::test]
    async fn migration_applies_cleanly() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let mig = migration();
        sqlx::query(mig.sql()).execute(pool.inner()).await.unwrap();
        // Verify tables exist
        let tables: Vec<(String,)> = sqlx::query_as(
            "SELECT name FROM sqlite_master WHERE type='table' \
             AND name IN ('notification_log','held_notifications')"
        )
        .fetch_all(pool.inner()).await.unwrap();
        assert_eq!(tables.len(), 2);
    }
}
```

- [ ] **Step 4: Run**

Run: `cargo nextest run -p notifications migration_applies_cleanly`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/notifications/src/lib.rs crates/notifications/src/migrations.rs
git commit -m "feat(notifications): FeatureMigration registration"
```

---

## Task 6: `QuietHoursPolicy` — Jiff-based evaluation

**Files:**
- Modify: `crates/notifications/src/quiet_hours.rs`
- Modify: `crates/config/src/schema/mod.rs` (+ or file matching the schema module layout)

- [ ] **Step 1: Add config schema**

In the config crate, find the top-level schema struct (likely `crates/config/src/schema/mod.rs`). Add a `NotificationsConfig` type and a `notifications: NotificationsConfig` field on `Config`:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct NotificationsConfig {
    #[serde(default)]
    pub quiet_hours: QuietHoursConfig,
    #[serde(default = "default_channels")]
    pub default_channels: Vec<String>,
    #[serde(default = "default_misfire_policy")]
    pub default_misfire_policy: String,    // "skip_if_stale" | "strict" | "coalesce"
    #[serde(default = "default_grace_window_secs")]
    pub default_grace_window_secs: i64,
    #[serde(default)]
    pub retry: RetryConfig,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuietHoursConfig {
    pub enabled: bool,
    pub start: String,            // "HH:MM"
    pub end: String,              // "HH:MM"
    #[serde(default = "default_true")]
    pub override_for_urgent_tasks: bool,
}

impl Default for QuietHoursConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            start: "22:00".into(),
            end: "07:00".into(),
            override_for_urgent_tasks: true,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RetryConfig {
    pub max_attempts: u32,
    pub base_delay_secs: u64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self { max_attempts: 3, base_delay_secs: 1 }
    }
}

fn default_true() -> bool { true }
fn default_channels() -> Vec<String> { vec!["os_native".into(), "tray".into()] }
fn default_misfire_policy() -> String { "skip_if_stale".into() }
fn default_grace_window_secs() -> i64 { 3600 }
```

Add to the top-level `Config` struct (camelCase field):

```rust
#[serde(default)]
pub notifications: NotificationsConfig,
```

- [ ] **Step 2: Write failing quiet-hours test**

`crates/notifications/src/quiet_hours.rs`:

```rust
//! Quiet-hours evaluation against a user's IANA timezone using Jiff.
use config::schema::QuietHoursConfig;
use jiff::{civil::Time, tz::TimeZone, Timestamp, Zoned};

use crate::error::{NotificationError, Result};

pub struct QuietHoursPolicy {
    cfg: QuietHoursConfig,
    tz: TimeZone,
}

impl QuietHoursPolicy {
    pub fn new(cfg: QuietHoursConfig, iana_tz: &str) -> Result<Self> {
        let tz = TimeZone::get(iana_tz)
            .map_err(|e| NotificationError::InvalidConfig(format!("tz {iana_tz}: {e}")))?;
        Ok(Self { cfg, tz })
    }

    /// Returns true iff the given instant is within the configured quiet window
    /// in the policy's timezone. Handles overnight windows (start > end).
    pub fn is_in_quiet_hours(&self, at: Timestamp) -> Result<bool> {
        if !self.cfg.enabled { return Ok(false); }
        let start = parse_hhmm(&self.cfg.start)?;
        let end = parse_hhmm(&self.cfg.end)?;
        let zoned: Zoned = at.to_zoned(self.tz.clone());
        let now = zoned.time();
        if start <= end {
            Ok(now >= start && now < end)
        } else {
            // overnight window (e.g. 22:00 → 07:00)
            Ok(now >= start || now < end)
        }
    }

    /// The next instant when the quiet window *ends* on/after `at`.
    /// Used to schedule held-release alarms.
    pub fn next_window_end(&self, at: Timestamp) -> Result<Timestamp> {
        let end = parse_hhmm(&self.cfg.end)?;
        let zoned: Zoned = at.to_zoned(self.tz.clone());
        let today_end = zoned
            .date()
            .at(end.hour(), end.minute(), 0, 0)
            .to_zoned(self.tz.clone())?;
        let candidate = if today_end.timestamp() > at {
            today_end
        } else {
            zoned.date().tomorrow()?
                .at(end.hour(), end.minute(), 0, 0)
                .to_zoned(self.tz.clone())?
        };
        Ok(candidate.timestamp())
    }

    pub fn override_for_urgent(&self) -> bool { self.cfg.override_for_urgent_tasks }
    pub fn enabled(&self) -> bool { self.cfg.enabled }
}

fn parse_hhmm(s: &str) -> Result<Time> {
    let (h, m) = s.split_once(':')
        .ok_or_else(|| NotificationError::InvalidConfig(format!("bad HH:MM {s}")))?;
    let h: i8 = h.parse().map_err(|_| NotificationError::InvalidConfig(format!("hour {s}")))?;
    let m: i8 = m.parse().map_err(|_| NotificationError::InvalidConfig(format!("min {s}")))?;
    Time::new(h, m, 0, 0)
        .map_err(|e| NotificationError::InvalidConfig(format!("{s}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(enabled: bool, start: &str, end: &str) -> QuietHoursConfig {
        QuietHoursConfig {
            enabled, start: start.into(), end: end.into(),
            override_for_urgent_tasks: true,
        }
    }

    fn ts(iso: &str) -> Timestamp { iso.parse().unwrap() }

    #[test]
    fn disabled_always_false() {
        let p = QuietHoursPolicy::new(cfg(false, "22:00", "07:00"), "UTC").unwrap();
        assert!(!p.is_in_quiet_hours(ts("2026-01-01T23:00:00Z")).unwrap());
    }

    #[test]
    fn overnight_window_midnight_inside() {
        let p = QuietHoursPolicy::new(cfg(true, "22:00", "07:00"), "UTC").unwrap();
        assert!(p.is_in_quiet_hours(ts("2026-01-01T23:30:00Z")).unwrap());
        assert!(p.is_in_quiet_hours(ts("2026-01-01T03:00:00Z")).unwrap());
    }

    #[test]
    fn overnight_window_midday_outside() {
        let p = QuietHoursPolicy::new(cfg(true, "22:00", "07:00"), "UTC").unwrap();
        assert!(!p.is_in_quiet_hours(ts("2026-01-01T12:00:00Z")).unwrap());
    }

    #[test]
    fn daytime_window_inside_outside() {
        let p = QuietHoursPolicy::new(cfg(true, "09:00", "17:00"), "UTC").unwrap();
        assert!(p.is_in_quiet_hours(ts("2026-01-01T10:00:00Z")).unwrap());
        assert!(!p.is_in_quiet_hours(ts("2026-01-01T20:00:00Z")).unwrap());
    }

    #[test]
    fn tz_shifts_boundary() {
        // 09:00 local in NY = 14:00 UTC in winter (EST = UTC-5)
        let p = QuietHoursPolicy::new(
            cfg(true, "09:00", "17:00"),
            "America/New_York",
        ).unwrap();
        assert!(p.is_in_quiet_hours(ts("2026-01-01T15:00:00Z")).unwrap());
        assert!(!p.is_in_quiet_hours(ts("2026-01-01T20:00:00Z")).unwrap());
    }

    #[test]
    fn next_window_end_overnight() {
        let p = QuietHoursPolicy::new(cfg(true, "22:00", "07:00"), "UTC").unwrap();
        // At 23:30 UTC, next end = 07:00 the next day
        let end = p.next_window_end(ts("2026-01-01T23:30:00Z")).unwrap();
        assert_eq!(end.to_string(), "2026-01-02T07:00:00Z");
    }
}
```

- [ ] **Step 3: Run tests — expect pass**

Run: `cargo nextest run -p notifications quiet_hours`
Expected: 6 tests PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/config/ crates/notifications/src/quiet_hours.rs
git commit -m "feat(notifications): QuietHoursPolicy + config schema"
```

---

## Task 7: Channel trait + three adapters

**Files:**
- Create: `crates/notifications/src/channel/mod.rs` (rewrite)
- Create: `crates/notifications/src/channel/os_native.rs`
- Create: `crates/notifications/src/channel/tray.rs`
- Create: `crates/notifications/src/channel/outbound.rs`

- [ ] **Step 1: Define the trait and registry**

`crates/notifications/src/channel/mod.rs`:

```rust
//! Channel trait + concrete adapters for notification fan-out.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::Result;

pub mod os_native;
pub mod outbound;
pub mod tray;

#[derive(Debug, Clone)]
pub struct NotificationPayload {
    pub alarm_id: String,
    pub title: String,
    pub body: String,
    pub priority: Priority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Priority { Normal, Urgent }

#[async_trait]
pub trait Channel: Send + Sync {
    fn name(&self) -> &str;
    async fn deliver(&self, payload: &NotificationPayload) -> Result<()>;
}

#[derive(Clone, Default)]
pub struct ChannelRegistry {
    channels: HashMap<String, Arc<dyn Channel>>,
}

impl ChannelRegistry {
    pub fn new() -> Self { Self::default() }

    pub fn register(&mut self, ch: Arc<dyn Channel>) {
        self.channels.insert(ch.name().to_string(), ch);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Channel>> {
        self.channels.get(name).cloned()
    }

    pub fn names(&self) -> Vec<String> { self.channels.keys().cloned().collect() }
}
```

- [ ] **Step 2: OS native adapter**

`crates/notifications/src/channel/os_native.rs`:

```rust
//! OS-native notification adapter. Wraps `common::NotificationSender`
//! (the same trait used today by the legacy dispatcher).
use async_trait::async_trait;
use std::sync::Arc;

use common::NotificationSender;

use super::{Channel, NotificationPayload};
use crate::error::{NotificationError, Result};

pub struct OsNativeChannel {
    sender: Arc<dyn NotificationSender>,
}

impl OsNativeChannel {
    pub fn new(sender: Arc<dyn NotificationSender>) -> Self { Self { sender } }
}

#[async_trait]
impl Channel for OsNativeChannel {
    fn name(&self) -> &str { "os_native" }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        self.sender.send(&payload.title, &payload.body).await
            .map_err(|e| NotificationError::Delivery {
                channel: "os_native".into(),
                reason: e.to_string(),
            })
    }
}
```

- [ ] **Step 3: Tray adapter (emits bus event)**

`crates/notifications/src/channel/tray.rs`:

```rust
//! Tray channel — emits `TrayNotificationRequested` on the bus so the
//! desktop tray badge / banner can react without coupling to Tauri.
use async_trait::async_trait;
use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};

use super::{Channel, NotificationPayload};
use crate::error::Result;

pub struct TrayChannel { bus: Arc<DomainEventBus> }

impl TrayChannel {
    pub fn new(bus: Arc<DomainEventBus>) -> Self { Self { bus } }
}

#[async_trait]
impl Channel for TrayChannel {
    fn name(&self) -> &str { "tray" }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        self.bus.publish(DomainEvent::TrayNotificationRequested {
            title: payload.title.clone(),
            body: payload.body.clone(),
            alarm_id: Some(payload.alarm_id.clone()),
        }).await;
        Ok(())
    }
}
```

(If `DomainEventBus::publish` is not already `async` or returns a `Result`, match its real signature — check `crates/bus/src/bus.rs` and adjust.)

- [ ] **Step 4: Outbound adapter (Telegram/Discord/Slack/Email)**

`crates/notifications/src/channel/outbound.rs`:

```rust
//! Outbound adapter — sends through the existing `mpsc::Sender<OutboundMessage>`
//! to reach Telegram / Discord / Slack / Email. One adapter instance per
//! channel name; the adapter records the *last active* `(channel, chat_id)`
//! for its own channel name and sends only when they match.
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use bus::OutboundMessage;
use common::{ChannelName, ChatId};

use super::{Channel, NotificationPayload};
use crate::error::{NotificationError, Result};

pub struct OutboundChannel {
    channel_name: String,
    tx: mpsc::Sender<OutboundMessage>,
    last_active: Arc<RwLock<Option<(ChannelName, ChatId)>>>,
}

impl OutboundChannel {
    pub fn new(
        channel_name: impl Into<String>,
        tx: mpsc::Sender<OutboundMessage>,
        last_active: Arc<RwLock<Option<(ChannelName, ChatId)>>>,
    ) -> Self {
        Self { channel_name: channel_name.into(), tx, last_active }
    }
}

#[async_trait]
impl Channel for OutboundChannel {
    fn name(&self) -> &str { &self.channel_name }

    async fn deliver(&self, payload: &NotificationPayload) -> Result<()> {
        let (ch, chat_id) = {
            let guard = self.last_active.read().await;
            match &*guard {
                Some((c, id)) if c.as_str() == self.channel_name => (c.clone(), id.clone()),
                _ => return Ok(()), // no active chat on this channel → drop silently
            }
        };
        let msg = OutboundMessage::new(
            ch, chat_id,
            format!("{}\n\n{}", payload.title, payload.body),
        );
        self.tx.send(msg).await.map_err(|e| NotificationError::Delivery {
            channel: self.channel_name.clone(),
            reason: e.to_string(),
        })
    }
}
```

- [ ] **Step 5: Write adapter tests**

Append to `crates/notifications/src/channel/mod.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct MockChannel { name: String, count: std::sync::atomic::AtomicUsize }

    #[async_trait]
    impl Channel for MockChannel {
        fn name(&self) -> &str { &self.name }
        async fn deliver(&self, _p: &NotificationPayload) -> Result<()> {
            self.count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn registry_dispatches_by_name() {
        let mut reg = ChannelRegistry::new();
        let m = Arc::new(MockChannel {
            name: "mock".into(),
            count: std::sync::atomic::AtomicUsize::new(0),
        });
        reg.register(m.clone());
        let ch = reg.get("mock").unwrap();
        ch.deliver(&NotificationPayload {
            alarm_id: "x".into(), title: "t".into(), body: "b".into(),
            priority: Priority::Normal,
        }).await.unwrap();
        assert_eq!(m.count.load(std::sync::atomic::Ordering::SeqCst), 1);
    }
}
```

- [ ] **Step 6: Run**

Run: `cargo nextest run -p notifications channel`
Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/notifications/src/channel/
git commit -m "feat(notifications): channel trait + os_native/tray/outbound adapters"
```

---

## Task 8: `NotificationDispatcher` — subscribe to AlarmFired, idempotency gate, fan-out

**Files:**
- Rewrite: `crates/notifications/src/dispatcher.rs`

- [ ] **Step 1: Write the dispatcher**

```rust
//! Subscribes to `DomainEvent::AlarmFired` and dispatches via channel registry.

use std::sync::Arc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use storage::repos::held_notifications::HeldNotificationsRepo;
use storage::repos::notification_log::NotificationLogRepo;

use crate::channel::{ChannelRegistry, NotificationPayload, Priority};
use crate::error::Result;
use crate::held::HeldReleaseService;
use crate::quiet_hours::QuietHoursPolicy;
use crate::retry::RetryPolicy;

pub struct NotificationDispatcher {
    bus: Arc<DomainEventBus>,
    channels: ChannelRegistry,
    default_channels: Vec<String>,
    quiet_hours: Option<QuietHoursPolicy>,
    log_repo: NotificationLogRepo,
    held_repo: HeldNotificationsRepo,
    held_release: HeldReleaseService,
    retry: RetryPolicy,
}

pub struct NotificationDispatcherHandle {
    pub join: JoinHandle<()>,
    pub shutdown: CancellationToken,
}

impl NotificationDispatcher {
    pub fn new(
        bus: Arc<DomainEventBus>,
        channels: ChannelRegistry,
        default_channels: Vec<String>,
        quiet_hours: Option<QuietHoursPolicy>,
        log_repo: NotificationLogRepo,
        held_repo: HeldNotificationsRepo,
        held_release: HeldReleaseService,
        retry: RetryPolicy,
    ) -> Self {
        Self { bus, channels, default_channels, quiet_hours, log_repo,
               held_repo, held_release, retry }
    }

    /// Spawn the event-loop task. Returns the handle for graceful shutdown.
    pub fn start(self) -> NotificationDispatcherHandle {
        let shutdown = CancellationToken::new();
        let token = shutdown.clone();
        let mut rx = self.bus.subscribe();
        let svc = Arc::new(self);

        let join = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = token.cancelled() => {
                        info!("notification dispatcher shutting down");
                        break;
                    }
                    ev = rx.recv() => {
                        match ev {
                            Ok(DomainEvent::AlarmFired { alarm_id, task_id: _, kind, payload_json }) => {
                                if let Err(e) = svc.handle_alarm_fired(
                                    &alarm_id, &kind, &payload_json
                                ).await {
                                    warn!("dispatch failure for {alarm_id}: {e}");
                                }
                            }
                            Ok(DomainEvent::HeldNotificationReleased { .. }) => { /* observability */ }
                            Ok(_) => {}
                            Err(e) => {
                                warn!("bus recv error: {e}");
                                tokio::time::sleep(
                                    std::time::Duration::from_millis(100)
                                ).await;
                            }
                        }
                    }
                }
            }
        });

        NotificationDispatcherHandle { join, shutdown }
    }

    async fn handle_alarm_fired(&self, alarm_id: &str, kind: &str, payload_json: &str)
        -> Result<()>
    {
        let payload = parse_payload(alarm_id, payload_json);
        let channels = self.resolve_channels(&payload);

        // Quiet hours gate
        let now = Timestamp::now();
        if let Some(qh) = &self.quiet_hours {
            if qh.enabled()
                && qh.is_in_quiet_hours(now)?
                && !(payload.priority == Priority::Urgent && qh.override_for_urgent())
            {
                let release_at = qh.next_window_end(now)?;
                self.held_release.hold(
                    alarm_id, &channels, &payload, release_at
                ).await?;
                return Ok(());
            }
        }

        for channel_name in channels {
            self.dispatch_one(&channel_name, &payload).await;
        }
        debug!("dispatched alarm {alarm_id} kind={kind}");
        Ok(())
    }

    async fn dispatch_one(&self, channel_name: &str, payload: &NotificationPayload) {
        let now_ms = Timestamp::now().as_millisecond();
        let inserted = match self.log_repo.try_insert(
            &payload.alarm_id, channel_name, now_ms
        ).await {
            Ok(v) => v,
            Err(e) => { warn!("log insert failed: {e}"); return; }
        };
        if !inserted {
            debug!("duplicate suppressed alarm={} channel={}",
                   payload.alarm_id, channel_name);
            return;
        }

        let Some(ch) = self.channels.get(channel_name) else {
            warn!("unknown channel {channel_name}");
            return;
        };

        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match ch.deliver(payload).await {
                Ok(()) => {
                    let _ = self.log_repo.record_ack(
                        &payload.alarm_id, channel_name,
                        Timestamp::now().as_millisecond()
                    ).await;
                    return;
                }
                Err(e) if attempt < self.retry.max_attempts => {
                    let delay = self.retry.delay_for(attempt);
                    warn!(
                        "delivery attempt {attempt} failed for {}/{}: {e}; retrying in {:?}",
                        payload.alarm_id, channel_name, delay
                    );
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    let msg = e.to_string();
                    warn!("delivery permanently failed {}/{}: {msg}",
                          payload.alarm_id, channel_name);
                    let _ = self.log_repo.record_error(
                        &payload.alarm_id, channel_name, &msg
                    ).await;
                    self.bus.publish(DomainEvent::NotificationDeliveryFailed {
                        alarm_id: payload.alarm_id.clone(),
                        channel: channel_name.to_string(),
                        error: msg,
                        attempts: attempt,
                    }).await;
                    return;
                }
            }
        }
    }

    fn resolve_channels(&self, payload: &NotificationPayload) -> Vec<String> {
        // Payload may carry an explicit channel_mask; for now, honour defaults.
        // Payload-driven override lands when TaskTool.alarms is wired (Phase 4).
        let _ = payload;
        self.default_channels.clone()
    }
}

fn parse_payload(alarm_id: &str, payload_json: &str) -> NotificationPayload {
    let v: serde_json::Value = serde_json::from_str(payload_json)
        .unwrap_or(serde_json::Value::Null);
    let title = v.get("title").and_then(|x| x.as_str()).unwrap_or("Reminder").to_string();
    let body = v.get("body").or_else(|| v.get("message"))
        .and_then(|x| x.as_str()).unwrap_or("").to_string();
    let priority = match v.get("priority").and_then(|x| x.as_str()) {
        Some("urgent") => Priority::Urgent,
        _ => Priority::Normal,
    };
    NotificationPayload { alarm_id: alarm_id.into(), title, body, priority }
}
```

Add dep to `crates/notifications/Cargo.toml`:

```toml
tokio-util = { workspace = true }
```

- [ ] **Step 2: Add integration test**

Create `crates/notifications/tests/dispatcher_idempotency.rs`:

```rust
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

use bus::{DomainEvent, DomainEventBus, OutboundMessage};
use common::NotificationSender;
use notifications::{
    channel::{ChannelRegistry, os_native::OsNativeChannel},
    dispatcher::NotificationDispatcher,
    held::HeldReleaseService,
    retry::RetryPolicy,
};
use storage::StoragePool;
use storage::repos::held_notifications::HeldNotificationsRepo;
use storage::repos::notification_log::NotificationLogRepo;

struct CountingSender(Arc<std::sync::atomic::AtomicUsize>);

#[async_trait::async_trait]
impl NotificationSender for CountingSender {
    async fn send(&self, _title: &str, _body: &str) -> common::Result<()> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }
}

#[tokio::test]
async fn duplicate_alarm_fires_once_per_channel() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(notifications::migration().sql()).execute(pool.inner()).await.unwrap();
    // Phase 2 migration (scheduled_fires) also needed for HeldReleaseService
    sqlx::query(scheduling::migration().sql()).execute(pool.inner()).await.unwrap();

    let bus = Arc::new(DomainEventBus::new(64));
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let mut registry = ChannelRegistry::new();
    registry.register(Arc::new(OsNativeChannel::new(
        Arc::new(CountingSender(counter.clone()))
    )));

    let log = NotificationLogRepo::new(pool.clone());
    let held = HeldNotificationsRepo::new(pool.clone());
    let fire_store = scheduling::FireStore::new(pool.clone());
    let held_rel = HeldReleaseService::new(held.clone(), fire_store);

    let dispatcher = NotificationDispatcher::new(
        bus.clone(), registry, vec!["os_native".into()], None,
        log.clone(), held, held_rel, RetryPolicy::default(),
    );
    let handle = dispatcher.start();

    let payload = serde_json::json!({"title":"t","body":"b"}).to_string();
    for _ in 0..3 {
        bus.publish(DomainEvent::AlarmFired {
            alarm_id: "fire_1".into(),
            task_id: None,
            kind: "task_alarm".into(),
            payload_json: payload.clone(),
        }).await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    handle.shutdown.cancel();
    let _ = handle.join.await;

    assert_eq!(counter.load(std::sync::atomic::Ordering::SeqCst), 1,
               "idempotency gate must suppress duplicates");
}
```

- [ ] **Step 3: Run**

Run: `cargo nextest run -p notifications --test dispatcher_idempotency`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/notifications/
git commit -m "feat(notifications): dispatcher with idempotency gate + retry"
```

---

## Task 9: `HeldReleaseService` — hold writes + release scheduling

**Files:**
- Rewrite: `crates/notifications/src/held.rs`

- [ ] **Step 1: Write failing test first**

Create `crates/notifications/tests/held_release.rs`:

```rust
use std::sync::Arc;

use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use notifications::{
    channel::{ChannelRegistry, NotificationPayload, Priority},
    held::HeldReleaseService,
};
use storage::StoragePool;
use storage::repos::held_notifications::HeldNotificationsRepo;

#[tokio::test]
async fn hold_inserts_row_and_schedules_release_fire() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(notifications::migration().sql()).execute(pool.inner()).await.unwrap();
    sqlx::query(scheduling::migration().sql()).execute(pool.inner()).await.unwrap();

    let held_repo = HeldNotificationsRepo::new(pool.clone());
    let fire_store = scheduling::FireStore::new(pool.clone());
    let svc = HeldReleaseService::new(held_repo.clone(), fire_store.clone());

    let payload = NotificationPayload {
        alarm_id: "fire_1".into(),
        title: "t".into(), body: "b".into(),
        priority: Priority::Normal,
    };
    let release_at = Timestamp::now() + std::time::Duration::from_secs(600);

    svc.hold("fire_1", &["telegram".into()], &payload, release_at).await.unwrap();

    // One held row pending
    let pending = held_repo.list_pending_before(i64::MAX).await.unwrap();
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].alarm_id, "fire_1");

    // One scheduled_fires(kind='held_release') scheduled at release_at
    let due = fire_store.pending_with_kind_before(i64::MAX, "held_release").await.unwrap();
    assert_eq!(due.len(), 1);
    assert_eq!(due[0].fire_at_ms, release_at.as_millisecond());
}
```

Run: `cargo nextest run -p notifications --test held_release` — FAIL.

- [ ] **Step 2: Implement the service**

`crates/notifications/src/held.rs`:

```rust
//! Writes `held_notifications` rows and schedules a companion
//! `scheduled_fires(kind='held_release')` row via Phase 2's FireStore.
//! On the release fire, `release_due(now)` moves held rows into delivery.

use jiff::Timestamp;
use serde_json::json;
use uuid::Uuid;

use scheduling::{FireStore, ScheduledFireSpec};
use storage::repos::held_notifications::HeldNotificationsRepo;
use storage::rows::held_notification::HeldNotificationRow;

use crate::channel::{NotificationPayload, Priority};
use crate::error::Result;

#[derive(Clone)]
pub struct HeldReleaseService {
    held: HeldNotificationsRepo,
    fire_store: FireStore,
}

impl HeldReleaseService {
    pub fn new(held: HeldNotificationsRepo, fire_store: FireStore) -> Self {
        Self { held, fire_store }
    }

    pub async fn hold(
        &self,
        alarm_id: &str,
        channels: &[String],
        payload: &NotificationPayload,
        release_at: Timestamp,
    ) -> Result<String> {
        let id = format!("held_{}", Uuid::new_v4());
        let priority_str = match payload.priority {
            Priority::Normal => "normal", Priority::Urgent => "urgent",
        };
        let row = HeldNotificationRow {
            id: id.clone(),
            alarm_id: alarm_id.into(),
            channels: json!(channels),
            payload: json!({
                "title": payload.title,
                "body": payload.body,
                "priority": priority_str,
            }),
            release_at_ms: release_at.as_millisecond(),
            released: false,
            held_at_ms: Timestamp::now().as_millisecond(),
        };
        self.held.insert(&row).await?;

        // Schedule the release fire via Phase 2's FireStore.
        self.fire_store.insert(ScheduledFireSpec {
            fire_at: release_at,
            kind: "held_release".into(),
            ref_id: Some(id.clone()),
            payload: json!({ "held_id": id }),
            dedup_prefix: Some(format!("held:{id}:")),
        }).await?;

        Ok(id)
    }

    /// Process all release-ready held rows at `now`. Returns the list of
    /// (held_id, alarm_id, channels, payload) tuples the caller must now
    /// dispatch. The caller is responsible for calling `mark_released` after
    /// successful dispatch.
    pub async fn release_due(&self, now: Timestamp) -> Result<Vec<ReleaseBatch>> {
        let rows = self.held.list_pending_before(now.as_millisecond()).await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let channels: Vec<String> = serde_json::from_value(r.channels)
                .unwrap_or_default();
            out.push(ReleaseBatch {
                held_id: r.id,
                alarm_id: r.alarm_id,
                channels,
                payload: r.payload,
            });
        }
        Ok(out)
    }

    pub async fn mark_released(&self, held_id: &str) -> Result<()> {
        self.held.mark_released(held_id).await?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ReleaseBatch {
    pub held_id: String,
    pub alarm_id: String,
    pub channels: Vec<String>,
    pub payload: serde_json::Value,
}
```

> **Note:** `ScheduledFireSpec` and `FireStore::pending_with_kind_before` are Phase 2 types. If the Phase 2 names differ in the code, match the real ones; the test in Step 1 must still pass. If `pending_with_kind_before` doesn't exist, add it in `crates/scheduling/src/temporal/fire_store.rs` (`SELECT … WHERE fired=0 AND kind=? AND fire_at_ms <= ?`).

- [ ] **Step 3: Run — expect pass**

Run: `cargo nextest run -p notifications --test held_release`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/notifications/ crates/scheduling/
git commit -m "feat(notifications): HeldReleaseService + release scheduling"
```

---

## Task 10: Retry policy

**Files:**
- Rewrite: `crates/notifications/src/retry.rs`

- [ ] **Step 1: Write with tests inline**

```rust
//! Exponential backoff retry policy (1s → 4s → 16s by default).
use config::schema::RetryConfig;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: u32,
    pub base_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self { max_attempts: 3, base_delay: Duration::from_secs(1) }
    }
}

impl RetryPolicy {
    pub fn from_config(cfg: &RetryConfig) -> Self {
        Self {
            max_attempts: cfg.max_attempts.max(1),
            base_delay: Duration::from_secs(cfg.base_delay_secs.max(1)),
        }
    }

    /// Delay *before* attempt `attempt` (1-indexed; no delay before attempt 1).
    /// For 1s base: attempt 2 → 1s, attempt 3 → 4s, attempt 4 → 16s.
    pub fn delay_for(&self, attempt: u32) -> Duration {
        if attempt <= 1 { return Duration::ZERO; }
        let multiplier = 4u64.pow(attempt.saturating_sub(2));
        self.base_delay.saturating_mul(multiplier as u32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn default_schedule_is_0_1_4_16() {
        let p = RetryPolicy::default();
        assert_eq!(p.delay_for(1), Duration::ZERO);
        assert_eq!(p.delay_for(2), Duration::from_secs(1));
        assert_eq!(p.delay_for(3), Duration::from_secs(4));
        assert_eq!(p.delay_for(4), Duration::from_secs(16));
    }

    #[test]
    fn custom_base_scales() {
        let p = RetryPolicy { max_attempts: 3, base_delay: Duration::from_secs(2) };
        assert_eq!(p.delay_for(2), Duration::from_secs(2));
        assert_eq!(p.delay_for(3), Duration::from_secs(8));
    }
}
```

- [ ] **Step 2: Run — expect pass**

Run: `cargo nextest run -p notifications retry`
Expected: PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/notifications/src/retry.rs
git commit -m "feat(notifications): exponential-backoff retry policy"
```

---

## Task 11: Wire held-release consumption into the dispatcher

**Files:**
- Modify: `crates/notifications/src/dispatcher.rs`

The Phase 2 `TemporalScheduler` already emits `AlarmFired{kind:"held_release", …}` when the release-time arrives. We extend the dispatcher to branch on `kind`.

- [ ] **Step 1: Write failing test**

`crates/notifications/tests/quiet_hours_release.rs`:

```rust
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use config::schema::QuietHoursConfig;
use jiff::Timestamp;
use notifications::{
    channel::{Channel, ChannelRegistry, NotificationPayload},
    dispatcher::NotificationDispatcher,
    held::HeldReleaseService,
    quiet_hours::QuietHoursPolicy,
    retry::RetryPolicy,
};
use storage::StoragePool;
use storage::repos::{held_notifications::HeldNotificationsRepo,
                     notification_log::NotificationLogRepo};

struct Counting { name: String, count: Arc<AtomicUsize> }

#[async_trait]
impl Channel for Counting {
    fn name(&self) -> &str { &self.name }
    async fn deliver(&self, _p: &NotificationPayload) -> notifications::Result<()> {
        self.count.fetch_add(1, Ordering::SeqCst); Ok(())
    }
}

#[tokio::test]
async fn quiet_hours_holds_then_release_fires_delivery() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(notifications::migration().sql()).execute(pool.inner()).await.unwrap();
    sqlx::query(scheduling::migration().sql()).execute(pool.inner()).await.unwrap();

    let bus = Arc::new(DomainEventBus::new(64));
    let counter = Arc::new(AtomicUsize::new(0));

    let mut reg = ChannelRegistry::new();
    reg.register(Arc::new(Counting { name: "telegram".into(), count: counter.clone() }));

    // A 24/7 quiet window so "now" is always quiet.
    let qh = QuietHoursPolicy::new(
        QuietHoursConfig {
            enabled: true, start: "00:00".into(), end: "23:59".into(),
            override_for_urgent_tasks: false,
        },
        "UTC",
    ).unwrap();

    let log = NotificationLogRepo::new(pool.clone());
    let held_repo = HeldNotificationsRepo::new(pool.clone());
    let fire_store = scheduling::FireStore::new(pool.clone());
    let held_rel = HeldReleaseService::new(held_repo.clone(), fire_store);

    let dispatcher = NotificationDispatcher::new(
        bus.clone(), reg, vec!["telegram".into()], Some(qh),
        log, held_repo.clone(), held_rel, RetryPolicy::default(),
    );
    let handle = dispatcher.start();

    // Initial fire — should be held.
    bus.publish(DomainEvent::AlarmFired {
        alarm_id: "fire_1".into(), task_id: None, kind: "task_alarm".into(),
        payload_json: serde_json::json!({"title":"t","body":"b"}).to_string(),
    }).await;

    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    assert_eq!(counter.load(Ordering::SeqCst), 0, "held, no delivery yet");
    assert_eq!(held_repo.list_pending_before(i64::MAX).await.unwrap().len(), 1);

    // Simulate the TemporalScheduler firing the held_release alarm by
    // publishing `AlarmFired{kind:"held_release", payload:{held_id}}`.
    let held_id = held_repo.list_pending_before(i64::MAX).await.unwrap()[0].id.clone();
    bus.publish(DomainEvent::AlarmFired {
        alarm_id: format!("release_{held_id}"),
        task_id: None,
        kind: "held_release".into(),
        payload_json: serde_json::json!({"held_id": held_id}).to_string(),
    }).await;

    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    handle.shutdown.cancel();
    let _ = handle.join.await;

    assert_eq!(counter.load(Ordering::SeqCst), 1, "held delivery released exactly once");
    assert!(held_repo.list_pending_before(i64::MAX).await.unwrap().is_empty());
}
```

Run: `cargo nextest run -p notifications --test quiet_hours_release` — FAIL.

- [ ] **Step 2: Extend dispatcher to branch on kind**

In `crates/notifications/src/dispatcher.rs`, change the `AlarmFired` arm:

```rust
Ok(DomainEvent::AlarmFired { alarm_id, task_id: _, kind, payload_json }) => {
    let result = if kind == "held_release" {
        svc.handle_held_release(&payload_json).await
    } else {
        svc.handle_alarm_fired(&alarm_id, &kind, &payload_json).await
    };
    if let Err(e) = result {
        warn!("dispatch failure for {alarm_id}: {e}");
    }
}
```

Add the method:

```rust
async fn handle_held_release(&self, payload_json: &str) -> Result<()> {
    let v: serde_json::Value = serde_json::from_str(payload_json)
        .unwrap_or(serde_json::Value::Null);
    let held_id = v.get("held_id").and_then(|x| x.as_str()).unwrap_or("").to_string();
    if held_id.is_empty() { return Ok(()); }

    let batches = self.held_release.release_due(Timestamp::now()).await?;
    for batch in batches.into_iter().filter(|b| b.held_id == held_id) {
        let payload = NotificationPayload {
            alarm_id: batch.alarm_id.clone(),
            title: batch.payload.get("title").and_then(|x| x.as_str())
                .unwrap_or("Reminder").to_string(),
            body:  batch.payload.get("body").and_then(|x| x.as_str())
                .unwrap_or("").to_string(),
            priority: match batch.payload.get("priority").and_then(|x| x.as_str()) {
                Some("urgent") => Priority::Urgent,
                _ => Priority::Normal,
            },
        };
        for ch in &batch.channels {
            self.dispatch_one(ch, &payload).await;
        }
        self.held_release.mark_released(&batch.held_id).await?;
        self.bus.publish(DomainEvent::HeldNotificationReleased {
            held_id: batch.held_id,
            alarm_id: batch.alarm_id,
            channels: batch.channels,
        }).await;
    }
    Ok(())
}
```

- [ ] **Step 3: Run — expect pass**

Run: `cargo nextest run -p notifications --test quiet_hours_release`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/notifications/src/dispatcher.rs crates/notifications/tests/quiet_hours_release.rs
git commit -m "feat(notifications): dispatch held releases on held_release alarm"
```

---

## Task 12: Wire `NotificationDispatcher` into app-core; remove legacy dispatcher

**Files:**
- Modify: `crates/app-core/Cargo.toml` (add `notifications.workspace = true`)
- Modify: `crates/app-core/src/init/mod.rs` (or wherever `TemporalScheduler` is wired in Phase 2)
- Modify: `crates/agent/src/agent_loop/builder.rs` (any call-sites using old dispatcher)
- Delete: `crates/agent/src/services/notifications.rs`
- Modify: `crates/agent/src/services/mod.rs` — remove the `pub mod notifications;` line
- Modify: `crates/klyntbot/Cargo.toml` + `src/lib.rs` — re-export `notifications::*`

- [ ] **Step 1: Survey legacy call-sites**

Run: `rg "services::notifications::NotificationDispatcher|agent::services::notifications" crates/ -n`
Expected output lists the exact files and lines.

- [ ] **Step 2: Build the new dispatcher in app-core startup**

In `crates/app-core/src/init/mod.rs` (or the matching notification-init file), replace the old `NotificationDispatcher::new(outbound_tx, config.todos.notifications.clone())` construction with:

```rust
use notifications::{
    channel::{ChannelRegistry, os_native::OsNativeChannel, tray::TrayChannel,
              outbound::OutboundChannel},
    dispatcher::NotificationDispatcher,
    held::HeldReleaseService,
    quiet_hours::QuietHoursPolicy,
    retry::RetryPolicy,
};

fn build_notification_dispatcher(
    bus: Arc<bus::DomainEventBus>,
    outbound_tx: tokio::sync::mpsc::Sender<bus::OutboundMessage>,
    last_active: Arc<tokio::sync::RwLock<
        Option<(common::ChannelName, common::ChatId)>>>,
    os_sender: Arc<dyn common::NotificationSender>,
    repos: &storage::Repos,
    pool: storage::StoragePool,
    cfg: &config::Config,
) -> notifications::NotificationDispatcherHandle {
    let mut registry = ChannelRegistry::new();
    registry.register(Arc::new(OsNativeChannel::new(os_sender)));
    registry.register(Arc::new(TrayChannel::new(bus.clone())));
    for ch in &["telegram", "discord", "slack", "email"] {
        registry.register(Arc::new(OutboundChannel::new(
            *ch, outbound_tx.clone(), last_active.clone(),
        )));
    }

    let qh = if cfg.notifications.quiet_hours.enabled {
        Some(QuietHoursPolicy::new(
            cfg.notifications.quiet_hours.clone(),
            &cfg.timezone,
        ).expect("valid quiet hours config"))
    } else { None };

    let fire_store = scheduling::FireStore::new(pool.clone());
    let held_rel = HeldReleaseService::new(
        repos.held_notifications.clone(), fire_store,
    );

    let dispatcher = NotificationDispatcher::new(
        bus, registry,
        cfg.notifications.default_channels.clone(),
        qh,
        repos.notification_log.clone(),
        repos.held_notifications.clone(),
        held_rel,
        RetryPolicy::from_config(&cfg.notifications.retry),
    );
    dispatcher.start()
}
```

Store the returned `NotificationDispatcherHandle` on `AppCore` (add an `Option<NotificationDispatcherHandle>` field next to the existing scheduler handles). On shutdown, call `handle.shutdown.cancel()` and await `handle.join`.

- [ ] **Step 3: Register notifications migration**

Wherever feature migrations are aggregated (likely `crates/app-core/src/init/migrations.rs` or similar):

```rust
migrations.push(notifications::migration());
```

- [ ] **Step 4: Remove legacy call-sites**

For each file found in Step 1:
1. Delete the import `use agent::services::notifications::NotificationDispatcher;`.
2. Replace any synchronous `dispatcher.notify(title, body).await` call in the agent with a `DomainEvent::AlarmFired { alarm_id, kind: "legacy_notify", payload_json: json!({"title":…,"body":…}).to_string() }` publish on the bus. (If there are no such sites outside `agent/src/services/reminders.rs` — which is still owned by `ReminderEngine` until Phase 4 — leave ReminderEngine's own constructor taking the *new* dispatcher via injection is also acceptable. Default: emit bus events.)

- [ ] **Step 5: Delete legacy file**

```bash
git rm crates/agent/src/services/notifications.rs
```

Remove `pub mod notifications;` from `crates/agent/src/services/mod.rs`.

- [ ] **Step 6: Re-export through klyntbot facade**

`crates/klyntbot/Cargo.toml`:

```toml
notifications = { workspace = true }
```

`crates/klyntbot/src/lib.rs`:

```rust
pub use notifications::{NotificationDispatcher, NotificationDispatcherHandle};
```

- [ ] **Step 7: Full workspace verification**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: all four exit 0.

- [ ] **Step 8: Commit**

```bash
git add -A
git commit -m "refactor(app-core): wire new NotificationDispatcher; remove legacy dispatcher"
```

---

## Task 13: End-to-end integration test through the facade

**Files:**
- Create: `tests/integration/notifications_dispatcher.rs`
- Modify: `tests/integration/mod.rs` (register new module)

- [ ] **Step 1: Write the e2e test**

```rust
//! End-to-end: insert a `scheduled_fires` row in the past →
//! TemporalScheduler fires AlarmFired → NotificationDispatcher delivers
//! to a mock channel → notification_log gated against duplicates.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use bus::{DomainEvent, DomainEventBus};
use jiff::Timestamp;
use klyntbot::{NotificationDispatcher};

use notifications::{
    channel::{Channel, ChannelRegistry, NotificationPayload},
    held::HeldReleaseService, retry::RetryPolicy,
};
use scheduling::{FireStore, ScheduledFireSpec, TemporalScheduler};
use storage::{Repos, StoragePool};

struct Mock { name: String, hits: Arc<AtomicUsize> }

#[async_trait]
impl Channel for Mock {
    fn name(&self) -> &str { &self.name }
    async fn deliver(&self, _p: &NotificationPayload) -> notifications::Result<()> {
        self.hits.fetch_add(1, Ordering::SeqCst); Ok(())
    }
}

#[tokio::test]
async fn scheduler_fires_dispatcher_delivers_e2e() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    sqlx::query(scheduling::migration().sql()).execute(pool.inner()).await.unwrap();
    sqlx::query(notifications::migration().sql()).execute(pool.inner()).await.unwrap();
    let repos = Repos::from_pool(pool.clone());

    let bus = Arc::new(DomainEventBus::new(64));

    // Dispatcher
    let hits = Arc::new(AtomicUsize::new(0));
    let mut reg = ChannelRegistry::new();
    reg.register(Arc::new(Mock { name: "tray".into(), hits: hits.clone() }));
    let fire_store = FireStore::new(pool.clone());
    let held_rel = HeldReleaseService::new(
        repos.held_notifications.clone(), fire_store.clone());
    let disp = NotificationDispatcher::new(
        bus.clone(), reg, vec!["tray".into()], None,
        repos.notification_log.clone(), repos.held_notifications.clone(),
        held_rel, RetryPolicy::default(),
    );
    let disp_handle = disp.start();

    // Scheduler
    let sched_handle = TemporalScheduler::start(pool.clone(), bus.clone()).await.unwrap();

    // Insert a past-due fire.
    let past = Timestamp::now() - std::time::Duration::from_secs(1);
    fire_store.insert(ScheduledFireSpec {
        fire_at: past, kind: "task_alarm".into(),
        ref_id: Some("task_1".into()),
        payload: serde_json::json!({"title":"T","body":"B"}),
        dedup_prefix: Some("task:task_1:".into()),
    }).await.unwrap();

    // Scheduler wake
    sched_handle.wake_signal.notify_one();

    tokio::time::sleep(std::time::Duration::from_millis(300)).await;

    sched_handle.shutdown.cancel();
    disp_handle.shutdown.cancel();
    let _ = sched_handle.join.await;
    let _ = disp_handle.join.await;

    assert_eq!(hits.load(Ordering::SeqCst), 1,
        "alarm fired exactly once and delivered once");
}
```

> Field names above (`wake_signal`, `shutdown`, `join`) mirror Phase 2's handle. If Phase 2 named them differently, match the real names — the test logic is the same.

- [ ] **Step 2: Register the module**

In `tests/integration/mod.rs` add `pub mod notifications_dispatcher;`.

- [ ] **Step 3: Run**

Run: `cargo nextest run -E 'test(scheduler_fires_dispatcher_delivers_e2e)'`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/integration/
git commit -m "test(integration): e2e TemporalScheduler → NotificationDispatcher"
```

---

## Task 14: Documentation + final verification

**Files:**
- Modify: `CLAUDE.md` (Architecture → add notifications crate entry to the L4 line)
- Modify: `docs/superpowers/plans/2026-04-18-phase-2-unified-temporal-scheduler-core.md` — add a short "superseded by Phase 3 for dispatcher" pointer at the top (one line only).

- [ ] **Step 1: Update CLAUDE.md L4 line**

Edit the Workspace section in `CLAUDE.md`:

Before:
```
L4: tools, feature-tasks, ..., simulator — 20+ tools, feature packages, WASM plugins, ...
```

After (add `notifications`):
```
L4: tools, feature-tasks, ..., simulator, notifications — 20+ tools, feature packages, WASM plugins, ..., notification dispatcher (AlarmFired subscriber, quiet hours, held release, multi-channel fan-out)
```

- [ ] **Step 2: Final workspace verification**

```bash
cargo build --workspace
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Expected: all exit 0, no new warnings.

- [ ] **Step 3: Commit**

```bash
git add CLAUDE.md docs/
git commit -m "docs: note notifications crate in CLAUDE.md"
```

---

## Self-Review Findings

**Spec coverage check** against `docs/superpowers/specs/2026-04-17-unified-temporal-scheduler-and-notifications-design.md`:

| Spec section | Task |
|---|---|
| §3.2 Default rules config subtree | Task 6 (partial — `defaultRules[]` itself is Phase 4 when task_alarms flow lands; §3.2's quiet-hours, default channels, default misfire/grace fields are covered here) |
| §4.1 `notification_log` | Task 2 |
| §4.1 `held_notifications` | Task 3 |
| §6.1 AlarmFired handler | Task 8 |
| §6.2 Channel routing table | Task 7 (os_native / tray / outbound); fine-grained quiet-hours-urgent override per-channel left for Phase 4 when per-alarm channel_mask lands |
| §6.3 Held release | Tasks 9 + 11 |
| §6.4 Retry policy | Tasks 10 + 8 (integrated into dispatch_one) |
| §10 HeldNotificationReleased / NotificationDeliveryFailed | Task 4 |
| §10 TrayNotificationRequested | Task 4 (new narrow event for the tray bridge) |
| §11 Delete `agent/services/notifications.rs` | Task 12 |

**Deferred to Phase 4** (noted in Non-Goals above): TaskTool.alarms+recurrence subfields, AlarmTool, recurrence instance materialization, `ReminderEngine` deletion, per-alarm `channel_mask` override resolution (§6.2's urgent-override granularity).

**Placeholder scan:** no TBD/TODO/fill-in-later strings. Every step has concrete code or an exact command.

**Type consistency check:**
- `NotificationPayload` shape consistent across Tasks 7/8/9/11.
- `Channel` trait signature consistent (Tasks 7, 8, 11, 13).
- `HeldReleaseService::hold` signature consistent between Task 9 (definition) and Task 8 (call site) and Task 11 (`release_due` + `mark_released`).
- `NotificationDispatcher::new` argument order consistent across Task 8 (definition), Tasks 8/11/13 (construction sites), Task 12 (app-core wiring).
- `FireStore::pending_with_kind_before` — must exist or be added in Phase 2 scheduling; Task 9 Step 2 notes adding it if missing.

**Ambiguity flag:** Task 12 Step 4 ("replace any synchronous `dispatcher.notify` call … with a bus publish") may touch sites inside `ReminderEngine` (which lives until Phase 4). If ReminderEngine is the only remaining caller, inject the new dispatcher into ReminderEngine as `Arc<NotificationDispatcherHandle>` instead — avoids churn. The task prefers the bus-publish approach for decoupling; reviewer's call.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-19-phase-3-notifications-dispatcher.md`.

Two execution options:

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per Task (14 tasks), review between Tasks. Tight quality gate per commit.

**2. Inline Execution** — Execute Tasks in this session with checkpoints at Tasks 5, 10, and 14.

Which approach?
