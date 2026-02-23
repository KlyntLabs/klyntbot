//! Calendar sync trait for dependency inversion.
//!
//! Defined here so TodoTool can call it without depending on the calendar crate.

use async_trait::async_trait;
use common::Result;

/// CalendarSyncHandler trait for dependency inversion.
/// Implemented by the CalendarHandler in the agent/calendar crate (Layer 4-5).
/// Defined here so the todo feature tool can trigger syncs without circular deps.
#[async_trait]
pub trait CalendarSyncHandler: Send + Sync {
    /// Trigger an immediate calendar sync (best-effort).
    async fn sync_calendar(&self) -> Result<()>;

    /// Push a single task to calendar by task ID.
    async fn push_task(&self, task_id: &str) -> Result<()>;

    /// Remove a calendar event by its UID.
    async fn remove_event(&self, calendar_event_uid: &str) -> Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    struct MockCalendarSync {
        pub called: std::sync::atomic::AtomicBool,
    }

    #[async_trait]
    impl CalendarSyncHandler for MockCalendarSync {
        async fn sync_calendar(&self) -> Result<()> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn push_task(&self, _task_id: &str) -> Result<()> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        async fn remove_event(&self, _calendar_event_uid: &str) -> Result<()> {
            self.called.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_trait_is_object_safe() {
        let handler: Arc<dyn CalendarSyncHandler> = Arc::new(MockCalendarSync {
            called: std::sync::atomic::AtomicBool::new(false),
        });
        handler.sync_calendar().await.unwrap();
    }
}
