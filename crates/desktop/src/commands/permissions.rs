//! macOS permission check commands.

use desktop_macros::klynt_raw_command;
use desktop_shared::permissions;
use desktop_shared::CommandResult;

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn permissions_check_accessibility() -> CommandResult<bool> {
    Ok(permissions::check_accessibility())
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn permissions_open_accessibility() -> CommandResult<()> {
    permissions::open_accessibility_settings();
    Ok(())
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn permissions_request_accessibility_for_input() -> CommandResult<bool> {
    Ok(permissions::request_accessibility_for_input())
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn permissions_check_screen_recording() -> CommandResult<bool> {
    Ok(permissions::check_screen_recording())
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn permissions_request_screen_recording() -> CommandResult<bool> {
    Ok(permissions::request_screen_recording())
}

#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub async fn permissions_open_screen_recording() -> CommandResult<()> {
    permissions::open_screen_recording_settings();
    Ok(())
}
