//! Voice capture handlers — business logic for voice commands.

use desktop_shared::commands::voice::*;
use desktop_shared::errors::ApiError;
use voice_engine::{EngineKind, ModelState, WhisperModelSize};

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

    /// Stop the current voice capture and finalize.
    pub async fn voice_stop_capture(&self) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        let result = service
            .stop_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;

        if let Some((transcript, metadata)) = result {
            // Create InboundMessage and publish to MessageBus
            let mut msg_metadata = std::collections::HashMap::new();
            msg_metadata.insert(
                "voice_metadata".to_string(),
                serde_json::to_value(&metadata).unwrap_or_default(),
            );

            let message = bus::InboundMessage {
                channel: common::ChannelName::new("desktop"),
                sender_id: "local".to_string(),
                chat_id: common::ChatId::new("desktop-voice"),
                content: transcript.text.clone(),
                timestamp: chrono::Utc::now(),
                media: vec![],
                metadata: msg_metadata,
                kind: bus::MessageKind::Voice,
            };

            if let Err(e) = self.bus.publish_inbound(message).await {
                tracing::warn!("Failed to publish voice transcript to bus: {e}");
            }
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
        let model_state = service.model_state();
        let engine = service.engine_kind().unwrap_or(EngineKind::Local);

        Ok(VoiceStatusResponse {
            state,
            model_state,
            engine,
            enabled: true,
        })
    }

    /// Get available voice models.
    pub async fn voice_get_models(&self) -> Result<Vec<VoiceModelInfo>, ApiError> {
        let service = self.voice_service()?;
        let models = vec![
            VoiceModelInfo {
                size: WhisperModelSize::Small,
                display_name: WhisperModelSize::Small.display_name().to_string(),
                size_bytes: WhisperModelSize::Small.size_bytes(),
                available: service.model_state() != ModelState::NotDownloaded,
            },
            VoiceModelInfo {
                size: WhisperModelSize::Medium,
                display_name: WhisperModelSize::Medium.display_name().to_string(),
                size_bytes: WhisperModelSize::Medium.size_bytes(),
                available: false, // Medium not downloaded by default
            },
        ];
        Ok(models)
    }

    /// Start downloading a voice model.
    pub async fn voice_download_model(
        &self,
        request: VoiceDownloadModelRequest,
    ) -> Result<(), ApiError> {
        let _service = self.voice_service()?;
        // TODO: Delegate to ModelManager::start_download
        Err(ApiError::new(
            "NOT_IMPLEMENTED",
            &format!(
                "Model download for {:?} not yet implemented",
                request.model_size
            ),
        ))
    }
}
