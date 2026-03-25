pub mod finance_searcher;
pub mod graph_searcher;
pub mod note_searcher;
pub mod task_searcher;

pub use finance_searcher::FinanceSearcher;
pub use graph_searcher::GraphSearcher;
pub use note_searcher::NoteSearcher;
pub use task_searcher::TaskSearcher;

/// Shared stop words for domain searcher keyword extraction.
const SEARCHER_STOP_WORDS: &[&str] = &[
    "background",
    "context",
    "current",
    "status",
    "related",
    "people",
    "teams",
    "risks",
    "blockers",
    "overview",
    "details",
    "skill",
    "the",
    "and",
    "for",
    "with",
    "about",
    "what",
    "how",
    "show",
    "tell",
    "give",
    "remind",
    "any",
    "updates",
    // Common verbs that shouldn't be search terms
    "are",
    "was",
    "were",
    "been",
    "being",
    "have",
    "has",
    "had",
    "does",
    "did",
    "doing",
    "going",
    "get",
    "got",
    "can",
    "could",
    "would",
    "should",
    "will",
    "might",
    "need",
    "want",
    "know",
    "think",
    "make",
    "like",
];

/// Extract the first meaningful keyword from a query for LIKE search.
/// Filters shared stop words plus any domain-specific extras.
fn extract_first_keyword(query: &str, extra_stop: &[&str]) -> String {
    query
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| {
            w.len() > 2
                && !SEARCHER_STOP_WORDS.contains(&w.to_lowercase().as_str())
                && !extra_stop.contains(&w.to_lowercase().as_str())
        })
        .next()
        .unwrap_or("")
        .to_string()
}
