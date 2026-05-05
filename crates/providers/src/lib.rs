//! Klyntbot Providers — LLM provider abstraction and implementations.
//!
//! This crate defines the [`LlmProvider`] trait and concrete adapters for
//! various LLM APIs (Anthropic native, OpenAI-compatible, transcription).
//! The [`factory`] module handles provider creation from [`Config`](config::Config).

/// Identifies the role a provider invocation serves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderRole {
    /// Per-turn Distiller (Phase 3+).
    Distiller,
    /// Reforge Phase 2.5 — Coding Synthesis.
    ReforgeSynth,
    /// Reforge Phase 3.5 — Rule Artifact Generation.
    ReforgeRules,
}

pub mod adapters;
mod factory;
pub mod manager;
pub mod registry;
pub(crate) mod streaming;
pub mod types;

// -- Adapters --
pub use adapters::{
    AnthropicNativeProvider, NoopProvider, OpenAiCompatProvider, TranscriptionProvider,
};

// -- Manager --
pub use manager::{
    CircuitBreakerConfig, DegradationLevel, OnCircuitOpen, OnProviderDegraded, ProviderManager,
};

// -- Registry --
pub use registry::{ProviderRegistry, ProviderSpec, PROVIDERS};

// -- Types --
pub use types::{
    tool_calls_to_messages, CacheAnchor, CacheBreakpoint, CacheTtl, ChatParams, ContentPart,
    DynProvider, FunctionCall, ImageUrl, LlmProvider, LlmResponse, LlmStream, LlmStreamChunk,
    Message, ProviderCapabilities, ProviderHealth, ResponseFormat, ToolCall, ToolCallDelta,
    ToolCallMessage, ToolContent, ToolContentPart, Usage, UserContent, DEFAULT_CONTEXT_WINDOW,
};

// -- Factory --
pub use factory::{
    cognitive_chat_params, create_cognitive_provider, create_provider,
    create_provider_with_failover, create_provider_with_failover_full,
};
