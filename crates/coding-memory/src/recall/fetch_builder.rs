//! Build `FullEntry` rows — joins fact/episode + provenance + supersede chain.
//! Causal edges are returned empty until Phase 6 wires `memory_causal_edges`.

use crate::recall::FullEntry;
use cognitive::{EpisodicMemoryRepo, SemanticFactRepo};
use std::sync::Arc;

/// Composite fetcher.
#[derive(Clone)]
pub struct FetchBuilder {
    fact_repo: Arc<SemanticFactRepo>,
    ep_repo: Arc<EpisodicMemoryRepo>,
}

impl std::fmt::Debug for FetchBuilder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FetchBuilder").finish()
    }
}

impl FetchBuilder {
    /// Construct.
    #[must_use]
    pub fn new(fact_repo: Arc<SemanticFactRepo>, ep_repo: Arc<EpisodicMemoryRepo>) -> Self {
        Self { fact_repo, ep_repo }
    }

    /// Fetch by ids. Looks up facts first, then episodes for misses.
    pub async fn fetch(
        &self,
        ids: &[String],
        include_provenance: bool,
        _include_causal_graph: bool,
    ) -> common::Result<Vec<FullEntry>> {
        let fact_futs = ids.iter().map(|id| {
            let repo = self.fact_repo.clone();
            let id = id.clone();
            async move {
                repo.get(&id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(format!("get: {e}")))
            }
        });
        let facts = futures::future::try_join_all(fact_futs).await?;

        let mut out = Vec::with_capacity(ids.len());
        let mut ep_misses: Vec<(usize, String)> = Vec::new();
        for (idx, (id, fact)) in ids.iter().zip(facts.into_iter()).enumerate() {
            if let Some(fact) = fact {
                let metadata = if include_provenance {
                    fact.metadata
                        .as_deref()
                        .unwrap_or("{}")
                        .parse()
                        .unwrap_or_default()
                } else {
                    serde_json::Value::Object(Default::default())
                };
                out.push(Some(FullEntry {
                    id: fact.id.parse().unwrap_or_else(|_| uuid::Uuid::nil()),
                    kind: fact.memory_type.clone(),
                    content: serde_json::json!({
                        "subject": fact.subject,
                        "predicate": fact.predicate,
                        "object": fact.object,
                        "confidence": fact.confidence,
                    }),
                    metadata,
                    causal_edges: Vec::new(),
                    supersedes: None,
                    superseded_by: fact.superseded_by.as_deref().and_then(|s| s.parse().ok()),
                }));
            } else {
                out.push(None);
                ep_misses.push((idx, id.clone()));
            }
        }

        let ep_futs = ep_misses.iter().map(|(_, id)| {
            let repo = self.ep_repo.clone();
            let id = id.clone();
            async move {
                repo.get(&id)
                    .await
                    .map_err(|e| common::KlyntbotError::Storage(format!("get: {e}")))
            }
        });
        let eps = futures::future::try_join_all(ep_futs).await?;
        for ((idx, _), ep) in ep_misses.into_iter().zip(eps.into_iter()) {
            let Some(ep) = ep else { continue };
            let metadata = if include_provenance {
                ep.metadata
                    .as_deref()
                    .unwrap_or("{}")
                    .parse()
                    .unwrap_or_default()
            } else {
                serde_json::Value::Object(Default::default())
            };
            out[idx] = Some(FullEntry {
                id: ep.id.parse().unwrap_or_else(|_| uuid::Uuid::nil()),
                kind: ep.kind.clone().unwrap_or_else(|| "episode".to_string()),
                content: ep
                    .content
                    .parse()
                    .unwrap_or_else(|_| serde_json::json!(ep.content)),
                metadata,
                causal_edges: Vec::new(),
                supersedes: None,
                superseded_by: None,
            });
        }

        Ok(out.into_iter().flatten().collect())
    }
}
