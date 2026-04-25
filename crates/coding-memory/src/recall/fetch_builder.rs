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
        include_causal_graph: bool,
    ) -> common::Result<Vec<FullEntry>> {
        let mut out = Vec::new();
        for id in ids {
            if let Some(fact) = self.fact_repo.get(id).await.map_err(|e| common::KlyntbotError::Storage(format!("get: {e}")))? {
                let metadata = if include_provenance {
                    fact.metadata.as_deref().unwrap_or("{}").parse().unwrap_or_default()
                } else {
                    serde_json::Value::Object(Default::default())
                };
                out.push(FullEntry {
                    id: fact.id.parse().unwrap_or_else(|_| uuid::Uuid::nil()),
                    kind: fact.memory_type.clone(),
                    content: serde_json::json!({
                        "subject": fact.subject,
                        "predicate": fact.predicate,
                        "object": fact.object,
                        "confidence": fact.confidence,
                    }),
                    metadata,
                    causal_edges: if include_causal_graph { Vec::new() } else { Vec::new() },
                    supersedes: None,
                    superseded_by: fact.superseded_by.as_deref().and_then(|s| s.parse().ok()),
                });
                continue;
            }
            if let Some(ep) = self.ep_repo.get(id).await.map_err(|e| common::KlyntbotError::Storage(format!("get: {e}")))? {
                let metadata = if include_provenance {
                    ep.metadata.as_deref().unwrap_or("{}").parse().unwrap_or_default()
                } else {
                    serde_json::Value::Object(Default::default())
                };
                out.push(FullEntry {
                    id: ep.id.parse().unwrap_or_else(|_| uuid::Uuid::nil()),
                    kind: ep.kind.clone().unwrap_or_else(|| "episode".to_string()),
                    content: ep.content.parse().unwrap_or_else(|_| serde_json::json!(ep.content)),
                    metadata,
                    causal_edges: Vec::new(),
                    supersedes: None,
                    superseded_by: None,
                });
            }
        }
        Ok(out)
    }
}
