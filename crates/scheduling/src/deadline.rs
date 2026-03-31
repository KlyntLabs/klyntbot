//! Event-driven deadline scheduler with priority-queue execution.
//!
//! [`DeadlineScheduler`] maintains an in-memory map of pending deadlines keyed
//! by a deduplication string. An executor task sleeps until the next deadline
//! fires, then calls the registered [`DeadlineHandler`]. Early wakes via
//! [`tokio::sync::Notify`] ensure additions and cancellations take effect
//! immediately.
//!
//! # Design
//!
//! - **Zero CPU when idle** — the executor calls `tokio::time::sleep_until`
//!   rather than polling. When no deadlines are pending it sleeps for 24 h,
//!   effectively dormant until a `Notify` wakes it.
//! - **Deduplication** — entries are keyed by [`DeadlineAction::dedup_key`].
//!   Re-scheduling the same logical action updates the fire time in place.
//! - **Prefix cancellation** — `cancel_by_prefix("task_reminder:")` removes
//!   all reminders for a given task without knowing individual keys.

use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use tokio::sync::{Notify, RwLock};
use tokio::task::JoinHandle;
use tokio::time::{Duration, Instant};
use tracing::{debug, info, warn};

use crate::deadline_actions::DeadlineAction;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Callback invoked when a deadline fires.
///
/// The closure receives the [`DeadlineAction`] and must be `Send + Sync`
/// because it is called from within a spawned Tokio task.
pub type DeadlineHandler = Arc<dyn Fn(DeadlineAction) + Send + Sync>;

/// A single pending deadline entry.
#[derive(Debug, Clone)]
pub struct DeadlineEntry {
    /// When the action should fire (UTC).
    pub fire_at: DateTime<Utc>,
    /// The action to execute.
    pub action: DeadlineAction,
}

// ---------------------------------------------------------------------------
// DeadlineScheduler
// ---------------------------------------------------------------------------

/// Shared inner state — lives inside `Arc<RwLock<...>>`.
struct Inner {
    /// Pending deadlines, keyed by `action.dedup_key()`.
    entries: BTreeMap<String, DeadlineEntry>,
    /// Set to `false` to signal the executor to exit its loop.
    running: bool,
}

impl Inner {
    fn new() -> Self {
        Self {
            entries: BTreeMap::new(),
            running: false,
        }
    }

    /// Find the entry with the earliest `fire_at`, if any.
    fn soonest(&self) -> Option<&DeadlineEntry> {
        self.entries.values().min_by_key(|e| e.fire_at)
    }
}

/// Event-driven timer that sleeps until the exact next deadline.
///
/// # Lifecycle
///
/// ```text
/// DeadlineScheduler::new(handler)
///     .start()          // spawns the executor task
///     .schedule(...)    // add / update deadlines
///     .cancel_by_key()  // remove one deadline
///     .stop()           // abort executor and clear state
/// ```
pub struct DeadlineScheduler {
    inner: Arc<RwLock<Inner>>,
    /// Wakes the executor whenever the pending set changes.
    wake: Arc<Notify>,
    /// The spawned executor task handle.
    task: Arc<RwLock<Option<JoinHandle<()>>>>,
    /// Callback invoked on each fired action.
    handler: DeadlineHandler,
}

impl DeadlineScheduler {
    /// Create a new scheduler with the given handler.
    ///
    /// Call [`start`](Self::start) to begin processing deadlines.
    pub fn new(handler: DeadlineHandler) -> Self {
        Self {
            inner: Arc::new(RwLock::new(Inner::new())),
            wake: Arc::new(Notify::new()),
            task: Arc::new(RwLock::new(None)),
            handler,
        }
    }

    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    /// Start the executor loop in a background Tokio task.
    ///
    /// Calling `start` more than once is a no-op when already running.
    pub async fn start(&self) {
        {
            let mut inner = self.inner.write().await;
            if inner.running {
                return;
            }
            inner.running = true;
        }

        let inner = Arc::clone(&self.inner);
        let wake = Arc::clone(&self.wake);
        let handler = Arc::clone(&self.handler);

        let task = tokio::spawn(async move {
            Self::executor_loop(inner, wake, handler).await;
        });

        *self.task.write().await = Some(task);
        info!("DeadlineScheduler started");
    }

