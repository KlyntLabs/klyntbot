//! Voice dictation Tauri command handlers.

use std::sync::Arc;

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

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "voice_start_dictation",
    "voice_stop_dictation",
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
        "voice_start_dictation" => dev::val(core.voice_start_dictation().await),
        "voice_stop_dictation" => dev::val(core.voice_stop_dictation().await),
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
