pub mod avspeech;
pub mod cloud_asr;
pub mod cloud_tts;
pub mod qwen3_asr;
#[cfg(target_os = "macos")]
pub mod qwen3_tts;

/// Default voice identifiers for Qwen3/cloud TTS engines.
pub(crate) const QWEN3_VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];

pub use avspeech::AvSpeechTtsEngine;
pub use cloud_asr::CloudAsrEngine;
pub use cloud_tts::CloudTtsEngine;
pub use qwen3_asr::Qwen3AsrEngine;
#[cfg(target_os = "macos")]
pub use qwen3_tts::Qwen3TtsEngine;
