//! Managed-block parser/writer. Filled in by Task 7.

use std::path::Path;
use thiserror::Error;

/// Managed-block error surface.
#[derive(Debug, Error)]
pub enum ManagedBlockError {
    /// IO failure.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    /// User edited managed range.
    #[error("user content found inside managed range — refusing to overwrite")]
    UserConflict,
}

/// Parsed managed block.
#[derive(Debug, Clone, Default)]
pub struct ManagedBlock {
    /// Lines before the managed start marker (preserved verbatim).
    pub before: String,
    /// Lines inside the managed range.
    pub inside: String,
    /// Lines after the managed end marker (preserved verbatim).
    pub after: String,
}

impl ManagedBlock {
    /// Read + parse a file. Filled in by Task 7.
    pub fn read(_path: &Path) -> Result<Self, ManagedBlockError> {
        Err(ManagedBlockError::Io(std::io::Error::other("phase 5 stub")))
    }
}
