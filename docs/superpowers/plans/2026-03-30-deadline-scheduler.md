# DeadlineScheduler — Event-Driven Task Timers

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace 4 high-frequency polling cron jobs with an event-driven `DeadlineScheduler` that fires at exact deadlines, and trim 8 unnecessary default cron jobs from first-launch registration.

**Architecture:** A new `DeadlineScheduler` in the `scheduling` crate maintains an in-memory priority queue of `(fire_time, action)` pairs. It subscribes to `DomainEventBus` for task mutations (focus, due date, recurring template) and registers one-shot timers. On app startup it scans existing data to populate the queue. The same `sleep_until + Notify` pattern already proven in `CronService` drives the executor. Phase 1 trims defaults by making 8 user jobs lazy-created. Phase 2 replaces 4 pollers with the scheduler.

**Tech Stack:** Rust, tokio (sleep_until, Notify), chrono, BTreeMap priority queue, DomainEventBus (broadcast channel)

---

## File Structure

### New files
| File | Responsibility |
|------|---------------|
| `crates/scheduling/src/deadline.rs` | `DeadlineScheduler` struct, priority queue, executor loop, public API |
| `crates/scheduling/src/deadline_actions.rs` | `DeadlineAction` enum and handler dispatch |

### Modified files
| File | Changes |
|------|---------|
| `crates/scheduling/src/lib.rs` | Export `DeadlineScheduler`, `DeadlineAction` |
| `crates/bus/src/domain_events.rs` | Add `TaskDueDateChanged` and `RecurringTemplateChanged` variants |
| `crates/feature-tasks/src/tool/actions/focus.rs` | Emit `TaskFocusStarted` with `deadline` field via domain bus |
| `crates/app-core/src/handlers/tasks/crud.rs` | Emit `TaskDueDateChanged` on create/update when due_date is set |
| `crates/app-core/src/init/cron.rs` | Remove 4 polling jobs + make 8 user jobs lazy |
| `crates/app-core/src/init/mod.rs` | Wire `DeadlineScheduler` startup + hold handle in `AppCore` |
| `crates/app-core/src/state.rs` | Add `deadline_scheduler: Option<Arc<DeadlineScheduler>>` field |
| `crates/scheduling/Cargo.toml` | Add `bus` dependency (for `DomainEventBus` subscription) |

---

## Phase 1 — Trim Default Cron Jobs

### Task 1: Remove polling cron jobs from default registration

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

These 4 jobs will be replaced by DeadlineScheduler in Phase 2. Remove them now so they don't conflict.

- [ ] **Step 1: Write test that the 4 polling jobs are NOT registered by default**

Add to the existing test module at the bottom of `cron.rs`:

```rust
#[tokio::test]
async fn polling_jobs_not_registered_by_default() {
    // These jobs are handled by DeadlineScheduler, not cron
    let removed_jobs = [
        "todo_focus_check",
        "todo_daily_digest",       // lazy: created on first task
        "todo_overdue_check",
        "__klyntbot_reminder_check",
        "__klyntbot_recurring_tasks",
    ];

    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    let config = config::Config::default();
    let tasks_config = feature_tasks::TasksConfig::default();

    let mut cron_service = scheduling::CronService::new(repos.cron.clone());
    cron_service.start().await.unwrap();
    let cron_service = std::sync::Arc::new(cron_service);

    super::ensure_cron_jobs(&cron_service, &config, &tasks_config)
        .await
        .unwrap();

    let jobs: Vec<String> = cron_service
        .list_jobs(true)
        .await
        .into_iter()
        .map(|j| j.name)
        .collect();

    for removed in &removed_jobs {
        assert!(
            !jobs.contains(&removed.to_string()),
            "Job '{removed}' should not be registered by default"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p app-core --test '*' -E 'test(polling_jobs_not_registered)'`
Expected: FAIL — these jobs are still registered.

- [ ] **Step 3: Remove the 4 polling job registrations from `ensure_cron_jobs`**

In `crates/app-core/src/init/cron.rs`, inside `ensure_cron_jobs()`, delete the `ensure_job!` blocks for:

1. `JOB_FOCUS_CHECK` (the `ensure_job!(JOB_FOCUS_CHECK, ...)` block around line 890-897)
2. `JOB_OVERDUE_CHECK` (the `ensure_job!(JOB_OVERDUE_CHECK, ...)` block around line 907-914)
3. `JOB_REMINDER_CHECK` (the `ensure_job!(JOB_REMINDER_CHECK, ...)` block around line 1011-1018)
4. `JOB_RECURRING_TASKS` (the `ensure_job!(JOB_RECURRING_TASKS, ...)` block around line 1019-1026)

Also remove `JOB_DAILY_DIGEST` — it will be lazy-created on first task (Task 2).

Keep the `register_handler` blocks in `register_cron_callbacks` — users who already have these jobs in their DB will still get them executed. We're just not auto-creating them for new installs.

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p app-core --test '*' -E 'test(polling_jobs_not_registered)'`
Expected: PASS

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/cron.rs
git commit -m "refactor(cron): remove 5 polling jobs from default registration

These jobs will be replaced by DeadlineScheduler (event-driven):
- todo_focus_check, todo_overdue_check, reminder_check, recurring_tasks
- todo_daily_digest becomes lazy (created on first task)"
```

---

