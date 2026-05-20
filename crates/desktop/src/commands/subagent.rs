use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn subagent_list_active(_thread_id: String) -> () {
    Err(ApiError::new("NOT_IMPLEMENTED", "subagent commands removed"))
}

#[klynt_command]
pub async fn subagent_cancel(_agent_id: String) -> () {
    Err(ApiError::new("NOT_IMPLEMENTED", "subagent commands removed"))
}

#[klynt_command]
pub async fn subagent_inspect(_agent_id: String) -> () {
    Err(ApiError::new("NOT_IMPLEMENTED", "subagent commands removed"))
}

#[klynt_command]
pub async fn subagent_list_for_session(_session_id: String) -> () {
    Err(ApiError::new("NOT_IMPLEMENTED", "subagent commands removed"))
}
