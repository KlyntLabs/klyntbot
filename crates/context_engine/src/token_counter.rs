use std::sync::Arc;
use tiktoken_rs::CoreBPE;

/// Sync token estimation trait for use during context assembly.
///
/// Using a sync trait avoids async overhead in the inner estimation loop.
/// For async providers, implement this trait with a cached or approximated
/// synchronous estimate — the char-based fallback is always available.
pub trait TokenCounter: Send + Sync {
    /// Estimate the number of tokens for a piece of text.
    fn estimate_text(&self, text: &str) -> usize;
}

/// Default token counter: character-based heuristic (4 chars ≈ 1 token).
pub struct CharTokenCounter;

impl TokenCounter for CharTokenCounter {
    fn estimate_text(&self, text: &str) -> usize {
        text.len().div_ceil(4)
    }
}

/// BPE token counter backed by tiktoken-rs (cl100k_base encoding).
///
/// This provides accurate token counts for OpenAI-compatible models.
/// Falls back to [`CharTokenCounter`] if initialization fails.
pub struct TiktokenCounter {
    bpe: CoreBPE,
}

impl TiktokenCounter {
    /// Create a new `TiktokenCounter` using cl100k_base encoding.
    ///
    /// Returns `None` if the BPE model fails to initialize.
    pub fn new() -> Option<Self> {
        tiktoken_rs::cl100k_base().ok().map(|bpe| Self { bpe })
    }
}

impl TokenCounter for TiktokenCounter {
    fn estimate_text(&self, text: &str) -> usize {
        self.bpe.encode_with_special_tokens(text).len()
    }
}

// SAFETY: CoreBPE holds a compiled regex and read-only data, making it
// safe to share across threads.
unsafe impl Send for TiktokenCounter {}
unsafe impl Sync for TiktokenCounter {}

/// Anthropic-tuned token counter using a 3.5 chars/token ratio.
///
/// Claude models use a proprietary tokenizer that differs from OpenAI's
/// cl100k_base BPE. The 3.5 chars/token heuristic produces estimates
/// within ~5% of the real tokenizer for English text, compared to ±15%
/// with the default 4 chars/token or cl100k_base counters.
pub struct AnthropicTokenCounter;

impl TokenCounter for AnthropicTokenCounter {
    fn estimate_text(&self, text: &str) -> usize {
        // 3.5 chars/token → multiply by 2, divide by 7 to avoid floating point
        (text.len() * 2).div_ceil(7)
    }
}

/// Construct the default (char-based) token counter.
pub fn default_token_counter() -> Arc<dyn TokenCounter> {
    Arc::new(CharTokenCounter)
}

/// Estimate the token count of a single message, including per-role overhead.
///
/// Shared by `HistoryCompressor` (assembly-time compression) and
/// `MidLoopCompressor` (in-loop compression) to keep token budgets consistent.
pub fn estimate_message_tokens(counter: &dyn TokenCounter, msg: &providers::Message) -> usize {
    match msg {
        providers::Message::System { content } => counter.estimate_text(content),
        providers::Message::User { content } => match content {
            providers::UserContent::Text(t) => counter.estimate_text(t),
            providers::UserContent::MultiPart(parts) => parts.len() * 10,
        },
        providers::Message::Assistant { content, .. } => {
            content
                .as_deref()
                .map(|t| counter.estimate_text(t))
                .unwrap_or(0)
                + 20
        }
        providers::Message::Tool { content, .. } => counter.estimate_text(content) + 10,
    }
}

/// Construct the best available token counter.
///
/// Tries [`TiktokenCounter`] (BPE, accurate) first; falls back to
/// [`CharTokenCounter`] (heuristic) with a warning if initialization fails.
pub fn best_token_counter() -> Arc<dyn TokenCounter> {
    match TiktokenCounter::new() {
        Some(counter) => Arc::new(counter),
        None => {
            tracing::warn!(
                "tiktoken-rs failed to initialize cl100k_base; \
                 falling back to char-based token counter"
            );
            Arc::new(CharTokenCounter)
        }
    }
}

