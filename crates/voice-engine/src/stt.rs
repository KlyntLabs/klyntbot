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

    /// Unload the model from memory if it has been idle.
    ///
    /// Returns `true` if the model was unloaded. Default: no-op (always returns `false`).
    fn unload_if_idle(&self) -> bool {
        false
    }

    /// Eagerly load the model so it's ready for the first transcription.
    /// Default: no-op. Engines with lazy loading should override this.
    async fn preload(&self) -> common::Result<()> {
        Ok(())
    }
}
