//! Voice transcription via Groq Whisper API.

use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;
use tracing::{debug, warn};

use common::{KlyntbotError, ProviderError, Result};

/// Default API base URL for Groq
const DEFAULT_GROQ_API_BASE: &str = "https://api.groq.com/openai/v1";

/// Transcription provider using Groq Whisper
pub struct TranscriptionProvider {
    client: Client,
    api_key: String,
    api_base: String,
}

impl TranscriptionProvider {
    /// Create a new transcription provider with the default Groq API base.
    pub fn new(api_key: impl Into<String>) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| ProviderError::Http(e.to_string()))?;

        Ok(Self {
            client,
            api_key: api_key.into(),
            api_base: DEFAULT_GROQ_API_BASE.to_string(),
        })
    }

    /// Set a custom API base URL.
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
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
            .post(format!(
                "{}/audio/transcriptions",
                self.api_base.trim_end_matches('/')
            ))
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

            return Err(super::types::map_http_error(status.as_u16(), error_text));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_api_base() {
        let provider = TranscriptionProvider::new("test-key").unwrap();
        assert_eq!(provider.api_base, "https://api.groq.com/openai/v1");
    }

    #[test]
    fn test_custom_api_base() {
        let provider = TranscriptionProvider::new("test-key")
            .unwrap()
            .with_api_base("https://custom.api.com/v1");
        assert_eq!(provider.api_base, "https://custom.api.com/v1");
    }

    #[test]
    fn test_trailing_slash_stripped_in_url() {
        let provider = TranscriptionProvider::new("test-key")
            .unwrap()
            .with_api_base("https://custom.api.com/v1/");
        // The trim_end_matches('/') in transcribe() handles this
        let expected_base = "https://custom.api.com/v1/";
        assert_eq!(provider.api_base, expected_base);
        // The actual URL built will strip the trailing slash:
        let url = format!(
            "{}/audio/transcriptions",
            provider.api_base.trim_end_matches('/')
        );
        assert_eq!(url, "https://custom.api.com/v1/audio/transcriptions");
    }
}
