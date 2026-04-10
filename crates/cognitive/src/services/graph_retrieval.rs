//! Graph-aware retrieval — extract entities from a query, traverse the entity
//! graph for neighborhood context, and compute a graph_path_boost score for
//! facts connected to those entities.
//!
//! The boost score is the 12th relevance weight factor. It rewards facts whose
//! subject or object mentions entities that are in the query's graph neighborhood.

use crate::repos::entity::EntityRepo;

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

    // Resolve entity names to graph neighborhood
    let mut neighborhood_names: std::collections::HashSet<String> =
        std::collections::HashSet::new();
    for name in &query_entities {
        if let Ok(entities) = entity_repo.find_by_name(name).await {
            for entity in &entities {
                neighborhood_names.insert(entity.name.to_lowercase());
                if let Ok(Some(hood)) = entity_repo.get_neighborhood(&entity.id, 1).await {
                    for neighbor in &hood.neighbors {
                        neighborhood_names.insert(neighbor.name.to_lowercase());
                    }
                }
            }
        }
    }

    if neighborhood_names.is_empty() {
        return boosts;
    }

    // Score each fact by entity overlap with the neighborhood
    for (fact_id, content) in fact_contents {
        let content_lower = content.to_lowercase();
        let matches = neighborhood_names
            .iter()
            .filter(|name| name.len() > 2 && content_lower.contains(name.as_str()))
            .count();
        if matches > 0 {
            let score = match matches {
                1 => 0.4,
                2 => 0.7,
                _ => 1.0,
            };
            boosts.insert((*fact_id).to_string(), score);
        }
    }

    boosts
}

/// Extract potential entity names from a query string.
/// Capitalized words that aren't sentence starters.
fn extract_query_entities(query: &str) -> Vec<String> {
    let words: Vec<&str> = query.split_whitespace().collect();
    let mut entities = Vec::new();

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

    entities.dedup();
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
}
