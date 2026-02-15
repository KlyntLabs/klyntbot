// CalendarProvider trait — abstraction for all calendar backends

use async_trait::async_trait;
use common::Result;

use crate::types::CalendarEvent;

/// Trait implemented by all calendar providers (Apple, Google, Generic CalDAV).
///
/// Each provider handles its own authentication, URL discovery, and CalDAV
/// dialect differences. The sync adapter calls these methods uniformly.
#[async_trait]
pub trait CalendarProvider: Send + Sync {
    /// Human-readable provider name (e.g., "Apple Calendar", "Google Calendar").
    fn name(&self) -> &str;

    /// Unique provider identifier used for sync state file naming
    /// (e.g., "apple", "google", "generic-nextcloud").
    fn provider_id(&self) -> &str;

    /// Fetch events from the remote calendar, optionally using an incremental sync token.
    /// Returns (events, new_sync_token).
    async fn get_events(
        &self,
        sync_token: Option<&str>,
    ) -> Result<(Vec<CalendarEvent>, Option<String>)>;

    /// Create or update an event on the remote calendar. Returns the ETag.
    async fn put_event(&self, event: &CalendarEvent) -> Result<String>;

    /// Delete an event from the remote calendar by UID.
    async fn delete_event(&self, uid: &str) -> Result<()>;

    /// Test connectivity and authentication. Returns Ok(()) on success.
    async fn test_connection(&self) -> Result<()>;
}
