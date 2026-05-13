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
    pub tty: bool,
    pub tty_rows: Option<u16>,
    pub tty_cols: Option<u16>,
    pub attached_user_at: Option<String>,
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
            tty: false,
            tty_rows: None,
            tty_cols: None,
            attached_user_at: None,
        }
    }
}

impl BashJobView {
    pub fn from_row(row: &storage::repos::BashJobRow) -> Self {
        let (failure_kind, failure_detail) = match row.status.as_str() {
            "Failed" | "Lost" | "Cancelled" => {
                (row.failure_kind.clone(), row.failure_detail.clone())
            }
            _ => (None, None),
        };
        Self {
            id: row.id.clone(),
            session_id: row.session_id.clone(),
            agent_id: row.agent_id.clone(),
            description: row.description.clone(),
            command: row.command.clone(),
            cwd: row.cwd.clone().into(),
            status: row.status.clone(),
            started_at: row.started_at.to_string(),
            finished_at: row.finished_at.map(|t| t.to_string()),
            exit_code: row.exit_code,
            failure_kind,
            failure_detail,
            failure_extracted: row.failure_extracted.as_deref().and_then(|s| serde_json::from_str(s).ok()),
            total_bytes_emitted: row.total_bytes_emitted as u64,
            last_polled_at: row.last_polled_at.map(|t| t.to_string()),
            last_seen_offset: row.last_seen_offset as u64,
            tty: row.tty,
            tty_rows: row.tty_rows,
            tty_cols: row.tty_cols,
            attached_user_at: row.attached_user_at.map(|t| t.to_string()),
        }
    }
}
