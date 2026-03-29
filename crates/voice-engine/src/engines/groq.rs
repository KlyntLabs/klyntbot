//! Groq Whisper cloud transcription engine.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tracing::{debug, warn};

use crate::stt::{AudioStream, TranscriptStream, TranscriptionEngine};
use crate::types::{Language, Transcript};

const DEFAULT_GROQ_API_BASE: &str = "https://api.groq.com/openai/v1";

fn mime_type_for_audio(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .as_deref()
    {
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("m4a") => "audio/mp4",
        Some("webm") => "audio/webm",
        Some("flac") => "audio/flac",
        Some("mp4") => "audio/mp4",
        Some("mpeg") | Some("mpga") => "audio/mpeg",
        _ => "audio/ogg",
    }
}

pub struct GroqWhisperEngine {
    client: reqwest::Client,
    api_key: String,
    api_base: String,
}

#[derive(serde::Deserialize)]
struct TranscriptionResponse {
    text: String,
}

impl GroqWhisperEngine {
    pub fn new(api_key: impl Into<String>) -> common::Result<Self> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
            })?;

        Ok(Self {
            client,
            api_key: api_key.into(),
            api_base: DEFAULT_GROQ_API_BASE.to_string(),
        })
    }

    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into();
        self
    }
}

#[async_trait]
impl TranscriptionEngine for GroqWhisperEngine {
    async fn transcribe_stream(&self, _audio: AudioStream) -> common::Result<TranscriptStream> {
        Err(common::KlyntbotError::Provider(
            common::ProviderError::InvalidResponse(
                "Groq engine does not support streaming transcription. Use transcribe_file instead."
                    .to_string(),
            ),
        ))
    }

    async fn transcribe_file(
        &self,
        path: &Path,
        _lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        if !path.exists() {
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(format!(
                    "Audio file not found: {}",
                    path.display()
                )),
            ));
        }

        debug!("Transcribing audio file via Groq: {}", path.display());

        let file = tokio::fs::read(path)
            .await
            .map_err(common::KlyntbotError::Io)?;

        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("audio.ogg");
        let mime_type = mime_type_for_audio(path);

        let part = reqwest::multipart::Part::bytes(file)
            .file_name(filename.to_string())
            .mime_str(mime_type)
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                    "Failed to create form part: {}",
                    e
                )))
            })?;

        let form = reqwest::multipart::Form::new()
            .part("file", part)
            .text("model", "whisper-large-v3");

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
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            warn!("Groq transcription failed: HTTP {}: {}", status, error_text);
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::Http(format!("HTTP {}: {}", status, error_text)),
            ));
        }

        let resp: TranscriptionResponse = response.json().await.map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Failed to parse response: {}",
                e
            )))
        })?;

        Ok(Transcript {
            text: resp.text,
            language: Language::new("en"),
            segments: vec![], // Groq doesn't provide word-level timestamps
            overall_confidence: 0.95,
        })
    }

    fn display_name(&self) -> &str {
        "Cloud (Groq Whisper)"
    }
}
