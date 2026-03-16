use async_trait::async_trait;

/// Trait for decomposing a user query into sub-queries for multi-dimensional retrieval.
#[async_trait]
pub trait QueryDecomposer: Send + Sync {
    /// Decompose `query` into a list of sub-queries.
    ///
    /// `context_hint` may provide additional signal (e.g. skill name) to
    /// guide decomposition.  Implementations must always include the original
    /// query as the first element.
    async fn decompose(&self, query: &str, context_hint: Option<&str>) -> Vec<String>;
}

/// Stop words filtered out during key-term extraction.
const STOP_WORDS: &[&str] = &[
    "a", "an", "the", "is", "are", "was", "were", "be", "been", "being", "have", "has", "had",
    "do", "does", "did", "will", "would", "shall", "should", "may", "might", "must", "can",
    "could", "am", "i", "me", "my", "we", "our", "you", "your", "he", "she", "it", "they", "them",
    "his", "her", "its", "to", "of", "in", "for", "on", "with", "at", "by", "from", "as", "into",
    "through", "during", "before", "after", "above", "below", "between", "out", "off", "over",
    "under", "again", "further", "then", "once", "here", "there", "when", "where", "why", "how",
    "all", "both", "each", "few", "more", "most", "other", "some", "such", "no", "not", "only",
    "own", "same", "so", "than", "too", "very", "just", "about", "up", "what", "which", "who",
    "whom", "this", "that", "these", "those", "and", "but", "or", "nor", "if",
    // Additional task-related stop words
    "help", "plan", "make", "get", "tell", "give", "show", "try", "ask",
];

/// Dimension suffixes appended to the extracted topic to form sub-queries.
const DIMENSION_SUFFIXES: &[&str] = &[
    "background context",
    "current status",
    "related people and teams",
    "risks and blockers",
    "timeline and deadlines",
];

/// A heuristic (non-LLM) query decomposer.
///
/// Extracts key terms via stop-word filtering, then generates sub-queries by
/// combining the topic with fixed dimension suffixes.
pub struct HeuristicDecomposer;

impl HeuristicDecomposer {
    /// Extract meaningful terms from `query` by lowercasing, splitting on
    /// whitespace, and filtering out stop words and very short tokens.
    fn extract_key_terms(query: &str) -> Vec<String> {
        query
            .split_whitespace()
            .map(|w| w.to_lowercase())
            // Strip common punctuation from each token edge
            .map(|w| w.trim_matches(|c: char| !c.is_alphanumeric()).to_string())
            .filter(|w| w.len() > 2 && !STOP_WORDS.contains(&w.as_str()))
            .collect()
    }
}

#[async_trait]
impl QueryDecomposer for HeuristicDecomposer {
    async fn decompose(&self, query: &str, _context_hint: Option<&str>) -> Vec<String> {
        let terms = Self::extract_key_terms(query);

        if terms.is_empty() {
            return vec![query.to_string()];
        }

        let topic = terms.join(" ");

        let mut sub_queries = Vec::with_capacity(1 + DIMENSION_SUFFIXES.len());
        // Original query always comes first.
        sub_queries.push(query.to_string());

        for suffix in DIMENSION_SUFFIXES {
            sub_queries.push(format!("{} {}", topic, suffix));
            // Cap at 5 total sub-queries.
            if sub_queries.len() >= 5 {
                break;
            }
        }

        sub_queries
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_heuristic_decomposer_produces_sub_queries() {
        let d = HeuristicDecomposer;
        let subs = d
            .decompose("Help me plan the API migration project", None)
            .await;

        assert!(
            subs.len() >= 3,
            "Expected at least 3 sub-queries, got {}",
            subs.len()
        );
        assert_eq!(
            subs[0], "Help me plan the API migration project",
            "First sub-query must be the original"
        );
    }

    #[tokio::test]
    async fn test_heuristic_with_short_query() {
        let d = HeuristicDecomposer;
        let subs = d.decompose("hi", None).await;
        assert_eq!(subs, vec!["hi"]);
    }

    #[tokio::test]
    async fn test_stop_word_filtering() {
        let terms = HeuristicDecomposer::extract_key_terms("Help me plan the API migration");
        assert!(terms.contains(&"api".to_string()), "api should be kept");
        assert!(
            terms.contains(&"migration".to_string()),
            "migration should be kept"
        );
        assert!(
            !terms.contains(&"help".to_string()),
            "help should be filtered"
        );
        assert!(
            !terms.contains(&"the".to_string()),
            "the should be filtered"
        );
        assert!(
            !terms.contains(&"plan".to_string()),
            "plan should be filtered"
        );
    }
}
