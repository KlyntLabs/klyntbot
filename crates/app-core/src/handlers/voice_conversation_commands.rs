//! Voice conversation command handlers — business logic for voice conversation IPC.

use desktop_shared::commands::voice_conversation::*;
use desktop_shared::errors::ApiError;

use crate::handlers::voice_conversation::{ConversationPhase, StartResponse};
use crate::state::AppCore;

impl From<StartResponse> for VoiceConversationStartResponse {
    fn from(r: StartResponse) -> Self {
        Self {
            session_key: r.session_key,
            session_title: r.session_title,
            is_continuing: r.is_continuing,
        }
    }
}

impl AppCore {
    /// Start a voice conversation (resolves/creates session).
    pub async fn voice_conversation_start(
        &self,
        session_key: Option<String>,
    ) -> Result<VoiceConversationStartResponse, ApiError> {
        let manager = self.voice_conversation_manager()?;
        let result = manager
            .start(session_key)
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", e.to_string()))?;
        Ok(result.into())
    }

    /// Pause the voice conversation.
    pub async fn voice_conversation_pause(&self) -> Result<(), ApiError> {
        let manager = self.voice_conversation_manager()?;
        manager
            .send_command(crate::handlers::voice_conversation::VoiceCommand::Pause)
            .await;
        Ok(())
    }

    /// Resume the voice conversation.
    pub async fn voice_conversation_resume(&self) -> Result<(), ApiError> {
        let manager = self.voice_conversation_manager()?;
        manager
            .send_command(crate::handlers::voice_conversation::VoiceCommand::Resume)
            .await;
        Ok(())
    }

    /// Interrupt TTS playback (user started speaking).
    pub async fn voice_conversation_interrupt(&self) -> Result<(), ApiError> {
        let manager = self.voice_conversation_manager()?;
        manager
            .send_command(crate::handlers::voice_conversation::VoiceCommand::Interrupt)
            .await;
        Ok(())
    }

    /// Continue TTS from where it was interrupted.
    pub async fn voice_conversation_continue(&self) -> Result<(), ApiError> {
        let manager = self.voice_conversation_manager()?;
        manager
            .send_command(crate::handlers::voice_conversation::VoiceCommand::Continue)
            .await;
        Ok(())
    }

    /// Start a brand new session (discards current context).
    pub async fn voice_conversation_new_session(
        &self,
    ) -> Result<VoiceConversationStartResponse, ApiError> {
        let manager = self.voice_conversation_manager()?;
        let result = manager
            .new_session()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", e.to_string()))?;
        Ok(result.into())
    }

    /// End the voice conversation entirely.
    pub async fn voice_conversation_end(&self) -> Result<(), ApiError> {
        let manager = self.voice_conversation_manager()?;
        manager
            .send_command(crate::handlers::voice_conversation::VoiceCommand::End)
            .await;
        Ok(())
    }

    /// Get the current voice conversation status.
    pub async fn voice_conversation_status(
        &self,
    ) -> Result<VoiceConversationStatusResponse, ApiError> {
        let manager = self.voice_conversation_manager()?;
        let state = manager.state.lock().await;
        let engine_kind = self
            .voice_service
            .as_ref()
            .and_then(|svc| svc.engine_kind())
            .map(|e| format!("{e:?}").to_lowercase());

        Ok(VoiceConversationStatusResponse {
            phase: state.phase.as_str().to_string(),
            session_key: state.session_key.as_ref().map(|k| k.to_string()),
            session_title: None, // Populated from DB if needed (avoids async in lock)
            turn_count: state.turn_count,
            paused: state.paused,
            continue_available: state.interrupted && state.pending_response_text.is_some(),
            engine_kind,
        })
    }

    /// Get status with session title resolved from DB.
    pub async fn voice_conversation_status_with_title(
        &self,
    ) -> Result<VoiceConversationStatusResponse, ApiError> {
        let mut status = self.voice_conversation_status().await?;
        if status.phase != ConversationPhase::Idle.as_str() {
            if let Some(ref key) = status.session_key {
                if let Ok(row) = self.repos.sessions.get_session(key).await {
                    status.session_title = row
                        .metadata
                        .get("title")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                }
            }
        }
        Ok(status)
    }
}
