//! Tauri command for opening per-repo coding windows.

use crate::lazy_window::get_or_create_window;
use desktop_macros::klynt_raw_command;

/// Open a coding window for a specific repo.
///
/// Creates a `coding:{repo_id}` window if it doesn't exist, or focuses
/// the existing one. The `repo_id` must be ASCII alphanumeric with
/// dashes/underscores, 1–64 characters.
#[klynt_raw_command]
#[tauri::command]
#[specta::specta]
pub fn coding_open_repo_window(app: tauri::AppHandle, repo_id: String) -> Result<(), String> {
    // Validate repo_id
    if repo_id.is_empty() || repo_id.len() > 64 {
        return Err("repo_id must be 1-64 characters".into());
    }
    if !repo_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err("repo_id must be ASCII alphanumeric, dash, or underscore".into());
    }

    let label = format!("coding:{repo_id}");
    get_or_create_window(&app, &label)
        .ok_or_else(|| format!("failed to create window for label '{label}'"))?;
    Ok(())
}
