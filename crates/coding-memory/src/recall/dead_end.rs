//! Dead-end checker — Tier B1 counterfactual matching.
//!
//! Given a candidate approach phrase, queries `semantic_facts` filtered by
//! `metadata.memory_type = 'counterfactual'` (and optional repo scope), runs
//! similarity scoring, and surfaces matches above a confidence floor.

use crate::recall::{DeadEndMatch, DeadEndResponse};
use cognitive::SemanticFactRepo;
use std::sync::Arc;

/// Tunables.
#[derive(Debug, Clone, Copy)]
pub struct DeadEndConfig {
    /// Per-match confidence floor.
    pub match_threshold: f32,
    /// Maximum matches to return.
    pub limit: usize,
}

impl Default for DeadEndConfig {
    fn default() -> Self {
        Self {
            match_threshold: 0.7,
            limit: 5,
        }
    }
}

/// Counterfactual match service.
#[derive(Debug, Clone)]
pub struct DeadEndChecker {
    fact_repo: Arc<SemanticFactRepo>,
    config: DeadEndConfig,
}

impl DeadEndChecker {
    /// Construct.
    #[must_use]
    pub fn new(fact_repo: Arc<SemanticFactRepo>, config: DeadEndConfig) -> Self {
        Self { fact_repo, config }
    }

    /// Match an approach against stored counterfactuals.
    pub async fn check(
        &self,
        approach: &str,
        repo: Option<&str>,
    ) -> common::Result<DeadEndResponse> {
        let candidates = self
            .fact_repo
            .list_by_memory_type("counterfactual", repo, 50)
            .await
            .map_err(|e| common::KlyntbotError::Storage(format!("list_by_memory_type: {e}")))?;
        let approach_lower = approach.to_lowercase();
        let approach_tokens: std::collections::HashSet<&str> =
            approach_lower.split_whitespace().collect();
        let mut matches: Vec<(f32, DeadEndMatch)> = Vec::new();
        for fact in candidates {
            let payload = format!("{} {}", fact.subject, fact.object);
            let payload_lower = payload.to_lowercase();
            let payload_tokens: std::collections::HashSet<&str> =
                payload_lower.split_whitespace().collect();
            let inter = approach_tokens.intersection(&payload_tokens).count() as f32;
            let union = approach_tokens.union(&payload_tokens).count().max(1) as f32;
            let jaccard = inter / union;
            let confidence = jaccard * fact.confidence as f32;
            if confidence < self.config.match_threshold {
                continue;
            }
            let meta: serde_json::Value = fact
                .metadata
                .as_ref()
                .and_then(|m| serde_json::from_str(m).ok())
                .unwrap_or_default();
            let problem_hash = meta
                .get("problem_hash")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let reason = meta
                .get("reason")
                .and_then(|v| v.as_str())
                .unwrap_or(&fact.object)
                .to_string();
            let attempt_id = meta
                .get("attempt_id")
                .and_then(|v| v.as_str())
                .and_then(|s| s.parse().ok())
                .unwrap_or_else(|| fact.id.parse().unwrap_or_else(|_| uuid::Uuid::nil()));
            let when = fact
                .recorded_at
                .parse()
                .unwrap_or_else(|_| jiff::Timestamp::now());
            matches.push((
                confidence,
                DeadEndMatch {
                    attempt_id,
                    problem_hash,
                    approach: fact.subject.clone(),
                    reason,
                    when,
                },
            ));
        }
        matches.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        matches.truncate(self.config.limit);
        let aggregate_confidence = matches.iter().map(|(c, _)| *c).fold(0.0f32, f32::max);
        Ok(DeadEndResponse {
            matches: matches.into_iter().map(|(_, m)| m).collect(),
            aggregate_confidence,
        })
    }
}
