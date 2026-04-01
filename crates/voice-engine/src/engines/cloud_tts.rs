//! Cloud TTS engine via OpenAI-compatible audio API.
//!
//! Calls `POST /audio/speech` with `{ model, input, voice }`.
//! DashScope (Alibaba Cloud) is the reference provider.

use async_trait::async_trait;
use tracing::debug;

use crate::tts::TtsEngine;
use crate::types::*;

pub struct CloudTtsEngine {
    client: reqwest::Client,
    api_url: String,
    api_key: String,
    model: String,
}

impl CloudTtsEngine {
    pub fn new(api_url: String, api_key: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_url,
            api_key,
            model: "qwen3-tts".to_string(),
        }
    }
}

#[async_trait]
impl TtsEngine for CloudTtsEngine {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        let voice = params.voice_name.as_deref().unwrap_or("alloy");
        let url = format!("{}/audio/speech", self.api_url);

        let response = self
            .client
            .post(&url)
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "model": self.model,
                "input": text,
                "voice": voice,
                "speed": params.speaking_rate,
                "response_format": "pcm",
            }))
            .send()
            .await
            .map_err(|e| {
                common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(common::KlyntbotError::Provider(
                common::ProviderError::InvalidResponse(format!("Cloud TTS HTTP {status}: {body}")),
            ));
        }

        let bytes = response.bytes().await.map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string()))
        })?;

        let samples = crate::dsp::decode_pcm_16bit(&bytes);

        debug!("Cloud TTS returned {} samples", samples.len());

        Ok(AudioClip {
            samples,
            sample_rate: 24_000,
            channels: 1,
        })
    }

    fn supports_language(&self, _lang: &Language) -> bool {
        true
    }

    fn available_voices(&self, _lang: &Language) -> Vec<VoiceInfo> {
        super::QWEN3_VOICES
            .iter()
            .map(|v| VoiceInfo {
                identifier: v.to_string(),
                display_name: v.to_string(),
                language: Language::new("en"),
            })
            .collect()
    }

    fn display_name(&self) -> &str {
        "Cloud TTS"
    }
}
