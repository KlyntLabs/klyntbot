//! Text-to-speech engine trait.

use async_trait::async_trait;

use crate::types::{AudioClip, Language, TtsParams, VoiceInfo};

#[async_trait]
pub trait TtsEngine: Send + Sync {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip>;
    fn supports_language(&self, lang: &Language) -> bool;
    fn available_voices(&self, lang: &Language) -> Vec<VoiceInfo>;
    fn display_name(&self) -> &str;

    /// Unload the model from memory if it has been idle.
    ///
    /// Returns `true` if the model was unloaded. Default: no-op (always returns `false`).
    fn unload_if_idle(&self) -> bool {
        false
    }

    /// Eagerly load the model into memory. Default: no-op.
    async fn preload(&self) -> common::Result<()> {
        Ok(())
    }
}
