pub mod avspeech;
#[cfg(feature = "kokoro")]
pub mod kokoro;
pub mod whisper_local;

pub use avspeech::AvSpeechTtsEngine;
#[cfg(feature = "kokoro")]
pub use kokoro::KokoroTtsEngine;
pub use whisper_local::WhisperLocalEngine;