    /// Stop the executor and abort the background task.
    ///
    /// Pending deadlines are retained in memory — call [`Self::start`] again
    /// to resume processing them.
    pub async fn stop(&self) {
        {
            let mut inner = self.inner.write().await;
            inner.running = false;
        }

        // Wake the loop so it exits promptly rather than waiting for the next
        // sleep to expire.
        self.wake.notify_one();

        let mut task_guard = self.task.write().await;
        if let Some(task) = task_guard.take() {
            task.abort();
        }

        info!("DeadlineScheduler stopped");
    }

    // -----------------------------------------------------------------------
    // Scheduling API
    // -----------------------------------------------------------------------

    /// Schedule an action to fire at `fire_at`.
    ///
    /// If an entry with the same [`DeadlineAction::dedup_key`] already exists
    /// it is replaced (updated in place).
    ///
    /// # Behaviour on past times
    ///
    /// If `fire_at` is in the past the executor will fire the action on its
    /// very next iteration (typically within milliseconds).
    pub async fn schedule(&self, fire_at: DateTime<Utc>, action: DeadlineAction) {
        let key = action.dedup_key();
        debug!("DeadlineScheduler: scheduling '{key}' at {fire_at}");

        {
            let mut inner = self.inner.write().await;
            inner.entries.insert(key, DeadlineEntry { fire_at, action });
        }

        // Wake the executor so it re-evaluates the soonest deadline.
        self.wake.notify_one();
    }

    /// Cancel the deadline with the exact dedup key.
    ///
    /// Returns `true` if an entry was removed, `false` if no matching entry
    /// was found.
    pub async fn cancel_by_key(&self, key: &str) -> bool {
        let removed = {
            let mut inner = self.inner.write().await;
            inner.entries.remove(key).is_some()
        };

        if removed {
            debug!("DeadlineScheduler: cancelled '{key}'");
            self.wake.notify_one();
        }

        removed
    }

    /// Cancel all deadlines whose keys start with `prefix`.
    ///
    /// Returns the number of entries removed.
    ///
    /// # Example
    ///
    /// ```ignore
    /// // Remove all reminders for a specific task when it is completed.
    /// scheduler.cancel_by_prefix("task_reminder:task-uuid-here").await;
    /// ```
    pub async fn cancel_by_prefix(&self, prefix: &str) -> usize {
        let count = {
            let mut inner = self.inner.write().await;
            let keys: Vec<String> = inner
                .entries
                .keys()
                .filter(|k| k.starts_with(prefix))
                .cloned()
                .collect();
            let n = keys.len();
            for k in keys {
                inner.entries.remove(&k);
            }
            n
        };

        if count > 0 {
            debug!("DeadlineScheduler: cancelled {count} entries with prefix '{prefix}'");
            self.wake.notify_one();
        }

        count
    }

    /// Return the number of pending deadline entries.
    pub async fn pending_count(&self) -> usize {
        self.inner.read().await.entries.len()
    }

    // -----------------------------------------------------------------------
    // Executor loop (private)
    // -----------------------------------------------------------------------

