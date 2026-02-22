use async_trait::async_trait;
use providers::Message;

/// Trait for abstractive summarization of conversation history chunks.
///
/// Implementations call an LLM (or other summarizer) to produce a concise
/// natural-language summary of a slice of messages.
///
/// The trait is object-safe: no generic methods.
#[async_trait]
pub trait SummaryProvider: Send + Sync {
    /// Summarize a slice of messages into a single string.
    ///
    /// Returns `Err(String)` on failure; callers should fall back to
    /// extractive summarization on error.
    async fn summarize(&self, messages: &[Message]) -> Result<String, String>;
}
