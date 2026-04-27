//! `SymbolExtractor` trait — Phase 6 fills this with a tree-sitter impl.

use crate::scope::AnchoredSymbol;
use std::path::Path;

/// Extracts symbol anchors from source. Implementations must be `Send + Sync`
/// since the Distiller and Reforge symbol-validation phase share one instance.
pub trait SymbolExtractor: Send + Sync + std::fmt::Debug {
    /// Extract anchors for the given file. `git_hash` is used to populate
    /// `AnchoredSymbol.git_hash`. Returns an empty vec when language is
    /// unsupported or parsing fails — never errors.
    fn extract(&self, path: &Path, source: &str, git_hash: &str) -> Vec<AnchoredSymbol>;
}

/// Tree-sitter–backed extractor. Stub until Task 5.
#[derive(Debug, Default)]
pub struct TreeSitterExtractor {
    _private: (),
}

impl TreeSitterExtractor {
    /// Construct.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl SymbolExtractor for TreeSitterExtractor {
    fn extract(&self, _path: &Path, _source: &str, _git_hash: &str) -> Vec<AnchoredSymbol> {
        Vec::new()
    }
}
