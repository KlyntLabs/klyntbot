//! LSP client crate — provides LSP diagnostics and document symbols for
//! the coding-in-chat feature.
//!
//! Each language server runs as a child process managed by [`LspServerPool`].
//! The public API is [`LspClientHandle`], which is `Clone + Send + Sync`.

pub mod diagnostics;
pub mod language;
pub mod server_pool;
pub mod symbols;

use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Handle to the LSP client subsystem. Clone-safe (wraps an internal pool).
#[derive(Clone)]
pub struct LspClientHandle {
    pool: Arc<server_pool::LspServerPool>,
}

impl LspClientHandle {
    pub fn new() -> Self {
        Self {
            pool: Arc::new(server_pool::LspServerPool::new()),
        }
    }

    /// Get diagnostics for a file after an edit.
    ///
    /// Opens the file, waits for `publishDiagnostics`, and returns the
    /// diagnostics. Returns an empty vec if no language server is available
    /// for this file type.
    pub async fn diagnostics_for(
        &self,
        path: &Path,
    ) -> common::Result<Vec<diagnostics::LspDiagnostic>> {
        let Some(lang) = language::language_for(path) else {
            return Ok(vec![]);
        };
        let root = workspace_root(path)?;
        let _server = self.pool.get_or_spawn(lang, &root).await?;
        // TODO(T5): Send textDocument/didOpen, wait for publishDiagnostics, return
        Ok(vec![])
    }

    /// Get document symbols for a file.
    ///
    /// Returns anchored symbols (name, kind, line range) for all top-level
    /// and nested symbols in the file.
    pub async fn document_symbols(
        &self,
        path: &Path,
    ) -> common::Result<Vec<symbols::AnchoredSymbol>> {
        let Some(lang) = language::language_for(path) else {
            return Ok(vec![]);
        };
        let root = workspace_root(path)?;
        let _server = self.pool.get_or_spawn(lang, &root).await?;
        // TODO(T5): Send textDocument/documentSymbol, parse response
        Ok(vec![])
    }
}

impl Default for LspClientHandle {
    fn default() -> Self {
        Self::new()
    }
}

/// Walk up from `path` to find a workspace root (directory containing
/// Cargo.toml, package.json, pyproject.toml, or .git).
fn workspace_root(path: &Path) -> common::Result<PathBuf> {
    let mut current = path.parent().unwrap_or_else(|| std::path::Path::new("/"));
    loop {
        for marker in &["Cargo.toml", "package.json", "pyproject.toml", ".git"] {
            if current.join(marker).exists() {
                return Ok(current.to_path_buf());
            }
        }
        match current.parent() {
            Some(parent) => current = parent,
            None => {
                // No workspace root found; fall back to the file's directory
                return Ok(path
                    .parent()
                    .unwrap_or_else(|| std::path::Path::new("/"))
                    .to_path_buf());
            }
        }
    }
}
