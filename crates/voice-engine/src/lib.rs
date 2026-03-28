//! Voice Engine — core audio capture, transcription, and synthesis for Klyntbot.
//!
//! This crate provides the `TranscriptionEngine` and `TtsEngine` traits,
//! concrete implementations (whisper-rs local, Groq cloud, AVSpeech),
//! the `AudioCapture` subsystem, and the `VoiceService` orchestrator.

pub mod mock;
pub mod pronunciation;
pub mod stt;
pub mod tts;
pub mod types;

pub use pronunciation::compute_pronunciation_report;
pub use stt::{PartialTranscript, TranscriptionEngine};
pub use tts::TtsEngine;
pub use types::*;
