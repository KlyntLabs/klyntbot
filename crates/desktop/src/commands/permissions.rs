//! macOS permission check commands.

use desktop_shared::CommandResult;
use desktop_shared::permissions;

#[tauri::command]
#[specta::specta]
pub async fn permissions_check_accessibility() -> CommandResult<bool> {
    Ok(permissions::check_accessibility())
}

#[tauri::command]
#[specta::specta]
pub async fn permissions_open_accessibility() -> CommandResult<()> {
    permissions::open_accessibility_settings();
    Ok(())
}
