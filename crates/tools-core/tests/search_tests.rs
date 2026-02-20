//! Tests for generic `Searchable` trait and `rrf_merge<T>` (AC 1.8).

use std::collections::HashMap;
use tools_core::{rrf_merge, Searchable};

// --- Test type implementing Searchable ---

#[derive(Debug, Clone, PartialEq)]
struct TestItem {
    id: String,
    title: String,
}

impl Searchable for TestItem {
    fn search_id(&self) -> &str {
        &self.id
    }
}

fn item(id: &str, title: &str) -> TestItem {
    TestItem {
        id: id.to_string(),
        title: title.to_string(),
    }
}

// ============================================================
// AC 1.8: rrf_merge with items in both lists
// ============================================================

#[test]
fn test_rrf_merge_items_in_both_lists() {
    let keyword = vec![item("a", "Alpha"), item("b", "Beta")];
    let semantic = vec![("a".to_string(), 0.95), ("c".to_string(), 0.80)];
    let mut map = HashMap::new();
    map.insert("c".to_string(), item("c", "Gamma"));

    let results = rrf_merge(&keyword, &semantic, 60, &map);

    assert_eq!(results.len(), 3);
    // Item "a" appears in both, should have highest score
    assert_eq!(results[0].0.id, "a");
    assert_eq!(results[0].2, "both");
}

// ============================================================
// AC 1.8: rrf_merge with items in only one list
// ============================================================

#[test]
fn test_rrf_merge_items_in_only_keyword() {
    let keyword = vec![item("a", "Alpha"), item("b", "Beta")];
    let semantic: Vec<(String, f64)> = vec![];
    let map = HashMap::new();

    let results = rrf_merge(&keyword, &semantic, 60, &map);

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].2, "keyword");
    assert_eq!(results[1].2, "keyword");
}

#[test]
fn test_rrf_merge_items_in_only_semantic() {
    let keyword: Vec<TestItem> = vec![];
    let semantic = vec![("a".to_string(), 0.9), ("b".to_string(), 0.7)];
    let mut map = HashMap::new();
    map.insert("a".to_string(), item("a", "Alpha"));
    map.insert("b".to_string(), item("b", "Beta"));

    let results = rrf_merge(&keyword, &semantic, 60, &map);

    assert_eq!(results.len(), 2);
    assert_eq!(results[0].2, "semantic");
    assert_eq!(results[1].2, "semantic");
}

// ============================================================
// AC 1.8: rrf_merge with empty lists
// ============================================================

#[test]
fn test_rrf_merge_both_empty() {
    let keyword: Vec<TestItem> = vec![];
    let semantic: Vec<(String, f64)> = vec![];
    let map: HashMap<String, TestItem> = HashMap::new();

    let results = rrf_merge(&keyword, &semantic, 60, &map);
    assert!(results.is_empty());
}

// ============================================================
// AC 1.8: rrf_merge score calculation correctness
// ============================================================

#[test]
fn test_rrf_merge_score_calculation() {
    let k: u32 = 60;
    let keyword = vec![item("a", "Alpha")];
    let semantic = vec![("a".to_string(), 0.95)];
    let map = HashMap::new();

    let results = rrf_merge(&keyword, &semantic, k, &map);

    // item "a" at keyword rank 0, semantic rank 0:
    // score = 1/(60+0+1) + 1/(60+0+1) = 2/61 ≈ 0.0328
    let expected = 2.0 / 61.0;
    assert!((results[0].1 - expected).abs() < 0.001);
}

#[test]
fn test_rrf_merge_results_sorted_by_score_descending() {
    let keyword = vec![item("a", "Alpha"), item("b", "Beta")];
    let semantic = vec![("b".to_string(), 0.9), ("c".to_string(), 0.8)];
    let mut map = HashMap::new();
    map.insert("c".to_string(), item("c", "Gamma"));

    let results = rrf_merge(&keyword, &semantic, 60, &map);

    // "b" appears in both (keyword rank 1, semantic rank 0) -> highest combined score
    // "a" keyword only (rank 0) -> 1/61
    // "c" semantic only (rank 1) -> 1/62
    for i in 1..results.len() {
        assert!(results[i - 1].1 >= results[i].1, "Results not sorted descending");
    }
}

#[test]
fn test_rrf_merge_custom_k_value() {
    let keyword = vec![item("a", "Alpha")];
    let semantic: Vec<(String, f64)> = vec![];
    let map = HashMap::new();

    let results_k10 = rrf_merge(&keyword, &semantic, 10, &map);
    let results_k60 = rrf_merge(&keyword, &semantic, 60, &map);

    // k=10: score = 1/(10+0+1) = 1/11 ≈ 0.0909
    // k=60: score = 1/(60+0+1) = 1/61 ≈ 0.0164
    assert!(results_k10[0].1 > results_k60[0].1);
}

// ============================================================
// AC 1.8: Searchable trait implementation
// ============================================================

#[test]
fn test_searchable_trait_search_id() {
    let item = TestItem {
        id: "abc".to_string(),
        title: "Test".to_string(),
    };
    assert_eq!(item.search_id(), "abc");
}

#[test]
fn test_searchable_trait_is_object_safe() {
    fn accepts_searchable<T: Searchable>(item: &T) -> &str {
        item.search_id()
    }
    let item = TestItem {
        id: "xyz".to_string(),
        title: "Test".to_string(),
    };
    assert_eq!(accepts_searchable(&item), "xyz");
}
