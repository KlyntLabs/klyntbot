//! Voice transcription via Groq Whisper API.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, warn};

use klyntbot_core::{KlyntbotError, ProviderError, Result};

/// Transcription provider using Groq Whisper
pub struct TranscriptionProvider {
    client: Client,
    api_key: String,
}

impl TranscriptionProvider {
    /// Create a new transcription provider
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        Ok(Self {
            client,
            api_key: api_key.into(),
        })
    }

    /// Transcribe an audio file using Groq Whisper
    pub async fn transcribe(&self, audio_path: &str) -> Result<String> {
        let path = Path::new(audio_path);
        if !path.exists() {
            return Err(KlyntbotError::Provider(ProviderError::InvalidResponse(
                format!("Audio file not found: {}", audio_path),
            )));
        }

        debug!("Transcribing audio file: {}", audio_path);

        // Read file
        let file = tokio::fs::read(audio_path)
            .await
            .map_err(KlyntbotError::Io)?;

        // Get filename
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.ogg");

        // Create multipart form
        let part = reqwest::multipart::Part::bytes(file)
            .file_name(filename.to_string())
            .mime_str("audio/ogg")
            .map_err(|e| {
                ProviderError::InvalidResponse(format!("Failed to create form part: {}", e))
            })?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-large-v3");

        // Send request
        let response = self
            .client
            .post("https://api.groq.com/openai/v1/audio/transcriptions")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .multipart(form)
            .send()
            .await
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        // Check status
        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            warn!("Transcription failed: HTTP {}: {}", status, error_text);

            return if status.as_u16() == 429 {
                Err(KlyntbotError::Provider(ProviderError::RateLimited))
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                Err(KlyntbotError::Provider(ProviderError::AuthFailed))
            } else {
                Err(KlyntbotError::Provider(ProviderError::InvalidResponse(
                    format!("HTTP {}: {}", status, error_text),
                )))
            };
        }

        // Parse response
        let transcription: TranscriptionResponse = response.json().await.map_err(|e| {
            ProviderError::InvalidResponse(format!("Failed to parse response: {}", e))
        })?;

        Ok(transcription.text)
    }
}

/// Groq transcription response
#[derive(Debug, Deserialize, Serialize)]
struct TranscriptionResponse {
    text: String,
}
