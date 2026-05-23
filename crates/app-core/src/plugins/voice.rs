use async_trait::async_trait;
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::plugin::context::PluginContext;
use crate::plugin::AppCorePlugin;
use crate::state::AppCore;

/// Result of voice plugin initialization.
pub struct VoiceInitResult {
    pub voice_service: Option<Arc<voice_engine::VoiceService>>,
    pub voice_conversation_manager:
        Option<Arc<crate::handlers::voice_conversation::VoiceConversationManager>>,
    pub voice_loop_handle: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    pub echo_provider: Option<Arc<dyn voice_engine::MemoryEchoProvider>>,
}

/// Plugin that initializes the voice service, conversation manager,
/// STT/TTS engines, and background model download / hot-swap.
pub struct VoicePlugin;

#[async_trait]
impl AppCorePlugin for VoicePlugin {
    fn name(&self) -> &str {
        "voice"
    }

    async fn init(&self, ctx: &mut PluginContext) -> common::Result<()> {
        let config_guard = ctx.deps.config.read().await;
        let voice_config = config_guard.voice.clone();
        if !voice_config.enabled {
            ctx.insert_handle(Arc::new(VoiceInitResult {
                voice_service: None,
                voice_conversation_manager: None,
                voice_loop_handle: std::sync::Mutex::new(None),
                echo_provider: None,
            }));
            return Ok(());
        }

        let data_dir = config_guard.data_dir_path();
        let model_manager = voice_engine::ModelManager::new(&data_dir);
        let has_custom_personas = voice_config
            .output
            .personas
            .values()
            .any(|p| matches!(p, config::schema::VoicePersona::Custom { .. }));

        // STT: config-driven engine selection with deployment mode
        let stt_local: Option<Arc<dyn voice_engine::TranscriptionEngine>> = {
            match voice_config.input.deployment {
                config::schema::EngineDeployment::Local => {
                    match voice_engine::Qwen3AsrEngine::new(
                        model_manager.models_dir(),
                        voice_config.input.allowed_languages.clone(),
                    ) {
                        Ok(engine) => {
                            info!("Qwen3-ASR engine ready (lazy-load on first use)");
                            Some(Arc::new(engine) as Arc<dyn voice_engine::TranscriptionEngine>)
                        }
                        Err(e) => {
                            warn!("Failed to create Qwen3-ASR: {e}");
                            None
                        }
                    }
                }
                config::schema::EngineDeployment::Cloud {
                    ref api_url,
                    ref api_key,
                } => Some(Arc::new(voice_engine::CloudAsrEngine::new(
                    api_url.clone(),
                    api_key.expose().to_string(),
                )) as Arc<dyn voice_engine::TranscriptionEngine>),
            }
        };

        drop(config_guard);

        // TTS: config-driven with engine manager wrapping primary + system fallback.
        let mut qwen3_tts_ref: Option<Arc<voice_engine::Qwen3TtsEngine>> = None;
        let system_tts = Arc::new(voice_engine::AvSpeechTtsEngine::new(&data_dir));
        let tts: Option<Arc<dyn voice_engine::TtsEngine>> = {
            match voice_config.output.deployment {
                config::schema::EngineDeployment::Cloud {
                    ref api_url,
                    ref api_key,
                } => {
                    let cloud = Arc::new(voice_engine::CloudTtsEngine::new(
                        api_url.clone(),
                        api_key.expose().to_string(),
                    ));
                    let manager = voice_engine::TtsEngineManager::new(
                        cloud,
                        Some(Arc::clone(&system_tts) as Arc<dyn voice_engine::TtsEngine>),
                    );
                    Some(Arc::new(manager) as Arc<dyn voice_engine::TtsEngine>)
                }
                config::schema::EngineDeployment::Local => {
                    let model_dir = match voice_config.output.tts_engine {
                        config::schema::TtsEngineKind::Qwen3 => model_manager
                            .qwen3_tts_instruct_model_dir()
                            .or_else(|| model_manager.qwen3_tts_model_dir()),
                        config::schema::TtsEngineKind::Qwen3Base => {
                            model_manager.qwen3_tts_model_dir()
                        }
                        config::schema::TtsEngineKind::System => None,
                    };
                    if let Some(dir) = model_dir {
                        match voice_engine::Qwen3TtsEngine::new(&dir).await {
                            Ok(engine) => {
                                info!("Qwen3-TTS loaded — wrapping with system fallback");
                                let engine = Arc::new(engine);
                                qwen3_tts_ref = Some(Arc::clone(&engine));
                                let manager = voice_engine::TtsEngineManager::new(
                                    engine,
                                    Some(Arc::clone(&system_tts)
                                        as Arc<dyn voice_engine::TtsEngine>),
                                );
                                Some(Arc::new(manager) as Arc<dyn voice_engine::TtsEngine>)
                            }
                            Err(e) => {
                                warn!("Qwen3-TTS failed, using system TTS: {e}");
                                Some(Arc::clone(&system_tts)
                                    as Arc<dyn voice_engine::TtsEngine>)
                            }
                        }
                    } else {
                        Some(Arc::clone(&system_tts) as Arc<dyn voice_engine::TtsEngine>)
                    }
                }
            }
        };

        // Always create VoiceService — even without a local engine yet.
        let svc_config = voice_engine::VoiceServiceConfig {
            capture: voice_engine::CaptureConfig {
                silence_threshold: 0.01,
                silence_duration: std::time::Duration::from_secs_f32(
                    voice_config.input.silence_threshold_secs,
                ),
                selected_device: voice_config.input.selected_device.clone(),
                noise_reduction: voice_config.input.noise_reduction,
                ..Default::default()
            },
            privacy_mode: crate::handlers::voice::privacy_level(
                voice_config.input.privacy_mode,
            ),
            data_dir: data_dir.clone(),
            native_audio: true,
            output_device: voice_config.output.selected_device.clone(),
        };

        let has_local_engine = stt_local.is_some();

        // Tier 3 memory recall: construct a MemoryRetriever from
        // cognitive UnifiedMemoryService (facts + conversation recall).
        let voice_memory_retriever: Option<Arc<dyn context_engine::MemoryRetriever>> = {
            let fact_repo =
                ::cognitive::SemanticFactRepo::new(ctx.deps.storage_pool.inner().clone());
            let retriever = ::cognitive::UnifiedMemoryService::new(fact_repo)
                .with_co_activation_repo(::cognitive::CoActivationRepo::new(
                    ctx.deps.storage_pool.inner().clone(),
                ))
                .with_embedder_opt(
                    ctx.host
                        .get::<crate::plugins::cognitive::CognitiveInitResult>()
                        .and_then(|r| r.cognitive_fact_embedder.clone()),
                );
            Some(Arc::new(retriever) as Arc<dyn context_engine::MemoryRetriever>)
        };

        let mirror_facade = ctx
            .host
            .get::<crate::plugins::mirror::MirrorInitResult>()
            .map(|r| Arc::clone(&r.facade));

        let echo_provider: Arc<dyn voice_engine::MemoryEchoProvider> =
            Arc::new(crate::handlers::voice_echo::AppMemoryEchoProvider::new(
                mirror_facade,
                voice_memory_retriever,
            ));

        let pronunciation_provider: Option<Arc<dyn voice_engine::PronunciationProvider>> = {
            let ll_config = ctx.deps.config.read().await;
            if ll_config.language_learning.enabled {
                info!("Language learning enabled — wiring pronunciation provider");
                Some(
                    Arc::new(feature_language_learning::AppPronunciationProvider::new())
                        as Arc<dyn voice_engine::PronunciationProvider>,
                )
            } else {
                None
            }
        };

        let service = voice_engine::VoiceService::new(
            stt_local,
            tts,
            Some(Arc::clone(&echo_provider)),
            pronunciation_provider,
            model_manager,
            svc_config,
        );

        let service = Arc::new(service);

        // Idle unload timers
        {
            let svc = Arc::clone(&service);
            crate::init::spawn_periodic_timer(&ctx.deps.shutdown_token, 300, move || {
                svc.try_unload_idle_stt()
            });
        }
        {
            let svc = Arc::clone(&service);
            crate::init::spawn_periodic_timer(&ctx.deps.shutdown_token, 300, move || {
                svc.try_unload_idle_tts()
            });
        }

        // Auto-download Qwen3 models in background on first launch.
        if !has_local_engine {
            if matches!(
                voice_config.input.deployment,
                config::schema::EngineDeployment::Local
            ) {
                let mm_dir = data_dir.clone();
                let svc_for_swap = Arc::clone(&service);
                let system_tts_for_swap =
                    Arc::clone(&system_tts) as Arc<dyn voice_engine::TtsEngine>;
                let allowed_langs = voice_config.input.allowed_languages.clone();
                let has_custom = has_custom_personas;
                ctx.spawn_background(async move {
                    let mm = voice_engine::ModelManager::new(&mm_dir);
                    use voice_engine::model_manager::Qwen3Model;
                    let svc_for_asr = Arc::clone(&svc_for_swap);
                    let svc_for_tts = Arc::clone(&svc_for_swap);
                    let (asr, tts) = tokio::join!(
                        mm.download_model_with_progress(
                            Qwen3Model::Asr,
                            move |downloaded, total| {
                                svc_for_asr.try_emit_event(
                                    voice_engine::VoiceEvent::DownloadProgress {
                                        model: "Qwen3-ASR".into(),
                                        downloaded,
                                        total,
                                    },
                                );
                            }
                        ),
                        mm.download_model_with_progress(
                            Qwen3Model::Tts,
                            move |downloaded, total| {
                                svc_for_tts.try_emit_event(
                                    voice_engine::VoiceEvent::DownloadProgress {
                                        model: "Qwen3-TTS".into(),
                                        downloaded,
                                        total,
                                    },
                                );
                            }
                        ),
                    );
                    if let Err(e) = &asr {
                        warn!("Qwen3-ASR download: {e}");
                    }
                    if let Err(e) = &tts {
                        warn!("Qwen3-TTS download: {e}");
                    }

                    // Hot-swap STT engine after successful download
                    if asr.is_ok() {
                        if let Ok(engine) = voice_engine::Qwen3AsrEngine::new(
                            mm.models_dir(),
                            allowed_langs,
                        ) {
                            svc_for_swap.set_local_engine(Arc::new(engine)
                                as Arc<dyn voice_engine::TranscriptionEngine>);
                            info!("Hot-swapped Qwen3-ASR into live VoiceService");
                        }
                    }

                    // Hot-swap TTS engine after successful download
                    if let Ok(ref dir) = tts {
                        match voice_engine::Qwen3TtsEngine::new(dir).await {
                            Ok(engine) => {
                                let manager = voice_engine::TtsEngineManager::new(
                                    Arc::new(engine),
                                    Some(system_tts_for_swap),
                                );
                                svc_for_swap
                                    .set_tts_engine(Arc::new(manager)
                                        as Arc<dyn voice_engine::TtsEngine>);
                                info!("Hot-swapped Qwen3-TTS into live VoiceService");
                            }
                            Err(e) => warn!(
                                "Qwen3-TTS engine creation after download failed: {e}"
                            ),
                        }
                    }

                    // Download 1.7B instruct model when custom personas are configured
                    if has_custom {
                        if let Err(e) = mm.download_model(Qwen3Model::TtsInstruct).await {
                            warn!("Qwen3-TTS 1.7B instruct download: {e}");
                        }
                    }
                });
            }
            info!("Voice service initialized (no local STT — Qwen3 download pending)");
        } else {
            info!("Voice service initialized (local engine ready)");
        }

        ctx.insert_handle(Arc::new(VoiceInitResult {
            voice_service: Some(service),
            voice_conversation_manager: None,
            voice_loop_handle: std::sync::Mutex::new(None),
            echo_provider: Some(echo_provider),
        }));

        Ok(())
    }

