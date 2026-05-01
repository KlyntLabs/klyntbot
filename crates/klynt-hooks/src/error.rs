use thiserror::Error;

#[derive(Debug, Error)]
pub enum HookError {
    #[error("hook config parse: {0}")]
    Config(#[from] toml::de::Error),
    #[error("hook subprocess io: {0}")]
    Io(#[from] std::io::Error),
    #[error("hook json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("hook timed out after {timeout_ms}ms")]
    Timeout { timeout_ms: u64 },
    #[error("hook returned block: {reason}")]
    Blocked { reason: String },
    #[error("hook other: {0}")]
    Other(String),
}

pub type HookResult<T> = std::result::Result<T, HookError>;
