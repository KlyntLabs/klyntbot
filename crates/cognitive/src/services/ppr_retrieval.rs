//! KCA Track 6 — Personalized PageRank over the entity graph.
//!
//! Pure algorithm; no LLM. Runs at retrieval time as an additive boost step
//! after embedding + BM25.

use petgraph::graph::{DiGraph, NodeIndex};
use petgraph::visit::EdgeRef;
use std::collections::HashMap;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
pub struct PprConfig {
    /// Teleportation probability — 1-alpha is restart-to-seed weight.
    /// Higher alpha = more exploration. Klynt default: 0.15 (matches HippoRAG).
    pub alpha: f32,
    /// Max iterations before forced stop.
    pub max_iterations: u32,
    /// Convergence tolerance (L1 norm change between iterations).
    pub tolerance: f32,
}

impl Default for PprConfig {
    fn default() -> Self {
        Self {
            alpha: 0.15,
            max_iterations: 30,
            tolerance: 1e-4,
        }
    }
}

/// Compute Personalized PageRank scores for every node in `graph` given seed nodes.
/// Returns a map from node name to score in [0, 1] summing to ~1.0.
pub fn personalized_pagerank(
    graph: &DiGraph<String, f32, u32>,
    seeds: &[NodeIndex],
    cfg: &PprConfig,
) -> HashMap<String, f32> {
    if graph.node_count() == 0 || seeds.is_empty() {
        return HashMap::new();
    }

    let n = graph.node_count();
    let _seed_set: std::collections::HashSet<NodeIndex> = seeds.iter().copied().collect();

    // Initial distribution: uniform over seeds.
    let mut current: Vec<f32> = vec![0.0; n];
    let seed_weight = 1.0 / seeds.len() as f32;
    for &s in seeds {
        current[s.index()] = seed_weight;
    }

    // Personalization vector — same as initial; teleportation pulls toward seeds.
    let personalization: Vec<f32> = current.clone();

    // Precompute out-degree weighted edges.
    let edges_out: Vec<Vec<(NodeIndex, f32)>> = (0..n)
        .map(|i| {
            let idx = NodeIndex::new(i);
            let outgoing: Vec<(NodeIndex, f32)> = graph
                .edges_directed(idx, petgraph::Direction::Outgoing)
                .map(|e| (e.target(), *e.weight()))
                .collect();
            let total: f32 = outgoing.iter().map(|(_, w)| w).sum();
            if total > 0.0 {
                outgoing.into_iter().map(|(t, w)| (t, w / total)).collect()
            } else {
                vec![]
            }
        })
        .collect();

    let mut next = vec![0.0_f32; n];

    for iter in 0..cfg.max_iterations {
        next.iter_mut().for_each(|x| *x = 0.0);

        // Random walk step (with damping (1-alpha)).
        for i in 0..n {
            let mass = current[i];
            if mass == 0.0 {
                continue;
            }
            let neighbors = &edges_out[i];
            if neighbors.is_empty() {
                // Dangling: distribute back to personalization vector.
                for j in 0..n {
                    next[j] += (1.0 - cfg.alpha) * mass * personalization[j];
                }
                continue;
            }
            for &(t, w) in neighbors {
                next[t.index()] += (1.0 - cfg.alpha) * mass * w;
            }
        }

        // Teleportation step: alpha * personalization vector.
        for j in 0..n {
            next[j] += cfg.alpha * personalization[j];
        }

        // Normalize to remove drift from float error.
        let total: f32 = next.iter().sum();
        if total > 0.0 {
            next.iter_mut().for_each(|x| *x /= total);
        }

        // Convergence check.
        let l1: f32 = current
            .iter()
            .zip(next.iter())
            .map(|(a, b)| (a - b).abs())
            .sum();
        std::mem::swap(&mut current, &mut next);
        if l1 < cfg.tolerance {
            tracing::debug!(iter = iter + 1, "ppr converged");
            break;
        }
    }

    let mut out = HashMap::with_capacity(n);
    for i in 0..n {
        out.insert(graph[NodeIndex::new(i)].clone(), current[i]);
    }
    out
}

