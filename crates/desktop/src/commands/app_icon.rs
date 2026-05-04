use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn app_icon_read(app_name: String) -> Option<String> {
    state.app_icon_read(&app_name).await.map_err(ApiError::from)
}
