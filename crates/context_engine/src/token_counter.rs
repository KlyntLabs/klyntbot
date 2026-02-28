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

/// Construct the default (char-based) token counter.
pub fn default_token_counter() -> Arc<dyn TokenCounter> {
    Arc::new(CharTokenCounter)
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
}
