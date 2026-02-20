//! Calendar-specific error types.

use thiserror::Error;

/// Calendar-specific errors
#[derive(Error, Debug)]
pub enum CalendarError {
    #[error("Authentication failed: {0}")]
    AuthFailed(String),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Sync failed: {0}")]
    SyncFailed(String),

    #[error("Calendar not found: {0}")]
    NotFound(String),

    #[error("CalDAV protocol error: {0}")]
    ProtocolError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl From<CalendarError> for common::KlyntbotError {
    fn from(e: CalendarError) -> Self {
        common::KlyntbotError::Calendar(e.to_string())
    }
}
