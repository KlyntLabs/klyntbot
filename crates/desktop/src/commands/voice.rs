//! Voice dictation and settings Tauri command handlers.

use desktop_macros::klynt_command;
use desktop_shared::commands::voice::{AudioDevicesResponse, VoiceModelStatusResponse};
use desktop_shared::CommandResult;

use crate::app_core::AppCore;

#[klynt_command]
pub async fn voice_start_dictation() -> () {
    state.voice_start_dictation().await
}

#[klynt_command]
pub async fn voice_stop_dictation() -> String {
    state.voice_stop_dictation().await
}

#[klynt_command]
pub async fn voice_simulate_event(
    event: desktop_shared::specta_helpers::JsonValueWrapper,
) -> () {
    state.voice_simulate_event(event.0).await
}

#[klynt_command]
pub async fn voice_list_devices() -> AudioDevicesResponse {
    state.voice_list_devices()
}

#[klynt_command]
pub async fn voice_model_status() -> VoiceModelStatusResponse {
    state.voice_model_status()
}

#[klynt_command]
pub async fn voice_download_model(model: String) -> () {
    state.voice_download_model(model).await
}

#[klynt_command]
pub async fn voice_delete_model(model: String) -> () {
    state.voice_delete_model(model).await
}

#[klynt_command]
pub async fn voice_test_persona(persona: String) -> () {
    state.voice_test_persona(persona).await
}

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    body: &serde_json::Value,
) -> Option<CommandResult<serde_json::Value>> {
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
