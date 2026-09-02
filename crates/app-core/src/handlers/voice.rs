//! Voice dictation and settings handlers.

use desktop_shared::commands::voice::{
    AudioDevicesResponse, ModelStatus, VoiceModelStatusResponse,
};
use desktop_shared::errors::ApiError;
use voice_engine::capture::{AudioCapture, CaptureConfig};
use voice_engine::model_manager::Qwen3Model;
use voice_engine::VoiceServiceConfig;

use crate::state::AppCore;

pub fn privacy_level(mode: config::schema::VoicePrivacyMode) -> voice_engine::PrivacyLevel {
    match mode {
        config::schema::VoicePrivacyMode::Standard => voice_engine::PrivacyLevel::Standard,
        config::schema::VoicePrivacyMode::Strict => voice_engine::PrivacyLevel::Strict,
        config::schema::VoicePrivacyMode::Off => voice_engine::PrivacyLevel::Off,
    }
}

fn parse_model(name: &str) -> Result<Qwen3Model, ApiError> {
    Qwen3Model::from_api_key(name)
        .ok_or_else(|| ApiError::new("INVALID_PARAMS", format!("Unknown model: {name}")))
}

/// Convert a voice persona config into TTS synthesis parameters.
pub(crate) fn tts_params_for_persona(
    persona: &config::schema::VoicePersona,
) -> voice_engine::TtsParams {
    match persona {
        config::schema::VoicePersona::Preset {
            speaker,
            speed,
            temperature,
        } => voice_engine::TtsParams {
            voice_name: Some(speaker.clone()),
            speaking_rate: *speed,
            temperature: Some(*temperature),
            instruct: None,
            ..Default::default()
        },
        config::schema::VoicePersona::Custom {
            description,
            speed,
            temperature,
        } => voice_engine::TtsParams {
            voice_name: None,
            speaking_rate: *speed,
            temperature: Some(*temperature),
            instruct: Some(description.clone()),
            ..Default::default()
        },
    }
}

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn voice_start_dictation(&self) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        service
            .start_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", e.to_string()))?;
        Ok(())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn voice_stop_dictation(&self) -> Result<String, ApiError> {
        let service = self.voice_service()?;
        let result = service
            .stop_capture()
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", e.to_string()))?;
        match result {
            Some((transcript, _metadata)) => Ok(transcript.text),
            None => Ok(String::new()),
        }
    }

    /// Cancel an in-progress dictation capture without transcribing it.
    #[tracing::instrument(skip(self), err)]
    pub async fn voice_cancel_dictation(&self) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        service.cancel().await;
        Ok(())
    }

    #[tracing::instrument(skip(self, event_json), err)]
    pub async fn voice_simulate_event(
        &self,
        event_json: serde_json::Value,
    ) -> Result<(), ApiError> {
        let service = self.voice_service()?;
        let event: voice_engine::VoiceEvent = serde_json::from_value(event_json)
            .map_err(|e| ApiError::new("VALIDATION", format!("Invalid VoiceEvent: {e}")))?;
        service
            .emit_event(event)
            .await
            .map_err(|e| ApiError::new("VOICE_ERROR", e.to_string()))
    }

    /// Preview a voice persona by synthesizing a short sample.
    #[tracing::instrument(skip(self), err)]
    pub async fn voice_test_persona(&self, persona_key: String) -> Result<(), ApiError> {
        let svc = self.voice_service()?.clone();
        let cfg = self.config.read().await;
        let persona = cfg
            .voice
            .output
            .personas
            .get(&persona_key)
            .cloned()
            .ok_or_else(|| {
                ApiError::new("NOT_FOUND", format!("Persona '{persona_key}' not found"))
            })?;
        drop(cfg);

        // Stop any in-progress preview before starting a new one
        svc.stop_playback();

        let tts_params = tts_params_for_persona(&persona);
        tracing::info!(
            "Testing persona '{}': voice={:?}, instruct={:?}",
            persona_key,
            tts_params.voice_name,
            tts_params.instruct,
        );
        let sample = "Hi there! This is how I sound. Hope you like it!";
        tokio::spawn(async move {
            if let Err(e) = svc.handle_response(sample, &tts_params).await {
                tracing::warn!("Voice persona test failed: {e}");
            }
        });
        Ok(())
    }

    // ── Voice settings ─────────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
    pub fn voice_list_devices(&self) -> Result<AudioDevicesResponse, ApiError> {
        let (input, output, default_input, default_output) = AudioCapture::enumerate_devices();
        Ok(AudioDevicesResponse {
            input,
            output,
            default_input,
            default_output,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub fn voice_model_status(&self) -> Result<VoiceModelStatusResponse, ApiError> {
        let svc = self.voice_service()?;
        let mm = svc.model_manager();
        Ok(VoiceModelStatusResponse {
            asr: ModelStatus {
                downloaded: mm.qwen3_asr_model_dir().is_some(),
                display_name: Qwen3Model::Asr.display_name().to_string(),
                dir_name: Qwen3Model::Asr.dir_name().to_string(),
            },
            tts: ModelStatus {
                downloaded: mm.qwen3_tts_model_dir().is_some(),
                display_name: Qwen3Model::Tts.display_name().to_string(),
                dir_name: Qwen3Model::Tts.dir_name().to_string(),
            },
            tts_instruct: ModelStatus {
                downloaded: mm.qwen3_tts_instruct_model_dir().is_some(),
                display_name: Qwen3Model::TtsInstruct.display_name().to_string(),
                dir_name: Qwen3Model::TtsInstruct.dir_name().to_string(),
            },
            stt_loaded: svc.is_stt_loaded(),
            tts_loaded: svc.is_tts_loaded(),
        })
    }

    /// Download a voice model and hot-swap the engine after completion.
    #[tracing::instrument(skip(self), err)]
    pub async fn voice_download_model(&self, model_name: String) -> Result<(), ApiError> {
        let model = parse_model(&model_name)?;
        let svc = self.voice_service()?.clone();
        let emitter = self.event_emitter.clone();
        let display = model.display_name().to_string();
        let config = self.config.clone();
        tokio::spawn(async move {
            let result = svc
                .model_manager()
                .download_model_with_progress(model, move |downloaded, total| {
                    let progress = if total > 0 {
                        downloaded as f64 / total as f64
                    } else {
                        0.0
                    };
                    emitter.emit_event(
                        "voice:model_progress",
                        serde_json::json!({
                            "model": display,
                            "progress": progress,
                            "downloadedBytes": downloaded,
                            "totalBytes": total,
                        }),
                    );
                })
                .await;
            match result {
                Ok(dir) => {
                    tracing::info!("{} downloaded successfully", model.display_name());
                    Self::hot_swap_after_download(&svc, &config, model, &dir).await;
                }
                Err(e) => tracing::warn!("Model download failed: {e}"),
            }
        });
        Ok(())
    }

    async fn hot_swap_after_download(
        svc: &voice_engine::VoiceService,
        config: &tokio::sync::RwLock<config::Config>,
        model: Qwen3Model,
        model_dir: &std::path::Path,
    ) {
        match model {
            Qwen3Model::Asr => {
                let cfg = config.read().await;
                let langs = cfg.voice.input.allowed_languages.clone();
                drop(cfg);
                // ASR uses from_pretrained internally — pass parent models_dir
                match voice_engine::Qwen3AsrEngine::new(svc.model_manager().models_dir(), langs) {
                    Ok(engine) => {
                        svc.set_local_engine(std::sync::Arc::new(engine)
                            as std::sync::Arc<dyn voice_engine::TranscriptionEngine>);
                        tracing::info!("Hot-swapped Qwen3-ASR into live VoiceService");
                    }
                    Err(e) => tracing::warn!("Failed to create ASR engine after download: {e}"),
                }
            }
            Qwen3Model::Tts | Qwen3Model::TtsInstruct => {
                let data_dir = svc.model_manager().data_dir();
                let system_fallback =
                    std::sync::Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir))
                        as std::sync::Arc<dyn voice_engine::TtsEngine>;
                #[cfg(target_os = "macos")]
                {
                    match voice_engine::Qwen3TtsEngine::new(model_dir).await {
                        Ok(engine) => {
                            let manager = voice_engine::TtsEngineManager::new(
                                std::sync::Arc::new(engine),
                                Some(system_fallback),
                            );
                            svc.set_tts_engine(std::sync::Arc::new(manager)
                                as std::sync::Arc<dyn voice_engine::TtsEngine>);
                            tracing::info!(
                                "Hot-swapped {} into live VoiceService",
                                model.display_name()
                            );
                        }
                        Err(e) => {
                            tracing::warn!("Failed to create TTS engine after download: {e}");
                            svc.set_tts_engine(system_fallback);
                        }
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = model_dir;
                    tracing::warn!("Qwen3-TTS is not available on this platform");
                    svc.set_tts_engine(system_fallback);
                }
            }
        }
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn voice_delete_model(&self, model_name: String) -> Result<(), ApiError> {
        let model = parse_model(&model_name)?;
        let svc = self.voice_service()?;
        let dir = svc.model_manager().models_dir().join(model.dir_name());
        if tokio::fs::try_exists(&dir).await.unwrap_or(false) {
            tokio::fs::remove_dir_all(&dir)
                .await
                .map_err(|e| ApiError::new("IO_ERROR", e.to_string()))?;
        }
        Ok(())
    }

    /// Push voice config changes to the live VoiceService and VoiceConversationManager.
    pub(crate) fn propagate_voice_config(&self, voice: &config::schema::VoiceConfig) {
        if let Ok(svc) = self.voice_service() {
            let new_config = VoiceServiceConfig {
                capture: CaptureConfig {
                    silence_threshold: 0.01,
                    silence_duration: std::time::Duration::from_secs_f32(
                        voice.input.silence_threshold_secs,
                    ),
                    selected_device: voice.input.selected_device.clone(),
                    noise_reduction: voice.input.noise_reduction,
                    ..Default::default()
                },
                privacy_mode: privacy_level(voice.input.privacy_mode),
                data_dir: svc.model_manager().data_dir(),
                native_audio: true,
                output_device: voice.output.selected_device.clone(),
            };
            svc.update_config(new_config);
        }
        if let Ok(mgr) = self.voice_conversation_manager() {
            if let Ok(mut cfg) = mgr.config.try_write() {
                *cfg = voice.clone();
            }
        }
        tracing::info!("Voice config hot-reloaded");
    }
}
