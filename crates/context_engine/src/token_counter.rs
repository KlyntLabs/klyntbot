use std::sync::Arc;

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

/// Construct the default (char-based) token counter.
pub fn default_token_counter() -> Arc<dyn TokenCounter> {
    Arc::new(CharTokenCounter)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_default_token_counter_is_char_based() {
        let counter = default_token_counter();
        assert_eq!(counter.estimate_text("hello world"), 3); // 11 chars → 3
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