### Task 2: Make remaining user jobs lazy-created

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`

Remove 3 more jobs from default registration. They'll be created when their feature is first used.

- [ ] **Step 1: Write test that lazy jobs are NOT registered by default**

```rust
#[tokio::test]
async fn lazy_user_jobs_not_registered_by_default() {
    let lazy_jobs = [
        "__klyntbot_weekly_report",
        "__klyntbot_morning_briefing",
        "__klyntbot_weekly_knowledge_digest",
    ];

    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    let repos = storage::Repos::from_pool(&pool);
    let config = config::Config::default();
    let tasks_config = feature_tasks::TasksConfig::default();

    let mut cron_service = scheduling::CronService::new(repos.cron.clone());
    cron_service.start().await.unwrap();
    let cron_service = std::sync::Arc::new(cron_service);

    super::ensure_cron_jobs(&cron_service, &config, &tasks_config)
        .await
        .unwrap();

    let jobs: Vec<String> = cron_service
        .list_jobs(true)
        .await
        .into_iter()
        .map(|j| j.name)
        .collect();

    for lazy in &lazy_jobs {
        assert!(
            !jobs.contains(&lazy.to_string()),
            "Job '{lazy}' should not be registered by default — should be lazy"
        );
    }
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p app-core --test '*' -E 'test(lazy_user_jobs)'`
Expected: FAIL

- [ ] **Step 3: Remove the 3 lazy job registrations from `ensure_cron_jobs`**

In `crates/app-core/src/init/cron.rs`, inside `ensure_cron_jobs()`, delete:

1. `JOB_WEEKLY_REPORT` ensure_job block (around line 915-923)
2. `JOB_MORNING_BRIEFING` ensure_job block (around line 980-988)
3. `JOB_WEEKLY_KNOWLEDGE_DIGEST` ensure_job block (around line 989-997)

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p app-core --test '*' -E 'test(lazy_user_jobs)'`
Expected: PASS

- [ ] **Step 5: Add public helper for lazy job creation**

Add a public function to `crates/app-core/src/init/cron.rs` that other modules can call to lazily register these jobs when the feature is first used:

```rust
/// Lazily ensure a user-facing cron job exists. Called when a feature is first used
/// (e.g., first task created → daily digest, first atom → morning briefing).
/// No-op if the job already exists.
pub async fn ensure_lazy_job(
    cron_service: &Arc<CronService>,
    name: &str,
    schedule: scheduling::CronSchedule,
    description: &str,
) -> Result<(), common::KlyntbotError> {
    let existing: std::collections::HashSet<String> = cron_service
        .list_jobs(true)
        .await
        .into_iter()
        .map(|j| j.name)
        .collect();

    if !existing.contains(name) {
        cron_service
            .add_job(
                name,
                schedule,
                description,
                false,
                None,
                None,
                false,
                scheduling::CronOrigin::User,
            )
            .await?;
        tracing::info!("Lazy-created cron job: {name}");
    }
    Ok(())
}
```

- [ ] **Step 6: Run all cron tests**

Run: `cargo nextest run -p app-core --test '*' -E 'test(cron)'`
Expected: All pass. Existing tests that don't depend on these specific jobs should be unaffected.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/init/cron.rs
git commit -m "refactor(cron): make 3 user jobs lazy-created on first feature use

weekly_report, morning_briefing, weekly_knowledge_digest are now only
registered when the user first uses the relevant feature. Adds
ensure_lazy_job() helper for on-demand registration."
```

---

## Phase 2 — DeadlineScheduler

### Task 3: Define DeadlineAction enum

**Files:**
- Create: `crates/scheduling/src/deadline_actions.rs`

- [ ] **Step 1: Write the action types**

```rust
//! Deadline action types — what fires when a deadline is reached.

use serde::{Deserialize, Serialize};

/// An action to execute when a deadline fires.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum DeadlineAction {
    /// Send a reminder notification for a task approaching its due date.
    TaskReminder {
        task_id: String,
        /// Human-readable label, e.g. "2h before due"
        label: String,
    },

    /// Warn about an approaching focus deadline (6h, 3h, 1h thresholds).
    FocusWarning {
        task_id: String,
        hours_left: u32,
    },

    /// Auto-expire a focus session that has passed its deadline.
    FocusExpire {
        task_id: String,
    },

    /// Spawn a recurring task instance from a template.
    SpawnRecurring {
        template_id: String,
    },
}

impl DeadlineAction {
    /// Unique key for deduplication — prevents scheduling the same action twice.
    pub fn dedup_key(&self) -> String {
        match self {
            Self::TaskReminder { task_id, label } => format!("reminder:{task_id}:{label}"),
            Self::FocusWarning { task_id, hours_left } => {
                format!("focus_warn:{task_id}:{hours_left}h")
            }
            Self::FocusExpire { task_id } => format!("focus_expire:{task_id}"),
            Self::SpawnRecurring { template_id } => format!("spawn:{template_id}"),
        }
    }
}
```

- [ ] **Step 2: Commit**

```bash
git add crates/scheduling/src/deadline_actions.rs
git commit -m "feat(scheduling): add DeadlineAction enum for event-driven timers"
```

---

### Task 4: Build DeadlineScheduler core

**Files:**
- Create: `crates/scheduling/src/deadline.rs`
- Modify: `crates/scheduling/src/lib.rs`
- Modify: `crates/scheduling/Cargo.toml`

- [ ] **Step 1: Add `bus` dependency to scheduling crate**

In `crates/scheduling/Cargo.toml`, add under `[dependencies]`:

```toml
bus.workspace = true
```

- [ ] **Step 2: Write the DeadlineScheduler**

Create `crates/scheduling/src/deadline.rs`:

```rust
//! Event-driven deadline scheduler.
//!
//! Maintains a priority queue of (fire_time, action) pairs. Sleeps until
//! the next deadline using `tokio::time::sleep_until`, wakes early via
//! `Notify` when deadlines are added or cancelled.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::sync::{Notify, RwLock};
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use crate::deadline_actions::DeadlineAction;

/// A scheduled deadline entry.
#[derive(Debug, Clone)]
struct DeadlineEntry {
    fire_at: DateTime<Utc>,
    action: DeadlineAction,
}

