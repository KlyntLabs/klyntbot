//! `DndManager` — orchestrates activate / deactivate / extend operations.
//!
//! Depends on two injected traits so callers can swap real impls for mocks:
//!
//! * [`DndScheduler`] — schedules the end-of-session alarm and returns an opaque id.
//! * [`FocusBridge`] — calls the OS-level Do-Not-Disturb toggle.

use std::sync::Arc;

use async_trait::async_trait;
use jiff::Timestamp;

use common::{KlyntbotError, Result};

use crate::repo::{FocusSession, FocusSessionRepo};
use crate::FocusMode;

// ── Traits ────────────────────────────────────────────────────────────────────

/// Schedules a one-shot alarm and returns an opaque string id.
/// Cancel removes that fire before it triggers.
#[async_trait]
pub trait DndScheduler: Send + Sync {
    async fn schedule(&self, fire_at: Timestamp) -> Result<String>;
    async fn cancel(&self, alarm_id: &str) -> Result<()>;
}

/// Turns the OS-level DND mode on/off.
/// Implementations must be idempotent — calling `turn_on` when already on is OK.
#[async_trait]
pub trait FocusBridge: Send + Sync {
    async fn turn_on(&self, mode: FocusMode) -> Result<()>;
    async fn turn_off(&self, mode: FocusMode) -> Result<()>;
    async fn is_ready(&self) -> Result<bool>;
}

// ── Manager ───────────────────────────────────────────────────────────────────

/// Coordinates the repo, OS bridge, and alarm scheduler for a timed DND session.
///
/// At most one session per [`FocusMode`] is active at any time (enforced by
/// both the `ix_focus_sessions_active` DB index and the `activate` guard).
pub struct DndManager {
    repo: FocusSessionRepo,
    scheduler: Arc<dyn DndScheduler>,
    bridge: Arc<dyn FocusBridge>,
}

impl DndManager {
    pub fn new(
        repo: FocusSessionRepo,
        scheduler: Arc<dyn DndScheduler>,
        bridge: Arc<dyn FocusBridge>,
    ) -> Self {
        Self {
            repo,
            scheduler,
            bridge,
        }
    }

