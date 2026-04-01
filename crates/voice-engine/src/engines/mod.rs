pub mod avspeech;
pub mod cloud_asr;
pub mod cloud_tts;
#[cfg(feature = "qwen3")]
pub mod qwen3_asr;
#[cfg(feature = "qwen3")]
pub mod qwen3_tts;

/// Default voice identifiers for Qwen3/cloud TTS engines.
pub(crate) const QWEN3_VOICES: &[&str] = &["alloy", "echo", "fable", "onyx", "nova", "shimmer"];

pub use avspeech::AvSpeechTtsEngine;
pub use cloud_asr::CloudAsrEngine;
pub use cloud_tts::CloudTtsEngine;
#[cfg(feature = "qwen3")]
pub use qwen3_asr::Qwen3AsrEngine;
#[cfg(feature = "qwen3")]
pub use qwen3_tts::Qwen3TtsEngine;
