//! Text-to-speech engine trait.

use async_trait::async_trait;

use crate::types::{AudioClip, Language, TtsParams, VoiceInfo};

#[async_trait]
pub trait TtsEngine: Send + Sync {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip>;
    fn supports_language(&self, lang: &Language) -> bool;
    fn available_voices(&self, lang: &Language) -> Vec<VoiceInfo>;
    fn display_name(&self) -> &str;
}
