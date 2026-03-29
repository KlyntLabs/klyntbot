//! Voice Engine — core audio capture, transcription, and synthesis for Klyntbot.
//!
//! This crate provides the `TranscriptionEngine` and `TtsEngine` traits,
//! concrete implementations (whisper-rs local, Groq cloud, AVSpeech),
//! the `AudioCapture` subsystem, and the `VoiceService` orchestrator.

pub mod capture;
pub mod engines;
pub mod events;
pub mod mock;
pub mod model_manager;
pub mod pronunciation;
pub mod router;
pub mod session;
pub mod stt;
pub mod tts;
pub mod types;

pub use capture::{AudioCapture, CaptureConfig, CaptureSession};
pub use engines::GroqWhisperEngine;
pub use events::{VoiceEvent, VOICE_EVENT};
pub use model_manager::{ModelManager, ModelState, WhisperModelSize};
pub use pronunciation::compute_pronunciation_report;
pub use router::VoiceRouter;
pub use session::VoiceSessionState;
pub use stt::{PartialTranscript, TranscriptionEngine};
pub use tts::TtsEngine;
pub use types::*;
