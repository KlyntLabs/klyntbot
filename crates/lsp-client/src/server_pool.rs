//! LSP server process pool — one server process per (language, workspace_root).

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Manages a pool of LSP server processes.
///
/// Each unique `(language_id, workspace_root)` pair gets exactly one server
/// process. The server is spawned on first access and reused thereafter.
pub struct LspServerPool {
    servers: Mutex<HashMap<(String, PathBuf), Arc<LspServerHandle>>>,
}

/// Handle to a running LSP server process.
pub struct LspServerHandle {
    /// Language server binary name (e.g. "rust-analyzer").
    #[allow(dead_code)]
    language: String,
    /// Workspace root path.
    #[allow(dead_code)]
    root: PathBuf,
    // TODO(T5): Store the async-lsp ClientSocket here once the server pool
    // is fully implemented. For now this is a placeholder that compiles.
}

impl LspServerPool {
    pub fn new() -> Self {
        Self {
            servers: Mutex::new(HashMap::new()),
        }
    }

    /// Get or spawn an LSP server for the given language and workspace root.
    ///
    /// Returns a handle to the running server. On first call for a given
    /// `(lang, root)` pair, spawns the server process and sends
    /// `initialize` + `initialized` handshake.
    pub async fn get_or_spawn(
        &self,
        lang: &str,
        root: &std::path::Path,
    ) -> common::Result<Arc<LspServerHandle>> {
        let key = (lang.to_string(), root.to_path_buf());
        let mut servers = self.servers.lock().await;
        if let Some(handle) = servers.get(&key) {
            return Ok(Arc::clone(handle));
        }

        let handle = Arc::new(Self::spawn_server(lang, root).await?);
        servers.insert(key, Arc::clone(&handle));
        Ok(handle)
    }

    /// Spawn a new LSP server process.
    async fn spawn_server(lang: &str, root: &std::path::Path) -> common::Result<LspServerHandle> {
        // TODO(T5): Full implementation:
        // 1. tokio::process::Command::new(lang).arg("--stdio").spawn()
        // 2. async-lsp ClientSocket::new over stdin/stdout
        // 3. Send initialize request with root_uri
        // 4. Send initialized notification
        // 5. Return handle with the socket
        //
        // For now, return a placeholder that allows the crate to compile
        // and the diagnostics diffing to be tested independently.
        tracing::info!(
            lang = %lang,
            root = %root.display(),
            "LSP server pool: spawn requested (stub implementation)"
        );
        Ok(LspServerHandle {
            language: lang.to_string(),
            root: root.to_path_buf(),
        })
    }

    /// Number of active server processes.
    pub async fn active_count(&self) -> usize {
        self.servers.lock().await.len()
    }

    /// Shutdown all server processes.
    pub async fn shutdown_all(&self) {
        let mut servers = self.servers.lock().await;
        // TODO(T5): Cancel each server's async-lsp connection
        servers.clear();
    }
}

impl Default for LspServerPool {
    fn default() -> Self {
        Self::new()
    }
}
