//! Speech-to-text transcription engine trait.

use std::path::Path;

use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::types::{AudioChunk, Language, Transcript, TranscriptSegment};

/// Streaming partial transcript result.
#[derive(Debug, Clone)]
pub struct PartialTranscript {
    pub text: String,
    pub segments: Vec<TranscriptSegment>,
    pub language: Language,
    pub is_final: bool,
}

pub type AudioStream = mpsc::Receiver<AudioChunk>;
pub type TranscriptStream = mpsc::Receiver<PartialTranscript>;

#[async_trait]
pub trait TranscriptionEngine: Send + Sync {
    async fn transcribe_stream(&self, audio: AudioStream) -> common::Result<TranscriptStream>;
    async fn transcribe_file(
        &self,
        path: &Path,
        lang_hint: Option<&Language>,
    ) -> common::Result<Transcript>;
    fn display_name(&self) -> &str;
}
