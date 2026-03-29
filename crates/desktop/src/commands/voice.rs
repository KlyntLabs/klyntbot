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
    let result = state.voice_start_capture().await?;
    crate::tray_countdown::VOICE_ACTIVE.store(true, std::sync::atomic::Ordering::Relaxed);
    Ok(result)
}

#[tauri::command]
pub async fn voice_stop_capture(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    let result = state.voice_stop_capture().await;
    crate::tray_countdown::VOICE_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    result
}

#[tauri::command]
pub async fn voice_dismiss(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    let result = state.voice_dismiss().await;
    crate::tray_countdown::VOICE_ACTIVE.store(false, std::sync::atomic::Ordering::Relaxed);
    result
}

#[tauri::command]
pub async fn voice_get_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceStatusResponse, ApiError> {
    state.voice_get_status().await
}

#[tauri::command]
pub async fn voice_get_models(
    state: State<'_, Arc<AppCore>>,
) -> Result<Vec<VoiceModelInfo>, ApiError> {
    state.voice_get_models().await
}

#[tauri::command]
pub async fn voice_download_model(
    state: State<'_, Arc<AppCore>>,
    request: VoiceDownloadModelRequest,
) -> Result<(), ApiError> {
    state.voice_download_model(request).await
}

#[tauri::command]
pub async fn voice_simulate_event(
    state: State<'_, Arc<AppCore>>,
    event: serde_json::Value,
) -> Result<(), ApiError> {
    state.voice_simulate_event(event).await
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
    "voice_simulate_event",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;

    Some(match cmd {
        "voice_start_capture" => dev::val(core.voice_start_capture().await),
        "voice_stop_capture" => dev::val(core.voice_stop_capture().await),
        "voice_dismiss" => dev::val(core.voice_dismiss().await),
        "voice_get_status" => dev::val(core.voice_get_status().await),
        "voice_get_models" => dev::val(core.voice_get_models().await),
        "voice_download_model" => {
            // For dev server, return mock success since model download isn't wired yet
            Ok(serde_json::json!(null))
        }
        "voice_simulate_event" => {
            let event = body
                .get("event")
                .cloned()
                .unwrap_or(serde_json::json!(null));
            dev::val(core.voice_simulate_event(event).await)
        }
        _ => return None,
    })
}
