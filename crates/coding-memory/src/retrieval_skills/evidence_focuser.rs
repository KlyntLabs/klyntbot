//! `EvidenceFocuser` — top-20 candidates → token-cosine rerank → top 5.

use super::*;
use async_trait::async_trait;
use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use uuid::Uuid;

/// Closure returning a list of `(id, text)` for the supplied candidate ids.
pub type FetchTextsFn = Arc<
    dyn Fn(
            Vec<Uuid>,
        )
            -> Pin<Box<dyn std::future::Future<Output = common::Result<Vec<(Uuid, String)>>> + Send>>
        + Send
        + Sync,
>;

/// Closure returning the initial top-20 ids for the active query.
pub type InitialIdsFn = Arc<dyn Fn() -> Vec<Uuid> + Send + Sync>;

/// Skill.
pub struct EvidenceFocuser {
    initial: InitialIdsFn,
    fetch_texts: FetchTextsFn,
}

impl std::fmt::Debug for EvidenceFocuser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EvidenceFocuser").finish()
    }
}

impl EvidenceFocuser {
    /// Construct.
    #[must_use]
    pub fn new(initial: InitialIdsFn, fetch_texts: FetchTextsFn) -> Self {
        Self {
            initial,
            fetch_texts,
        }
    }
}

#[async_trait]
impl RetrievalSkill for EvidenceFocuser {
    fn name(&self) -> &'static str {
        "evidence_focuser"
    }
    fn description(&self) -> &'static str {
        "Token-cosine rerank on top-20 → top 5."
    }
    fn tier(&self) -> BudgetTier {
        BudgetTier::DeepThink
    }
    async fn apply(&self, ctx: &EscalationContext) -> common::Result<EscalationOutcome> {
        let candidate_ids = (self.initial)();
        if candidate_ids.is_empty() {
            return Ok(EscalationOutcome {
                succeeded: false,
                coverage_after: ctx.coverage_score,
                added_context: String::new(),
                added_ids: vec![],
            });
        }
        let texts = (self.fetch_texts)(candidate_ids.clone()).await?;
        let q_vec = bag(&ctx.query);
        let mut scored: Vec<(Uuid, f32)> = texts
            .into_iter()
            .map(|(id, t)| (id, cosine(&q_vec, &bag(&t))))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        let top5: Vec<Uuid> = scored.iter().take(5).map(|(id, _)| *id).collect();
        let coverage_after = if scored.is_empty() {
            0.0
        } else {
            let top: Vec<f32> = scored.iter().take(5).map(|(_, s)| *s).collect();
            let mean: f32 = top.iter().sum::<f32>() / top.len() as f32;
            let min = top.iter().cloned().fold(f32::INFINITY, f32::min);
            mean - min
        };
        Ok(EscalationOutcome {
            succeeded: coverage_after > ctx.coverage_score + 0.05,
            coverage_after,
            added_context: format!("Focused {} → 5 candidates.", candidate_ids.len()),
            added_ids: top5,
        })
    }
}

fn bag(s: &str) -> HashMap<String, f32> {
    let mut m: HashMap<String, f32> = HashMap::new();
    for tok in s.to_lowercase().split_whitespace() {
        *m.entry(tok.to_string()).or_default() += 1.0;
    }
    m
}

fn cosine(a: &HashMap<String, f32>, b: &HashMap<String, f32>) -> f32 {
    let mut dot = 0.0;
    for (k, v) in a {
        if let Some(bv) = b.get(k) {
            dot += v * bv;
        }
    }
    let na: f32 = a.values().map(|v| v * v).sum::<f32>().sqrt();
    let nb: f32 = b.values().map(|v| v * v).sum::<f32>().sqrt();
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na * nb)
}