/// Callback type for deadline execution.
pub type DeadlineHandler = Arc<dyn Fn(DeadlineAction) + Send + Sync>;

/// Event-driven deadline scheduler. Zero CPU cost when no deadlines are pending.
pub struct DeadlineScheduler {
    /// Sorted by fire_at. Key = dedup_key, value = entry.
    entries: Arc<RwLock<BTreeMap<String, DeadlineEntry>>>,
    wake: Arc<Notify>,
    handler: DeadlineHandler,
    cancel: CancellationToken,
    task_handle: RwLock<Option<JoinHandle<()>>>,
}

impl DeadlineScheduler {
    pub fn new(handler: DeadlineHandler) -> Self {
        Self {
            entries: Arc::new(RwLock::new(BTreeMap::new())),
            wake: Arc::new(Notify::new()),
            handler,
            cancel: CancellationToken::new(),
            task_handle: RwLock::new(None),
        }
    }

    /// Schedule a deadline. Replaces any existing entry with the same dedup key.
    pub async fn schedule(&self, fire_at: DateTime<Utc>, action: DeadlineAction) {
        let key = action.dedup_key();
        let entry = DeadlineEntry { fire_at, action };
        self.entries.write().await.insert(key, entry);
        self.wake.notify_one();
    }

    /// Cancel a deadline by its dedup key.
    pub async fn cancel_by_key(&self, key: &str) {
        let removed = self.entries.write().await.remove(key).is_some();
        if removed {
            self.wake.notify_one();
        }
    }

    /// Cancel all deadlines matching a prefix (e.g. "focus_warn:task123").
    pub async fn cancel_by_prefix(&self, prefix: &str) {
        let mut entries = self.entries.write().await;
        let keys_to_remove: Vec<String> = entries
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        let removed = !keys_to_remove.is_empty();
        for key in keys_to_remove {
            entries.remove(&key);
        }
        drop(entries);
        if removed {
            self.wake.notify_one();
        }
    }

    /// Number of pending deadlines.
    pub async fn pending_count(&self) -> usize {
        self.entries.read().await.len()
    }

    /// Start the executor loop.
    pub async fn start(&self) {
        let entries = Arc::clone(&self.entries);
        let wake = Arc::clone(&self.wake);
        let handler = Arc::clone(&self.handler);
        let cancel = self.cancel.clone();

        let handle = tokio::spawn(async move {
            info!("DeadlineScheduler started");
            loop {
                // Find the soonest deadline
                let next = {
                    let map = entries.read().await;
                    map.values()
                        .min_by_key(|e| e.fire_at)
                        .map(|e| e.fire_at)
                };

                match next {
                    Some(fire_at) => {
                        let now = Utc::now();
                        if fire_at <= now {
                            // Fire all due deadlines
                            Self::fire_due(&entries, &handler).await;
                        } else {
                            let delay = (fire_at - now)
                                .to_std()
                                .unwrap_or(Duration::from_secs(1));
                            let deadline = Instant::now() + delay;
                            tokio::select! {
                                _ = tokio::time::sleep_until(deadline) => {
                                    Self::fire_due(&entries, &handler).await;
                                }
                                _ = wake.notified() => {
                                    // Re-check — a new earlier deadline may have been added
                                }
                                _ = cancel.cancelled() => {
                                    debug!("DeadlineScheduler cancelled");
                                    break;
                                }
                            }
                        }
                    }
                    None => {
                        // No deadlines — sleep until woken
                        tokio::select! {
                            _ = wake.notified() => {}
                            _ = cancel.cancelled() => {
                                debug!("DeadlineScheduler cancelled (idle)");
                                break;
                            }
                        }
                    }
                }
            }
        });

        *self.task_handle.write().await = Some(handle);
    }

    /// Stop the scheduler.
    pub async fn stop(&self) {
        self.cancel.cancel();
        if let Some(handle) = self.task_handle.write().await.take() {
            let _ = handle.await;
        }
    }

    /// Fire all entries whose fire_at <= now.
    async fn fire_due(
        entries: &Arc<RwLock<BTreeMap<String, DeadlineEntry>>>,
        handler: &DeadlineHandler,
    ) {
        let now = Utc::now();
        let mut map = entries.write().await;
        let due_keys: Vec<String> = map
            .iter()
            .filter(|(_, e)| e.fire_at <= now)
            .map(|(k, _)| k.clone())
            .collect();

        for key in due_keys {
            if let Some(entry) = map.remove(&key) {
                debug!(action = ?entry.action, "Firing deadline");
                handler(entry.action);
            }
        }
    }
}
```

- [ ] **Step 3: Export from lib.rs**

In `crates/scheduling/src/lib.rs`, add:

```rust
pub mod deadline;
pub mod deadline_actions;

pub use deadline::DeadlineScheduler;
pub use deadline_actions::DeadlineAction;
```

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p scheduling`
Expected: Compiles with 0 errors.

- [ ] **Step 5: Commit**

```bash
git add crates/scheduling/src/deadline.rs crates/scheduling/src/deadline_actions.rs crates/scheduling/src/lib.rs crates/scheduling/Cargo.toml
git commit -m "feat(scheduling): add DeadlineScheduler with priority queue executor

Event-driven timer that sleeps until the exact next deadline.
Zero CPU when no deadlines are pending. Uses sleep_until + Notify
pattern proven in CronService."
```

---

### Task 5: Test DeadlineScheduler

**Files:**
- Modify: `crates/scheduling/src/deadline.rs` (add test module)

- [ ] **Step 1: Write unit tests**

