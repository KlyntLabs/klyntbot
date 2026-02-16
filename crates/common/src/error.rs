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
    fn test_tool_error_display() {
        let err = ToolError::NotFound("test_tool".to_string());
        assert_eq!(err.to_string(), "Tool not found: test_tool");

        let err = ToolError::InvalidParams("missing param".to_string());
        assert_eq!(err.to_string(), "Invalid parameters: missing param");

        let err = ToolError::ExecutionFailed("command failed".to_string());
        assert_eq!(err.to_string(), "Execution failed: command failed");

        let err = ToolError::PermissionDenied("access denied".to_string());
        assert_eq!(err.to_string(), "Permission denied: access denied");
    }

    #[test]
    fn test_provider_error_display() {
        let err = ProviderError::InvalidResponse("bad JSON".to_string());
        assert_eq!(err.to_string(), "Invalid response: bad JSON");

        let err = ProviderError::RateLimited;
        assert_eq!(err.to_string(), "Rate limited");

        let err = ProviderError::AuthFailed;
        assert_eq!(err.to_string(), "Authentication failed");
    }

    #[test]
    fn test_channel_error_display() {
        let err = ChannelError::ConnectionFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Connection failed: timeout");

        let err = ChannelError::SendFailed("network error".to_string());
        assert_eq!(err.to_string(), "Send failed: network error");

        let err = ChannelError::InvalidConfig("missing token".to_string());
        assert_eq!(err.to_string(), "Invalid configuration: missing token");
    }

    #[test]
    fn test_session_error_display() {
        let err = SessionError::NotFound("session123".to_string());
        assert_eq!(err.to_string(), "Session not found: session123");

        let err = SessionError::LoadFailed("corrupt file".to_string());
        assert_eq!(err.to_string(), "Failed to load session: corrupt file");

        let err = SessionError::SaveFailed("disk full".to_string());
        assert_eq!(err.to_string(), "Failed to save session: disk full");
    }

    #[test]
    fn test_config_error_display() {
        let err = ConfigError::NotFound("config.json".to_string());
        assert_eq!(err.to_string(), "Config file not found: config.json");

        let err = ConfigError::Invalid("bad format".to_string());
        assert_eq!(err.to_string(), "Invalid configuration: bad format");

        let err = ConfigError::MissingField("api_key".to_string());
        assert_eq!(err.to_string(), "Missing required field: api_key");
    }

    #[test]
    fn test_cron_error_display() {
        let err = CronError::InvalidExpression("bad cron".to_string());
        assert_eq!(err.to_string(), "Invalid cron expression: bad cron");

        let err = CronError::JobNotFound("job123".to_string());
        assert_eq!(err.to_string(), "Job not found: job123");

        let err = CronError::ExecutionFailed("timeout".to_string());
        assert_eq!(err.to_string(), "Job execution failed: timeout");
    }

    #[test]
    fn test_klyntbot_error_from_tool_error() {
        let tool_err = ToolError::NotFound("test".to_string());
        let klyntbot_err: KlyntbotError = tool_err.into();
        assert!(klyntbot_err.to_string().contains("Tool error"));
    }

    #[test]
    fn test_klyntbot_error_from_provider_error() {
        let provider_err = ProviderError::AuthFailed;
        let klyntbot_err: KlyntbotError = provider_err.into();
        assert!(klyntbot_err.to_string().contains("Provider error"));
    }

    #[test]
    fn test_klyntbot_error_from_channel_error() {
        let channel_err = ChannelError::SendFailed("test".to_string());
        let klyntbot_err: KlyntbotError = channel_err.into();
        assert!(klyntbot_err.to_string().contains("Channel error"));
    }

    #[test]
    fn test_klyntbot_error_from_session_error() {
        let session_err = SessionError::NotFound("test".to_string());
        let klyntbot_err: KlyntbotError = session_err.into();
        assert!(klyntbot_err.to_string().contains("Session error"));
    }

    #[test]
    fn test_klyntbot_error_from_config_error() {
        let config_err = ConfigError::Invalid("test".to_string());
        let klyntbot_err: KlyntbotError = config_err.into();
        assert!(klyntbot_err.to_string().contains("Config error"));
    }

    #[test]
    fn test_klyntbot_error_from_cron_error() {
        let cron_err = CronError::InvalidExpression("test".to_string());
        let klyntbot_err: KlyntbotError = cron_err.into();
        assert!(klyntbot_err.to_string().contains("Cron error"));
    }

    #[test]
    fn test_bus_disconnected_error() {
        let err = KlyntbotError::BusDisconnected;
        assert_eq!(err.to_string(), "Bus disconnected");
    }

    #[test]
    fn test_goal_error_display() {
        let err = GoalError::NotFound("goal-123".to_string());
        assert_eq!(err.to_string(), "Goal not found: goal-123");

        let err = GoalError::InvalidState("cannot transition from active to draft".to_string());
        assert_eq!(
            err.to_string(),
            "Invalid goal state: cannot transition from active to draft"
        );

        let err = GoalError::StoreFailed("disk full".to_string());
        assert_eq!(err.to_string(), "Goal store error: disk full");

        let err = GoalError::ValidationFailed("title is required".to_string());
        assert_eq!(
            err.to_string(),
            "Goal validation failed: title is required"
        );
    }

    #[test]
    fn test_klyntbot_error_from_goal_error() {
        let goal_err = GoalError::NotFound("test".to_string());
        let klyntbot_err: KlyntbotError = goal_err.into();
        assert!(klyntbot_err.to_string().contains("Goal error"));
    }
}
