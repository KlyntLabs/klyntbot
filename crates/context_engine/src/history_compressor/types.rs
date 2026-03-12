use providers::Message;

/// Default snippet length for extractive summaries (characters).
pub(super) const DEFAULT_SNIPPET_LENGTH: usize = 200;

/// Summary of a range of compressed messages.
pub struct HistorySummary {
    /// The summarized content text.
    pub content: String,
    /// Indices (start, end) of the original messages this summary covers.
    pub message_range: (usize, usize),
    /// Estimated token count for this summary.
    pub token_count: usize,
}

/// Result of compressing conversation history.
pub struct CompressedHistory {
    /// Summaries of older messages that were compressed.
    pub summaries: Vec<HistorySummary>,
    /// Recent messages kept verbatim.
    pub recent_messages: Vec<Message>,
    /// Estimated total token count across summaries + recent messages.
    pub total_tokens: usize,
}

/// Configuration for the history compressor.
#[derive(Debug, Clone)]
pub struct CompressorConfig {
    /// Maximum snippet length in characters for extractive summaries.
    pub snippet_length: usize,
    /// Compression mode.
    pub mode: CompressorMode,
    /// Number of messages per summary chunk when compressing older history.
    pub chunk_size: usize,
    /// Minimum number of recent messages to always keep verbatim.
    pub min_recent_messages: usize,
}

impl Default for CompressorConfig {
    fn default() -> Self {
        Self {
            snippet_length: DEFAULT_SNIPPET_LENGTH,
            mode: CompressorMode::Extractive,
            chunk_size: 5,
            min_recent_messages: 4,
        }
    }
}

/// Compression mode for history summarization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompressorMode {
    /// Extractive: takes a snippet from each message (no LLM call).
    Extractive,
    /// Abstractive: future mode, currently falls back to Extractive.
    Abstractive,
}