Add to the bottom of `crates/scheduling/src/deadline.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use tokio::time;

    fn make_handler(counter: Arc<AtomicU32>) -> DeadlineHandler {
        Arc::new(move |_action| {
            counter.fetch_add(1, Ordering::SeqCst);
        })
    }

    #[tokio::test]
    async fn schedule_and_fire() {
        let counter = Arc::new(AtomicU32::new(0));
        let scheduler = DeadlineScheduler::new(make_handler(Arc::clone(&counter)));
        scheduler.start().await;

        // Schedule a deadline 50ms from now
        let fire_at = Utc::now() + chrono::Duration::milliseconds(50);
        scheduler
            .schedule(
                fire_at,
                DeadlineAction::TaskReminder {
                    task_id: "t1".into(),
                    label: "test".into(),
                },
            )
            .await;

        assert_eq!(scheduler.pending_count().await, 1);

        // Wait for it to fire
        time::sleep(Duration::from_millis(150)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
        assert_eq!(scheduler.pending_count().await, 0);

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn dedup_replaces_existing() {
        let counter = Arc::new(AtomicU32::new(0));
        let scheduler = DeadlineScheduler::new(make_handler(Arc::clone(&counter)));
        scheduler.start().await;

        let action = DeadlineAction::FocusWarning {
            task_id: "t1".into(),
            hours_left: 3,
        };

        // Schedule then replace with a later time
        scheduler
            .schedule(Utc::now() + chrono::Duration::milliseconds(50), action.clone())
            .await;
        scheduler
            .schedule(Utc::now() + chrono::Duration::seconds(10), action)
            .await;

        // Only 1 entry (deduped)
        assert_eq!(scheduler.pending_count().await, 1);

        // First deadline passed but entry was replaced — should NOT fire yet
        time::sleep(Duration::from_millis(150)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn cancel_by_prefix() {
        let counter = Arc::new(AtomicU32::new(0));
        let scheduler = DeadlineScheduler::new(make_handler(Arc::clone(&counter)));
        scheduler.start().await;

        let fire_at = Utc::now() + chrono::Duration::milliseconds(100);
        scheduler
            .schedule(
                fire_at,
                DeadlineAction::FocusWarning {
                    task_id: "t1".into(),
                    hours_left: 6,
                },
            )
            .await;
        scheduler
            .schedule(
                fire_at,
                DeadlineAction::FocusWarning {
                    task_id: "t1".into(),
                    hours_left: 3,
                },
            )
            .await;
        scheduler
            .schedule(
                fire_at,
                DeadlineAction::FocusExpire {
                    task_id: "t1".into(),
                },
            )
            .await;
        assert_eq!(scheduler.pending_count().await, 3);

        // Cancel all focus-related entries for task t1
        scheduler.cancel_by_prefix("focus_warn:t1").await;
        scheduler.cancel_by_prefix("focus_expire:t1").await;
        assert_eq!(scheduler.pending_count().await, 0);

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn fires_past_deadlines_immediately() {
        let counter = Arc::new(AtomicU32::new(0));
        let scheduler = DeadlineScheduler::new(make_handler(Arc::clone(&counter)));
        scheduler.start().await;

        // Schedule a deadline in the past
        let fire_at = Utc::now() - chrono::Duration::seconds(10);
        scheduler
            .schedule(
                fire_at,
                DeadlineAction::SpawnRecurring {
                    template_id: "tpl1".into(),
                },
            )
            .await;

        // Should fire almost immediately
        time::sleep(Duration::from_millis(100)).await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);

        scheduler.stop().await;
    }
}
```

- [ ] **Step 2: Run the tests**

Run: `cargo nextest run -p scheduling -E 'test(deadline)'`
Expected: All 4 tests pass.

- [ ] **Step 3: Commit**

```bash
git add crates/scheduling/src/deadline.rs
git commit -m "test(scheduling): add DeadlineScheduler unit tests

Covers: schedule-and-fire, dedup replacement, cancel-by-prefix,
past-deadline immediate fire."
```

---

### Task 6: Add missing DomainEvent variants

**Files:**
- Modify: `crates/bus/src/domain_events.rs`

The existing events `TaskCreated` and `TaskFocusStarted` lack the fields we need (due_date, focus_deadline). Add new purpose-built variants.

- [ ] **Step 1: Add TaskDueDateChanged and RecurringTemplateChanged variants**

In `crates/bus/src/domain_events.rs`, inside the `DomainEvent` enum, after the `TaskFieldUpdated` variant (around line 169), add:

```rust
    /// Emitted when a task's due date is set or changed. Used by DeadlineScheduler.
    TaskDueDateChanged {
        task_id: String,
        /// None means the due date was cleared.
        due_date: Option<String>,
    },

    /// Emitted when a task is focused with a deadline. Used by DeadlineScheduler.
    TaskFocusChanged {
        task_id: String,
        /// None means unfocused.
        focus_deadline: Option<String>,
    },

    /// Emitted when a recurring template's next_instance_date changes.
    RecurringTemplateAdvanced {
        template_id: String,
        next_instance_date: Option<String>,
    },
```

- [ ] **Step 2: Add the new variants to the domain classification match arm**

In the same file, find the `pub fn domain(&self) -> &str` method and add the new variants to the `"work"` arm (alongside the other Task* variants):

```rust
            | Self::TaskDueDateChanged { .. }
            | Self::TaskFocusChanged { .. }
            | Self::RecurringTemplateAdvanced { .. }
```

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p bus`
Expected: Compiles with 0 errors. (The `DomainEvent` enum derives `Serialize`/`Deserialize` and `Clone`, which are auto-derived for the new variants.)

- [ ] **Step 4: Commit**

```bash
git add crates/bus/src/domain_events.rs
git commit -m "feat(bus): add TaskDueDateChanged, TaskFocusChanged, RecurringTemplateAdvanced events

