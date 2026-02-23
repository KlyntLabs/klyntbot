//! Mock CalendarHandler for testing calendar reconciliation

use async_trait::async_trait;
use calendar::CalendarEvent;
use common::Result;
use serde_json::Value;
use std::sync::{Arc, Mutex};
use tools::calendar_tool::CalendarHandler;

/// Simple mock CalendarHandler that returns a predefined list of events
#[allow(dead_code)]
pub struct MockCalendarHandler {
    events: Arc<Mutex<Vec<CalendarEvent>>>,
}

impl MockCalendarHandler {
    pub fn new(events: Vec<CalendarEvent>) -> Self {
        Self {
            events: Arc::new(Mutex::new(events)),
        }
    }

    pub fn set_events(&self, events: Vec<CalendarEvent>) {
        *self.events.lock().unwrap() = events;
    }
}

#[async_trait]
impl CalendarHandler for MockCalendarHandler {
    async fn sync_calendar(&self) -> Result<Value> {
        Ok(serde_json::json!({"status": "mock"}))
    }

    async fn list_events(&self, _limit: usize) -> Result<Value> {
        Ok(serde_json::json!({"events": []}))
    }

    async fn create_event(
        &self,
        _summary: String,
        _description: Option<String>,
        _start: String,
        _end: String,
    ) -> Result<Value> {
        Ok(serde_json::json!({"status": "created"}))
    }

    async fn get_status(&self) -> Result<Value> {
        Ok(serde_json::json!({"status": "ok"}))
    }

    async fn get_event(&self, uid: &str) -> Result<Option<CalendarEvent>> {
        let events = self.events.lock().unwrap();
        Ok(events.iter().find(|e| e.uid == uid).cloned())
    }

    async fn get_events_for_reconciliation(&self) -> Result<Vec<CalendarEvent>> {
        Ok(self.events.lock().unwrap().clone())
    }

    async fn push_single_task(&self, _task_id: &str) -> Result<Value> {
        Ok(serde_json::json!({"status": "pushed"}))
    }

    async fn remove_single_event(&self, _calendar_event_uid: &str) -> Result<Value> {
        Ok(serde_json::json!({"status": "removed"}))
    }

    async fn pull_all_events(&self) -> Result<Value> {
        Ok(serde_json::json!({"status": "success", "events_pulled": 0}))
    }

    async fn push_all_tasks(&self) -> Result<Value> {
        Ok(serde_json::json!({"status": "success", "tasks_pushed": 0}))
    }
}
