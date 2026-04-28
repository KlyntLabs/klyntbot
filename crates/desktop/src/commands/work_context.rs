use desktop_macros::klynt_command;
use desktop_shared::commands::{
    ContextResumeResponse, ContextTimelineBlockResponse, DashboardIntelligenceResponse,
    InferenceConfigUpdate, InferenceStatsResponse, WorkContextDetailResponse, WorkContextResponse,
    WorkContextUpdateParams,
};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn list_work_contexts(status: Option<String>) -> Vec<WorkContextResponse> {
    state.list_work_contexts(status).await
}

#[klynt_command]
pub async fn get_work_context(id: String) -> Option<WorkContextResponse> {
    state.get_work_context(id).await
}

#[klynt_command]
pub async fn get_work_context_detail(id: String) -> WorkContextDetailResponse {
    state.get_work_context_detail(id).await
}

#[klynt_command]
pub async fn update_work_context(params: WorkContextUpdateParams) -> WorkContextResponse {
    state.update_work_context(params).await
}

#[klynt_command]
pub async fn archive_work_context(id: String) -> WorkContextResponse {
    state.archive_work_context(id).await
}

#[klynt_command]
pub async fn merge_work_contexts(keep_id: String, remove_id: String) -> WorkContextResponse {
    state.merge_work_contexts(keep_id, remove_id).await
}

#[klynt_command]
pub async fn search_work_contexts(query: String) -> Vec<WorkContextResponse> {
    state.search_work_contexts(query).await
}

#[klynt_command]
pub async fn get_context_timeline(
    date: String,
    tz_offset_mins: Option<i32>,
) -> Vec<ContextTimelineBlockResponse> {
    state.get_context_timeline(date, tz_offset_mins).await
}

#[klynt_command]
pub async fn get_context_resume_data(context_id: String) -> ContextResumeResponse {
    state.get_context_resume_data(context_id).await
}

#[klynt_command]
pub async fn get_inference_stats() -> InferenceStatsResponse {
    state.get_inference_stats().await
}

#[klynt_command]
pub async fn get_dashboard_intelligence(
    date: String,
    tz_offset_mins: Option<i32>,
) -> DashboardIntelligenceResponse {
    state
        .get_dashboard_intelligence(&date, tz_offset_mins)
        .await
}

#[klynt_command]
pub async fn update_inference_config(config: InferenceConfigUpdate) -> () {
    state.update_inference_config(config).await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
    use super::dev_helpers::{self as dev, try_field};
    Some(match cmd {
        "list_work_contexts" => dev::val(core.list_work_contexts(dev::get(body, "status")).await),
        "get_work_context" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.get_work_context(id).await)
        }
        "get_work_context_detail" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.get_work_context_detail(id).await)
        }
        "update_work_context" => dev::val(
            core.update_work_context(try_field!(dev::parse_params(body)))
                .await,
        ),
        "archive_work_context" => {
            let id = try_field!(dev::get_str(body, "id"));
            dev::val(core.archive_work_context(id).await)
        }
        "merge_work_contexts" => {
            let keep_id = try_field!(dev::get_str(body, "keepId"));
            let remove_id = try_field!(dev::get_str(body, "removeId"));
            dev::val(core.merge_work_contexts(keep_id, remove_id).await)
        }
        "search_work_contexts" => {
            let query = try_field!(dev::get_str(body, "query"));
            dev::val(core.search_work_contexts(query).await)
        }
        "get_context_timeline" => {
            let date = try_field!(dev::get_str(body, "date"));
            dev::val(
                core.get_context_timeline(date, dev::get(body, "tzOffsetMins"))
                    .await,
            )
        }
        "get_context_resume_data" => {
            let context_id = try_field!(dev::get_str(body, "contextId"));
            dev::val(core.get_context_resume_data(context_id).await)
        }
        "get_inference_stats" => dev::val(core.get_inference_stats().await),
        "get_dashboard_intelligence" => {
            let date = try_field!(dev::get_str(body, "date"));
            dev::val(
                core.get_dashboard_intelligence(&date, dev::get(body, "tzOffsetMins"))
                    .await,
            )
        }
        "update_inference_config" => dev::val(
            core.update_inference_config(try_field!(dev::parse_params(body)))
                .await,
        ),
        _ => return None,
    })
}