New DomainEvent variants for the DeadlineScheduler to subscribe to.
These carry the exact data needed to schedule/cancel timers."
```

---

### Task 7: Emit new events from task CRUD and focus handlers

**Files:**
- Modify: `crates/app-core/src/handlers/tasks/crud.rs`
- Modify: `crates/feature-tasks/src/tool/actions/focus.rs`
- Modify: `crates/feature-tasks/src/tool/mod.rs` (TaskTool needs `domain_bus` field)

- [ ] **Step 1: Emit TaskDueDateChanged from task_create**

In `crates/app-core/src/handlers/tasks/crud.rs`, inside `task_create()`, after the existing `bus.publish(DomainEvent::TaskCreated { ... })` block (around line 155-166), add:

```rust
            // Notify DeadlineScheduler of due date
            if created.due_date.is_some() {
                bus.publish(bus::DomainEvent::TaskDueDateChanged {
                    task_id: id.clone(),
                    due_date: created.due_date.map(|d| d.to_rfc3339()),
                });
            }
```

- [ ] **Step 2: Emit TaskDueDateChanged from task_update**

In the same file, inside `task_update()`, within the `if let Some(ref old) = old_task` block (around line 233-287), after the existing diff checks, add:

```rust
                // Emit due date change for DeadlineScheduler
                if let Some(ref new_due) = patch.due_date {
                    let old_due = old.due_date;
                    let changed = match (old_due, new_due) {
                        (None, None) => false,
                        (Some(_), None) | (None, Some(_)) => true,
                        (Some(a), Some(b)) => Some(a) != *b,
                    };
                    if changed {
                        bus.publish(bus::DomainEvent::TaskDueDateChanged {
                            task_id: task_id.clone(),
                            due_date: new_due.map(|d| d.to_rfc3339()),
                        });
                    }
                }
```

- [ ] **Step 3: Add domain_bus to TaskTool**

In `crates/feature-tasks/src/tool/mod.rs`, find the `TaskTool` struct and add a field:

```rust
    pub domain_bus: Option<Arc<bus::DomainEventBus>>,
