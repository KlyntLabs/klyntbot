//! Search utilities for Klyntbot — generic Reciprocal Rank Fusion (RRF) and search result types.
//!
//! This module provides reusable search infrastructure for merging ranked lists from
//! multiple sources (keyword, semantic, etc.) using the RRF algorithm.

use crate::conversation_embedding::ConversationEmbeddingRecord;
use crate::todo_types::Todo;
use std::collections::HashMap;

/// Search result types that can be merged via RRF
#[derive(Debug, Clone)]
pub enum SearchResult {
    /// Todo task search result (boxed to reduce enum size variance)
    Todo(Box<Todo>),
    /// Conversation message search result
    Conversation(ConversationEmbeddingRecord),
}

impl SearchResult {
    /// Get the unique ID for this search result
    pub fn id(&self) -> &str {
        match self {
            SearchResult::Todo(todo) => &todo.id,
            SearchResult::Conversation(record) => &record.id,
        }
    }
}

/// Reciprocal Rank Fusion: merge ranked lists from multiple sources.
///
/// RRF formula: `score(d) = sum(1 / (k + rank_i))` for each list where `d` appears.
///
/// # Arguments
/// * `keyword_results` - Results from keyword search (ordered by relevance)
/// * `semantic_results` - Results from semantic search (ID, similarity score pairs, ordered)
/// * `k` - RRF parameter (typical value: 60). Higher k = less weight to top ranks
/// * `items_by_id` - Lookup map for retrieving full items by ID
///
/// # Returns
/// Vec of (item, rrf_score, source) tuples, sorted by RRF score descending.
/// Source can be "keyword", "semantic", or "both".
///
/// # Edge Cases
/// - Empty lists: Returns empty vec (EC-15)
/// - Single source: Ranks based on that source only (EC-16)
/// - Overlapping items: Scores accumulate from both sources
pub fn rrf_merge(
    keyword_results: &[SearchResult],
    semantic_results: &[(String, f64)], // (id, similarity)
    k: u32,
    items_by_id: &HashMap<String, SearchResult>,
) -> Vec<(SearchResult, f64, &'static str)> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut sources: HashMap<String, u8> = HashMap::new(); // bitmask: 1=keyword, 2=semantic

    // Process keyword results (rank-based scoring)
    for (rank, result) in keyword_results.iter().enumerate() {
        let id = result.id().to_string();
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        *sources.entry(id).or_insert(0) |= 1;
    }

    // Process semantic results (rank-based scoring, ignore similarity for RRF)
    for (rank, (id, _sim)) in semantic_results.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        *sources.entry(id.clone()).or_insert(0) |= 2;
    }

    // Sort by RRF score descending
    let mut ranked: Vec<_> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    // Build final results with source labels
    ranked
        .into_iter()
        .filter_map(|(id, score)| {
            // Retrieve the full item (prefer keyword results, fallback to lookup map)
            let item = keyword_results
                .iter()
                .find(|r| r.id() == id)
                .cloned()
                .or_else(|| items_by_id.get(&id).cloned());

            let source_bits = sources.get(&id).copied().unwrap_or(0);
            let source = match source_bits {
                3 => "both",
                2 => "semantic",
                1 => "keyword",
                _ => "unknown",
            };

            item.map(|i| (i, score, source))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: Create a mock Todo for testing
    fn mock_todo(id: &str, title: &str) -> Todo {
        use crate::todo_types::TodoStatus;
        use chrono::Utc;
        Todo {
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            status: TodoStatus::Todo,
            priority: Some(3),
            due_date: None,
            tags: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            completed_at: None,
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            parent_id: None,
            project_id: None,
            attachments: Vec::new(),
            time_entries: Vec::new(),
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            blocked_by: Vec::new(),
            blocks: Vec::new(),
        }
    }

    #[test]
    fn test_rrf_merge_formula() {
        // Test RRF formula: 1/(k + rank)
        let k = 60;
        let keyword = vec![
            SearchResult::Todo(Box::new(mock_todo("1", "First"))),
            SearchResult::Todo(Box::new(mock_todo("2", "Second"))),
        ];
        let semantic = vec![("1".to_string(), 0.9), ("3".to_string(), 0.8)];
        let mut map = HashMap::new();
        map.insert(
            "3".to_string(),
            SearchResult::Todo(Box::new(mock_todo("3", "Third"))),
        );

        let results = rrf_merge(&keyword, &semantic, k, &map);

        // ID "1" appears in both: 1/(60+0+1) + 1/(60+0+1) = 2/61 ≈ 0.0328
        // ID "2" keyword only: 1/(60+1+1) = 1/62 ≈ 0.0161
        // ID "3" semantic only: 1/(60+1+1) = 1/62 ≈ 0.0161

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0.id(), "1"); // Highest score
        assert_eq!(results[0].2, "both");
        assert!((results[0].1 - 0.0328).abs() < 0.001); // Verify RRF score
    }

    #[test]
    fn test_rrf_merge_empty_lists() {
        let keyword: Vec<SearchResult> = vec![];
        let semantic: Vec<(String, f64)> = vec![];
        let map = HashMap::new();

        let results = rrf_merge(&keyword, &semantic, 60, &map);

        assert!(
            results.is_empty(),
            "Empty inputs should return empty results"
        );
    }

    #[test]
    fn test_rrf_merge_single_source() {
        // Test keyword-only
        let keyword = vec![SearchResult::Todo(Box::new(mock_todo("1", "Only keyword")))];
        let semantic = vec![];
        let map = HashMap::new();

        let results = rrf_merge(&keyword, &semantic, 60, &map);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id(), "1");
        assert_eq!(results[0].2, "keyword");

        // Test semantic-only
        let keyword = vec![];
        let semantic = vec![("2".to_string(), 0.9)];
        let mut map = HashMap::new();
        map.insert(
            "2".to_string(),
            SearchResult::Todo(Box::new(mock_todo("2", "Only semantic"))),
        );

        let results = rrf_merge(&keyword, &semantic, 60, &map);

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0.id(), "2");
        assert_eq!(results[0].2, "semantic");
    }

    #[test]
    fn test_rrf_merge_with_overlap() {
        let k = 60;
        let keyword = vec![
            SearchResult::Todo(Box::new(mock_todo("A", "Alpha"))),
            SearchResult::Todo(Box::new(mock_todo("B", "Beta"))),
        ];
        let semantic = vec![
            ("B".to_string(), 0.95), // B appears in both
            ("C".to_string(), 0.85),
        ];
        let mut map = HashMap::new();
        map.insert(
            "C".to_string(),
            SearchResult::Todo(Box::new(mock_todo("C", "Gamma"))),
        );

        let results = rrf_merge(&keyword, &semantic, k, &map);

        // Verify "B" has highest score (appears in both)
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].0.id(), "B");
        assert_eq!(results[0].2, "both");

        // Verify scores are accumulated
        let expected_b_score = 1.0 / (k as f64 + 1.0 + 1.0) + 1.0 / (k as f64 + 0.0 + 1.0);
        assert!((results[0].1 - expected_b_score).abs() < 0.001);
    }
}
