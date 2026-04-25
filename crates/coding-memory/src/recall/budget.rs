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
    /// uses byte-prefix heuristic and verifies; concrete impls may override.
    fn truncate_to(&self, s: &str, budget: usize) -> String {
        if self.count(s) <= budget {
            return s.to_string();
        }
        // Binary-search by char count.
        let chars: Vec<char> = s.chars().collect();
        let mut lo = 0usize;
        let mut hi = chars.len();
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let candidate: String = chars[..mid].iter().collect();
            if self.count(&candidate) <= budget {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        let mut out: String = chars[..lo].iter().collect();
        if out.len() < s.len() {
            if self.count(&out) + 1 <= budget {
                out.push('…');
            }
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
        tiktoken_rs::cl100k_base()
            .ok()
            .map(|bpe| Self { bpe: std::sync::Arc::new(bpe) })
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
