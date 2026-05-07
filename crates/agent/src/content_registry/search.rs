//! Keyword-based content search with term scoring.

use super::types::*;

/// Search docs and skills by keyword relevance.
///
/// Scores entries by matching query terms against name (3x weight),
/// tags (2x), and description (1x). Returns results sorted by score.
pub fn search_content(
    docs: &[DocEntry],
    skills: &[SkillEntry],
    query: &str,
    limit: usize,
) -> Vec<ContentSearchResult> {
    let query_lower = query.to_lowercase();
    let terms: Vec<&str> = query_lower.split_whitespace().collect();

    if terms.is_empty() {
        return Vec::new();
    }

    let mut results: Vec<ContentSearchResult> = Vec::new();

    for doc in docs {
        let score = score_entry(&doc.name, &doc.description, &doc.tags, &terms);
        if score > 0.0 {
            results.push(ContentSearchResult {
                entry: ContentEntry::Doc(doc.clone()),
                score,
            });
        }
    }

    for skill in skills {
        let score = score_entry(&skill.name, &skill.description, &skill.tags, &terms);
        if score > 0.0 {
            results.push(ContentSearchResult {
                entry: ContentEntry::Skill(skill.clone()),
                score,
            });
        }
    }

    results.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    results
}

/// Score an entry against query terms.
///
/// Weights: name match = 3.0, tag match = 2.0, description match = 1.0.
/// Case-insensitive `contains` for ASCII needles.
fn icontains(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    let mut chars = haystack.chars();
    loop {
        if chars
            .clone()
            .zip(needle.chars())
            .all(|(a, b)| a.eq_ignore_ascii_case(&b))
        {
            return true;
        }
        if chars.next().is_none() {
            break;
        }
    }
    false
}

fn score_entry(name: &str, description: &str, tags: &[String], terms: &[&str]) -> f64 {
    let mut score = 0.0;
    for term in terms {
        if icontains(name, term) {
            score += 3.0;
        }
        if icontains(description, term) {
            score += 1.0;
        }
        if tags.iter().any(|t| icontains(t, term)) {
            score += 2.0;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_docs() -> Vec<DocEntry> {
        vec![
            DocEntry {
                id: "stripe/api".into(),
                name: "Stripe API".into(),
                description: "Payment processing REST API".into(),
                source: "community".into(),
                tags: vec!["payment".into(), "api".into()],
                content_source: "test".into(),
                languages: vec![],
            },
            DocEntry {
                id: "react/hooks".into(),
                name: "React Hooks".into(),
                description: "React state management hooks".into(),
                source: "community".into(),
                tags: vec!["react".into(), "frontend".into()],
                content_source: "test".into(),
                languages: vec![],
            },
        ]
    }

    #[test]
    fn test_search_finds_matching_docs() {
        let docs = sample_docs();
        let results = search_content(&docs, &[], "payment API", 10);
        assert!(!results.is_empty());
    }

    #[test]
    fn test_search_ranks_by_relevance() {
        let docs = sample_docs();
        let results = search_content(&docs, &[], "payment API", 10);
        // Stripe should rank higher for "payment API"
        if let ContentEntry::Doc(doc) = &results[0].entry {
            assert_eq!(doc.id, "stripe/api");
        } else {
            panic!("Expected Doc entry");
        }
    }

    #[test]
    fn test_search_empty_query_returns_nothing() {
        let docs = sample_docs();
        let results = search_content(&docs, &[], "", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_no_match_returns_empty() {
        let docs = sample_docs();
        let results = search_content(&docs, &[], "kubernetes deployment", 10);
        assert!(results.is_empty());
    }

    #[test]
    fn test_search_respects_limit() {
        let docs = sample_docs();
        let results = search_content(&docs, &[], "api", 1);
        assert_eq!(results.len(), 1);
    }
}
