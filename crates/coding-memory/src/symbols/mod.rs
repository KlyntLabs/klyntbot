//! Tree-sitter symbol extraction for `anchored_symbols` populating + git-invalidation.
//!
//! Phase 6 wires Rust / TypeScript / JavaScript / Python / Go. Other languages
//! are silently skipped — the absence of anchored symbols is not an error,
//! only a degradation in invalidation precision.

pub mod cache;
pub mod extractor;
pub mod language;

pub use cache::SymbolCache;
pub use extractor::{SymbolExtractor, TreeSitterExtractor};
pub use language::Language;
