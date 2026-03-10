use async_trait::async_trait;
use providers::Message;

/// Trait for abstractive summarization of conversation history.
///
/// Implementations call an LLM (or other summarizer) to produce concise
/// natural-language summaries of conversation segments in batch.
///
/// The trait is object-safe: no generic methods.
#[async_trait]
pub trait SummaryProvider: Send + Sync {
    /// Summarize multiple conversation segments in a single batch.
    ///
    /// Each element in `segments` is a group of messages to summarize together.
    /// Returns one `Result` per input segment — individual segments may fail
    /// independently, allowing callers to fall back to extractive summarization
    /// on a per-segment basis.
    async fn summarize_batch(&self, segments: Vec<Vec<Message>>) -> Vec<Result<String, String>>;
}
