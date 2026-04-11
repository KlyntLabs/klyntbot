use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use async_trait::async_trait;

use crate::memory_retriever::{MemoryEntry, MemoryRetriever};
use crate::rewriter::RetrievalContext;

use super::traits::QueryStage;
use super::types::{EnhancementBudget, QueryBundle, QuerySource};

// ---------------------------------------------------------------------------
// PrfConfig
// ---------------------------------------------------------------------------

/// Configuration for the Pseudo-Relevance Feedback stage.
#[derive(Debug, Clone)]
pub struct PrfConfig {
    /// Number of documents to fetch in the initial retrieval pass.
    pub initial_fetch_limit: usize,
    /// Minimum score for a retrieved entry to be used for expansion.
    pub min_score_threshold: f64,
    /// Maximum number of expansion terms to append to the query.
    pub max_expansion_terms: usize,
}

impl Default for PrfConfig {
    fn default() -> Self {
        Self {
            initial_fetch_limit: 3,
            min_score_threshold: 0.6,
            max_expansion_terms: 5,
        }
    }
}

// ---------------------------------------------------------------------------
// PrfStage
// ---------------------------------------------------------------------------

/// Query enhancement stage that uses Pseudo-Relevance Feedback.
///
/// Performs a small initial retrieval using the primary query, extracts
/// discriminative terms from the highest-scoring results, and appends them
/// as a query variant. Zero LLM cost — all expansion is lexical.
pub struct PrfStage {
    retriever: Arc<dyn MemoryRetriever>,
    config: PrfConfig,
}

impl PrfStage {
    /// Create a new PRF stage with the given retriever and config.
    pub fn new(retriever: Arc<dyn MemoryRetriever>, config: PrfConfig) -> Self {
        Self { retriever, config }
    }
}

#[async_trait]
impl QueryStage for PrfStage {
    fn name(&self) -> QuerySource {
        QuerySource::PseudoRelevanceFeedback
    }

    async fn transform(
        &self,
        mut input: QueryBundle,
        _context: &RetrievalContext,
        budget: &EnhancementBudget,
    ) -> common::Result<QueryBundle> {
        // Budget guard: require at least 50ms headroom.
        if budget.max_latency_ms < 50 {
            return Ok(input);
        }

        // Initial retrieval pass.
        let entries = self
            .retriever
            .retrieve(&input.primary, self.config.initial_fetch_limit)
            .await;

        // Filter to strong results only.
        let strong: Vec<&MemoryEntry> = entries
            .iter()
            .filter(|e| e.score >= self.config.min_score_threshold)
            .collect();

        if strong.is_empty() {
            return Ok(input);
        }

        // Extract discriminative terms from the strong results.
        let terms =
            extract_discriminative_terms(&strong, &input.primary, self.config.max_expansion_terms);

        if terms.is_empty() {
            return Ok(input);
        }

        // Build the expanded query variant.
        let expansion = terms.join(" ");
        let variant = format!("{} {}", input.primary, expansion);
        input.variants.push(variant);
        input.sources.push(QuerySource::PseudoRelevanceFeedback);

        Ok(input)
    }
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

/// Split text on whitespace, strip non-alphanumeric characters, lowercase,
/// and filter empty tokens.
///
/// Exported because the `heuristic_rerank` module reuses this tokenizer.
pub fn tokenize(text: &str) -> Vec<String> {
    text.split_whitespace()
        .map(|word| {
            word.chars()
                .filter(|c| c.is_alphanumeric())
                .collect::<String>()
                .to_lowercase()
        })
        .filter(|s| !s.is_empty())
        .collect()
}

/// Return `true` if the word is a common English stopword.
fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "the"
            | "a"
            | "an"
            | "and"
            | "or"
            | "but"
            | "in"
            | "on"
            | "at"
            | "to"
            | "for"
            | "of"
            | "with"
            | "by"
            | "from"
            | "is"
            | "it"
            | "this"
            | "that"
            | "are"
            | "was"
            | "were"
            | "be"
            | "been"
            | "has"
            | "have"
            | "had"
            | "do"
            | "does"
            | "did"
            | "will"
            | "would"
            | "could"
            | "should"
            | "may"
            | "might"
            | "can"
            | "not"
            | "no"
            | "so"
            | "if"
            | "then"
            | "than"
            | "too"
            | "very"
            | "just"
            | "about"
            | "up"
            | "out"
            | "all"
            | "also"
            | "how"
            | "what"
            | "when"
            | "where"
            | "who"
            | "which"
            | "why"
            | "there"
            | "here"
            | "some"
            | "any"
            | "each"
            | "every"
            | "both"
            | "few"
            | "more"
            | "most"
            | "other"
            | "into"
            | "over"
            | "after"
            | "before"
            | "between"
            | "under"
            | "through"
            | "during"
            | "without"
            | "within"
            | "its"
            | "my"
            | "your"
            | "his"
            | "her"
            | "our"
            | "their"
            | "me"
            | "him"
            | "them"
            | "you"
            | "she"
            | "he"
            | "we"
            | "they"
    )
}

