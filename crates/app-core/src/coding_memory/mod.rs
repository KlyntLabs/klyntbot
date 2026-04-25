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

    /// Return per-CLI health rows. Fills `enabled` from `config.codingMemory.cli`.
    pub async fn coding_memory_cli_health(
        self: &std::sync::Arc<Self>,
    ) -> Result<Vec<desktop_shared::commands::coding_memory::CliHealthRow>, ApiError> {
        let mut rows = handlers::cli_health(self.storage_pool.inner())
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        let cfg = self.config.read().await;
        let toggles = &cfg.coding_memory.cli;
        for r in &mut rows {
            r.enabled = match r.cli.as_str() {
                "claude-code" => toggles.claude_code.enabled,
                "codex" => toggles.codex.enabled,
                "kimi-cli" => toggles.kimi_cli.enabled,
                "opencode" => toggles.opencode.enabled,
                _ => false,
            };
        }
        Ok(rows)
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
        self.set_cli_enabled(&cli, true).await
    }

    /// Disable a coding CLI (currently only claude-code is supported).
    pub async fn coding_memory_disable_cli(&self, cli: String) -> Result<(), ApiError> {
        self.set_cli_enabled(&cli, false).await
    }

    async fn set_cli_enabled(&self, cli: &str, enabled: bool) -> Result<(), ApiError> {
        if cli != "claude-code" {
            return Err(ApiError::new("BAD_REQUEST", "only claude-code is supported"));
        }
        let settings = claude_code_settings_path()?;
        if enabled {
            let binary = hook_binary_path()?;
            tokio::task::spawn_blocking(move || {
                crate::coding_memory::installer::ClaudeCodeInstaller::install(&settings, &binary)
            })
        } else {
            tokio::task::spawn_blocking(move || {
                crate::coding_memory::installer::ClaudeCodeInstaller::uninstall(&settings)
            })
        }
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        let mut cfg = self.config.write().await;
        cfg.coding_memory.cli.claude_code.enabled = enabled;
        config::save(&cfg)
            .await
            .map_err(crate::errors::map_config_save_err)?;
        Ok(())
    }

    /// Memory Browser — paginated list of active coding-domain facts.
    pub async fn coding_memory_browser(
        self: &std::sync::Arc<Self>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<desktop_shared::commands::coding_memory::MemoryBrowserRow>, ApiError> {
        handlers::memory_browser(
            self.storage_pool.inner(),
            limit.unwrap_or(200),
            offset.unwrap_or(0),
        )
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }

    /// Activity Timeline — daily ingest event counts.
    pub async fn coding_memory_activity(
        self: &std::sync::Arc<Self>,
        days: Option<i64>,
    ) -> Result<Vec<desktop_shared::commands::coding_memory::ActivityBucket>, ApiError> {
        handlers::activity_timeline(self.storage_pool.inner(), days.unwrap_or(30))
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }

    /// Cost Tracker — rich per-(date × model) breakdown with aggregate totals.
    pub async fn coding_memory_cost(
        self: &std::sync::Arc<Self>,
        days: Option<i64>,
    ) -> Result<desktop_shared::commands::coding_memory::CostBreakdown, ApiError> {
        handlers::cost_breakdown(self.storage_pool.inner(), days.unwrap_or(30))
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
    }

    /// Debug — force one Phase A+B+C distillation pass for an unprocessed turn.
    /// Returns a JSON summary of the `DistillationReport`.
    pub async fn coding_memory_distill_now(
        self: &std::sync::Arc<Self>,
        session_id: String,
        turn_id: Option<String>,
    ) -> Result<serde_json::Value, ApiError> {
        let distiller = self
            .distiller
            .clone()
            .ok_or_else(|| ApiError::new("INTERNAL_ERROR", "distiller not initialized"))?;
        let report = distiller
            .distill_turn(&session_id, turn_id.as_deref())
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(serde_json::json!({
            "episodicWrites": report.episodic_writes,
            "semanticWrites": report.semantic_writes,
            "llmCalls": report.llm_calls,
            "turnTraceId": report.turn_trace_id.map(|u| u.to_string()),
        }))
    }

    /// Sensitivity Inspector — facts annotated with a sensitivity tier.
    pub async fn coding_memory_sensitivity(
        self: &std::sync::Arc<Self>,
        limit: Option<i64>,
        offset: Option<i64>,
    ) -> Result<Vec<desktop_shared::commands::coding_memory::SensitivityRow>, ApiError> {
        handlers::sensitivity_inspector(
            self.storage_pool.inner(),
            limit.unwrap_or(200),
            offset.unwrap_or(0),
        )
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))
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
    // The desktop binary itself acts as the hook (via `--hook` early-exit).
    // Resolution order:
    //   1. KLYNTBOT_HOOK_BINARY env var (test/override).
    //   2. The currently-running desktop executable.
    if let Ok(p) = std::env::var("KLYNTBOT_HOOK_BINARY") {
        if !p.trim().is_empty() {
            return Ok(std::path::PathBuf::from(p));
        }
    }
    std::env::current_exe()
        .map_err(|e| desktop_shared::errors::ApiError::new("INTERNAL_ERROR", e.to_string()))
}
