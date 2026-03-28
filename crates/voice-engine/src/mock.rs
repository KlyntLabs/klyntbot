//! Mock implementations for testing without real audio hardware or models.

use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::stt::{AudioStream, PartialTranscript, TranscriptStream, TranscriptionEngine};
use crate::tts::TtsEngine;
use crate::types::{AudioClip, Language, Transcript, TranscriptSegment, TtsParams, VoiceInfo};

pub struct MockTranscriptionEngine {
    pub transcript: Transcript,
    pub partials: Vec<PartialTranscript>,
}

impl MockTranscriptionEngine {
    pub fn new(text: &str) -> Self {
        let segments = text
            .split_whitespace()
            .enumerate()
            .map(|(i, word)| TranscriptSegment {
                text: word.to_string(),
                start: Duration::from_millis(i as u64 * 300),
                end: Duration::from_millis((i as u64 + 1) * 300),
                confidence: 0.90,
            })
            .collect::<Vec<_>>();

        Self {
            transcript: Transcript {
                text: text.to_string(),
                language: Language::new("en"),
                overall_confidence: 0.90,
                segments,
            },
            partials: vec![],
        }
    }

    pub fn with_partials(mut self, partials: Vec<PartialTranscript>) -> Self {
        self.partials = partials;
        self
    }
}

#[async_trait]
impl TranscriptionEngine for MockTranscriptionEngine {
    async fn transcribe_stream(&self, mut audio: AudioStream) -> common::Result<TranscriptStream> {
        let (tx, rx) = mpsc::channel(32);
        let partials = self.partials.clone();
        let transcript = self.transcript.clone();

        tokio::spawn(async move {
            while audio.recv().await.is_some() {}
            for partial in partials {
                let _ = tx.send(partial).await;
            }
            let _ = tx
                .send(PartialTranscript {
                    text: transcript.text.clone(),
                    segments: transcript.segments.clone(),
                    language: transcript.language.clone(),
                    is_final: true,
                })
                .await;
        });

        Ok(rx)
    }

    async fn transcribe_file(
        &self,
        _path: &Path,
        _lang_hint: Option<&Language>,
    ) -> common::Result<Transcript> {
        Ok(self.transcript.clone())
    }

    fn display_name(&self) -> &str {
        "Mock"
    }
}

pub struct MockTtsEngine;

#[async_trait]
impl TtsEngine for MockTtsEngine {
    async fn synthesize(&self, _text: &str, _params: &TtsParams) -> common::Result<AudioClip> {
        Ok(AudioClip {
            samples: vec![0.0; 16000],
            sample_rate: 16000,
            channels: 1,
        })
    }

    fn supports_language(&self, _lang: &Language) -> bool {
        true
    }

    fn available_voices(&self, lang: &Language) -> Vec<VoiceInfo> {
        vec![VoiceInfo {
            identifier: "mock-voice".to_string(),
            display_name: "Mock Voice".to_string(),
            language: lang.clone(),
        }]
    }

    fn display_name(&self) -> &str {
        "Mock"
    }
}
