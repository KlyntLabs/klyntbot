//! Voice Engine — core audio capture, transcription, and synthesis for Klyntbot.
//!
//! This crate provides the `TranscriptionEngine` and `TtsEngine` traits,
//! concrete implementations (whisper-rs local, Groq cloud, AVSpeech),
//! the `AudioCapture` subsystem, and the `VoiceService` orchestrator.

pub mod pronunciation;
pub mod types;

pub use types::*;
