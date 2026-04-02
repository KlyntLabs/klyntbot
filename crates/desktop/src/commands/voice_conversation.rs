//! Voice conversation Tauri command handlers — thin adapter layer.

use std::sync::Arc;

use desktop_shared::commands::voice_conversation::*;
use desktop_shared::errors::ApiError;
use tauri::State;

use crate::app_core::AppCore;

fn set_tray_voice(active: bool, phase: u8) {
    crate::tray_countdown::VOICE_ACTIVE.store(active, std::sync::atomic::Ordering::Relaxed);
    crate::tray_countdown::VOICE_PHASE.store(phase, std::sync::atomic::Ordering::Relaxed);
}

#[tauri::command]
pub async fn voice_conversation_start(
    state: State<'_, Arc<AppCore>>,
    session_key: Option<String>,
) -> Result<VoiceConversationStartResponse, ApiError> {
    let result = state.voice_conversation_start(session_key).await?;
    set_tray_voice(true, 1);
    Ok(result)
}

#[tauri::command]
pub async fn voice_conversation_pause(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    let result = state.voice_conversation_pause().await;
    set_tray_voice(false, 0);
    result
}

#[tauri::command]
pub async fn voice_conversation_resume(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    let result = state.voice_conversation_resume().await;
    set_tray_voice(true, 1);
    result
}

#[tauri::command]
pub async fn voice_conversation_interrupt(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    state.voice_conversation_interrupt().await
}

#[tauri::command]
pub async fn voice_conversation_continue(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    state.voice_conversation_continue().await
}

#[tauri::command]
pub async fn voice_conversation_new_session(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceConversationStartResponse, ApiError> {
    let result = state.voice_conversation_new_session().await?;
    set_tray_voice(true, 1);
    Ok(result)
}

#[tauri::command]
pub async fn voice_conversation_end(state: State<'_, Arc<AppCore>>) -> Result<(), ApiError> {
    let result = state.voice_conversation_end().await;
    set_tray_voice(false, 0);
    result
}

#[tauri::command]
pub async fn voice_conversation_status(
    state: State<'_, Arc<AppCore>>,
) -> Result<VoiceConversationStatusResponse, ApiError> {
    state.voice_conversation_status_with_title().await
}

// ── Dev server dispatch ─────────────────────────────────────────────

#[cfg(test)]
pub(crate) const DEV_COMMANDS: &[&str] = &[
    "voice_conversation_start",
    "voice_conversation_pause",
    "voice_conversation_resume",
    "voice_conversation_interrupt",
    "voice_conversation_continue",
    "voice_conversation_new_session",
    "voice_conversation_end",
    "voice_conversation_status",
];

#[cfg(debug_assertions)]
pub(crate) async fn dispatch_dev(
    cmd: &str,
    core: &AppCore,
    _body: &serde_json::Value,
) -> Option<Result<serde_json::Value, ApiError>> {
    use super::dev_helpers as dev;

    Some(match cmd {
        "voice_conversation_start" => {
            let session_key = _body
                .get("sessionKey")
                .and_then(|v| v.as_str())
                .map(String::from);
            dev::val(core.voice_conversation_start(session_key).await)
        }
        "voice_conversation_pause" => dev::val(core.voice_conversation_pause().await),
        "voice_conversation_resume" => dev::val(core.voice_conversation_resume().await),
        "voice_conversation_interrupt" => dev::val(core.voice_conversation_interrupt().await),
        "voice_conversation_continue" => dev::val(core.voice_conversation_continue().await),
        "voice_conversation_new_session" => dev::val(core.voice_conversation_new_session().await),
        "voice_conversation_end" => dev::val(core.voice_conversation_end().await),
        "voice_conversation_status" => dev::val(core.voice_conversation_status_with_title().await),
        _ => return None,
    })
}