    /// Start a new DND session for `mode`, ending at `ends_at`.
    ///
    /// Returns `Err(StorageConflict)` when a session for the same mode is already active.
    /// Callers that want to move the end time forward should call [`extend`] instead.
    pub async fn activate(
        &self,
        mode: FocusMode,
        ends_at: Timestamp,
    ) -> Result<FocusSession> {
        // Guard: refuse double-activation.
        if self
            .repo
            .active(mode)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?
            .is_some()
        {
            return Err(KlyntbotError::StorageConflict(
                "focus session already active for this mode".into(),
            ));
        }

        // Turn on OS DND first (side-effect before we schedule).
        self.bridge
            .turn_on(mode)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("bridge.turn_on failed: {e}")))?;

        // Schedule the end alarm. If this fails we attempt a cleanup turn_off.
        let alarm_id = match self.scheduler.schedule(ends_at).await {
            Ok(id) => id,
            Err(e) => {
                // Best-effort cleanup — ignore secondary error.
                let _ = self.bridge.turn_off(mode).await;
                return Err(e);
            }
        };

        // Persist the session.
        let session = self
            .repo
            .insert_active(
                mode,
                Timestamp::now(),
                ends_at,
                Some(&alarm_id),
                "launcher",
            )
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        Ok(session)
    }

    /// End the active session for `mode`. Idempotent — returns `Ok(())` if nothing
    /// was active.
    pub async fn deactivate(&self, mode: FocusMode) -> Result<()> {
        let Some(session) = self
            .repo
            .active(mode)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?
        else {
            return Ok(());
        };

        self.bridge
            .turn_off(mode)
            .await
            .map_err(|e| KlyntbotError::Storage(format!("bridge.turn_off failed: {e}")))?;

        if let Some(ref alarm_id) = session.alarm_id {
            let _ = self.scheduler.cancel(alarm_id).await;
        }

        self.repo
            .end(session.id, Timestamp::now())
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        Ok(())
    }

    /// Move the end time of the active session for `mode` to `new_ends_at`.
    ///
    /// Returns `Err(StorageNotFound)` when no session is active.
    pub async fn extend(
        &self,
        mode: FocusMode,
        new_ends_at: Timestamp,
    ) -> Result<FocusSession> {
        let session = self
            .repo
            .active(mode)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?
            .ok_or_else(|| {
                KlyntbotError::StorageNotFound(
                    "no active focus session to extend".into(),
                )
            })?;

        // Cancel old alarm (best-effort).
        if let Some(ref old_alarm_id) = session.alarm_id {
            let _ = self.scheduler.cancel(old_alarm_id).await;
        }

        // Schedule the new alarm.
        let new_alarm_id = self.scheduler.schedule(new_ends_at).await?;

        // Update the DB.
        self.repo
            .extend(session.id, new_ends_at, Some(&new_alarm_id))
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?;

        // Return the updated session.
        let updated = self
            .repo
            .active(mode)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))?
            .ok_or_else(|| {
                KlyntbotError::StorageNotFound(
                    "active session disappeared after extend".into(),
                )
            })?;

        Ok(updated)
    }

    /// Return the current active session for `mode`, or `None`.
    pub async fn active(&self, mode: FocusMode) -> Result<Option<FocusSession>> {
        self.repo
            .active(mode)
            .await
            .map_err(|e| KlyntbotError::Storage(e.to_string()))
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use storage::StoragePool;
    use tools_core::FeatureMigration;

    // ── Mocks ─────────────────────────────────────────────────────────────

    #[derive(Debug, Default)]
    struct MockScheduler {
        calls: Mutex<Vec<String>>,
        next_id: Mutex<u32>,
    }

    #[async_trait]
    impl DndScheduler for MockScheduler {
        async fn schedule(&self, _fire_at: Timestamp) -> Result<String> {
            let mut n = self.next_id.lock().unwrap();
            *n += 1;
            let id = format!("alarm-{}", *n);
            self.calls.lock().unwrap().push(format!("schedule:{id}"));
            Ok(id)
        }
        async fn cancel(&self, alarm_id: &str) -> Result<()> {
            self.calls.lock().unwrap().push(format!("cancel:{alarm_id}"));
            Ok(())
        }
    }

    #[derive(Debug, Default)]
    struct MockBridge {
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl FocusBridge for MockBridge {
        async fn turn_on(&self, mode: FocusMode) -> Result<()> {
            self.calls.lock().unwrap().push(format!("turn_on:{}", mode.as_str()));
            Ok(())
        }
        async fn turn_off(&self, mode: FocusMode) -> Result<()> {
            self.calls.lock().unwrap().push(format!("turn_off:{}", mode.as_str()));
            Ok(())
        }
        async fn is_ready(&self) -> Result<bool> {
            Ok(true)
        }
    }

    async fn make_manager() -> (DndManager, Arc<MockScheduler>, Arc<MockBridge>) {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &[FeatureMigration {
                feature_name: "focus".into(),
                version: 1,
                description: "focus_sessions table".into(),
                sql: include_str!("migrations/001_focus_sessions.sql").into(),
            }],
        )
        .await
        .unwrap();
        let repo = FocusSessionRepo::new(pool.inner().clone());
        let scheduler = Arc::new(MockScheduler::default());
        let bridge = Arc::new(MockBridge::default());
        let manager = DndManager::new(
            repo,
            Arc::clone(&scheduler) as Arc<dyn DndScheduler>,
            Arc::clone(&bridge) as Arc<dyn FocusBridge>,
        );
        (manager, scheduler, bridge)
    }

    fn future() -> Timestamp {
        Timestamp::from_second(9_999_999_999).unwrap()
    }

    // -- Tests ---------------------------------------------------------------

    #[tokio::test]
    async fn activate_creates_active_session_and_fires_bridge() {
        let (mgr, sched, bridge) = make_manager().await;
        let session = mgr.activate(FocusMode::Dnd, future()).await.unwrap();
        assert_eq!(session.mode, FocusMode::Dnd);
        assert_eq!(session.alarm_id.as_deref(), Some("alarm-1"));

        // active() now returns Some
        let active = mgr.active(FocusMode::Dnd).await.unwrap();
        assert!(active.is_some());

        // bridge.turn_on was called exactly once
        let b_calls = bridge.calls.lock().unwrap().clone();
        assert_eq!(b_calls, vec!["turn_on:dnd"]);

        // scheduler.schedule was called exactly once
        let s_calls = sched.calls.lock().unwrap().clone();
        assert!(s_calls.iter().any(|c: &String| c.starts_with("schedule:")));
        assert_eq!(s_calls.len(), 1);
    }

    #[tokio::test]
    async fn activate_twice_returns_conflict_no_double_fire() {
        let (mgr, sched, bridge) = make_manager().await;
        mgr.activate(FocusMode::Dnd, future()).await.unwrap();

        let err = mgr.activate(FocusMode::Dnd, future()).await.unwrap_err();
        assert!(
            matches!(err, KlyntbotError::StorageConflict(_)),
            "expected StorageConflict, got {err:?}"
        );

        // Only one turn_on and one schedule total
        assert_eq!(bridge.calls.lock().unwrap().len(), 1);
        assert_eq!(sched.calls.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn activate_then_deactivate_clears_session() {
        let (mgr, sched, bridge) = make_manager().await;
        mgr.activate(FocusMode::Dnd, future()).await.unwrap();
        mgr.deactivate(FocusMode::Dnd).await.unwrap();

        assert!(mgr.active(FocusMode::Dnd).await.unwrap().is_none());

        let b = bridge.calls.lock().unwrap().clone();
        assert!(b.contains(&"turn_on:dnd".to_string()));
        assert!(b.contains(&"turn_off:dnd".to_string()));

        let s = sched.calls.lock().unwrap().clone();
        assert!(s.iter().any(|c: &String| c.starts_with("schedule:")));
        assert!(s.iter().any(|c: &String| c.starts_with("cancel:")));
    }

    #[tokio::test]
    async fn activate_then_extend_updates_alarm_and_ends_at() {
        let (mgr, sched, _bridge) = make_manager().await;
        mgr.activate(FocusMode::Dnd, future()).await.unwrap();

        let new_end = Timestamp::from_second(10_000_000_000).unwrap();
        let updated = mgr.extend(FocusMode::Dnd, new_end).await.unwrap();
        assert_eq!(updated.ends_at, new_end);
        // alarm-1 cancelled, alarm-2 scheduled
        assert_eq!(updated.alarm_id.as_deref(), Some("alarm-2"));

        let s = sched.calls.lock().unwrap().clone();
        assert!(s.iter().any(|c: &String| c == "cancel:alarm-1"));
        assert!(s.iter().any(|c: &String| c == "schedule:alarm-2"));
    }

    #[tokio::test]
    async fn deactivate_when_nothing_active_is_ok_and_no_bridge_call() {
        let (mgr, _sched, bridge) = make_manager().await;
        mgr.deactivate(FocusMode::Dnd).await.unwrap(); // must not panic
        assert!(bridge.calls.lock().unwrap().is_empty());
    }
}
