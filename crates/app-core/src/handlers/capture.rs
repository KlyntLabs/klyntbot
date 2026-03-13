//! Capture-related handlers: ingestion API, shell hook management, file watcher status.

use desktop_shared::commands::{
    CaptureStatusResponse, IngestRequest, IngestResponse, ShellHookStatusResponse,
};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

// ── Ingestion handlers ──────────────────────────────────────────────

impl AppCore {
    pub async fn ingest_event(
        &self,
        token: &str,
        req: IngestRequest,
    ) -> Result<IngestResponse, ApiError> {
        self.validate_ingestion_token(token)?;

        let entry = req
            .into_activity_log_entry()
            .map_err(|e| ApiError::new("VALIDATION", e))?;

        let activity_svc = self
            .activity_ingestion_service
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "Activity log not initialized"))?;

        match activity_svc.ingest(entry).await {
            Ok(Some(id)) => Ok(IngestResponse {
                id: Some(id),
                status: "ok".to_string(),
            }),
            Ok(None) => Ok(IngestResponse {
                id: None,
                status: "excluded".to_string(),
            }),
            Err(e) => Err(ApiError::new("INTERNAL", format!("Ingestion failed: {e}"))),
        }
    }

    pub async fn ingest_batch(
        &self,
        token: &str,
        reqs: Vec<IngestRequest>,
    ) -> Result<desktop_shared::commands::BatchIngestResponse, ApiError> {
        self.validate_ingestion_token(token)?;

        let total = reqs.len();
        let mut entries = Vec::with_capacity(total);
        for req in reqs {
            match req.into_activity_log_entry() {
                Ok(entry) => entries.push(entry),
                Err(e) => {
                    tracing::warn!("Skipping invalid ingest request: {e}");
                }
            }
        }

        let activity_svc = self
            .activity_ingestion_service
            .as_ref()
            .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "Activity log not initialized"))?;

        match activity_svc.ingest_batch(entries).await {
            Ok(count) => Ok(desktop_shared::commands::BatchIngestResponse {
                ingested: count,
                total,
                status: "ok".to_string(),
            }),
            Err(e) => Err(ApiError::new(
                "INTERNAL",
                format!("Batch ingestion failed: {e}"),
            )),
        }
    }

    fn validate_ingestion_token(&self, token: &str) -> Result<(), ApiError> {
        let cfg = self
            .config
            .try_read()
            .map_err(|_| ApiError::new("INTERNAL", "Config lock busy"))?;
        let expected = cfg.capture.ingestion_api.token.as_deref().unwrap_or("");
        if expected.is_empty() || token != expected {
            return Err(ApiError::new("UNAUTHORIZED", "Invalid ingestion token"));
        }
        Ok(())
    }

    // ── Shell hook management ───────────────────────────────────────

    pub async fn install_shell_hook(&self) -> Result<String, ApiError> {
        let cfg = self.config.read().await;
        let shell = crate::infrastructure::shell_hook::detect_shell();
        let api_url = format!("http://127.0.0.1:{}", cfg.capture.ingestion_api.port);
        let token = cfg
            .capture
            .ingestion_api
            .token
            .as_deref()
            .ok_or_else(|| ApiError::new("VALIDATION", "Ingestion token not configured"))?;

        crate::infrastructure::shell_hook::install(&shell, &api_url, token)
            .map_err(|e| ApiError::new("INTERNAL", format!("{e}")))
    }

    pub async fn uninstall_shell_hook(&self) -> Result<String, ApiError> {
        let shell = crate::infrastructure::shell_hook::detect_shell();
        crate::infrastructure::shell_hook::uninstall(&shell).map_err(|e| ApiError::new("INTERNAL", format!("{e}")))
    }

    pub async fn get_shell_hook_status(&self) -> Result<ShellHookStatusResponse, ApiError> {
        let shell = crate::infrastructure::shell_hook::detect_shell();
        let rc_file = crate::infrastructure::shell_hook::rc_file_for_shell(&shell)
            .map_err(|e| ApiError::new("INTERNAL", format!("{e}")))?;
        let installed = std::fs::read_to_string(&rc_file)
            .map(|c| c.contains(crate::infrastructure::shell_hook::MARKER_START))
            .unwrap_or(false);

        Ok(ShellHookStatusResponse {
            installed,
            shell,
            rc_file,
        })
    }

    // ── Capture status ──────────────────────────────────────────────

    pub async fn get_capture_status(&self) -> Result<CaptureStatusResponse, ApiError> {
        // Read config and release lock before async I/O
        let (
            file_watcher_active,
            file_watcher_directories,
            ingestion_api_enabled,
            ingestion_api_port,
        ) = {
            let cfg = self.config.read().await;
            (
                cfg.capture.file_watcher.enabled,
                cfg.capture.file_watcher.directories.clone(),
                cfg.capture.ingestion_api.enabled,
                cfg.capture.ingestion_api.port,
            )
        };

        let shell_status = self.get_shell_hook_status().await?;

        // Query event counts by source for last 24h
        let event_counts = activity_log::ActivityLogRepo::count_by_source_since(
            &self.storage_pool,
            chrono::Utc::now() - chrono::Duration::hours(24),
        )
        .await
        .unwrap_or_default();

        Ok(CaptureStatusResponse {
            shell_hook_installed: shell_status.installed,
            file_watcher_active,
            file_watcher_directories,
            ingestion_api_enabled,
            ingestion_api_port,
            event_counts_24h: event_counts,
        })
    }

    // ── Ingestion token management ──────────────────────────────────

    pub async fn get_ingestion_token(&self) -> Result<String, ApiError> {
        let cfg = self.config.read().await;
        Ok(cfg.capture.ingestion_api.token.clone().unwrap_or_default())
    }

    pub async fn regenerate_ingestion_token(&self) -> Result<String, ApiError> {
        let mut cfg = self.config.write().await;
        let new_token = uuid::Uuid::new_v4().to_string();
        cfg.capture.ingestion_api.token = Some(new_token.clone());
        config::save(&cfg)
            .await
            .map_err(crate::errors::map_config_save_err)?;
        Ok(new_token)
    }
}
