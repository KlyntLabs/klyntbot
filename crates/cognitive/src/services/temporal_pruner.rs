//! KCA Track 13 — temporal pruning at retrieval time.

use serde::{Deserialize, Serialize};
use crate::services::retrieval::ScoredFact;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneInput {
    pub facts: Vec<PruneFactRef>,
    pub query_time: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PruneFactRef {
    pub fact_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub valid_at: String,
    pub valid_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PruneOutput {
    pub keep: Vec<String>,
    pub drop: Vec<DropDecision>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DropDecision {
    pub fact_id: String,
    pub reason: String,
}

#[async_trait::async_trait]
pub trait TemporalPrunerHandler: Send + Sync {
    async fn prune(&self, input: PruneInput) -> common::Result<PruneOutput>;
}

pub struct NoopTemporalPruner;

#[async_trait::async_trait]
impl TemporalPrunerHandler for NoopTemporalPruner {
    async fn prune(&self, input: PruneInput) -> common::Result<PruneOutput> {
        Ok(PruneOutput { keep: input.facts.iter().map(|f| f.fact_id.clone()).collect(), drop: vec![] })
    }
}

/// Filter scored facts by the pruner's keep list.
pub fn apply_prune(facts: Vec<ScoredFact>, output: &PruneOutput) -> Vec<ScoredFact> {
    let drop_set: std::collections::HashSet<&str> = output.drop.iter().map(|d| d.fact_id.as_str()).collect();
    facts.into_iter().filter(|s| !drop_set.contains(s.fact.id.as_str())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SemanticFact;

    #[test]
    fn apply_prune_filters_dropped_facts() {
        let f1 = ScoredFact { fact: SemanticFact {
            id: "f1".into(), domain: "test".into(), subject: "a".into(), predicate: "p".into(),
            object: "b".into(), confidence: 0.5, source: "t".into(), valid_from: "2026-01-01".into(),
            valid_until: None, recorded_at: "2026-01-01".into(), superseded_at: None, superseded_by: None,
            stability: 1.0, last_accessed: None, access_count: 0, convergence_score: 0.0,
            project_id: None, memory_type: "fact".into(), scope_type: "system".into(),
            scope_id: None, scope_repo_id: None, metadata: None,
        }, score: 0.7, similarity: None };
        let f2 = ScoredFact { fact: SemanticFact {
            id: "f2".into(), domain: "test".into(), subject: "c".into(), predicate: "p".into(),
            object: "d".into(), confidence: 0.5, source: "t".into(), valid_from: "2026-01-01".into(),
            valid_until: None, recorded_at: "2026-01-01".into(), superseded_at: None, superseded_by: None,
            stability: 1.0, last_accessed: None, access_count: 0, convergence_score: 0.0,
            project_id: None, memory_type: "fact".into(), scope_type: "system".into(),
            scope_id: None, scope_repo_id: None, metadata: None,
        }, score: 0.6, similarity: None };

        let out = PruneOutput {
            keep: vec!["f2".into()],
            drop: vec![DropDecision { fact_id: "f1".into(), reason: "stale".into() }],
        };

        let kept = apply_prune(vec![f1, f2], &out);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].fact.id, "f2");
    }
}
