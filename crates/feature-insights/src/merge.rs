//! Smart Merge Engine — deduplication and parent insight detection.
//!
//! Before generating a new insight, the merge engine checks:
//! 1. Exact hash match (same content → return cached)
//! 2. Scope overlap via Jaccard similarity (> threshold → set as parent)
//!
//! When a parent is found, the prompt builder injects the parent's synthesis
//! so the LLM focuses on what's new or different.

use std::collections::HashSet;

use crate::repo::InsightReviewRepo;
use crate::types::{InsightReviewRow, ScopeConfig};

/// Computes Jaccard similarity between two sets of note IDs.
pub fn scope_overlap(scope_a: &[String], scope_b: &[String]) -> f64 {
    let set_a: HashSet<&str> = scope_a.iter().map(|s| s.as_str()).collect();
    let set_b: HashSet<&str> = scope_b.iter().map(|s| s.as_str()).collect();
    let intersection = set_a.intersection(&set_b).count();
    let union = set_a.union(&set_b).count();
    if union == 0 {
        return 0.0;
    }
    intersection as f64 / union as f64
}

/// Result of a merge check.
#[derive(Debug)]
pub struct MergeResult {
    /// If Some, this insight should be the parent (high scope overlap).
    pub parent: Option<InsightReviewRow>,
    /// The Jaccard overlap score with the parent (0.0 if no parent).
    pub overlap_score: f64,
}

/// Engine for detecting overlapping insights and selecting parents.
#[derive(Debug, Clone)]
pub struct SmartMergeEngine {
    repo: InsightReviewRepo,
}

impl SmartMergeEngine {
    pub fn new(repo: InsightReviewRepo) -> Self {
        Self { repo }
    }

    /// Check for a parent insight among existing insights for the scope notes.
    ///
    /// Searches all non-superseded insights whose `note_id` is in `scope_note_ids`,
    /// parses their `scope_config` to extract their scope, and computes Jaccard
    /// overlap with the current scope. Returns the best match if above threshold.
    pub async fn find_parent(
        &self,
        note_id: &str,
        scope_note_ids: &[String],
        merge_threshold: f64,
    ) -> Result<MergeResult, sqlx::Error> {
        if scope_note_ids.is_empty() {
            return Ok(MergeResult {
                parent: None,
                overlap_score: 0.0,
            });
        }

        // Collect candidate insights: latest non-superseded insight for each note in scope
        let mut candidates: Vec<(InsightReviewRow, f64)> = Vec::new();

        for scope_note_id in scope_note_ids {
            if scope_note_id == note_id {
                continue; // Don't match against our own note's insights
            }
            if let Some(row) = self.repo.get_latest(scope_note_id).await? {
                // Parse the stored scope_config to get the note IDs that insight used
                let stored_scope: ScopeConfig = match serde_json::from_str(&row.scope_config) {
                    Ok(sc) => sc,
                    Err(e) => {
                        tracing::warn!(
                            insight_id = %row.id,
                            "malformed scope_config in insight_reviews, skipping: {e}"
                        );
                        continue;
                    }
                };
                let stored_ids = &stored_scope.node_ids;

                let overlap = scope_overlap(scope_note_ids, stored_ids);
                if overlap >= merge_threshold {
                    candidates.push((row, overlap));
                }
            }
        }

        // Pick the best parent: highest overlap, then most recent
        candidates.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.0.generated_at.cmp(&a.0.generated_at))
        });

        match candidates.into_iter().next() {
            Some((row, score)) => Ok(MergeResult {
                parent: Some(row),
                overlap_score: score,
            }),
            None => Ok(MergeResult {
                parent: None,
                overlap_score: 0.0,
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scope_overlap_identical() {
        let a = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let b = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert!((scope_overlap(&a, &b) - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_overlap_partial() {
        let a = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let b = vec!["b".to_string(), "c".to_string(), "d".to_string()];
        // intersection={b,c}=2, union={a,b,c,d}=4 → 0.5
        assert!((scope_overlap(&a, &b) - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_overlap_disjoint() {
        let a = vec!["a".to_string(), "b".to_string()];
        let b = vec!["c".to_string(), "d".to_string()];
        assert!((scope_overlap(&a, &b)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_overlap_empty() {
        let a: Vec<String> = vec![];
        let b: Vec<String> = vec![];
        assert!((scope_overlap(&a, &b)).abs() < f64::EPSILON);
    }

    #[test]
    fn test_scope_overlap_one_empty() {
        let a = vec!["a".to_string()];
        let b: Vec<String> = vec![];
        assert!((scope_overlap(&a, &b)).abs() < f64::EPSILON);
    }
}