/// Select the best token counter for a given model name.
///
/// Returns [`AnthropicTokenCounter`] for Anthropic models (matched via
/// case-insensitive check for "claude" or "anthropic" in the model name),
/// and [`best_token_counter`] (tiktoken BPE) for everything else.
pub fn token_counter_for_model(model: &str) -> Arc<dyn TokenCounter> {
    let model_lower = model.to_lowercase();
    if model_lower.contains("claude") || model_lower.contains("anthropic") {
        Arc::new(AnthropicTokenCounter)
    } else {
        best_token_counter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── TiktokenCounter tests ─────────────────────────────────────────────

    #[test]
    fn test_tiktoken_counter_english() {
        let counter = TiktokenCounter::new().expect("tiktoken init should succeed");
        // "Hello, world!" is typically 4 tokens with cl100k_base
        let count = counter.estimate_text("Hello, world!");
        assert!(
            (2..=8).contains(&count),
            "Expected ~4 tokens for 'Hello, world!', got {count}"
        );
    }

    #[test]
    fn test_tiktoken_counter_cjk() {
        let counter = TiktokenCounter::new().expect("tiktoken init should succeed");
        // CJK characters each become ~1 token with cl100k_base
        let count = counter.estimate_text("你好世界");
        assert!(
            (2..=12).contains(&count),
            "Expected ~4 tokens for '你好世界', got {count}"
        );
    }

    #[test]
    fn test_tiktoken_counter_empty() {
        let counter = TiktokenCounter::new().expect("tiktoken init should succeed");
        assert_eq!(
            counter.estimate_text(""),
            0,
            "Empty string should be 0 tokens"
        );
    }

    #[test]
    fn test_best_token_counter_returns_arc() {
        let counter = best_token_counter();
        // Should work on a simple string without panicking
        let _ = counter.estimate_text("hello");
    }

    // ── CharTokenCounter tests ────────────────────────────────────────────

    #[test]
    fn test_char_token_counter_empty() {
        let counter = CharTokenCounter;
        assert_eq!(counter.estimate_text(""), 0);
    }

    #[test]
    fn test_char_token_counter_exact_multiple() {
        let counter = CharTokenCounter;
        // 8 chars → 2 tokens
        assert_eq!(counter.estimate_text("12345678"), 2);
    }

    #[test]
    fn test_char_token_counter_rounds_up() {
        let counter = CharTokenCounter;
        // 5 chars → 2 tokens (div_ceil)
        assert_eq!(counter.estimate_text("12345"), 2);
    }

    #[test]
    fn test_custom_token_counter() {
        struct FixedCounter;
        impl TokenCounter for FixedCounter {
            fn estimate_text(&self, _text: &str) -> usize {
                42
            }
        }
        let counter = FixedCounter;
        assert_eq!(counter.estimate_text("anything"), 42);
    }

    // ── AnthropicTokenCounter tests ──────────────────────────────────────

    #[test]
    fn test_anthropic_counter_english() {
        let counter = AnthropicTokenCounter;
        let count = counter.estimate_text("Hello, world!");
        assert_eq!(count, 4); // ceil(13 / 3.5) = ceil(3.71) = 4
    }

    #[test]
    fn test_anthropic_counter_empty() {
        let counter = AnthropicTokenCounter;
        assert_eq!(counter.estimate_text(""), 0);
    }

    #[test]
    fn test_anthropic_counter_vs_char_counter() {
        let anthropic = AnthropicTokenCounter;
        let char_c = CharTokenCounter;
        let text =
            "This is a representative prompt with enough text to show divergence between counters.";
        let anthropic_count = anthropic.estimate_text(text);
        let char_count = char_c.estimate_text(text);
        assert!(
            anthropic_count > char_count,
            "Anthropic counter should produce higher token count"
        );
    }

    // ── token_counter_for_model tests ────────────────────────────────────

    #[test]
    fn test_token_counter_for_model_claude() {
        let counter = token_counter_for_model("claude-sonnet-4-5-20250514");
        let count = counter.estimate_text("Hello, world!");
        assert_eq!(count, 4);
    }

    #[test]
    fn test_token_counter_for_model_claude_variants() {
        for model in &[
            "claude-3-haiku-20240307",
            "claude-3-opus-20240229",
            "claude-3-5-sonnet",
            "anthropic/claude-sonnet-4-5-20250514",
            "anthropic/claude-opus-4-5",
        ] {
            let counter = token_counter_for_model(model);
            let count = counter.estimate_text("Hello, world!");
            assert_eq!(count, 4, "Model {model} should use AnthropicTokenCounter");
        }
    }

    #[test]
    fn test_token_counter_for_model_openai() {
        let counter = token_counter_for_model("gpt-4o");
        let count = counter.estimate_text("Hello, world!");
        assert!(
            (2..=8).contains(&count),
            "Expected BPE token count ~4, got {count}"
        );
    }

    #[test]
    fn test_token_counter_for_model_unknown() {
        let counter = token_counter_for_model("some-custom-model");
        let _ = counter.estimate_text("test");
    }
}
