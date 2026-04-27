//! Bounded LRU of parsed symbol vectors. Keyed by `(path, content_hash)`.

use crate::scope::AnchoredSymbol;
use std::path::PathBuf;

/// Stub until Task 7.
#[derive(Debug)]
pub struct SymbolCache {
    _private: (),
}

impl Default for SymbolCache {
    fn default() -> Self {
        Self { _private: () }
    }
}

impl SymbolCache {
    /// Construct with a max-entry cap.
    #[must_use]
    pub fn with_capacity(_cap: usize) -> Self {
        Self::default()
    }

    /// Lookup by `(path, content_hash)`.
    #[must_use]
    pub fn get(&self, _path: &PathBuf, _content_hash: &[u8; 32]) -> Option<Vec<AnchoredSymbol>> {
        None
    }

    /// Insert.
    pub fn insert(&self, _path: PathBuf, _content_hash: [u8; 32], _symbols: Vec<AnchoredSymbol>) {}
}