    /// The core executor loop.  Runs until `inner.running` becomes `false`.
    async fn executor_loop(inner: Arc<RwLock<Inner>>, wake: Arc<Notify>, handler: DeadlineHandler) {
        loop {
            // Exit check.
            if !inner.read().await.running {
                break;
            }

            // Compute how long to sleep until the next deadline.
            let sleep_duration = {
                let guard = inner.read().await;
                match guard.soonest() {
                    Some(entry) => {
                        let now = Utc::now();
                        let delta = entry.fire_at.signed_duration_since(now);
                        // `max(0)` ensures past deadlines fire immediately.
                        let millis = delta.num_milliseconds().max(0) as u64;
                        Duration::from_millis(millis)
                    }
                    // No pending deadlines — park for 24 h; a Notify will
                    // wake us if something is scheduled before then.
                    None => Duration::from_secs(86_400),
                }
            };

            let deadline = Instant::now() + sleep_duration;

            // Sleep until the next deadline OR until we are woken early.
            tokio::select! {
                _ = tokio::time::sleep_until(deadline) => {}
                _ = wake.notified() => {}
            }

            // Exit check after wake.
            if !inner.read().await.running {
                break;
            }

            // Collect all entries whose fire_at is now (or in the past).
            let now = Utc::now();
            let due: Vec<DeadlineEntry> = {
                let mut guard = inner.write().await;
                let due_keys: Vec<String> = guard
                    .entries
                    .iter()
                    .filter(|(_, e)| e.fire_at <= now)
                    .map(|(k, _)| k.clone())
                    .collect();

                due_keys
                    .into_iter()
                    .filter_map(|k| guard.entries.remove(&k))
                    .collect()
            };

            // Fire each due action.
            for entry in due {
                let key = entry.action.dedup_key();
                info!("DeadlineScheduler: firing '{key}'");
                // Unwind safety: if the handler panics we log and continue
                // rather than crashing the executor.
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handler(entry.action.clone());
                }));
                if let Err(e) = result {
                    warn!("DeadlineScheduler: handler panicked for '{key}': {:?}", e);
                }
            }
        }

        debug!("DeadlineScheduler executor loop exited");
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Arc as StdArc;

    fn make_handler(counter: StdArc<AtomicU32>) -> DeadlineHandler {
        Arc::new(move |_action: DeadlineAction| {
            counter.fetch_add(1, Ordering::SeqCst);
        })
    }

    #[tokio::test]
    async fn test_schedule_and_fire() {
        let counter = StdArc::new(AtomicU32::new(0));
        let scheduler = DeadlineScheduler::new(make_handler(counter.clone()));
        scheduler.start().await;

        // Schedule in the past — should fire on the next executor iteration.
        let past = Utc::now() - chrono::Duration::seconds(1);
        scheduler
            .schedule(
                past,
                DeadlineAction::TaskReminder {
                    task_id: "t1".into(),
                    label: "Test".into(),
                },
            )
            .await;

        // Give the executor a moment to process.
        tokio::time::sleep(Duration::from_millis(100)).await;

        assert_eq!(counter.load(Ordering::SeqCst), 1);

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn test_dedup_replaces_entry() {
        let counter = StdArc::new(AtomicU32::new(0));
        let scheduler = DeadlineScheduler::new(make_handler(counter.clone()));
        scheduler.start().await;

        let far_future = Utc::now() + chrono::Duration::hours(24);

        scheduler
            .schedule(
                far_future,
                DeadlineAction::TaskReminder {
                    task_id: "t2".into(),
                    label: "First".into(),
                },
            )
            .await;
        // Schedule again with same task_id — replaces the existing entry.
        scheduler
            .schedule(
                far_future,
                DeadlineAction::TaskReminder {
                    task_id: "t2".into(),
                    label: "Second".into(),
                },
            )
            .await;

        assert_eq!(scheduler.pending_count().await, 1);
        assert_eq!(counter.load(Ordering::SeqCst), 0); // nothing fired yet

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn test_cancel_by_key() {
        let counter = StdArc::new(AtomicU32::new(0));
        let scheduler = DeadlineScheduler::new(make_handler(counter.clone()));
        scheduler.start().await;

        let far_future = Utc::now() + chrono::Duration::hours(24);
        scheduler
            .schedule(
                far_future,
                DeadlineAction::FocusExpire {
                    task_id: "t3".into(),
                },
            )
            .await;

        assert_eq!(scheduler.pending_count().await, 1);

        let removed = scheduler.cancel_by_key("focus_expire:t3").await;
        assert!(removed);
        assert_eq!(scheduler.pending_count().await, 0);

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn test_cancel_by_prefix() {
        let counter = StdArc::new(AtomicU32::new(0));
        let scheduler = DeadlineScheduler::new(make_handler(counter.clone()));
        scheduler.start().await;

        let far_future = Utc::now() + chrono::Duration::hours(24);

        for i in 0..3u32 {
            scheduler
                .schedule(
                    far_future,
                    DeadlineAction::TaskReminder {
                        task_id: format!("task-{i}"),
                        label: "label".into(),
                    },
                )
                .await;
        }
        // Also add an unrelated entry.
        scheduler
            .schedule(
                far_future,
                DeadlineAction::SpawnRecurring {
                    template_id: "tmpl-1".into(),
                },
            )
            .await;

        assert_eq!(scheduler.pending_count().await, 4);

        let removed = scheduler.cancel_by_prefix("task_reminder:").await;
        assert_eq!(removed, 3);
        assert_eq!(scheduler.pending_count().await, 1);

        // Make sure the drop count is still 0 — nothing fired.
        assert_eq!(counter.load(Ordering::SeqCst), 0);

        scheduler.stop().await;
    }

    #[tokio::test]
    async fn test_pending_count_zero_when_empty() {
        let scheduler = DeadlineScheduler::new(Arc::new(|_| {}));
        assert_eq!(scheduler.pending_count().await, 0);
    }
}
