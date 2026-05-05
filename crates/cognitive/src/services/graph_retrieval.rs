//! Graph-aware retrieval — extract entities from a query, traverse the entity
//! graph for neighborhood context, and compute a graph_path_boost score for
//! facts connected to those entities.
//!
//! The boost score is the 12th relevance weight factor. It rewards facts whose
//! subject or object mentions entities that are in the query's graph neighborhood.

use crate::repos::entity::{EdgeType, EntityRepo};

/// Compute graph_path_boost scores for a set of facts based on query entity context.
///
/// Returns a map of fact_id → boost score (0.0–1.0).
/// Facts mentioning entities in the query's graph neighborhood get higher scores.
pub async fn compute_graph_boosts(
    entity_repo: &EntityRepo,
    query: &str,
    fact_contents: &[(&str, &str)], // (fact_id, fact_content)
) -> std::collections::HashMap<String, f64> {
    let mut boosts = std::collections::HashMap::new();

    let query_entities = extract_query_entities(query);
    if query_entities.is_empty() {
        return boosts;
    }

    // Resolve entity names to graph neighborhood with edge types.
    let mut neighborhood: std::collections::HashMap<String, EdgeType> =
        std::collections::HashMap::new();
    for name in &query_entities {
        if let Ok(entities) = entity_repo.find_by_name(name).await {
            for entity in &entities {
                neighborhood.insert(entity.name.to_lowercase(), EdgeType::Correlational);
                if let Ok(edges) = entity_repo.get_neighborhood_with_edges(&entity.id, 1).await {
                    for edge in &edges {
                        neighborhood.insert(edge.neighbor.name.to_lowercase(), edge.edge_type);
                    }
                }
            }
        }
    }

    if neighborhood.is_empty() {
        return boosts;
    }

    // Score each fact by weighted entity overlap with the neighborhood.
    for (fact_id, content) in fact_contents {
        let content_lower = content.to_lowercase();
        let mut weighted_score = 0.0;
        for (name, edge_type) in &neighborhood {
            if name.len() > 2 && content_lower.contains(name.as_str()) {
                weighted_score += edge_type.weight();
                if weighted_score >= 1.0 {
                    break;
                }
            }
        }
        if weighted_score > 0.0 {
            let score = weighted_score.min(1.0);
            boosts.insert((*fact_id).to_string(), score);
        }
    }

    boosts
}

/// Extract potential entity names from a query string.
/// Capitalized words that aren't sentence starters.
/// Wave 1: heuristic entity extractor with two layers.
///
/// Layer A (capitalized, high precision): keeps tokens that begin with an
/// uppercase letter and aren't at sentence start — "Alice", "Rust",
/// "Klynt". Existing behavior, kept verbatim.
///
/// Layer B (lowercase fallback): when Layer A returns empty (e.g.
/// `"what is alice's job"`), keeps tokens ≥5 chars that aren't in a
/// small stopword set of common conversational fillers. This catches
/// chat-style queries that downcase proper nouns. False-positive risk is
/// bounded by the FTS5 query — non-entity tokens rarely match indexed
/// subjects.
///
/// We do not lookup against the entity table here because the function
/// is sync. Wave 3 adds an async `_with_repo` variant that does proper
/// alias resolution.
pub(crate) fn extract_query_entities(query: &str) -> Vec<String> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut entities = Vec::new();

    // Layer A
    for (i, word) in words.iter().enumerate() {
        let is_sentence_start = i == 0
            || words.get(i.wrapping_sub(1)).is_some_and(|prev| {
                prev.ends_with('.') || prev.ends_with('?') || prev.ends_with('!')
            });

        if !is_sentence_start
            && word.len() > 1
            && word.chars().next().is_some_and(|c| c.is_uppercase())
            && !word.chars().all(|c| c.is_uppercase())
        {
            let clean = word.trim_end_matches(|c: char| c.is_ascii_punctuation());
            if clean.len() > 1 {
                entities.push(clean.to_string());
            }
        }
    }

    if !entities.is_empty() {
        entities.dedup();
        return entities;
    }

    // Layer B — lowercase fallback. Stopword list deliberately small;
    // generic (no LoCoMo-specific words) per anti-tuning rule 5.
    const STOPWORDS: &[&str] = &[
        "about",
        "above",
        "after",
        "again",
        "against",
        "before",
        "below",
        "between",
        "could",
        "doing",
        "during",
        "every",
        "first",
        "going",
        "having",
        "knows",
        "might",
        "ought",
        "other",
        "place",
        "should",
        "since",
        "still",
        "their",
        "there",
        "these",
        "thing",
        "think",
        "those",
        "through",
        "today",
        "tonight",
        "until",
        "where",
        "which",
        "while",
        "would",
        "yesterday",
    ];
    for word in &words {
        // Strip surrounding punctuation, then split on apostrophe to drop
        // possessive suffix ("alice's" → "alice"). Skip token if any char
        // remaining is non-alphabetic.
        let trimmed = word.trim_matches(|c: char| c.is_ascii_punctuation());
        let stem = trimmed.split('\'').next().unwrap_or("").to_lowercase();
        if stem.len() < 5 {
            continue;
        }
        if !stem.chars().all(|c| c.is_alphabetic()) {
            continue;
        }
        if STOPWORDS.contains(&stem.as_str()) {
            continue;
        }
        if !entities
            .iter()
            .any(|e: &String| e.eq_ignore_ascii_case(&stem))
        {
            entities.push(stem);
        }
    }
    entities
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_entities_skips_sentence_starters() {
        let entities = extract_query_entities("Tell me about Rust and the Klynt project");
        assert!(entities.contains(&"Rust".to_string()));
        assert!(entities.contains(&"Klynt".to_string()));
        assert!(!entities.contains(&"Tell".to_string()));
    }

    #[test]
    fn extract_entities_empty_on_lowercase() {
        let entities = extract_query_entities("what time is it");
        assert!(entities.is_empty());
    }

    #[test]
    fn extract_entities_strips_punctuation() {
        let entities = extract_query_entities("I was talking to Sarah, about the project.");
        assert!(entities.contains(&"Sarah".to_string()));
    }

    #[test]
    fn wave1_lowercase_fallback_catches_chat_proper_nouns() {
        // Chat-style downcased query — Layer A returns empty; Layer B
        // should keep ≥5-char alphabetic tokens that aren't stopwords.
        let entities = extract_query_entities("what is alice's favorite color");
        assert!(
            entities.iter().any(|e| e == "alice"),
            "Expected lowercase 'alice' (after stripping possessive); got {:?}",
            entities
        );
        assert!(
            entities.iter().any(|e| e == "favorite"),
            "Expected 'favorite' (5 chars, not stopword); got {:?}",
            entities
        );
        // "color" is 5 chars and not in our stoplist; it'll appear too.
        // Acceptable: FTS will harmlessly miss on non-entity tokens.
    }

    #[test]
    fn wave1_short_query_still_empty() {
        // Original test contract: "what time is it" yields no entities.
        // Layer B requires ≥5 chars — "what" (4) and "time" (4) excluded.
        let entities = extract_query_entities("what time is it");
        assert!(entities.is_empty(), "got {:?}", entities);
    }
}
