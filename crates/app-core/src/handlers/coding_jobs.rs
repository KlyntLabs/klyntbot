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
