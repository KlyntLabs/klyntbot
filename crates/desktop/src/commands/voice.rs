//! Voice capture Tauri command handlers — thin adapter layer.

use std::sync::Arc;

use desktop_shared::commands::voice::*;
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn voice_start_capture(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceCaptureInfo, ApiError> {
    let _ = state;
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice capture not yet wired"))
}

#[tauri::command]
pub async fn voice_stop_capture(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    let _ = state;
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice capture not yet wired"))
}

#[tauri::command]
pub async fn voice_dismiss(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    let _ = state;
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice capture not yet wired"))
}

#[tauri::command]
pub async fn voice_get_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceStatusResponse, ApiError> {
    let _ = state;
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice status not yet wired"))
}

#[tauri::command]
pub async fn voice_get_models(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<VoiceModelInfo>, ApiError> {
    let _ = state;
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice models not yet wired"))
}

#[tauri::command]
pub async fn voice_download_model(
    state: State<'_, Arc<AppCore>>,
    request: VoiceDownloadModelRequest,
) -> Result<(), ApiError> {
    let _ = state;
    let _ = request;
    Err(ApiError::new("NOT_IMPLEMENTED", "Voice download not yet wired"))
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "voice_start_capture",
    "voice_stop_capture",
    "voice_dismiss",
    "voice_get_status",
    "voice_get_models",
    "voice_download_model",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    _core: &AppCore,
    _body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    Some(match cmd {
        "voice_start_capture"
        | "voice_stop_capture"
        | "voice_dismiss"
        | "voice_get_status"
        | "voice_get_models"
        | "voice_download_model" => Err(ApiError::new("NOT_IMPLEMENTED", "Voice not yet wired")),
        _ => return None,
    })
}
