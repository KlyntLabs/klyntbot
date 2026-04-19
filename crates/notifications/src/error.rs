//! Error type for the notifications crate.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum NotificationError {
    #[error("storage error: {0}")]
    Storage(#[from] common::KlyntbotError),
    #[error("scheduler error: {0}")]
    Scheduler(#[from] scheduling::error::SchedulerError),
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
