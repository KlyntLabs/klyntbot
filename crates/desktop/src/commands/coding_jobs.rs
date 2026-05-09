use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;
use feature_coding_bash::{BashJobView, BashJobsPanelView};

#[derive(serde::Serialize, specta::Type)]
pub struct OpenLogResult {
    pub opened: bool,
}

#[klynt_command]
pub async fn coding_job_list(
    thread_id: String,
    agent_chain: Vec<String>,
    active_only: Option<bool>,
) -> BashJobsPanelView {
    let jobs = state
        .coding_job_list(&thread_id, &agent_chain, active_only.unwrap_or(false))
        .await
        .map_err(ApiError::from)?;
    Ok(BashJobsPanelView { jobs })
}

#[klynt_command]
pub async fn coding_job_output(
    job_id: String,
    since: Option<u64>,
) -> app_core::handlers::coding_jobs::JobOutputView {
    state
        .coding_job_output(&job_id, since.unwrap_or(0))
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_job_stop(job_id: String) -> BashJobView {
    state
        .coding_job_stop(&job_id)
        .await
        .map_err(ApiError::from)
}

#[klynt_command]
pub async fn coding_job_open_log(job_id: String) -> OpenLogResult {
    let path = state.coding_job_log_path(&job_id).map_err(ApiError::from)?;
    if !path.exists() {
        return Err(ApiError::new(
            "NOT_FOUND",
            format!("log file not found: {}", path.display()),
        ));
    }
    open::that(&path).map_err(|e| ApiError::new("OPEN_ERROR", format!("failed to open log: {e}")))?;
    Ok(OpenLogResult { opened: true })
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &::app_core::state::AppCore,
    body: &serde_json::Value,
) -> Option<desktop_shared::CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "coding_job_list" => {
            let thread_id = try_field!(dev::get_str(body, "threadId"));
            let agent_chain = body.get("agentChain")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
                .unwrap_or_else(|| vec!["root".into()]);
            let active_only = body.get("activeOnly").and_then(|v| v.as_bool()).unwrap_or(false);
            dev::val(
                core.coding_job_list(&thread_id, &agent_chain, active_only)
                    .await
                    .map(|jobs| serde_json::json!({ "jobs": jobs }))
                    .map_err(ApiError::from),
            )
        }
        "coding_job_output" => {
            let job_id = try_field!(dev::get_str(body, "jobId"));
            let since = body.get("since").and_then(|v| v.as_u64()).unwrap_or(0);
            dev::val(core.coding_job_output(&job_id, since).await.map_err(ApiError::from))
        }
        "coding_job_stop" => {
            let job_id = try_field!(dev::get_str(body, "jobId"));
            dev::val(
                core.coding_job_stop(&job_id)
                    .await
                    .map_err(ApiError::from),
            )
        }
        "coding_job_open_log" => {
            let job_id = try_field!(dev::get_str(body, "jobId"));
            dev::val(
                core.coding_job_log_path(&job_id)
                    .and_then(|path| {
                        if !path.exists() {
                            Err(ApiError::new(
                                "NOT_FOUND",
                                format!("log file not found: {}", path.display()),
                            ))
                        } else {
                            open::that(&path)
                                .map(|_| OpenLogResult { opened: true })
                                .map_err(|e| ApiError::new("OPEN_ERROR", format!("failed to open log: {e}")))
                        }
                    }),
            )
        }
        _ => return None,
    })
}
