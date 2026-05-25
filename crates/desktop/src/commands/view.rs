use desktop_macros::klynt_command;
use desktop_shared::commands::view::{ActiveViewResponse, SetActiveViewParams};
#[klynt_command]
pub async fn view_set_active(params: SetActiveViewParams) -> () {
    state.view_set_active(params).await
}

#[klynt_command]
pub async fn view_clear_active() -> () {
    state.view_clear_active().await
}

#[klynt_command]
pub async fn view_get_active() -> ActiveViewResponse {
    state.view_get_active().await
}
