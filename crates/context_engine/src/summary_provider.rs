use async_trait::async_trait;
use providers::Message;

use crate::history_compressor::CompressionTier;

/// Trait for abstractive summarization of conversation history.
///
/// Implementations call an LLM to produce summaries at a specified
/// compression tier (Detailed or Condensed).
#[async_trait]
pub trait SummaryProvider: Send + Sync {
    /// Summarize multiple conversation segments in a single batch.
    ///
    /// Each element in `segments` is a group of messages to summarize together.
    /// `tier` controls the prompt and compression aggressiveness.
    /// Returns one `Result` per input segment — individual segments may fail
    /// independently, allowing callers to fall back to extractive summarization
    /// on a per-segment basis.
    async fn summarize_batch(
        &self,
        segments: Vec<Vec<Message>>,
        tier: CompressionTier,
    ) -> Vec<Result<String, String>>;
}
