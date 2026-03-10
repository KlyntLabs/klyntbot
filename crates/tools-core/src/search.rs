//! Generic search utilities — Searchable trait and Reciprocal Rank Fusion (RRF).
//!
//! Provides reusable search infrastructure for merging ranked lists from
//! multiple sources (keyword, semantic, etc.) using the RRF algorithm.

use std::collections::HashMap;

/// Trait for items that can be identified in search results.
pub trait Searchable {
    /// Return the unique identifier for this item.
    fn search_id(&self) -> &str;
}

/// Triple-source Reciprocal Rank Fusion: keyword + semantic + BM25.
///
/// Like `rrf_merge` but with a third BM25 signal. BM25 results are
/// `(id, score)` pairs where score is the negated FTS5 rank.
pub fn rrf_merge_triple<T: Searchable + Clone>(
    keyword_results: &[T],
    semantic_results: &[(String, f64)],
    bm25_results: &[(String, f64)],
    k: u32,
    items_by_id: &HashMap<String, T>,
) -> Vec<(T, f64, &'static str)> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut sources: HashMap<String, u8> = HashMap::new(); // bitmask: 1=keyword, 2=semantic, 4=bm25

    for (rank, result) in keyword_results.iter().enumerate() {
        let id = result.search_id().to_string();
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        *sources.entry(id).or_insert(0) |= 1;
    }

    for (rank, (id, _sim)) in semantic_results.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        *sources.entry(id.clone()).or_insert(0) |= 2;
    }

    for (rank, (id, _score)) in bm25_results.iter().enumerate() {
        *scores.entry(id.clone()).or_insert(0.0) += 1.0 / (k as f64 + rank as f64 + 1.0);
        *sources.entry(id.clone()).or_insert(0) |= 4;
    }

    let mut ranked: Vec<_> = scores.into_iter().collect();
    ranked.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    ranked
        .into_iter()
        .filter_map(|(id, score)| {
            let item = keyword_results
                .iter()
                .find(|r| r.search_id() == id)
                .cloned()
                .or_else(|| items_by_id.get(&id).cloned());

            let source_bits = sources.get(&id).copied().unwrap_or(0);
            let source = match source_bits {
                7 => "all",
                6 => "semantic+bm25",
                5 => "keyword+bm25",
                4 => "bm25",
                3 => "both",     // keyword+semantic (backward compat)
                2 => "semantic",
                1 => "keyword",
                _ => "unknown",
            };

            item.map(|i| (i, score, source))
        })
        .collect()
}

/// Reciprocal Rank Fusion: merge ranked lists from multiple sources.
///
/// RRF formula: `score(d) = sum(1 / (k + rank_i + 1))` for each list where `d` appears.
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
pub fn rrf_merge<T: Searchable + Clone>(
    keyword_results: &[T],
    semantic_results: &[(String, f64)],
    k: u32,
    items_by_id: &HashMap<String, T>,
) -> Vec<(T, f64, &'static str)> {
    let mut scores: HashMap<String, f64> = HashMap::new();
    let mut sources: HashMap<String, u8> = HashMap::new(); // bitmask: 1=keyword, 2=semantic

    // Process keyword results (rank-based scoring)
    for (rank, result) in keyword_results.iter().enumerate() {
        let id = result.search_id().to_string();
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
                .find(|r| r.search_id() == id)
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
