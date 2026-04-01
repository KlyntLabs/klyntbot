//! Cloud ASR engine via OpenAI-compatible audio API.
//!
//! Calls `POST /audio/transcriptions` with multipart form data.

use std::path::Path;

use async_trait::async_trait;
use tokio::sync::mpsc;
use tracing::warn;

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::types::*;

pub struct CloudAsrEngine {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl CloudAsrEngine {
    pub fn new(api_url: String, api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url,
            api_key,
            model: "qwen3-asr".to_string(),
        }
    }
}

#[async_trait]
impl TranscriptionEngine for CloudAsrEngine {
    async fn transcribe_stream(&self, mut audio: AudioStream) -> common::Result<TranscriptStream> {
        let (tx, rx) = mpsc::channel::<PartialTranscript>(32);

        let client = self.client.clone();
        let api_url = self.api_url.clone();
        let api_key = self.api_key.clone();
        let model = self.model.clone();

        tokio::spawn(async move {
            let mut all_samples: Vec<f32> = Vec::with_capacity(16_000 * 30);
            while let Some(chunk) = audio.recv().await {
                all_samples.extend_from_slice(&chunk.samples);
            }

            if all_samples.is_empty() {
                warn!("Empty audio stream for cloud ASR");
                return;
            }

            let wav_bytes = crate::dsp::encode_wav_bytes(&all_samples, 16000);
            let url = format!("{}/audio/transcriptions", api_url);

            let form = reqwest::multipart::Form::new()
                .part(
                    "file",
                    reqwest::multipart::Part::bytes(wav_bytes)
                        .file_name("audio.wav")
                        .mime_str("audio/wav")
                        .unwrap(),
                )
                .text("model", model)
                .text("response_format", "verbose_json")
                .text("timestamp_granularities[]", "word");

            match client
                .post(&url)
                .bearer_auth(&api_key)
                .multipart(form)
                .send()
                .await
            {
                Ok(response) if response.status().is_success() => {
                    if let Ok(text) = response.text().await {
                        if let Ok(result) = serde_json::from_str::<serde_json::Value>(&text) {
                            let transcript_text = result["text"].as_str().unwrap_or("").to_string();
                            let language = result["language"].as_str().unwrap_or("en").to_string();

                            let _ = tx
                                .send(PartialTranscript {
                                    text: transcript_text,
                                    segments: vec![],
                                    language: Language::new(language),
                                    is_final: true,
                                })
                                .await;
                        }
                    }
                }
                Ok(response) => {
                    warn!("Cloud ASR failed: HTTP {}", response.status());
                }
                Err(e) => {
                    warn!("Cloud ASR request failed: {e}");
                }
            }
        });

        Ok(rx)
    }

    async fn transcribe_file(
        &self,
        path: &Path,
        _lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        let file_bytes = tokio::fs::read(path)
            .await
            .map_err(common::KlyntbotError::Io)?;
        let url = format!("{}/audio/transcriptions", self.api_url);

        let form = reqwest::multipart::Form::new()
            .part(
                "file",
                reqwest::multipart::Part::bytes(file_bytes)
                    .file_name(
                        path.file_name()
                            .unwrap_or_default()
                            .to_string_lossy()
                            .to_string(),
                    )
                    .mime_str("audio/wav")
                    .unwrap(),
            )
            .text("model", self.model.clone())
            .text("response_format", "verbose_json");

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .multipart(form)
            .send()
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
            })?;

        let text = response.text().await.map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
        })?;

        let result: serde_json::Value = serde_json::from_str(&text)?;

        Ok(Transcript {
            text: result["text"].as_str().unwrap_or("").to_string(),
            language: Language::new(result["language"].as_str().unwrap_or("en")),
            segments: vec![],
            overall_confidence: 0.0,
        })
    }

    fn display_name(&self) -> &str {
        "Cloud ASR"
    }
}
