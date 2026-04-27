use common::error::{
    ChannelError, ConfigError, KlyntbotError, ProviderError, SessionError, ToolError,
};
use serde::{Deserialize, Serialize};

/// Standardized API error response for Tauri commands.
/// Sent to the frontend with structured error information.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    /// Machine-readable error code (e.g., "NOT_FOUND", "INVALID_PARAMS")
    pub code: String,
    /// Human-readable error message
    pub message: String,
}

impl ApiError {
    /// Create a new ApiError with code and message
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ApiError {}

/// Convert KlyntbotError to ApiError
impl From<KlyntbotError> for ApiError {
    fn from(err: KlyntbotError) -> Self {
        match err {
            // Tool errors
            KlyntbotError::Tool(ToolError::NotFound(msg)) => ApiError::new("TOOL_NOT_FOUND", msg),
            KlyntbotError::Tool(ToolError::InvalidParams(msg)) => {
                ApiError::new("INVALID_PARAMS", msg)
            }
            KlyntbotError::Tool(ToolError::ExecutionFailed(msg)) => {
                ApiError::new("EXECUTION_FAILED", msg)
            }
            KlyntbotError::Tool(ToolError::PermissionDenied(msg)) => {
                ApiError::new("PERMISSION_DENIED", msg)
            }

            // Provider errors
            KlyntbotError::Provider(ProviderError::Http(msg)) => ApiError::new("HTTP_ERROR", msg),
            KlyntbotError::Provider(ProviderError::InvalidResponse(msg)) => {
                ApiError::new("INVALID_RESPONSE", msg)
            }
            KlyntbotError::Provider(ProviderError::RateLimited {
                provider,
                retry_after,
            }) => {
                let message = if let Some(secs) = retry_after {
                    format!("Rate limited by {} (retry after {}s)", provider, secs)
                } else {
                    format!("Rate limited by {}", provider)
                };
                ApiError::new("RATE_LIMITED", message)
            }
            KlyntbotError::Provider(ProviderError::AuthFailed {
                provider,
                config_key,
            }) => {
                let message = format!(
                    "Authentication failed for {} (check config key: {})",
                    provider, config_key
                );
                ApiError::new("AUTH_FAILED", message)
            }

            // Channel errors
            KlyntbotError::Channel(ChannelError::ConnectionFailed(msg)) => {
                ApiError::new("CONNECTION_FAILED", msg)
            }
            KlyntbotError::Channel(ChannelError::SendFailed(msg)) => {
                ApiError::new("SEND_FAILED", msg)
            }
            KlyntbotError::Channel(ChannelError::InvalidConfig(msg)) => {
                ApiError::new("INVALID_CONFIG", msg)
            }

            // Session errors
            KlyntbotError::Session(SessionError::NotFound(msg)) => {
                ApiError::new("SESSION_NOT_FOUND", msg)
            }
            KlyntbotError::Session(SessionError::LoadFailed(msg)) => {
                ApiError::new("SESSION_LOAD_FAILED", msg)
            }
            KlyntbotError::Session(SessionError::SaveFailed(msg)) => {
                ApiError::new("SESSION_SAVE_FAILED", msg)
            }
            KlyntbotError::Session(SessionError::Io(e)) => {
                ApiError::new("SESSION_IO_ERROR", e.to_string())
            }
            KlyntbotError::Session(SessionError::Json(e)) => {
                ApiError::new("SESSION_JSON_ERROR", e.to_string())
            }

            // Config errors
            KlyntbotError::Config(ConfigError::NotFound(msg)) => {
                ApiError::new("CONFIG_NOT_FOUND", msg)
            }
            KlyntbotError::Config(ConfigError::Invalid(msg)) => {
                ApiError::new("INVALID_CONFIG_FILE", msg)
            }
            KlyntbotError::Config(ConfigError::MissingField(msg)) => {
                ApiError::new("MISSING_CONFIG_FIELD", msg)
            }
            KlyntbotError::Config(ConfigError::Io(e)) => {
                ApiError::new("CONFIG_IO_ERROR", e.to_string())
            }
            KlyntbotError::Config(ConfigError::Json(e)) => {
                ApiError::new("CONFIG_JSON_ERROR", e.to_string())
            }

            // Storage errors
            KlyntbotError::Storage(msg) => ApiError::new("STORAGE_ERROR", msg),
            KlyntbotError::StorageNotFound(msg) => ApiError::new("NOT_FOUND", msg),
            KlyntbotError::StorageConflict(msg) => ApiError::new("CONFLICT", msg),

            // Other errors
            KlyntbotError::Bus(msg) => ApiError::new("BUS_ERROR", msg),
            KlyntbotError::BusDisconnected => {
                ApiError::new("BUS_DISCONNECTED", "Message bus disconnected")
            }
            KlyntbotError::Cron(msg) => ApiError::new("CRON_ERROR", msg),
            KlyntbotError::Io(e) => ApiError::new("IO_ERROR", e.to_string()),
            KlyntbotError::Json(e) => ApiError::new("JSON_ERROR", e.to_string()),
            KlyntbotError::Timeout(msg) => ApiError::new("TIMEOUT", msg),
            KlyntbotError::NotImplemented(msg) => ApiError::new("NOT_IMPLEMENTED", msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_error_from_storage_not_found() {
        let err: ApiError = KlyntbotError::StorageNotFound("todo-123".into()).into();
        assert_eq!(err.code, "NOT_FOUND");
        assert!(err.message.contains("todo-123"));
    }

    #[test]
    fn test_api_error_from_storage_conflict() {
        let err: ApiError = KlyntbotError::StorageConflict("duplicate key".into()).into();
        assert_eq!(err.code, "CONFLICT");
        assert!(err.message.contains("duplicate key"));
    }

    #[test]
    fn test_api_error_from_tool_invalid_params() {
        let err: ApiError =
            KlyntbotError::Tool(ToolError::InvalidParams("missing field".into())).into();
        assert_eq!(err.code, "INVALID_PARAMS");
        assert!(err.message.contains("missing field"));
    }

    #[test]
    fn test_api_error_from_rate_limited() {
        let err: ApiError = KlyntbotError::Provider(ProviderError::RateLimited {
            provider: "anthropic".into(),
            retry_after: Some(60),
        })
        .into();
        assert_eq!(err.code, "RATE_LIMITED");
        assert!(err.message.contains("anthropic"));
        assert!(err.message.contains("60s"));
    }

    #[test]
    fn test_api_error_display() {
        let err = ApiError::new("TEST_CODE", "test message");
        assert_eq!(err.to_string(), "TEST_CODE: test message");
    }
}
