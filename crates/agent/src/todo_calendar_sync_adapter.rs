//! Adapter: bridges CalendarHandler (tools crate) → CalendarSyncHandler (feature-todo).
//!
//! feature-todo defines `CalendarSyncHandler` with a single `sync_calendar()` method.
//! The agent's `CalendarSyncAdapter` implements the broader `CalendarHandler` from tools.
//! This thin wrapper adapts the two traits without creating a circular dependency.

use async_trait::async_trait;
use common::Result;
use feature_todo::CalendarSyncHandler;
use std::sync::Arc;
use tools::calendar_tool::CalendarHandler;

/// Wraps an Arc<CalendarHandler> to satisfy feature-todo's narrower CalendarSyncHandler.
pub struct TodoCalendarSyncAdapter {
    inner: Arc<dyn CalendarHandler>,
}

impl TodoCalendarSyncAdapter {
    pub fn new(inner: Arc<dyn CalendarHandler>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl CalendarSyncHandler for TodoCalendarSyncAdapter {
    async fn sync_calendar(&self) -> Result<()> {
        // CalendarHandler::sync_calendar returns Result<Value>; we discard the value.
        self.inner.sync_calendar().await.map(|_| ())
    }

    async fn push_task(&self, task_id: &str) -> Result<()> {
        self.inner.push_single_task(task_id).await.map(|_| ())
    }

    async fn remove_event(&self, calendar_event_uid: &str) -> Result<()> {
        self.inner
            .remove_single_event(calendar_event_uid)
            .await
            .map(|_| ())
    }
}
