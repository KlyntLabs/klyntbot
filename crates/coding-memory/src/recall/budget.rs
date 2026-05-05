//! Token budgeter — counts and truncates strings to a token budget.
//!
//! `TiktokenBudgeter` uses `tiktoken-rs` (cl100k_base) for OpenAI-compatible counts.
//! `HeuristicBudgeter` is the always-available fallback (`chars / 4`).
//! Renderers depend on the trait; tests use `HeuristicBudgeter` for determinism.

/// Pluggable token counter + truncator.
pub trait TokenBudgeter: Send + Sync {
    /// Count tokens in `s`.
    fn count(&self, s: &str) -> usize;

    /// Truncate `s` so its token count is at most `budget`. Default impl
    /// estimates a tokens/char ratio with a single tokenization, then slices
    /// at a char boundary and verifies/shrinks once. Avoids per-iteration
    /// re-tokenization and Vec<char> allocation.
    fn truncate_to(&self, s: &str, budget: usize) -> String {
        let total = self.count(s);
        if total <= budget {
            return s.to_string();
        }
        let char_count = s.chars().count();
        if char_count == 0 || total == 0 {
            return String::new();
        }
        let target_chars = budget.saturating_mul(char_count) / total;
        let mut end = s
            .char_indices()
            .nth(target_chars)
            .map_or(s.len(), |(i, _)| i);
        // Verify; if still over budget, walk back one char at a time.
        while end > 0 && self.count(&s[..end]) > budget {
            end = s[..end].char_indices().next_back().map_or(0, |(i, _)| i);
        }
        let mut out = s[..end].to_string();
        if end < s.len() && self.count(&out) < budget {
            out.push('…');
        }
        out
    }
}

/// `chars / 4` heuristic. Always available.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeuristicBudgeter;

impl TokenBudgeter for HeuristicBudgeter {
    fn count(&self, s: &str) -> usize {
        s.chars().count().div_ceil(4)
    }
}

/// `tiktoken-rs` cl100k_base counter. Constructed lazily — encoding load can fail.
#[derive(Debug, Clone)]
pub struct TiktokenBudgeter {
    bpe: std::sync::Arc<tiktoken_rs::CoreBPE>,
}

impl TiktokenBudgeter {
    /// Try to load cl100k_base. Returns `None` if the encoding cannot be built.
    pub fn try_new() -> Option<Self> {
        tiktoken_rs::cl100k_base().ok().map(|bpe| Self {
            bpe: std::sync::Arc::new(bpe),
        })
    }
}

impl TokenBudgeter for TiktokenBudgeter {
    fn count(&self, s: &str) -> usize {
        self.bpe.encode_with_special_tokens(s).len()
    }
}

/// Pick the best budgeter available — `Tiktoken` if loadable, else `Heuristic`.
#[must_use]
pub fn default_budgeter() -> std::sync::Arc<dyn TokenBudgeter> {
    if let Some(t) = TiktokenBudgeter::try_new() {
        std::sync::Arc::new(t)
    } else {
        std::sync::Arc::new(HeuristicBudgeter)
    }
}
