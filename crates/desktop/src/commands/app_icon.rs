use desktop_macros::klynt_command;
use desktop_shared::errors::ApiError;

#[klynt_command]
pub async fn app_icon_read(_app_name: String) -> Option<String> {
    Err(ApiError::new("NOT_IMPLEMENTED", "app_icon_read removed"))
}
