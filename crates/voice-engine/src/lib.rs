//! Voice Engine — core audio capture, transcription, and synthesis for Klyntbot.
//!
//! This crate provides the `TranscriptionEngine` and `TtsEngine` traits,
//! concrete implementations (AVSpeech TTS, Qwen3 planned),
//! the `AudioCapture` subsystem, and the `VoiceService` orchestrator.

pub mod capture;
pub mod dsp;
pub mod engine_manager;
pub mod engines;
pub mod error_classifier;
pub mod events;
pub mod feedback_decider;
pub mod mock;
pub mod model_manager;
pub mod phoneme_aligner;
pub mod pronunciation;
pub mod pronunciation_analyzer;
pub mod router;
pub mod service;
pub mod session;
pub mod stt;
pub mod tone_analyzer;
pub mod tts;
pub mod types;
pub mod vad;

pub use capture::{AudioCapture, CaptureConfig, CaptureSession, MonitorSession};
pub use engine_manager::TtsEngineManager;
pub use engines::{AvSpeechTtsEngine, CloudAsrEngine, CloudTtsEngine};
#[cfg(feature = "qwen3")]
pub use engines::{Qwen3AsrEngine, Qwen3TtsEngine};
pub use events::{VoiceEvent, VOICE_EVENT};
pub use model_manager::ModelManager;
pub use pronunciation::compute_pronunciation_report;
pub use router::VoiceRouter;
pub use service::{MemoryEchoProvider, VoiceService, VoiceServiceConfig};
pub use session::VoiceSessionState;
pub use stt::{PartialTranscript, TranscriptionEngine};
pub use tts::TtsEngine;
pub use types::*;