/// Build a directed graph from `entities` + `entity_relationships` tables.
/// Edge weights are scaled by edge_type (causal=1.5×, structural=1.2×, temporal=1.1×).
pub async fn build_graph_from_entities(
    repo: &crate::repos::EntityRepo,
) -> common::Result<(DiGraph<String, f32, u32>, HashMap<String, NodeIndex>)> {
    let mut g: DiGraph<String, f32, u32> = DiGraph::new();
    let mut idx_by_id: HashMap<String, NodeIndex> = HashMap::new();
    let mut name_by_idx: HashMap<String, NodeIndex> = HashMap::new();

    let entities = repo
        .list_all_entities(5000)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    for e in &entities {
        let i = g.add_node(e.name.clone());
        idx_by_id.insert(e.id.clone(), i);
        name_by_idx.insert(e.name.to_lowercase(), i);
    }

    let edges = repo
        .list_all_edges(20000)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
    for edge in edges {
        let src = match idx_by_id.get(&edge.source_id) {
            Some(i) => *i,
            None => continue,
        };
        let tgt = match idx_by_id.get(&edge.target_id) {
            Some(i) => *i,
            None => continue,
        };
        let multiplier = crate::repos::entity::EdgeType::parse(&edge.edge_type).weight() as f32;
        g.add_edge(src, tgt, edge.strength as f32 * multiplier);
        // Add reverse for undirected expansion.
        g.add_edge(tgt, src, edge.strength as f32 * multiplier * 0.5);
    }

    Ok((g, name_by_idx))
}

type PprCacheEntry = (DiGraph<String, f32, u32>, HashMap<String, NodeIndex>, Instant);

pub struct CachedPprGraph {
    repo: crate::repos::EntityRepo,
    cache: RwLock<Option<PprCacheEntry>>,
    ttl: Duration,
}

impl CachedPprGraph {
    pub fn new(repo: crate::repos::EntityRepo, ttl: Duration) -> Self {
        Self {
            repo,
            cache: RwLock::new(None),
            ttl,
        }
    }

    pub async fn get_or_rebuild(
        &self,
    ) -> common::Result<(DiGraph<String, f32, u32>, HashMap<String, NodeIndex>)> {
        {
            let r = self.cache.read().await;
            if let Some((g, n, when)) = r.as_ref() {
                if when.elapsed() < self.ttl {
                    return Ok((g.clone(), n.clone()));
                }
            }
        }
        let (g, n) = build_graph_from_entities(&self.repo).await?;
        let mut w = self.cache.write().await;
        *w = Some((g.clone(), n.clone(), Instant::now()));
        Ok((g, n))
    }
}

/// Retrieve facts expanded via PPR from seed entities in the query.
pub async fn retrieve_with_ppr_boost(
    fact_repo: &crate::repos::SemanticFactRepo,
    entity_repo: &crate::repos::EntityRepo,
    cache: &std::sync::Arc<CachedPprGraph>,
    seed_query: &str,
    limit: usize,
) -> common::Result<Vec<crate::services::retrieval::ScoredFact>> {
    // 1. Find seed entities by name match (cheap).
    let seed_names = extract_entity_names_from_query(seed_query);
    let mut seeds: Vec<NodeIndex> = Vec::new();
    let (graph, name_to_idx) = cache.get_or_rebuild().await?;
    for name in &seed_names {
        if let Some(&i) = name_to_idx.get(name) {
            seeds.push(i);
        }
    }
    if seeds.is_empty() {
        // Fall through to flat retrieval.
        return Ok(Vec::new());
    }

    // 2. PPR.
    let scores = personalized_pagerank(&graph, &seeds, &PprConfig::default());

    // 3. Top-K entities by PPR.
    let mut by_score: Vec<(String, f32)> = scores.into_iter().collect();
    by_score.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    let top_entities: Vec<String> = by_score
        .iter()
        .take(limit * 3)
        .map(|(n, _)| n.clone())
        .collect();

    // 4. Fetch facts for these entities.
    let mut out = Vec::new();
    for name in top_entities {
        if let Ok(nodes) = entity_repo.find_by_name(&name).await {
            if let Some(node) = nodes.first() {
                let facts = fact_repo
                    .find_facts_by_entity_id(&node.id, 5)
                    .await
                    .unwrap_or_default();
                let ppr_score = by_score
                    .iter()
                    .find(|(n, _)| n == &name)
                    .map(|(_, s)| *s)
                    .unwrap_or(0.0);
                for f in facts {
                    out.push(crate::services::retrieval::ScoredFact {
                        fact: f,
                        score: ppr_score as f64,
                        similarity: None,
                    });
                }
            }
        }
    }

    // 5. Dedup by fact.id, keep top-N.
    out.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| seen.insert(s.fact.id.clone()));
    out.truncate(limit);
    Ok(out)
}

