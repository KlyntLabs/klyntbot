//! Mock CalDAV provider for testing calendar reconciliation without real server
//!
//! Provides in-memory calendar event storage with configurable behaviors:
//! - Add/remove/modify events
//! - Simulate network failures
//! - Inject latency
//! - Track request history

use async_trait::async_trait;
use calendar::{CalendarEvent, CalendarProvider, EventSource};
use chrono::{DateTime, Duration, Utc};
use common::{CalendarError, KlyntbotError, Result};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// MockCalendarProvider - In-memory calendar for testing
#[derive(Clone)]
pub struct MockCalendarProvider {
    name: String,
    events: Arc<Mutex<HashMap<String, CalendarEvent>>>,
    sync_token: Arc<Mutex<Option<String>>>,
    fail_next_request: Arc<Mutex<bool>>,
    request_delay_ms: Arc<Mutex<u64>>,
    request_count: Arc<Mutex<usize>>,
}

impl MockCalendarProvider {
    /// Create a new mock calendar provider
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            events: Arc::new(Mutex::new(HashMap::new())),
            sync_token: Arc::new(Mutex::new(None)),
            fail_next_request: Arc::new(Mutex::new(false)),
            request_delay_ms: Arc::new(Mutex::new(0)),
            request_count: Arc::new(Mutex::new(0)),
        }
    }

    /// Add a calendar event
    pub fn add_event(&self, uid: &str, summary: &str, start: DateTime<Utc>) {
        let event = CalendarEvent {
            uid: uid.to_string(),
            summary: summary.to_string(),
            description: None,
            start,
            end: start + Duration::hours(1),
            source: EventSource::TodoItem,
            etag: Some(format!("etag-{}", uid)),
        };
        self.events.lock().unwrap().insert(uid.to_string(), event);
    }

    /// Add a calendar event with full details
    pub fn add_event_full(
        &self,
        uid: &str,
        summary: &str,
        description: Option<String>,
        start: DateTime<Utc>,
        end: DateTime<Utc>,
    ) {
        let event = CalendarEvent {
            uid: uid.to_string(),
            summary: summary.to_string(),
            description,
            start,
            end,
            source: EventSource::TodoItem,
            etag: Some(format!("etag-{}", uid)),
        };
        self.events.lock().unwrap().insert(uid.to_string(), event);
    }

    /// Update event start time
    pub fn set_event_start(&self, uid: &str, new_start: DateTime<Utc>) {
        if let Some(event) = self.events.lock().unwrap().get_mut(uid) {
            let duration = event.end - event.start;
            event.start = new_start;
            event.end = new_start + duration;
            // Update etag to simulate CalDAV behavior
            event.etag = Some(format!("etag-{}-{}", uid, new_start.timestamp()));
        }
    }

    /// Update event summary (title)
    pub fn set_event_summary(&self, uid: &str, new_summary: &str) {
        if let Some(event) = self.events.lock().unwrap().get_mut(uid) {
            event.summary = new_summary.to_string();
            event.etag = Some(format!("etag-{}-updated", uid));
        }
    }

    /// Update event description
    pub fn set_event_description(&self, uid: &str, new_description: Option<String>) {
        if let Some(event) = self.events.lock().unwrap().get_mut(uid) {
            event.description = new_description;
            event.etag = Some(format!("etag-{}-desc", uid));
        }
    }

    /// Mark event as completed (stored in description for mock)
    pub fn set_event_status(&self, uid: &str, status: &str) {
        if let Some(event) = self.events.lock().unwrap().get_mut(uid) {
            // Store status in description field (mock implementation)
            let current_desc = event.description.clone().unwrap_or_default();
            event.description = Some(format!("STATUS:{}\n{}", status, current_desc));
            event.etag = Some(format!("etag-{}-status", uid));
        }
    }

    /// Get event status from description
    pub fn get_event_status(&self, uid: &str) -> Option<String> {
        self.events.lock().unwrap().get(uid).and_then(|event| {
            event.description.as_ref().and_then(|desc| {
                if desc.starts_with("STATUS:") {
                    desc.lines().next().map(|line| {
                        line.strip_prefix("STATUS:")
                            .unwrap_or("")
                            .to_string()
                    })
                } else {
                    None
                }
            })
        })
    }

    /// Remove an event (simulates deletion)
    pub fn remove_event(&self, uid: &str) {
        self.events.lock().unwrap().remove(uid);
    }

    /// Clear all events
    pub fn clear_events(&self) {
        self.events.lock().unwrap().clear();
    }

    /// Get event count
    pub fn event_count(&self) -> usize {
        self.events.lock().unwrap().len()
    }

    /// Check if event exists
    pub fn has_event(&self, uid: &str) -> bool {
        self.events.lock().unwrap().contains_key(uid)
    }

    /// Get event by UID
    pub fn get_event(&self, uid: &str) -> Option<CalendarEvent> {
        self.events.lock().unwrap().get(uid).cloned()
    }

    /// Simulate network failure on next request
    pub fn fail_next_request(&self) {
        *self.fail_next_request.lock().unwrap() = true;
    }

    /// Set network delay in milliseconds
    pub fn set_delay(&self, delay_ms: u64) {
        *self.request_delay_ms.lock().unwrap() = delay_ms;
    }

    /// Get number of requests made
    pub fn request_count(&self) -> usize {
        *self.request_count.lock().unwrap()
    }

    /// Reset request counter
    pub fn reset_request_count(&self) {
        *self.request_count.lock().unwrap() = 0;
    }

    /// Get current sync token
    pub fn current_sync_token(&self) -> Option<String> {
        self.sync_token.lock().unwrap().clone()
    }
}

