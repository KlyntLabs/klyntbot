//! Closed enum mapping file extensions → tree-sitter language.

use std::path::Path;

/// Languages with a Phase-6 tree-sitter extractor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Language {
    /// Rust (`.rs`).
    Rust,
    /// TypeScript (`.ts`, `.tsx`).
    TypeScript,
    /// JavaScript (`.js`, `.mjs`, `.cjs`).
    JavaScript,
    /// Python (`.py`).
    Python,
    /// Go (`.go`).
    Go,
}

impl Language {
    /// Map a file path's extension to a language. `None` means we cannot
    /// extract symbols — invalidation degrades to file-level coarseness.
    #[must_use]
    pub fn from_path(path: &Path) -> Option<Self> {
        let ext = path.extension()?.to_str()?;
        match ext {
            "rs" => Some(Self::Rust),
            "ts" | "tsx" => Some(Self::TypeScript),
            "js" | "mjs" | "cjs" => Some(Self::JavaScript),
            "py" => Some(Self::Python),
            "go" => Some(Self::Go),
            _ => None,
        }
    }

    /// Stable string slug — used in `AnchoredSymbol.kind` formatting.
    #[must_use]
    pub fn slug(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::TypeScript => "typescript",
            Self::JavaScript => "javascript",
            Self::Python => "python",
            Self::Go => "go",
        }
    }
}
