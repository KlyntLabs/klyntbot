//! Voice conversation Tauri command handlers — thin adapter layer.

use desktop_macros::klynt_command;
use desktop_shared::commands::voice_conversation::*;

fn set_tray_voice(active: bool, phase: u8) {
    crate::tray_countdown::VOICE_ACTIVE.store(active, std::sync::atomic::Ordering::Relaxed);
    crate::tray_countdown::VOICE_PHASE.store(phase, std::sync::atomic::Ordering::Relaxed);
    crate::tray_countdown::wake();
}

#[klynt_command]
pub async fn voice_conversation_start(
    session_key: Option<String>,
) -> VoiceConversationStartResponse {
    let result = state.voice_conversation_start(session_key).await?;
    set_tray_voice(true, 1);
    Ok(result)
}

#[klynt_command]
pub async fn voice_conversation_pause() -> () {
    let result = state.voice_conversation_pause().await;
    set_tray_voice(false, 0);
    result
}

#[klynt_command]
pub async fn voice_conversation_resume() -> () {
    let result = state.voice_conversation_resume().await;
    set_tray_voice(true, 1);
    result
}

#[klynt_command]
pub async fn voice_conversation_interrupt() -> () {
    state.voice_conversation_interrupt().await
}

#[klynt_command]
pub async fn voice_conversation_continue() -> () {
    state.voice_conversation_continue().await
}

#[klynt_command]
pub async fn voice_conversation_new_session() -> VoiceConversationStartResponse {
    let result = state.voice_conversation_new_session().await?;
    set_tray_voice(true, 1);
    Ok(result)
}

#[klynt_command]
pub async fn voice_conversation_end() -> () {
    let result = state.voice_conversation_end().await;
    set_tray_voice(false, 0);
    result
}

#[klynt_command]
pub async fn voice_conversation_status() -> VoiceConversationStatusResponse {
    state.voice_conversation_status_with_title().await
}
