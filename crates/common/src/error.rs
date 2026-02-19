//! Unified error types for klyntbot.

use thiserror::Error;

/// Main error type for klyntbot
#[derive(Error, Debug)]
pub enum KlyntbotError {
    #[error("Bus error: {0}")]
    Bus(String),

    #[error("Bus disconnected")]
    BusDisconnected,

    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),

    #[error("Provider error: {0}")]
    Provider(#[from] ProviderError),

    #[error("Channel error: {0}")]
    Channel(#[from] ChannelError),

    #[error("Session error: {0}")]
    Session(#[from] SessionError),

    #[error("Config error: {0}")]
    Config(#[from] ConfigError),

    #[error("Cron error: {0}")]
    Cron(#[from] CronError),

    #[error("Calendar error: {0}")]
    Calendar(#[from] CalendarError),

    #[error("Goal error: {0}")]
    Goal(#[from] GoalError),

    #[error("Plan error: {0}")]
    Plan(#[from] PlanError),

    #[error("Storage error: {0}")]
    Storage(String),

    #[error("Storage not found: {0}")]
    StorageNotFound(String),

    #[error("Storage conflict: {0}")]
    StorageConflict(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Tool-specific errors
#[derive(Error, Debug)]
pub enum ToolError {
    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),
}

/// Provider-specific errors
#[derive(Error, Debug)]
pub enum ProviderError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    #[error("Rate limited")]
    RateLimited,

    #[error("Authentication failed")]
    AuthFailed,
}

/// Channel-specific errors
#[derive(Error, Debug)]
pub enum ChannelError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("Send failed: {0}")]
    SendFailed(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

/// Session-specific errors
#[derive(Error, Debug)]
pub enum SessionError {
    #[error("Session not found: {0}")]
    NotFound(String),

    #[error("Failed to load session: {0}")]
    LoadFailed(String),

    #[error("Failed to save session: {0}")]
    SaveFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Config-specific errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Config file not found: {0}")]
    NotFound(String),

    #[error("Invalid configuration: {0}")]
    Invalid(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Cron-specific errors
#[derive(Error, Debug)]
pub enum CronError {
    #[error("Invalid cron expression: {0}")]
    InvalidExpression(String),

    #[error("Job not found: {0}")]
    JobNotFound(String),

    #[error("Job execution failed: {0}")]
    ExecutionFailed(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Calendar-specific errors (Phase 1 prep for Phase 3)
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

/// Goal-specific errors
#[derive(Error, Debug)]
pub enum GoalError {
    #[error("Goal not found: {0}")]
    NotFound(String),

    #[error("Invalid goal state: {0}")]
    InvalidState(String),

    #[error("Goal store error: {0}")]
    StoreFailed(String),

    #[error("Goal validation failed: {0}")]
    ValidationFailed(String),
}

/// Plan-specific errors
#[derive(Error, Debug)]
pub enum PlanError {
    #[error("Plan not found: {0}")]
    NotFound(String),

    #[error("Plan generation failed: {0}")]
    GenerationFailed(String),

    #[error("Invalid plan state: {0}")]
    InvalidState(String),

    #[error("Execution stalled at step {step_index}: {reason}")]
    ExecutionStalled { step_index: usize, reason: String },

    #[error("Backtrack limit reached at step {0}")]
    BacktrackLimitReached(usize),

    #[error("Plan store error: {0}")]
    StoreFailed(String),
}

/// Type alias for Result with KlyntbotError
pub type Result<T> = std::result::Result<T, KlyntbotError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        // ToolError
        assert_eq!(
            ToolError::NotFound("test_tool".into()).to_string(),
            "Tool not found: test_tool"
        );
        assert_eq!(
            ToolError::InvalidParams("missing param".into()).to_string(),
            "Invalid parameters: missing param"
        );
        assert_eq!(
            ToolError::ExecutionFailed("command failed".into()).to_string(),
            "Execution failed: command failed"
        );
        assert_eq!(
            ToolError::PermissionDenied("access denied".into()).to_string(),
            "Permission denied: access denied"
        );

        // ProviderError
        assert_eq!(
            ProviderError::InvalidResponse("bad JSON".into()).to_string(),
            "Invalid response: bad JSON"
        );
        assert_eq!(ProviderError::RateLimited.to_string(), "Rate limited");
        assert_eq!(
            ProviderError::AuthFailed.to_string(),
            "Authentication failed"
        );

        // ChannelError
        assert_eq!(
            ChannelError::ConnectionFailed("timeout".into()).to_string(),
            "Connection failed: timeout"
        );
        assert_eq!(
            ChannelError::SendFailed("network error".into()).to_string(),
            "Send failed: network error"
        );
        assert_eq!(
            ChannelError::InvalidConfig("missing token".into()).to_string(),
            "Invalid configuration: missing token"
        );

        // SessionError
        assert_eq!(
            SessionError::NotFound("session123".into()).to_string(),
            "Session not found: session123"
        );
        assert_eq!(
            SessionError::LoadFailed("corrupt file".into()).to_string(),
            "Failed to load session: corrupt file"
        );
        assert_eq!(
            SessionError::SaveFailed("disk full".into()).to_string(),
            "Failed to save session: disk full"
        );

        // ConfigError
        assert_eq!(
            ConfigError::NotFound("config.json".into()).to_string(),
            "Config file not found: config.json"
        );
        assert_eq!(
            ConfigError::Invalid("bad format".into()).to_string(),
            "Invalid configuration: bad format"
        );
        assert_eq!(
            ConfigError::MissingField("api_key".into()).to_string(),
            "Missing required field: api_key"
        );

        // CronError
        assert_eq!(
            CronError::InvalidExpression("bad cron".into()).to_string(),
            "Invalid cron expression: bad cron"
        );
        assert_eq!(
            CronError::JobNotFound("job123".into()).to_string(),
            "Job not found: job123"
        );
        assert_eq!(
            CronError::ExecutionFailed("timeout".into()).to_string(),
            "Job execution failed: timeout"
        );

        // GoalError
        assert_eq!(
            GoalError::NotFound("goal-123".into()).to_string(),
            "Goal not found: goal-123"
        );
        assert_eq!(
            GoalError::InvalidState("cannot transition from active to draft".into()).to_string(),
            "Invalid goal state: cannot transition from active to draft"
        );
        assert_eq!(
            GoalError::StoreFailed("disk full".into()).to_string(),
            "Goal store error: disk full"
        );
        assert_eq!(
            GoalError::ValidationFailed("title is required".into()).to_string(),
            "Goal validation failed: title is required"
        );

        // KlyntbotError direct variants
        assert_eq!(
            KlyntbotError::BusDisconnected.to_string(),
            "Bus disconnected"
        );
        assert_eq!(
            KlyntbotError::StorageNotFound("todo-123".into()).to_string(),
            "Storage not found: todo-123"
        );
        assert_eq!(
            KlyntbotError::StorageConflict("duplicate key".into()).to_string(),
            "Storage conflict: duplicate key"
        );
    }

    #[test]
    fn test_klyntbot_error_from_conversions() {
        let cases: Vec<(&str, KlyntbotError)> = vec![
            ("Tool error", ToolError::NotFound("test".into()).into()),
            ("Provider error", ProviderError::AuthFailed.into()),
            (
                "Channel error",
                ChannelError::SendFailed("test".into()).into(),
            ),
            (
                "Session error",
                SessionError::NotFound("test".into()).into(),
            ),
            ("Config error", ConfigError::Invalid("test".into()).into()),
            (
                "Cron error",
                CronError::InvalidExpression("test".into()).into(),
            ),
            ("Goal error", GoalError::NotFound("test".into()).into()),
        ];

        for (expected_prefix, err) in cases {
            assert!(
                err.to_string().contains(expected_prefix),
                "Expected '{}' in '{}'",
                expected_prefix,
                err
            );
        }
    }
}
