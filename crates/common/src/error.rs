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
    Cron(String),

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

    #[error("Rate limited by {provider}{}", retry_after.map(|s| format!(" (retry after {}s)", s)).unwrap_or_default())]
    RateLimited {
        provider: String,
        retry_after: Option<u64>,
    },

    #[error("Authentication failed for {provider} (check config key: {config_key})")]
    AuthFailed {
        provider: String,
        config_key: String,
    },
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
        assert_eq!(
            ProviderError::RateLimited {
                provider: "openai".into(),
                retry_after: Some(30)
            }
            .to_string(),
            "Rate limited by openai (retry after 30s)"
        );
        assert_eq!(
            ProviderError::RateLimited {
                provider: "anthropic".into(),
                retry_after: None
            }
            .to_string(),
            "Rate limited by anthropic"
        );
        assert_eq!(
            ProviderError::AuthFailed {
                provider: "openai".into(),
                config_key: "providers.openai.apiKey".into()
            }
            .to_string(),
            "Authentication failed for openai (check config key: providers.openai.apiKey)"
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

        // KlyntbotError String variants (domain errors moved to their crates)
        assert_eq!(
            KlyntbotError::Cron("bad cron".into()).to_string(),
            "Cron error: bad cron"
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
            (
                "Provider error",
                ProviderError::AuthFailed {
                    provider: "test".into(),
                    config_key: "providers.test.apiKey".into(),
                }
                .into(),
            ),
            (
                "Channel error",
                ChannelError::SendFailed("test".into()).into(),
            ),
            (
                "Session error",
                SessionError::NotFound("test".into()).into(),
            ),
            ("Config error", ConfigError::Invalid("test".into()).into()),
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
