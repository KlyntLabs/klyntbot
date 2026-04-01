//! macOS AVSpeechSynthesizer TTS engine wrapping platform-macos speech module.

use std::path::PathBuf;

use async_trait::async_trait;

use crate::tts::TtsEngine;
use crate::types::*;

pub struct AvSpeechTtsEngine {
    data_dir: PathBuf,
}

impl AvSpeechTtsEngine {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        let data_dir = data_dir.into();
        // Ensure data_dir exists at construction time (sync is fine during init).
        let _ = std::fs::create_dir_all(&data_dir);
        Self { data_dir }
    }
}

#[async_trait]
impl TtsEngine for AvSpeechTtsEngine {
    async fn synthesize(&self, text: &str, params: &TtsParams) -> common::Result<AudioClip> {
        let output_path = self
            .data_dir
            .join(format!("tts_{}.wav", chrono::Utc::now().timestamp_millis()));

        let voice = params.voice_name.clone();
        let rate = if (params.speaking_rate - 1.0).abs() < 0.01 {
            None
        } else {
            Some(params.speaking_rate)
        };

        // Run say CLI in blocking task (it's a subprocess)
        let text = text.to_string();
        let path = output_path.clone();
        tokio::task::spawn_blocking(move || {
            platform_macos::speech::synthesize_to_file(&text, voice.as_deref(), rate, &path)
        })
        .await
        .map_err(|e| common::KlyntbotError::Provider(common::ProviderError::Http(e.to_string())))?
        .map_err(|e| common::KlyntbotError::Provider(common::ProviderError::Http(e)))?;

        // Read the WAV output
        let reader = hound::WavReader::open(&output_path).map_err(|e| {
            common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(format!(
                "Failed to read TTS output: {e}"
            )))
        })?;

        let spec = reader.spec();
        let samples: Vec<f32> = match spec.sample_format {
            hound::SampleFormat::Float => reader
                .into_samples::<f32>()
                .filter_map(|s| s.ok())
                .collect(),
            hound::SampleFormat::Int => {
                let max = (1 << (spec.bits_per_sample - 1)) as f32;
                reader
                    .into_samples::<i32>()
                    .filter_map(|s| s.ok())
                    .map(|s| s as f32 / max)
                    .collect()
            }
        };

        // Clean up the temp WAV file now that we've read it into memory.
        let _ = std::fs::remove_file(&output_path);

        Ok(AudioClip {
            samples,
            sample_rate: spec.sample_rate,
            channels: spec.channels,
        })
    }

    fn supports_language(&self, _lang: &Language) -> bool {
        true // macOS say supports many languages
    }

    fn available_voices(&self, _lang: &Language) -> Vec<VoiceInfo> {
        platform_macos::speech::list_voices()
            .into_iter()
            .map(|v| VoiceInfo {
                identifier: v.name.clone(),
                display_name: v.name,
                language: Language::new(v.language),
            })
            .collect()
    }

    fn display_name(&self) -> &str {
        "macOS (AVSpeech)"
    }
}
