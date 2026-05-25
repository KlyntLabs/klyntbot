use desktop_macros::klynt_command;
use desktop_shared::commands::{
    ContextResumeResponse, ContextTimelineBlockResponse, DashboardIntelligenceResponse,
    InferenceConfigUpdate, InferenceStatsResponse, WorkContextDetailResponse, WorkContextResponse,
    WorkContextUpdateParams,
};
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
