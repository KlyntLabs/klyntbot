//! macOS permission check commands.

use desktop_shared::errors::ApiError;
use desktop_shared::permissions;

#[tauri::command]
pub async fn permissions_check_accessibility() -> Result<bool, ApiError> {
    Ok(permissions::check_accessibility())
}

#[tauri::command]
pub async fn permissions_open_accessibility() -> Result<(), ApiError> {
    permissions::open_accessibility_settings();
    Ok(())
}
