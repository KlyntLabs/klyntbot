//! `QueryRewriter` — PRF + multi-query expansion (3 rewrites, RRF-merge).

use super::*;
use async_trait::async_trait;
use std::collections::HashSet;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Async retrieval callback — caller injects the host service's retrieve fn.
pub type RetrieveFn = Arc<
    dyn Fn(
            String,
        ) -> Pin<
            Box<dyn std::future::Future<Output = common::Result<(Vec<f32>, Vec<Uuid>)>> + Send>,
        > + Send
        + Sync,
>;

/// Skill instance.
pub struct QueryRewriter {
    retrieve: RetrieveFn,
}

impl std::fmt::Debug for QueryRewriter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryRewriter").finish()
    }
}

impl QueryRewriter {
    /// Construct with the host's retrieval closure.
    #[must_use]
    pub fn new(retrieve: RetrieveFn) -> Self {
        Self { retrieve }
    }
}

#[async_trait]
impl RetrievalSkill for QueryRewriter {
    fn name(&self) -> &'static str {
        "query_rewriter"
    }
    fn description(&self) -> &'static str {
        "PRF + multi-query expansion."
    }
    fn tier(&self) -> BudgetTier {
        BudgetTier::DeepThink
    }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let rewrites = generate_rewrites(&ctx.query);
        let mut id_to_rank_sum: std::collections::HashMap<Uuid, f32> = Default::default();
        let mut sims: Vec<f32> = Vec::new();
        let mut all_ids: HashSet<Uuid> = HashSet::new();
        for q in rewrites {
            let (s, ids) = (self.retrieve)(q).await?;
            sims.extend(s.iter().copied());
            for (rank, id) in ids.iter().enumerate() {
                let rrf = 1.0_f32 / (60.0 + rank as f32);
                *id_to_rank_sum.entry(*id).or_default() += rrf;
                all_ids.insert(*id);
            }
        }
        let mut merged: Vec<(Uuid, f32)> = id_to_rank_sum.into_iter().collect();
        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let added_ids: Vec<Uuid> = merged.into_iter().take(10).map(|(id, _)| id).collect();
        let coverage_after = if sims.is_empty() {
            0.0
        } else {
            let mean: f32 = sims.iter().sum::<f32>() / sims.len() as f32;
            let min: f32 = sims.iter().cloned().fold(f32::INFINITY, f32::min);
            mean - min
        };
        Ok(EscalationOutcome {
            succeeded: coverage_after > ctx.coverage_score + 0.05,
            coverage_after,
            added_context: format!(
                "Query rewriter merged {} ids across rewrites.",
                added_ids.len()
            ),
            added_ids,
        })
    }
}

fn generate_rewrites(q: &str) -> Vec<String> {
    let stop: HashSet<&str> = ["the", "a", "an", "in", "of", "to", "for", "is"]
        .into_iter()
        .collect();
    // Rewrite 1: original.
    let mut out = vec![q.to_string()];
    // Rewrite 2: stopword-stripped.
    let stripped: String = q
        .split_whitespace()
        .filter(|w| !stop.contains(*w))
        .collect::<Vec<_>>()
        .join(" ");
    out.push(if stripped.is_empty() {
        q.to_string()
    } else {
        stripped
    });
    // Rewrite 3: synonym-expanded.
    let syn = expand_synonyms(q);
    out.push(syn);
    out
}

fn expand_synonyms(q: &str) -> String {
    let table: &[(&str, &[&str])] = &[
        ("bug", &["defect", "issue"]),
        ("null", &["nil", "none"]),
        ("fix", &["patch", "resolve"]),
        ("error", &["fault", "failure"]),
    ];
    let mut s = q.to_lowercase();
    for (k, syns) in table {
        if s.contains(k) {
            for syn in *syns {
                s.push(' ');
                s.push_str(syn);
            }
        }
    }
    s
}