#[async_trait]
impl CalendarProvider for MockCalendarProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn get_events(
        &self,
        _sync_token: Option<&str>,
    ) -> Result<(Vec<CalendarEvent>, Option<String>)> {
        // Increment request counter
        *self.request_count.lock().unwrap() += 1;

        // Simulate network failure if configured
        if *self.fail_next_request.lock().unwrap() {
            *self.fail_next_request.lock().unwrap() = false;
            return Err(KlyntbotError::Calendar(CalendarError::ConnectionFailed(
                "Mock network failure".to_string(),
            )));
        }

        // Simulate network delay if configured
        let delay = *self.request_delay_ms.lock().unwrap();
        if delay > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        // Return all events
        let events: Vec<CalendarEvent> = self
            .events
            .lock()
            .unwrap()
            .values()
            .cloned()
            .collect();

        // Generate new sync token
        let new_token = Some(format!("mock-token-{}", Utc::now().timestamp()));
        *self.sync_token.lock().unwrap() = new_token.clone();

        Ok((events, new_token))
    }

    async fn put_event(&self, event: &CalendarEvent) -> Result<Option<String>> {
        // Increment request counter
        *self.request_count.lock().unwrap() += 1;

        // Simulate network failure if configured
        if *self.fail_next_request.lock().unwrap() {
            *self.fail_next_request.lock().unwrap() = false;
            return Err(KlyntbotError::Calendar(CalendarError::ConnectionFailed(
                "Mock network failure".to_string(),
            )));
        }

        // Simulate network delay if configured
        let delay = *self.request_delay_ms.lock().unwrap();
        if delay > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        // Store event
        self.events
            .lock()
            .unwrap()
            .insert(event.uid.clone(), event.clone());

        // Return new etag
        Ok(Some(format!("etag-{}-put", event.uid)))
    }

    async fn delete_event(&self, uid: &str) -> Result<()> {
        // Increment request counter
        *self.request_count.lock().unwrap() += 1;

        // Simulate network failure if configured
        if *self.fail_next_request.lock().unwrap() {
            *self.fail_next_request.lock().unwrap() = false;
            return Err(KlyntbotError::Calendar(CalendarError::ConnectionFailed(
                "Mock network failure".to_string(),
            )));
        }

        // Simulate network delay if configured
        let delay = *self.request_delay_ms.lock().unwrap();
        if delay > 0 {
            tokio::time::sleep(tokio::time::Duration::from_millis(delay)).await;
        }

        // Remove event
        self.events.lock().unwrap().remove(uid);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_provider_add_event() {
        let provider = MockCalendarProvider::new("test");
        let start = Utc::now();
        provider.add_event("event-1", "Test Event", start);

        assert_eq!(provider.event_count(), 1);
        assert!(provider.has_event("event-1"));
    }

    #[tokio::test]
    async fn test_mock_provider_remove_event() {
        let provider = MockCalendarProvider::new("test");
        let start = Utc::now();
        provider.add_event("event-1", "Test Event", start);
        provider.remove_event("event-1");

        assert_eq!(provider.event_count(), 0);
        assert!(!provider.has_event("event-1"));
    }

    #[tokio::test]
    async fn test_mock_provider_update_event_time() {
        let provider = MockCalendarProvider::new("test");
        let original_start = Utc::now();
        provider.add_event("event-1", "Test Event", original_start);

        let new_start = original_start + Duration::hours(2);
        provider.set_event_start("event-1", new_start);

        let event = provider.get_event("event-1").unwrap();
        assert_eq!(event.start, new_start);
    }

    #[tokio::test]
    async fn test_mock_provider_set_status() {
        let provider = MockCalendarProvider::new("test");
        let start = Utc::now();
        provider.add_event("event-1", "Test Event", start);
        provider.set_event_status("event-1", "COMPLETED");

        let status = provider.get_event_status("event-1");
        assert_eq!(status, Some("COMPLETED".to_string()));
    }

    #[tokio::test]
    async fn test_mock_provider_get_events() {
        let provider = MockCalendarProvider::new("test");
        let start = Utc::now();
        provider.add_event("event-1", "Event 1", start);
        provider.add_event("event-2", "Event 2", start + Duration::hours(1));

        let (events, token) = provider.get_events(None).await.unwrap();
        assert_eq!(events.len(), 2);
        assert!(token.is_some());
    }

    #[tokio::test]
    async fn test_mock_provider_fail_next_request() {
        let provider = MockCalendarProvider::new("test");
        provider.fail_next_request();

        let result = provider.get_events(None).await;
        assert!(result.is_err());

        // Second request should succeed
        let result2 = provider.get_events(None).await;
        assert!(result2.is_ok());
    }

    #[tokio::test]
    async fn test_mock_provider_request_counting() {
        let provider = MockCalendarProvider::new("test");
        assert_eq!(provider.request_count(), 0);

        provider.get_events(None).await.unwrap();
        assert_eq!(provider.request_count(), 1);

        provider.get_events(None).await.unwrap();
        assert_eq!(provider.request_count(), 2);

        provider.reset_request_count();
        assert_eq!(provider.request_count(), 0);
    }

    #[tokio::test]
    async fn test_mock_provider_network_delay() {
        use std::time::Instant;

        let provider = MockCalendarProvider::new("test");
        provider.set_delay(100); // 100ms delay

        let start = Instant::now();
        provider.get_events(None).await.unwrap();
        let elapsed = start.elapsed();

        assert!(elapsed.as_millis() >= 100);
    }
}