/// Extract the most discriminative terms from retrieved entries relative to
/// the query.
///
/// A term is discriminative if it:
/// - Does not appear in the query.
/// - Is not a stopword.
/// - Has at least 3 characters.
///
/// Terms are ranked by document frequency (how many entries contain the term),
/// with alphabetical order as a tiebreaker.
fn extract_discriminative_terms(
    entries: &[&MemoryEntry],
    query: &str,
    max_terms: usize,
) -> Vec<String> {
    let query_terms: HashSet<String> = tokenize(query).into_iter().collect();

    // Count how many entries each candidate term appears in.
    let mut freq: HashMap<String, usize> = HashMap::new();

    for entry in entries {
        let entry_terms: HashSet<String> = tokenize(&entry.content).into_iter().collect();
        for term in entry_terms {
            if !query_terms.contains(&term) && !is_stopword(&term) && term.len() >= 3 {
                *freq.entry(term).or_insert(0) += 1;
            }
        }
    }

    // Sort by frequency descending, then alphabetically for stable output.
    let mut ranked: Vec<(String, usize)> = freq.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));

    ranked.into_iter().take(max_terms).map(|(t, _)| t).collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    use crate::memory_retriever::{MemoryEntry, MemorySource};
    use crate::rewriter::RetrievalContext;

    struct MockRetriever {
        entries: Vec<MemoryEntry>,
    }

    #[async_trait]
    impl MemoryRetriever for MockRetriever {
        async fn retrieve(&self, _query: &str, limit: usize) -> Vec<MemoryEntry> {
            self.entries.iter().take(limit).cloned().collect()
        }
    }

    fn make_entry(id: &str, content: &str, score: f64) -> MemoryEntry {
        MemoryEntry {
            id: id.to_string(),
            content: content.to_string(),
            score,
            source: MemorySource::CognitiveFact,
            raw_score: score,
        }
    }

    fn make_context() -> RetrievalContext {
        RetrievalContext::default()
    }

    #[tokio::test]
    async fn test_prf_extracts_expansion_terms() {
        // Three facts about budget/finance/quarterly. "quarterly" appears in 2 of them.
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                make_entry(
                    "1",
                    "The quarterly finance report shows budget overrun",
                    0.85,
                ),
                make_entry("2", "Quarterly review of finance targets and budget", 0.80),
                make_entry("3", "Finance department annual planning", 0.75),
            ],
        });

        let config = PrfConfig {
            initial_fetch_limit: 3,
            min_score_threshold: 0.6,
            max_expansion_terms: 5,
        };
        let stage = PrfStage::new(retriever, config);
        let budget = EnhancementBudget::normal();
        let ctx = make_context();

        let input = QueryBundle::passthrough("show me budget");
        let output = stage.transform(input, &ctx, &budget).await.unwrap();

        assert_eq!(
            output.variants.len(),
            1,
            "expected exactly one variant to be added"
        );
        let variant = &output.variants[0];
        assert!(
            variant.contains("quarterly"),
            "expected 'quarterly' in variant (appears in 2+ results), got: {variant}"
        );
        assert!(
            output
                .sources
                .contains(&QuerySource::PseudoRelevanceFeedback),
            "PseudoRelevanceFeedback source should be recorded"
        );
    }

    #[tokio::test]
    async fn test_prf_passthrough_on_low_scores() {
        // All results score below the 0.6 threshold.
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                make_entry("1", "quarterly finance report budget overrun", 0.4),
                make_entry("2", "quarterly review finance targets budget", 0.3),
                make_entry("3", "finance department annual planning budget", 0.2),
            ],
        });

        let config = PrfConfig::default();
        let stage = PrfStage::new(retriever, config);
        let budget = EnhancementBudget::normal();
        let ctx = make_context();

        let input = QueryBundle::passthrough("show me budget");
        let output = stage.transform(input, &ctx, &budget).await.unwrap();

        assert!(
            output.variants.is_empty(),
            "no variants should be added when all scores are below threshold"
        );
        assert!(
            !output
                .sources
                .contains(&QuerySource::PseudoRelevanceFeedback),
            "PRF source should not be recorded when skipped"
        );
    }

    #[tokio::test]
    async fn test_prf_skips_on_tight_budget() {
        // Even though results are strong, the budget is too tight to run.
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                make_entry("1", "quarterly finance report budget overrun", 0.9),
                make_entry("2", "quarterly review finance targets budget", 0.85),
            ],
        });

        let config = PrfConfig::default();
        let stage = PrfStage::new(retriever, config);

        // max_latency_ms = 30 < 50 threshold
        let budget = EnhancementBudget {
            max_latency_ms: 30,
            max_llm_calls: 0,
            max_expansion_tokens: 0,
        };
        let ctx = make_context();

        let input = QueryBundle::passthrough("show me budget");
        let output = stage.transform(input, &ctx, &budget).await.unwrap();

        assert!(
            output.variants.is_empty(),
            "no variants should be added when budget is too tight"
        );
        assert!(
            !output
                .sources
                .contains(&QuerySource::PseudoRelevanceFeedback),
            "PRF source should not be recorded when skipped due to budget"
        );
    }

    #[tokio::test]
    async fn test_prf_no_duplicate_query_terms() {
        // Results contain the query term "budget" prominently — it must not
        // appear in the expansion.
        let retriever = Arc::new(MockRetriever {
            entries: vec![
                make_entry(
                    "1",
                    "budget planning is critical for quarterly success",
                    0.85,
                ),
                make_entry(
                    "2",
                    "budget allocations reviewed quarterly by the finance team",
                    0.80,
                ),
                make_entry("3", "quarterly financial review covers budget items", 0.75),
            ],
        });

        let config = PrfConfig::default();
        let stage = PrfStage::new(retriever, config);
        let budget = EnhancementBudget::normal();
        let ctx = make_context();

        let input = QueryBundle::passthrough("show me budget");
        let output = stage.transform(input, &ctx, &budget).await.unwrap();

        assert_eq!(output.variants.len(), 1, "expected one variant");
        let variant = &output.variants[0];

        // The variant starts with the primary query; expansion should not
        // re-include "budget", "show", or "me".
        let expansion = variant
            .strip_prefix("show me budget ")
            .expect("variant should start with primary query");
        let expansion_terms: Vec<&str> = expansion.split_whitespace().collect();

        for term in &expansion_terms {
            assert_ne!(
                *term, "budget",
                "'budget' is a query term and must not appear in expansion"
            );
            assert_ne!(
                *term, "show",
                "'show' is a query term and must not appear in expansion"
            );
            assert_ne!(
                *term, "me",
                "'me' is a stopword/query term and must not appear in expansion"
            );
        }
    }
}
