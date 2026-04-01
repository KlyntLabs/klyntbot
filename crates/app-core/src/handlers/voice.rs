//! Voice capture handlers — business logic for voice commands.

use desktop_shared::commands::voice::*;
use desktop_shared::errors::ApiError;
use voice_engine::EngineKind;

use crate::state::AppCore;

impl AppCore {
    /// Start a voice capture session.
    pub async fn voice_start_capture(&self) -> Result<VoiceCaptureInfo, ApiError> {
        let service = self.voice_service()?;
        let (session_id, engine) = service
            .start_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;

        Ok(VoiceCaptureInfo {
            session_id,
            engine,
            state: voice_engine::VoiceSessionState::Capturing,
        })
    }

    /// Stop the current voice capture, send transcript through agent, relay response to TTS.
    pub async fn voice_stop_capture(&self) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        let result = service
            .stop_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;

        if let Some((transcript, _metadata)) = result {
            if transcript.text.trim().is_empty() {
                return Ok(());
            }

            // Send transcript through the agent pipeline and relay response to TTS.
            let session_key = "desktop-voice".to_string();
            let voice_svc = self.voice_service()?.clone();
            let agent = self.agent.clone();

            tokio::spawn(async move {
                match agent
                    .process_direct_streaming(transcript.text.clone(), session_key)
                    .await
                {
                    Ok(streaming_handle) => {
                        let mut event_rx = streaming_handle.event_rx;
                        while let Some(event) = event_rx.recv().await {
                            if let agent::AgentEvent::Done { content, .. } = event {
                                if let Err(e) = voice_svc
                                    .handle_response(&content, &voice_engine::TtsParams::default())
                                    .await
                                {
                                    tracing::warn!("Voice TTS response failed: {e}");
                                }
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!("Agent processing of voice transcript failed: {e}");
                    }
                }
            });
        }

        Ok(())
    }

    /// Dismiss the voice orb (processing continues in background).
    pub async fn voice_dismiss(&self) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        service.dismiss().await;
        Ok(())
    }

    /// Get current voice status.
    pub async fn voice_get_status(&self) -> Result<VoiceStatusResponse, ApiError> {
        let service = self.voice_service()?;
        let state = service.session_state().await;
        let engine = service.engine_kind().unwrap_or(EngineKind::Local);

        Ok(VoiceStatusResponse {
            state,
            engine,
            enabled: true,
        })
    }

    /// Simulate a VoiceEvent for dev/testing (inject event into the frontend stream).
    pub async fn voice_simulate_event(
        &self,
        event_json: serde_json::Value,
    ) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        let event: voice_engine::VoiceEvent = serde_json::from_value(event_json)
            .map_err(|e| ApiError::new("VALIDATION", &format!("Invalid VoiceEvent: {e}")))?;
        service
            .emit_event(event)
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))
    }
}