fn extract_entity_names_from_query(query: &str) -> Vec<String> {
    // Heuristic: split on whitespace, keep tokens of length ≥3 that start with letter.
    query
        .split_whitespace()
        .filter(|t| t.len() >= 3 && t.chars().next().is_some_and(|c| c.is_alphabetic()))
        .map(|t| {
            t.trim_matches(|c: char| !c.is_alphanumeric())
                .to_lowercase()
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::entity::{EntityRepo, NewEntity};
    use storage::StoragePool;

    #[tokio::test]
    async fn build_graph_from_entity_repo() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migrations = crate::repos::cognitive_migrations();
        StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();
        let repo = EntityRepo::new(pool.inner().clone());
        let a = repo
            .upsert_entity(&NewEntity {
                name: "A".into(),
                entity_type: "concept".into(),
                description: None,
                source: "t".into(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();
        let b = repo
            .upsert_entity(&NewEntity {
                name: "B".into(),
                entity_type: "concept".into(),
                description: None,
                source: "t".into(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();
        let c = repo
            .upsert_entity(&NewEntity {
                name: "C".into(),
                entity_type: "concept".into(),
                description: None,
                source: "t".into(),
                source_id: None,
                metadata: None,
            })
            .await
            .unwrap();
        repo.upsert_relationship_typed(&a.id, &b.id, "x", "causal", 0.9, None, "t")
            .await
            .unwrap();
        repo.upsert_relationship_typed(&b.id, &c.id, "y", "correlational", 0.5, None, "t")
            .await
            .unwrap();

        let (graph, name_to_idx) = build_graph_from_entities(&repo).await.unwrap();
        assert_eq!(graph.node_count(), 3);
        assert!(graph.edge_count() >= 2);
        assert!(
            name_to_idx.contains_key("a"),
            "name_to_idx keys are lowercase"
        );
        // Causal weighted higher than correlational by edge_type:
        let a_idx = name_to_idx["a"];
        let b_idx = name_to_idx["b"];
        let edge = graph.find_edge(a_idx, b_idx).unwrap();
        let w = *graph.edge_weight(edge).unwrap();
        assert!(
            w >= 0.9 * 1.5,
            "causal weight {} should be ≥ 0.9 * 1.5 (causal multiplier)",
            w
        );
    }

    #[tokio::test]
    async fn cached_graph_reuses_within_ttl() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let migrations = crate::repos::cognitive_migrations();
        StoragePool::run_feature_migrations(pool.inner(), &migrations)
            .await
            .unwrap();
        let repo = EntityRepo::new(pool.inner().clone());
        repo.upsert_entity(&NewEntity {
            name: "A".into(),
            entity_type: "c".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
        let cache = CachedPprGraph::new(repo.clone(), Duration::from_secs(60));
        let (g1, _) = cache.get_or_rebuild().await.unwrap();

        // Insert another entity; cache should NOT reflect it (within TTL).
        repo.upsert_entity(&NewEntity {
            name: "B".into(),
            entity_type: "c".into(),
            description: None,
            source: "t".into(),
            source_id: None,
            metadata: None,
        })
        .await
        .unwrap();
        let (g2, _) = cache.get_or_rebuild().await.unwrap();
        assert_eq!(
            g1.node_count(),
            g2.node_count(),
            "cache should still report stale view"
        );
    }
}
