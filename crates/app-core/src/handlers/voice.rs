//! Voice dictation handlers — speech-to-text for the chat input field.

use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
    /// Start voice dictation — begins capturing audio for speech-to-text.
    pub async fn voice_start_dictation(&self) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        service
            .start_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;
        Ok(())
    }

    /// Stop voice dictation — finalizes capture and returns the transcript text.
    pub async fn voice_stop_dictation(&self) -> Result<String, ApiError> {
        let service = self.voice_service()?;
        let result = service
            .stop_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", &e.to_string()))?;

        match result {
            Some((transcript, _metadata)) => Ok(transcript.text),
            None => Ok(String::new()),
        }
    }

    /// Simulate a VoiceEvent for dev/testing.
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
