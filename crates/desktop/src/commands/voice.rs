//! Voice dictation and settings Tauri command handlers.

use std::sync::Arc;

use desktop_shared::commands::voice::{AudioDevicesResponse, VoiceModelStatusResponse};
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

#[tauri::command]
pub async fn voice_start_dictation(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    state.voice_start_dictation().await
}

#[tauri::command]
pub async fn voice_stop_dictation(state: State<'_, Arc<AppCore>>) -> Result<String, ApiError> {
    state.voice_stop_dictation().await
}

#[tauri::command]
pub async fn voice_simulate_event(
    state: State<'_, Arc<AppCore>>,
    event: serde_json::Value,
) -> Result<(), ApiError> {
    state.voice_simulate_event(event).await
}

#[tauri::command]
pub async fn voice_list_devices(
    state: State<'_, Arc<AppCore>>,
) -> Result<AudioDevicesResponse, ApiError> {
    state.voice_list_devices()
}

#[tauri::command]
pub async fn voice_model_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceModelStatusResponse, ApiError> {
    state.voice_model_status()
}

#[tauri::command]
pub async fn voice_download_model(
    state: State<'_, Arc<AppCore>>,
    model: String,
) -> Result<(), ApiError> {
    state.voice_download_model(model).await
}

#[tauri::command]
pub async fn voice_delete_model(
    state: State<'_, Arc<AppCore>>,
    model: String,
) -> Result<(), ApiError> {
    state.voice_delete_model(model).await
}

#[tauri::command]
pub async fn voice_test_persona(
    state: State<'_, Arc<AppCore>>,
    persona: String,
) -> Result<(), ApiError> {
    state.voice_test_persona(persona).await
}

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "voice_start_dictation",
    "voice_stop_dictation",
    "voice_simulate_event",
    "voice_list_devices",
    "voice_model_status",
    "voice_download_model",
    "voice_delete_model",
    "voice_test_persona",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;

    Some(match cmd {
        "voice_start_dictation" => dev::val(core.voice_start_dictation().await),
        "voice_stop_dictation" => dev::val(core.voice_stop_dictation().await),
        "voice_simulate_event" => {
            let event = body
                .get("event")
                .cloned()
                .unwrap_or(serde_json::json!(null));
            dev::val(core.voice_simulate_event(event).await)
        }
        "voice_list_devices" => dev::val(core.voice_list_devices()),
        "voice_model_status" => dev::val(core.voice_model_status()),
        "voice_download_model" => match dev::get_str(body, "model") {
            Ok(model) => dev::val(core.voice_download_model(model).await),
            Err(e) => Err(e),
        },
        "voice_delete_model" => match dev::get_str(body, "model") {
            Ok(model) => dev::val(core.voice_delete_model(model).await),
            Err(e) => Err(e),
        },
        "voice_test_persona" => match dev::get_str(body, "persona") {
            Ok(persona) => dev::val(core.voice_test_persona(persona).await),
            Err(e) => Err(e),
        },
        _ => return None,
    })
}
