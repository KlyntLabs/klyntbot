use serde::{Deserialize, Serialize};

// ── Capture / Ingestion ───────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestRequest {
    pub source: String,
    pub actor: Option<String>,
    pub resource_type: Option<String>,
    pub resource_id: Option<String>,
    pub resource_name: Option<String>,
    pub action: String,
    pub content_preview: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub app_name: Option<String>,
    pub project_id: Option<String>,
    pub duration_secs: Option<i64>,
}

impl IngestRequest {
    pub fn into_activity_log_entry(self) -> Result<activity_log::ActivityLogEntry, String> {
        let source = activity_log::ActivitySource::parse(&self.source)
            .ok_or_else(|| format!("Unknown source: {}", self.source))?;
        let actor = self
            .actor
            .as_deref()
            .and_then(activity_log::ActivityActor::parse)
            .unwrap_or(activity_log::ActivityActor::User);

        Ok(activity_log::ActivityLogEntry {
            id: activity_log::normalizers::new_ulid(),
            timestamp: common::time::bridge::chrono_to_jiff(chrono::Utc::now()),
            source,
            actor,
            resource_type: self.resource_type,
            resource_id: self.resource_id,
            resource_name: self.resource_name,
            action: self.action,
            content_preview: self.content_preview,
            content_hash: None,
            metadata: self.metadata,
            app_name: self.app_name,
            project_id: self.project_id,
            work_context_id: None,
            embedding_id: None,
            duration_secs: self.duration_secs,
            session_key: None,
            is_sensitive: false,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IngestResponse {
    pub id: Option<String>,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchIngestResponse {
    pub ingested: usize,
    pub total: usize,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellHookStatusResponse {
    pub installed: bool,
    pub shell: String,
    pub rc_file: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureStatusResponse {
    pub shell_hook_installed: bool,
    pub file_watcher_active: bool,
    pub file_watcher_directories: Vec<String>,
    pub ingestion_api_enabled: bool,
    pub ingestion_api_port: u16,
    pub event_counts_24h: std::collections::HashMap<String, i64>,
}
