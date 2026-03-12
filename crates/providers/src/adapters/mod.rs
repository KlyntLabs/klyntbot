//! LLM provider adapter implementations.
//!
//! Each adapter translates the [`LlmProvider`](crate::types::LlmProvider) trait
//! into a specific provider's HTTP API format.

pub mod anthropic_native;
pub mod openai_compat;
pub mod transcription;

mod noop;

pub use anthropic_native::AnthropicNativeProvider;
pub use noop::NoopProvider;
pub use openai_compat::OpenAiCompatProvider;
pub use transcription::TranscriptionProvider;
