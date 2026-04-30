use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("invalid op: {0}")]
    InvalidOp(String),
    #[error("serialization: {0}")]
    Serialization(#[from] serde_json::Error),
}
