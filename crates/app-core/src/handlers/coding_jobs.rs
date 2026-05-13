//! Background bash job handlers — list, output, stop, log path.

use std::sync::Arc;

use desktop_shared::errors::ApiError;
use feature_coding_bash::BashJobView;
use tools_core::JobSupervisorHandle;

use crate::state::AppCore;

fn parse_job_id(job_id: &str) -> Result<tools_core::JobId, ApiError> {
    tools_core::JobId::from_str(job_id).map_err(|e| ApiError::new("INVALID_JOB_ID", e.to_string()))
}

fn require_supervisor(
    core: &AppCore,
) -> Result<&Arc<feature_coding_bash::JobSupervisor>, ApiError> {
    core.job_supervisor
        .as_ref()
        .ok_or_else(|| ApiError::new("FEATURE_DISABLED", "background bash jobs not initialized"))
}

#[tracing::instrument(skip(core), err)]
pub async fn coding_jobs_list(
    core: &AppCore,
    thread_id: &str,
    agent_chain: &[String],
    active_only: bool,
) -> Result<Vec<BashJobView>, ApiError> {
    let views = require_supervisor(core)?
        .list_for_thread(thread_id, agent_chain, active_only)
        .await;
    Ok(views.into_iter().map(BashJobView::from_job_view).collect())
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct JobOutputView {
    pub bytes: String,
    pub new_offset: u64,
    pub bisect_generation: u64,
    pub bisect_occurred_since: bool,
    pub total_bytes_emitted: u64,
}

impl From<tools_core::RingRead> for JobOutputView {
    fn from(read: tools_core::RingRead) -> Self {
        Self {
            bytes: String::from_utf8_lossy(&read.bytes).into_owned(),
            new_offset: read.new_offset,
            bisect_generation: read.bisect_generation,
            bisect_occurred_since: read.bisect_occurred_since,
            total_bytes_emitted: read.total_bytes_emitted,
        }
    }
}

#[tracing::instrument(skip(core), err)]
pub async fn coding_jobs_output(
    core: &AppCore,
    job_id: &str,
    since: u64,
) -> Result<JobOutputView, ApiError> {
    let id = parse_job_id(job_id)?;
    let read = require_supervisor(core)?
        .output_delta(&id, since, false, 0)
        .await
        .map_err(|e| ApiError::new("JOB_ERROR", e.to_string()))?;
    Ok(JobOutputView::from(read))
}

#[tracing::instrument(skip(core), err)]
pub async fn coding_jobs_stop(core: &AppCore, job_id: &str) -> Result<BashJobView, ApiError> {
    let id = parse_job_id(job_id)?;
    let view = require_supervisor(core)?
        .stop(&id, "user requested")
        .await
        .map_err(|e| ApiError::new("JOB_ERROR", e.to_string()))?;
    Ok(BashJobView::from_job_view(view))
}

#[tracing::instrument(skip(core), err)]
pub fn coding_jobs_log_path(core: &AppCore, job_id: &str) -> Result<std::path::PathBuf, ApiError> {
    let id = parse_job_id(job_id)?;
    Ok(require_supervisor(core)?.log_path(&id))
}

#[derive(serde::Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct AttachResult {
    pub ws_url: String,
    pub rows: u16,
    pub cols: u16,
    pub tail_b64: String,
}

impl From<tools_core::AttachHandle> for AttachResult {
    fn from(h: tools_core::AttachHandle) -> Self {
        Self {
            ws_url: h.ws_url,
            rows: h.rows,
            cols: h.cols,
            tail_b64: h.tail_b64,
        }
    }
}

#[tracing::instrument(skip(core), err)]
pub async fn coding_task_attach(
    core: &AppCore,
    job_id: &str,
) -> Result<AttachResult, ApiError> {
    let id = parse_job_id(job_id)?;
    let handle = require_supervisor(core)?
        .attach(&id)
        .await
        .map_err(map_attach_error)?;
    Ok(handle.into())
}

#[tracing::instrument(skip(core), err)]
pub async fn coding_task_detach(core: &AppCore, job_id: &str) -> Result<(), ApiError> {
    let id = parse_job_id(job_id)?;
    require_supervisor(core)?
        .detach(&id)
        .await
        .map_err(map_attach_error)?;
    Ok(())
}

fn map_attach_error(e: tools_core::AttachError) -> ApiError {
    use tools_core::AttachError;
    match e {
        AttachError::NotFound(id) => ApiError::new("NOT_FOUND", format!("job not found: {id}")),
        AttachError::NotPty => ApiError::new("NOT_PTY", "job is not a PTY"),
        AttachError::AlreadyAttached => {
            ApiError::new("ALREADY_ATTACHED", "another window is already attached")
        }
        AttachError::Storage(msg) => ApiError::new("STORAGE_ERROR", msg),
        AttachError::Io(e) => ApiError::new("IO_ERROR", e.to_string()),
        AttachError::Ws(msg) => ApiError::new("WS_ERROR", msg),
        AttachError::Supervisor(msg) => ApiError::new("SUPERVISOR_ERROR", msg),
    }
}
