//! Voice dictation and settings Tauri command handlers.

use desktop_macros::klynt_command;
use desktop_shared::commands::voice::{AudioDevicesResponse, VoiceModelStatusResponse};
#[klynt_command]
pub async fn voice_start_dictation() -> () {
    state.voice_start_dictation().await
}

#[klynt_command]
pub async fn voice_stop_dictation() -> String {
    state.voice_stop_dictation().await
}

#[klynt_command]
pub async fn voice_simulate_event(event: desktop_shared::specta_helpers::JsonValueWrapper) -> () {
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
