//! Klyntbot Providers — LLM provider abstraction and implementations.
//!
//! This crate defines the [`LlmProvider`] trait and concrete adapters for
//! various LLM APIs (Anthropic native, OpenAI-compatible, transcription).
//! The [`factory`] module handles provider creation from [`Config`](config::Config).

pub mod adapters;
mod factory;
pub mod manager;
pub mod registry;
pub(crate) mod streaming;
pub mod types;

// ── Adapters ──────────────────────────────────────────────
pub use adapters::{
    AnthropicNativeProvider, NoopProvider, OpenAiCompatProvider, TranscriptionProvider,
};

// ── Manager ───────────────────────────────────────────────
pub use manager::{CircuitBreakerConfig, ProviderManager};

// ── Registry ──────────────────────────────────────────────
pub use registry::{ProviderRegistry, ProviderSpec, PROVIDERS};

// ── Types ─────────────────────────────────────────────────
pub use types::{
    tool_calls_to_messages, ChatParams, ContentPart, DynProvider, FunctionCall, ImageUrl,
    LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message, ProviderCapabilities,
    ProviderHealth, ResponseFormat, ToolCall, ToolCallDelta, ToolCallMessage, Usage, UserContent,
    DEFAULT_CONTEXT_WINDOW,
};

// ── Factory ───────────────────────────────────────────────
pub use factory::{
    cognitive_chat_params, create_cognitive_provider, create_provider,
    create_provider_with_failover,
};
