//! `coding-memory` adapters owned by `app-core`.

/// Handlers backing desktop Tauri commands.
pub mod handlers;
/// Claude Code settings.json installer.
pub mod installer;

use desktop_shared::errors::ApiError;

impl crate::AppCore {
    /// Return coding-memory status (daemon liveness, buffer size, unprocessed count).
    pub async fn coding_memory_status(
        self: &std::sync::Arc<Self>,
    ) -> Result<desktop_shared::commands::coding_memory::CodingMemoryStatusResponse, ApiError> {
        let repo = coding_ingest::store::IngestEventLogRepo::new(self.storage_pool.inner().clone());
        let data_dir = self.config.read().await.data_dir_path();
        handlers::status(&repo, &data_dir)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }

    /// Return per-CLI health rows.
    pub async fn coding_memory_cli_health(
        self: &std::sync::Arc<Self>,
    ) -> Result<Vec<desktop_shared::commands::coding_memory::CliHealthRow>, ApiError> {
        handlers::cli_health(self.storage_pool.inner())
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }

    /// Return paginated session replay entries.
    pub async fn coding_memory_session_replay(
        self: &std::sync::Arc<Self>,
        session_id: Option<String>,
        limit: i64,
        offset: i64,
    ) -> Result<Vec<desktop_shared::commands::coding_memory::SessionReplayEntry>, ApiError> {
        handlers::session_replay(self.storage_pool.inner(), session_id, limit, offset)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }

    /// Enable a coding CLI (currently only claude-code is supported).
    pub async fn coding_memory_enable_cli(&self, cli: String) -> Result<(), ApiError> {
        if cli != "claude-code" {
            return Err(ApiError::new(
                "BAD_REQUEST",
                "Only claude-code is wired in Phase 2",
            ));
        }
        let settings = claude_code_settings_path()?;
        let binary = hook_binary_path()?;
        tokio::task::spawn_blocking(move || {
            crate::coding_memory::installer::ClaudeCodeInstaller::install(&settings, &binary)
        })
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    /// Disable a coding CLI (currently only claude-code is supported).
    pub async fn coding_memory_disable_cli(&self, cli: String) -> Result<(), ApiError> {
        if cli != "claude-code" {
            return Err(ApiError::new(
                "BAD_REQUEST",
                "Only claude-code is wired in Phase 2",
            ));
        }
        let settings = claude_code_settings_path()?;
        tokio::task::spawn_blocking(move || {
            crate::coding_memory::installer::ClaudeCodeInstaller::uninstall(&settings)
        })
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    /// Diagnose a coding CLI hook binary.
    pub async fn coding_memory_diagnose_cli(
        &self,
        cli: String,
    ) -> Result<desktop_shared::commands::coding_memory::DiagnoseResult, ApiError> {
        if cli != "claude-code" {
            return Ok(desktop_shared::commands::coding_memory::DiagnoseResult {
                ok: false,
                message: "only claude-code supported".into(),
            });
        }
        let binary = hook_binary_path()?;
        let outcome = tokio::task::spawn_blocking(move || {
            crate::coding_memory::installer::ClaudeCodeInstaller::diagnose(&binary)
        })
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        match outcome {
            Ok(()) => Ok(desktop_shared::commands::coding_memory::DiagnoseResult {
                ok: true,
                message: "hook exited 0".into(),
            }),
            Err(e) => Ok(desktop_shared::commands::coding_memory::DiagnoseResult {
                ok: false,
                message: e.to_string(),
            }),
        }
    }
}

fn claude_code_settings_path() -> Result<std::path::PathBuf, desktop_shared::errors::ApiError> {
    let home = std::env::var("HOME")
        .map_err(|_| desktop_shared::errors::ApiError::new("INTERNAL_ERROR", "no $HOME"))?;
    Ok(std::path::PathBuf::from(home)
        .join(".claude")
        .join("settings.json"))
}

fn hook_binary_path() -> Result<std::path::PathBuf, desktop_shared::errors::ApiError> {
    // Resolution order:
    //   1. KLYNTBOT_HOOK_BINARY env var (production override, packaging story).
    //   2. Co-located binary next to the running executable (dev + bundled app).
    //   3. Bare `klyntbot-hook` — relies on PATH.
    if let Ok(p) = std::env::var("KLYNTBOT_HOOK_BINARY") {
        if !p.trim().is_empty() {
            return Ok(std::path::PathBuf::from(p));
        }
    }
    let exe = std::env::current_exe()
        .map_err(|e| desktop_shared::errors::ApiError::new("INTERNAL_ERROR", e.to_string()))?;
    let dir = exe
        .parent()
        .ok_or_else(|| desktop_shared::errors::ApiError::new("INTERNAL_ERROR", "bad exe path"))?;
    let candidate = dir.join("klyntbot-hook");
    if candidate.exists() {
        return Ok(candidate);
    }
    Ok(std::path::PathBuf::from("klyntbot-hook"))
}
