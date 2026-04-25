//! `QueryDecomposer` — split compound queries into 2-4 sub-queries; merge via RRF.

use super::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Same retrieval callback shape as `QueryRewriter`.
pub type RetrieveFn = Arc<
    dyn Fn(
            String,
        ) -> Pin<
            Box<dyn std::future::Future<Output = common::Result<(Vec<f32>, Vec<Uuid>)>> + Send>,
        > + Send
        + Sync,
>;

/// Skill.
pub struct QueryDecomposer {
    retrieve: RetrieveFn,
}

impl std::fmt::Debug for QueryDecomposer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("QueryDecomposer").finish()
    }
}

impl QueryDecomposer {
    /// Construct.
    #[must_use]
    pub fn new(retrieve: RetrieveFn) -> Self {
        Self { retrieve }
    }
}

#[async_trait]
impl RetrievalSkill for QueryDecomposer {
    fn name(&self) -> &'static str {
        "query_decomposer"
    }
    fn description(&self) -> &'static str {
        "Split compound queries into 2-4 sub-queries."
    }
    fn tier(&self) -> BudgetTier {
        BudgetTier::DeepThink
    }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let subs = decompose(&ctx.query);
        let mut id_rank: HashMap<Uuid, f32> = HashMap::new();
        let mut sims_all = Vec::new();
        for q in &subs {
            let (sims, ids) = (self.retrieve)(q.clone()).await?;
            sims_all.extend(sims);
            for (rank, id) in ids.iter().enumerate() {
                let rrf = 1.0_f32 / (60.0 + rank as f32);
                *id_rank.entry(*id).or_default() += rrf;
            }
        }
        let mut merged: Vec<(Uuid, f32)> = id_rank.into_iter().collect();
        merged.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let added_ids: Vec<Uuid> = merged.into_iter().take(10).map(|(id, _)| id).collect();
        let coverage_after = if sims_all.is_empty() {
            0.0
        } else {
            let mean: f32 = sims_all.iter().sum::<f32>() / sims_all.len() as f32;
            let min: f32 = sims_all.iter().cloned().fold(f32::INFINITY, f32::min);
            mean - min
        };
        Ok(EscalationOutcome {
            succeeded: coverage_after > ctx.coverage_score + 0.05,
            coverage_after,
            added_context: format!("Decomposed into {} sub-queries.", subs.len()),
            added_ids,
        })
    }
}

fn decompose(q: &str) -> Vec<String> {
    // Split on " and "/" ; "/", " then ", " or ".
    let lowered = q.to_lowercase();
    let separators = [" and ", "; ", ", ", " then ", " or "];
    let mut parts: Vec<String> = vec![lowered.clone()];
    for sep in separators {
        parts = parts
            .into_iter()
            .flat_map(|s| {
                s.split(sep)
                    .map(|t| t.trim().to_string())
                    .filter(|t| !t.is_empty())
                    .collect::<Vec<_>>()
            })
            .collect();
    }
    if parts.len() < 2 {
        return vec![q.to_string()];
    }
    parts.truncate(4);
    parts
}