    async fn post_init(&self, app: &AppCore) -> common::Result<()> {
        let config_guard = app.config.read().await;
        let voice_config = config_guard.voice.clone();
        drop(config_guard);
        if !voice_config.enabled {
            return Ok(());
        }

        let voice_result = app
            .host
            .get::<VoiceInitResult>()
            .ok_or_else(|| common::KlyntbotError::Config(common::ConfigError::Invalid(
                "VoiceInitResult not found in host".to_string(),
            )))?;
        let service = match voice_result.voice_service {
            Some(ref svc) => Arc::clone(svc),
            None => return Ok(()),
        };

        let voice_config_arc = Arc::new(RwLock::new(voice_config));
        let echo_provider = voice_result
            .echo_provider
            .clone()
            .ok_or_else(|| common::KlyntbotError::Config(common::ConfigError::Invalid(
                "Voice echo provider not found in host".to_string(),
            )))?;
        let voice_conv_manager = Arc::new(
            crate::handlers::voice_conversation::VoiceConversationManager::new(
                service,
                app.repos.clone(),
                Arc::clone(&app.agent),
                Arc::clone(&app.event_emitter),
                echo_provider,
                voice_config_arc,
            ),
        );
        let loop_handle = voice_conv_manager.spawn_supervised_loop().await;

        // Re-insert updated VoiceInitResult with conversation manager.
        app.host.insert(Arc::new(VoiceInitResult {
            voice_service: voice_result.voice_service.clone(),
            voice_conversation_manager: Some(voice_conv_manager),
            voice_loop_handle: std::sync::Mutex::new(Some(loop_handle)),
            echo_provider: voice_result.echo_provider.clone(),
        }));

        Ok(())
    }
}
