use thiserror::Error;

#[derive(Debug, Error)]
pub enum SandboxError {
    #[error("sandbox unavailable on this platform: {0}")]
    Unavailable(String),
    #[error("sandbox launch spawn failed: {0}")]
    Spawn(#[from] std::io::Error),
    #[error("policy generation failed: {0}")]
    PolicyGen(String),
    #[error("sandbox child exited with status {0}")]
    ChildExit(i32),
}
