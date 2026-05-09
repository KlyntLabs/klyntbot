use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BashJobView {
    pub id: String,
    pub session_id: String,
    pub agent_id: String,
    pub description: String,
    pub command: String,
    pub cwd: PathBuf,
    pub status: String,
    pub started_at: String,
    pub finished_at: Option<String>,
    pub exit_code: Option<i32>,
    pub failure_kind: Option<String>,
    pub failure_detail: Option<String>,
    pub failure_extracted: Option<serde_json::Value>,
    pub total_bytes_emitted: u64,
    pub last_polled_at: Option<String>,
    pub last_seen_offset: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
pub struct BashJobsPanelView {
    pub jobs: Vec<BashJobView>,
}

impl BashJobView {
    pub fn from_job_view(v: tools_core::JobView) -> Self {
        let (failure_kind, failure_detail) = match &v.gate_result {
            Some(tools_core::GateResult::Failed { kind, detail, .. }) => {
                (Some(kind.as_db_str().into_owned()), Some(detail.clone()))
            }
            _ => (None, None),
        };
        Self {
            id: v.id.0,
            session_id: v.session_id,
            agent_id: v.agent_id,
            description: v.description,
            command: v.command,
            cwd: v.cwd,
            status: format!("{:?}", v.status),
            started_at: v.started_at.to_string(),
            finished_at: v.finished_at.map(|t| t.to_string()),
            exit_code: v.exit_code,
            failure_kind,
            failure_detail,
            failure_extracted: v.failure_extracted,
            total_bytes_emitted: v.total_bytes_emitted,
            last_polled_at: v.last_polled_at.map(|t| t.to_string()),
            last_seen_offset: v.last_seen_offset,
        }
    }
}
