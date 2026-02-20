//! Search utilities for the feature-todo crate.
//!
//! Provides keyword and hybrid (RRF) search for todos.

use std::collections::HashMap;

use crate::types::Todo;
use tools_core::rrf_merge;

/// Merge keyword results and semantic results into an RRF-ranked list.
///
/// Returns Vec<(Todo, rrf_score, source)> sorted by score descending.
pub fn hybrid_merge(
    keyword_results: &[Todo],
    semantic_results: &[(String, f64)],
    rrf_k: u32,
) -> Vec<(Todo, f64, &'static str)> {
    let items_by_id: HashMap<String, Todo> = keyword_results
        .iter()
        .map(|t| (t.id.clone(), t.clone()))
        .collect();
    rrf_merge(keyword_results, semantic_results, rrf_k, &items_by_id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Todo;
    

    fn make_todo(id: &str, title: &str) -> Todo {
        let mut t = Todo::default_instance();
        t.id = id.to_string();
        t.title = title.to_string();
        t
    }

    #[test]
    fn test_hybrid_merge_keyword_only() {
        let todos = vec![make_todo("a1b2c3d4", "fix auth bug")];
        let semantic: Vec<(String, f64)> = vec![];
        let merged = hybrid_merge(&todos, &semantic, 60);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].2, "keyword");
    }

    #[test]
    fn test_hybrid_merge_semantic_only() {
        let todos: Vec<Todo> = vec![];
        let semantic = vec![("a1b2c3d4".to_string(), 0.9)];
        let merged = hybrid_merge(&todos, &semantic, 60);
        // No items in keyword results and no lookup map, so nothing is returned
        assert_eq!(merged.len(), 0);
    }

    #[test]
    fn test_hybrid_merge_both_sources() {
        let todo = make_todo("a1b2c3d4", "auth bug");
        let todos = vec![todo.clone()];
        let semantic = vec![(todo.id.clone(), 0.85)];
        let merged = hybrid_merge(&todos, &semantic, 60);
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].2, "both");
    }

    #[test]
    fn test_rrf_score_ordering() {
        let t1 = make_todo("aaaa0001", "task one");
        let t2 = make_todo("aaaa0002", "task two");
        let todos = vec![t1.clone(), t2.clone()];
        // t2 appears in semantic but t1 doesn't
        let semantic = vec![(t2.id.clone(), 0.9)];
        let merged = hybrid_merge(&todos, &semantic, 60);
        // t2 should score higher (both keyword and semantic)
        assert!(merged[0].1 >= merged[1].1);
    }
}