```

Also update the constructor / builder to accept this field. Find where `TaskTool` is constructed (search for `TaskTool {` or `TaskTool::new`) and add `domain_bus: None` as default, or wire it from the builder.

- [ ] **Step 4: Emit TaskFocusChanged from handle_focus**

In `crates/feature-tasks/src/tool/actions/focus.rs`, after the successful focus (inside the `if self.repo.focus(...).await?` block, after the activity log), add:

```rust
            // Notify DeadlineScheduler
            if let Some(ref bus) = self.domain_bus {
                bus.publish(bus::DomainEvent::TaskFocusChanged {
                    task_id: id.to_string(),
                    focus_deadline: deadline.map(|d| d.to_rfc3339()),
                });
            }
```

- [ ] **Step 5: Emit TaskFocusChanged from handle_unfocus**

In the same file, after the successful unfocus (inside the `if self.repo.unfocus(id).await?` block), add:

```rust
            // Notify DeadlineScheduler
            if let Some(ref bus) = self.domain_bus {
                bus.publish(bus::DomainEvent::TaskFocusChanged {
                    task_id: id.to_string(),
                    focus_deadline: None,
                });
            }
```

- [ ] **Step 6: Verify it compiles**

Run: `cargo build -p app-core -p feature-tasks`
Expected: Compiles with 0 errors.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/handlers/tasks/crud.rs crates/feature-tasks/src/tool/actions/focus.rs crates/feature-tasks/src/tool/mod.rs
git commit -m "feat(tasks): emit DueDateChanged and FocusChanged domain events

task_create, task_update → TaskDueDateChanged
handle_focus, handle_unfocus → TaskFocusChanged
These events drive the DeadlineScheduler."
```

---

### Task 8: Wire DeadlineScheduler into AppCore

**Files:**
- Modify: `crates/app-core/src/state.rs`
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/app-core/src/init/cron.rs` (move action handlers here)

- [ ] **Step 1: Add scheduler field to AppCore**

In `crates/app-core/src/state.rs`, find the `AppCore` struct definition and add:

```rust
    pub deadline_scheduler: Option<Arc<scheduling::DeadlineScheduler>>,
```

Also update the constructor(s) where `AppCore` is built to initialize this field as `None` initially (it gets set after init).

- [ ] **Step 2: Create the deadline handler closure in init**

In `crates/app-core/src/init/mod.rs`, after the cron service is initialized and the `notification_dispatcher` is available, build the `DeadlineScheduler`. Add this section near where other services are wired:

```rust
    // ── DeadlineScheduler ───────────────────────────────────────────
    let deadline_handler: scheduling::deadline::DeadlineHandler = {
        let todo_repo = repos.tasks.clone();
        let dispatcher = Arc::clone(&cron_result.notification_dispatcher);
        let config_focus = config.todo.focus.clone();
        let timezone = config.timezone.clone();
        let rt = tokio::runtime::Handle::current();

        Arc::new(move |action: scheduling::DeadlineAction| {
            let todo_repo = todo_repo.clone();
            let dispatcher = Arc::clone(&dispatcher);
            let config_focus = config_focus.clone();
            let timezone = timezone.clone();
            tokio::task::block_in_place(|| {
                rt.block_on(async move {
                    match action {
                        scheduling::DeadlineAction::TaskReminder { task_id, label } => {
                            if let Ok(Some(task)) = todo_repo.get(&task_id).await {
                                if task.status != "done" && task.last_reminded_at.is_none() {
                                    let _ = dispatcher
                                        .notify(
                                            &format!("⏰ Task Due: {label}"),
                                            &format!("\"{}\" — deadline approaching!", task.title),
                                        )
                                        .await;
                                    // Mark as reminded
                                    let patch = storage::TaskPatch {
                                        id: task_id,
                                        last_reminded_at: Some(Some(chrono::Utc::now())),
                                        ..Default::default()
                                    };
                                    let _ = todo_repo.update(&patch).await;
                                }
                            }
                        }
                        scheduling::DeadlineAction::FocusWarning { task_id, hours_left } => {
                            if let Ok(Some(task)) = todo_repo.get(&task_id).await {
                                if task.focused_at.is_some() {
                                    let _ = dispatcher
                                        .notify(
                                            &format!("⏰ Focus Deadline: {hours_left}h left"),
                                            &format!("\"{}\" — stay on track", task.title),
                                        )
                                        .await;
                                }
                            }
                        }
                        scheduling::DeadlineAction::FocusExpire { task_id } => {
                            if let Ok(Some(task)) = todo_repo.get(&task_id).await {
                                if task.focus_deadline.map(|d| d < chrono::Utc::now()).unwrap_or(false) {
                                    let _ = todo_repo.unfocus(&task_id).await;
                                    let _ = dispatcher
                                        .notify(
                                            "⏰ Focus Expired",
                                            &format!(
                                                "\"{}\" — auto-unfocused ({}h deadline)",
                                                task.title, config_focus.deadline_hours
                                            ),
                                        )
                                        .await;
                                }
                            }
                        }
                        scheduling::DeadlineAction::SpawnRecurring { template_id } => {
                            if let Err(e) = agent::services::recurring_tasks::RecurringTaskSpawner::check_and_spawn_static(
                                &todo_repo,
                                &timezone,
                            )
                            .await
                            {
                                tracing::warn!("Recurring spawn for {template_id} failed: {e}");
                            }
                        }
                    }
                })
            });
        })
    };

    let deadline_scheduler = Arc::new(scheduling::DeadlineScheduler::new(deadline_handler));
    deadline_scheduler.start().await;
```

- [ ] **Step 3: Subscribe to DomainEvents and populate scheduler**

After creating the scheduler, spawn a subscriber task:

```rust
    // ── DeadlineScheduler event subscriber ──────────────────────────
    {
        let scheduler = Arc::clone(&deadline_scheduler);
        let mut rx = domain_event_bus.subscribe();
        let focus_hours = config.todo.focus.deadline_hours as i64;

        tokio::spawn(async move {
            while let Ok(event) = rx.recv().await {
                match event {
                    bus::DomainEvent::TaskDueDateChanged { task_id, due_date } => {
                        // Cancel existing reminders for this task
                        scheduler.cancel_by_prefix(&format!("reminder:{task_id}")).await;

                        if let Some(due_str) = due_date {
                            if let Ok(due) = chrono::DateTime::parse_from_rfc3339(&due_str) {
                                let due = due.with_timezone(&chrono::Utc);
                                let remind_at = due - chrono::Duration::hours(2);
                                if remind_at > chrono::Utc::now() {
                                    scheduler
                                        .schedule(
                                            remind_at,
                                            scheduling::DeadlineAction::TaskReminder {
                                                task_id,
                                                label: "2h before due".into(),
                                            },
                                        )
                                        .await;
                                }
                            }
                        }
                    }
                    bus::DomainEvent::TaskFocusChanged { task_id, focus_deadline } => {
                        // Cancel all existing focus timers for this task
                        scheduler.cancel_by_prefix(&format!("focus_warn:{task_id}")).await;
                        scheduler.cancel_by_prefix(&format!("focus_expire:{task_id}")).await;

                        if let Some(dl_str) = focus_deadline {
                            if let Ok(dl) = chrono::DateTime::parse_from_rfc3339(&dl_str) {
                                let dl = dl.with_timezone(&chrono::Utc);
                                let now = chrono::Utc::now();

                                // Schedule warnings at 6h, 3h, 1h before deadline
                                for hours in [6u32, 3, 1] {
                                    let warn_at = dl - chrono::Duration::hours(hours as i64);
                                    if warn_at > now {
                                        scheduler
                                            .schedule(
                                                warn_at,
                                                scheduling::DeadlineAction::FocusWarning {
                                                    task_id: task_id.clone(),
                                                    hours_left: hours,
                                                },
                                            )
                                            .await;
                                    }
                                }

                                // Schedule auto-expire at the deadline itself
                                if dl > now {
                                    scheduler
                                        .schedule(
                                            dl,
                                            scheduling::DeadlineAction::FocusExpire {
                                                task_id,
                                            },
                                        )
                                        .await;
                                }
                            }
                        }
                    }
                    bus::DomainEvent::RecurringTemplateAdvanced { template_id, next_instance_date } => {
                        scheduler.cancel_by_prefix(&format!("spawn:{template_id}")).await;

                        if let Some(date_str) = next_instance_date {
                            if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(&date_str) {
                                let dt = dt.with_timezone(&chrono::Utc);
                                if dt > chrono::Utc::now() {
                                    scheduler
                                        .schedule(
                                            dt,
                                            scheduling::DeadlineAction::SpawnRecurring {
                                                template_id,
                                            },
                                        )
                                        .await;
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        });
    }
```

- [ ] **Step 4: Populate from existing data on startup**

After the subscriber is spawned, add an initial scan:

```rust
    // ── Populate DeadlineScheduler from existing data ───────────────
    {
        let scheduler = Arc::clone(&deadline_scheduler);
        let todo_repo = repos.tasks.clone();
        let focus_hours = config.todo.focus.deadline_hours as i64;

        tokio::spawn(async move {
            let now = chrono::Utc::now();

            // Scan tasks with due dates
            let filter = storage::TaskFilter {
                status: Some("todo".into()),
                ..Default::default()
            };
            if let Ok(rows) = todo_repo.list(&filter).await {
                for row in &rows {
                    if let Some(due) = row.due_date {
                        let remind_at = due - chrono::Duration::hours(2);
                        if remind_at > now {
                            scheduler
                                .schedule(
                                    remind_at,
                                    scheduling::DeadlineAction::TaskReminder {
                                        task_id: row.id.clone(),
                                        label: "2h before due".into(),
                                    },
                                )
                                .await;
                        }
                    }
                }
            }

            // Scan tasks with "doing" status too
            let filter_doing = storage::TaskFilter {
                status: Some("doing".into()),
                ..Default::default()
            };
            if let Ok(rows) = todo_repo.list(&filter_doing).await {
                for row in &rows {
                    if let Some(due) = row.due_date {
                        let remind_at = due - chrono::Duration::hours(2);
                        if remind_at > now {
                            scheduler
                                .schedule(
                                    remind_at,
                                    scheduling::DeadlineAction::TaskReminder {
                                        task_id: row.id.clone(),
                                        label: "2h before due".into(),
                                    },
                                )
                                .await;
                        }
                    }
                }
            }

            // Scan focused tasks
            if let Ok(focused) = todo_repo.list_focused().await {
                for row in &focused {
                    if let Some(dl) = row.focus_deadline {
                        for hours in [6u32, 3, 1] {
                            let warn_at = dl - chrono::Duration::hours(hours as i64);
                            if warn_at > now {
                                scheduler
                                    .schedule(
                                        warn_at,
                                        scheduling::DeadlineAction::FocusWarning {
                                            task_id: row.id.clone(),
                                            hours_left: hours,
                                        },
                                    )
                                    .await;
                            }
                        }
                        if dl > now {
                            scheduler
                                .schedule(
                                    dl,
                                    scheduling::DeadlineAction::FocusExpire {
                                        task_id: row.id.clone(),
                                    },
                                )
                                .await;
                        }
                    }
                }
            }

            // Scan recurring templates
            if let Ok(templates) = todo_repo.list_templates().await {
                for tpl in &templates {
                    if let Some(next) = tpl.next_instance_date {
                        if next > now {
                            scheduler
                                .schedule(
                                    next,
                                    scheduling::DeadlineAction::SpawnRecurring {
                                        template_id: tpl.id.clone(),
                                    },
                                )
                                .await;
                        }
                    }
                }
            }

            let count = scheduler.pending_count().await;
            if count > 0 {
                tracing::info!("DeadlineScheduler populated with {count} deadlines from existing data");
            }
        });
    }
```

- [ ] **Step 5: Store scheduler in AppCore**

Where `AppCore` is assembled (later in `init/mod.rs`), set:

```rust
    app_core.deadline_scheduler = Some(deadline_scheduler);
```

- [ ] **Step 6: Verify full workspace compiles**

Run: `cargo build --workspace`
Expected: 0 errors.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/init/mod.rs crates/app-core/src/state.rs crates/app-core/src/init/cron.rs
git commit -m "feat(init): wire DeadlineScheduler into AppCore startup

- Handler dispatches TaskReminder, FocusWarning, FocusExpire, SpawnRecurring
- Event subscriber reacts to TaskDueDateChanged, TaskFocusChanged, RecurringTemplateAdvanced
- Initial scan populates from existing tasks on startup
- Replaces 4 polling cron jobs with zero-cost event-driven timers"
```

---

### Task 9: Emit RecurringTemplateAdvanced from spawner

**Files:**
- Modify: `crates/agent/src/services/recurring_tasks.rs`

- [ ] **Step 1: Add domain_bus parameter to check_and_spawn**

The `RecurringTaskSpawner::check_and_spawn` currently doesn't emit events. Add a `domain_bus` parameter and emit `RecurringTemplateAdvanced` after advancing `next_instance_date`:

In `crates/agent/src/services/recurring_tasks.rs`, change the `check_and_spawn_static` signature to accept an optional bus:

```rust
    pub async fn check_and_spawn_static(
        repo: &storage::TaskRepo,
        timezone: &str,
    ) -> common::Result<()> {
        Self::check_and_spawn(repo, timezone).await
    }
```

Inside `check_and_spawn`, after the `repo.update(&patch).await` call that advances `next_instance_date` (around line 121-132), add the event emission. Since `check_and_spawn` doesn't have the bus, we'll emit from the `DeadlineScheduler` handler instead — after spawning, it re-queries the template and re-schedules. No code change needed here; the startup scan already handles it.

Actually, the cleaner approach: after the spawner runs, the DeadlineScheduler handler should re-query the template to get the new `next_instance_date` and re-schedule. Update the `SpawnRecurring` handler in Task 8 to do this:

```rust
scheduling::DeadlineAction::SpawnRecurring { template_id } => {
    if let Err(e) = agent::services::recurring_tasks::RecurringTaskSpawner::check_and_spawn_static(
        &todo_repo,
        &timezone,
    )
    .await
    {
        tracing::warn!("Recurring spawn for {template_id} failed: {e}");
    }
    // Re-schedule: check if template has a new next_instance_date
    if let Ok(Some(tpl)) = todo_repo.get(&template_id).await {
        if let Some(next) = tpl.next_instance_date {
            if next > chrono::Utc::now() {
                // Re-add to the scheduler (self-scheduling loop)
                // This is handled via the startup scan pattern —
                // the handler doesn't have &scheduler, so emit a domain event instead.
            }
        }
    }
}
```

The cleanest pattern: make `check_and_spawn` emit `RecurringTemplateAdvanced` after advancing the date, which the subscriber picks up and re-schedules.

- [ ] **Step 2: Thread domain_bus through RecurringTaskSpawner**

Add `domain_bus: Option<Arc<bus::DomainEventBus>>` to the spawner or make `check_and_spawn` accept it. In `check_and_spawn`, after `repo.update(&patch)` succeeds:

```rust
            // Emit event for DeadlineScheduler to re-schedule
            // (handled at the call site since check_and_spawn is a static method)
```

For now, the simplest approach: after `SpawnRecurring` fires, the handler re-queries and re-schedules directly. Pass `Arc<DeadlineScheduler>` into the handler closure (it's already captured). Update the `SpawnRecurring` arm in the handler:

```rust
scheduling::DeadlineAction::SpawnRecurring { ref template_id } => {
    let _ = agent::services::recurring_tasks::RecurringTaskSpawner::check_and_spawn_static(
        &todo_repo,
        &timezone,
    )
    .await;
    // Self-reschedule: query updated template for next fire time
    if let Ok(Some(tpl)) = todo_repo.get(template_id).await {
        if let Some(next) = tpl.next_instance_date {
            if next > chrono::Utc::now() {
                // We can't call scheduler.schedule() from inside the handler.
                // Instead, emit the domain event so the subscriber handles it.
                if let Some(ref bus) = domain_bus_for_handler {
                    bus.publish(bus::DomainEvent::RecurringTemplateAdvanced {
                        template_id: template_id.clone(),
                        next_instance_date: Some(next.to_rfc3339()),
                    });
                }
            }
        }
    }
}
```

Capture `domain_event_bus` in the handler closure alongside the other deps.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p app-core`
Expected: 0 errors.

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/mod.rs crates/agent/src/services/recurring_tasks.rs
git commit -m "feat(recurring): self-reschedule via RecurringTemplateAdvanced event

After spawning a recurring instance, emits RecurringTemplateAdvanced
so the DeadlineScheduler re-schedules the next fire time."
```

---

### Task 10: Full integration test

**Files:**
- Modify: `crates/scheduling/src/deadline.rs` (integration-style test)

- [ ] **Step 1: Write an end-to-end scenario test**

Add to the test module in `crates/scheduling/src/deadline.rs`:

```rust
    #[tokio::test]
    async fn full_lifecycle_focus_then_unfocus() {
        let fired: Arc<RwLock<Vec<DeadlineAction>>> = Arc::new(RwLock::new(Vec::new()));
        let fired_clone = Arc::clone(&fired);
        let handler: DeadlineHandler = Arc::new(move |action| {
            let fired = Arc::clone(&fired_clone);
            tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(async {
                    fired.write().await.push(action);
                });
            });
        });

        let scheduler = DeadlineScheduler::new(handler);
        scheduler.start().await;

        // Simulate: task focused with deadline 200ms from now
        let deadline = Utc::now() + chrono::Duration::milliseconds(200);
        scheduler
            .schedule(
                deadline - chrono::Duration::milliseconds(150),
                DeadlineAction::FocusWarning {
                    task_id: "t1".into(),
                    hours_left: 1,
                },
            )
            .await;
        scheduler
            .schedule(
                deadline,
                DeadlineAction::FocusExpire {
                    task_id: "t1".into(),
                },
            )
            .await;
        assert_eq!(scheduler.pending_count().await, 2);

        // Simulate: user unfocuses before deadline
        scheduler.cancel_by_prefix("focus_warn:t1").await;
        scheduler.cancel_by_prefix("focus_expire:t1").await;
        assert_eq!(scheduler.pending_count().await, 0);

        // Wait past the original deadline — nothing should fire
        time::sleep(Duration::from_millis(300)).await;
        assert!(fired.read().await.is_empty());

        scheduler.stop().await;
    }
```

- [ ] **Step 2: Run all scheduling tests**

Run: `cargo nextest run -p scheduling`
Expected: All tests pass.

- [ ] **Step 3: Run workspace clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (or only pre-existing desktop exceptions).

- [ ] **Step 4: Commit**

```bash
git add crates/scheduling/src/deadline.rs
git commit -m "test(scheduling): add full lifecycle integration test

Verifies focus-then-unfocus scenario: deadlines scheduled then cancelled
before firing, nothing fires after cancellation."
```

---

## Summary

| Phase | Tasks | What changes |
|-------|-------|-------------|
| **Phase 1** | Tasks 1-2 | Remove 8 user jobs from auto-registration. Fresh install: 27 → ~14 jobs. |
| **Phase 2** | Tasks 3-10 | New `DeadlineScheduler` replaces 4 polling jobs. Event-driven, zero idle cost, exact-time firing. |

### Jobs after both phases

**Remaining default jobs (all system):**
- `__klyntbot_cognitive_weekly_reflection` — weekly LLM reflection
- `autotuner_nightly` — experiment evaluation (if enabled)
- `__klyntbot_atom_decay_daily` — knowledge decay
- `__klyntbot_atom_extraction_catchall` — catch unprocessed notes
- `__klyntbot_session_cleanup` — stale session deletion
- `__klyntbot_blackboard_cleanup` — stale blackboard entries
- `__klyntbot_memory_maintenance` — embedding pruning
- `__klyntbot_analytics_cleanup` — analytics + fact pruning
- `__klyntbot_learning_analysis` — tool outcome analysis
- `__klyntbot_cross_domain_nightly` — insight batch
- `__klyntbot_mirror_weekly_narrative` — mirror reflection
- `__klyntbot_mirror_cleanup` — mirror data cleanup
- `__klyntbot_insight_refresh` — insight snapshots
- `__klyntbot_daily_planning` (if enabled)
- `__klyntbot_finance_*` (if enabled)
- `proactive_scan` (if enabled)

**Lazy-created on first use:**
- `todo_daily_digest` — first task created
- `__klyntbot_weekly_report` — user enables from automation page
- `__klyntbot_morning_briefing` — first knowledge atom
- `__klyntbot_weekly_knowledge_digest` — first knowledge atom

**Replaced by DeadlineScheduler:**
- ~~`todo_focus_check`~~ → `FocusWarning` deadline
- ~~`todo_overdue_check`~~ → `FocusExpire` deadline
- ~~`__klyntbot_reminder_check`~~ → `TaskReminder` deadline
- ~~`__klyntbot_recurring_tasks`~~ → `SpawnRecurring` deadline
