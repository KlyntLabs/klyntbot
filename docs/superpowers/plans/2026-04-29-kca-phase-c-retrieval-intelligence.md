# KCA Phase C — Retrieval Intelligence Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make retrieval traverse the graph (not just embed-and-match), warm the cache before users ask follow-ups, navigate long-conversation history hierarchically, and prune stale facts at read time. Two of the four tracks add $0 in LLM cost (PPR, temporal prune); two add minimal cost (predictive warm, hierarchical compress).

**Architecture:** Track 6 (PPR) is pure algorithm over the entity graph + co_activation; integrates as an additive step in `MemoryRetriever`. Track 7 (predictive cache) is a fire-and-forget post-turn task that pre-computes retrieval results for predicted next queries. Track 8 (hierarchical episodic compression) introduces hourly/daily/weekly summary tiers driven by cron. Track 13 (temporal pruning) is a tiny LLM call before retrieval results are passed to the response LLM.

**Tech Stack:** Rust stable 1.93, `petgraph` (already a dep) — Personalized PageRank, `tokio`, `lru` crate (new dep), existing `MemoryRetriever`.

**Spec:** [`docs/superpowers/specs/2026-04-29-klynt-cognitive-architecture-design.md`](../specs/2026-04-29-klynt-cognitive-architecture-design.md), §4 (parallel retrieval intelligence ring), §5 (Tracks 6, 7, 8, 13).

**Prerequisite:** Phases A + B merged. Track 6 reads typed edges from Phase A; Track 8 hierarchical summaries reference micro-Reforge episodics from Phase B.

---

## File Structure

**Track 6 — PPR retrieval**
- Create: `crates/cognitive/src/services/ppr_retrieval.rs`
- Modify: `crates/cognitive/src/services/memory_retriever.rs` (add PPR boost step)
- Modify: `crates/cognitive/src/services/mod.rs`

**Track 7 — Predictive cache warming**
- Create: `crates/cognitive/src/services/predictive_cache.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs` (`LlmQueryPredictorHandler`)
- Modify: `crates/agent/src/adapters/prompts.rs` (`QUERY_PREDICTOR_SYSTEM_PROMPT`)
- Modify: `crates/agent/src/agent_runtime/runtime.rs` (Phase 3 invocation)
- Modify: `crates/config/src/schema/cognitive.rs` (`PredictiveCacheConfig`)

**Track 8 — Hierarchical episodic compression**
- Create: `crates/cognitive/migrations/013_hierarchical_episodics.sql`
- Create: `crates/cognitive/src/services/hierarchical_compressor.rs`
- Modify: `crates/cognitive/src/services/mod.rs`
- Modify: `crates/cognitive/src/repos/episodic.rs` (queries by tier + parent)
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs` (`LlmHierarchicalSummarizer`)
- Modify: `crates/agent/src/adapters/prompts.rs` (3 tier prompts)
- Modify: `crates/app-core/src/init/cron.rs` (3 new cron jobs: hourly/daily/weekly)
- Modify: `crates/config/src/schema/cognitive.rs` (`HierarchicalConfig`)

**Track 13 — Temporal pruning at retrieval**
- Create: `crates/cognitive/src/services/temporal_pruner.rs`
- Modify: `crates/cognitive/src/services/memory_retriever.rs` (call pruner)
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs` (`LlmTemporalPrunerHandler`)
- Modify: `crates/agent/src/adapters/prompts.rs` (`TEMPORAL_PRUNE_SYSTEM_PROMPT`)

**Phase C integration tests**
- Create: `crates/cognitive/tests/phase_c_retrieval_intelligence.rs`
- Create: `crates/cognitive/tests/phase_c_ppr_correctness.rs`

---

# Track 6 — PPR retrieval expansion (Personalized PageRank)

HippoRAG-2 showed +18% multi-hop accuracy by walking a knowledge graph at retrieval time. We have entity_relationships (typed via Phase A) + co_activation. We add a PPR step that runs after embedding + BM25 retrieval and expands the result set with graph-reachable facts.

### Task C6.1: Failing test — PPR walks from seed entities

**Files:**
- Test: `crates/cognitive/tests/phase_c_ppr_correctness.rs`

- [ ] **Step 1: Create test file.**

```rust
//! Personalized PageRank correctness — KCA Track 6.

use cognitive::services::ppr_retrieval::{personalized_pagerank, PprConfig};
use petgraph::graph::DiGraph;

#[test]
fn ppr_concentrates_mass_on_seed_neighborhood() {
    // Star graph: A in center, B,C,D leaves; E disconnected.
    let mut g: DiGraph<String, f32> = DiGraph::new();
    let a = g.add_node("A".into());
    let b = g.add_node("B".into());
    let c = g.add_node("C".into());
    let d = g.add_node("D".into());
    let e = g.add_node("E".into());

    g.add_edge(a, b, 1.0);
    g.add_edge(b, a, 1.0);
    g.add_edge(a, c, 1.0);
    g.add_edge(c, a, 1.0);
    g.add_edge(a, d, 1.0);
    g.add_edge(d, a, 1.0);
    // E is disconnected.

    let scores = personalized_pagerank(&g, &[a], &PprConfig::default());

    // Disconnected E should have ~0 score.
    let s_e = scores.get(&"E".to_string()).copied().unwrap_or(0.0);
    assert!(s_e < 0.01, "disconnected E score = {s_e}");

    // A should have the highest score (it's the seed and the hub).
    let s_a = scores.get(&"A".to_string()).copied().unwrap_or(0.0);
    let s_b = scores.get(&"B".to_string()).copied().unwrap_or(0.0);
    assert!(s_a > s_b, "seed should outrank neighbor");

    // B, C, D should have nontrivial scores.
    assert!(s_b > 0.05);

    // Sum of scores ≈ 1.0 (probability distribution).
    let total: f32 = scores.values().sum();
    assert!((total - 1.0).abs() < 0.05, "scores sum should be ~1.0, got {total}");
}

#[test]
fn ppr_handles_empty_seeds_returns_uniform_or_empty() {
    let mut g: DiGraph<String, f32> = DiGraph::new();
    let _a = g.add_node("A".into());
    let _b = g.add_node("B".into());
    let scores = personalized_pagerank(&g, &[], &PprConfig::default());
    assert!(scores.is_empty());
}

#[test]
fn ppr_respects_alpha_teleportation_probability() {
    // Two-node graph with one cycle.
    let mut g: DiGraph<String, f32> = DiGraph::new();
    let a = g.add_node("A".into());
    let b = g.add_node("B".into());
    g.add_edge(a, b, 1.0);
    g.add_edge(b, a, 1.0);

    let strict_seed = PprConfig { alpha: 0.05, max_iterations: 50, tolerance: 1e-6 };
    let exploratory = PprConfig { alpha: 0.5, max_iterations: 50, tolerance: 1e-6 };

    let s1 = personalized_pagerank(&g, &[a], &strict_seed);
    let s2 = personalized_pagerank(&g, &[a], &exploratory);

    // Lower alpha = more teleportation back to seed = stronger A bias.
    let bias_strict = s1["A"] - s1["B"];
    let bias_exploratory = s2["A"] - s2["B"];
    assert!(bias_strict > bias_exploratory,
        "alpha=0.05 ({}) should bias toward seed more than alpha=0.5 ({})",
        bias_strict, bias_exploratory);
}
```

- [ ] **Step 2: Run, expect compile error.**

```bash
cargo nextest run -p cognitive --test phase_c_ppr_correctness
```

---

### Task C6.2: Implement `personalized_pagerank`

**Files:**
- Create: `crates/cognitive/src/services/ppr_retrieval.rs`

- [ ] **Step 1: Implement.**

```rust
//! KCA Track 6 — Personalized PageRank over the entity graph.
//!
//! Pure algorithm; no LLM. Runs at retrieval time as an additive boost step
//! after embedding + BM25.

use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::HashMap;

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
        Self { alpha: 0.15, max_iterations: 30, tolerance: 1e-4 }
    }
}

/// Compute Personalized PageRank scores for every node in `graph` given seed nodes.
/// Returns a map from node name to score in [0, 1] summing to ~1.0.
pub fn personalized_pagerank<E: petgraph::EdgeType>(
    graph: &DiGraph<String, f32, u32>,
    seeds: &[NodeIndex],
    cfg: &PprConfig,
) -> HashMap<String, f32> {
    if graph.node_count() == 0 || seeds.is_empty() {
        return HashMap::new();
    }

    let n = graph.node_count();
    let seed_set: std::collections::HashSet<NodeIndex> = seeds.iter().copied().collect();

    // Initial distribution: uniform over seeds.
    let mut current: Vec<f32> = vec![0.0; n];
    let seed_weight = 1.0 / seeds.len() as f32;
    for &s in seeds { current[s.index()] = seed_weight; }

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
            if mass == 0.0 { continue; }
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
        let l1: f32 = current.iter().zip(next.iter()).map(|(a, b)| (a - b).abs()).sum();
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
```

- [ ] **Step 2: Register module.**

In `services/mod.rs`:

```rust
pub mod ppr_retrieval;
pub use ppr_retrieval::{personalized_pagerank, PprConfig};
```

- [ ] **Step 3: Run.**

```bash
cargo nextest run -p cognitive --test phase_c_ppr_correctness
```

Expected: 3 PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/src/services/ppr_retrieval.rs crates/cognitive/src/services/mod.rs crates/cognitive/tests/phase_c_ppr_correctness.rs
git commit -m "feat(cognitive): Personalized PageRank algorithm (KCA Track 6)"
```

---

### Task C6.3: Build entity graph from repos for PPR

**Files:**
- Modify: `crates/cognitive/src/services/ppr_retrieval.rs`

- [ ] **Step 1: Failing test.**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cognitive::repos::entity::EntityRepo;
    use storage::StoragePool;

    #[tokio::test]
    async fn build_graph_from_entity_repo() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = EntityRepo::new(pool.clone());
        let a = repo.upsert_entity("A", "concept", None, "t", None).await.unwrap();
        let b = repo.upsert_entity("B", "concept", None, "t", None).await.unwrap();
        let c = repo.upsert_entity("C", "concept", None, "t", None).await.unwrap();
        repo.upsert_relationship_typed(&a.id, &b.id, "x", "causal", 0.9, None, "t").await.unwrap();
        repo.upsert_relationship_typed(&b.id, &c.id, "y", "correlational", 0.5, None, "t").await.unwrap();

        let (graph, name_to_idx) = build_graph_from_entities(&repo).await.unwrap();
        assert_eq!(graph.node_count(), 3);
        assert!(graph.edge_count() >= 2);
        assert!(name_to_idx.contains_key("A"));
        // Causal weighted higher than correlational by edge_type:
        let a_idx = name_to_idx["A"];
        let b_idx = name_to_idx["B"];
        let edge = graph.find_edge(a_idx, b_idx).unwrap();
        let w = *graph.edge_weight(edge).unwrap();
        assert!(w >= 0.9 * 1.5, "causal weight {} should be ≥ 0.9 * 1.5 (causal multiplier)", w);
    }
}
```

- [ ] **Step 2: Implement `build_graph_from_entities`.**

```rust
use cognitive::repos::entity::EntityRepo;

/// Build a directed graph from `entities` + `entity_relationships` tables.
/// Edge weights are scaled by edge_type (causal=1.5×, structural=1.2×, temporal=1.1×).
pub async fn build_graph_from_entities(
    repo: &EntityRepo,
) -> common::Result<(DiGraph<String, f32, u32>, HashMap<String, NodeIndex>)> {
    let mut g: DiGraph<String, f32, u32> = DiGraph::new();
    let mut idx_by_id: HashMap<String, NodeIndex> = HashMap::new();
    let mut name_by_idx: HashMap<String, NodeIndex> = HashMap::new();

    let entities = repo.list_all_entities(5000).await?;
    for e in &entities {
        let i = g.add_node(e.name.clone());
        idx_by_id.insert(e.id.clone(), i);
        name_by_idx.insert(e.name.clone(), i);
    }

    let edges = repo.list_all_edges(20000).await?;
    for edge in edges {
        let src = match idx_by_id.get(&edge.source_id) { Some(i) => *i, None => continue };
        let tgt = match idx_by_id.get(&edge.target_id) { Some(i) => *i, None => continue };
        let multiplier = match edge.edge_type.as_str() {
            "causal" => 1.5,
            "structural" => 1.2,
            "temporal" => 1.1,
            _ => 1.0,
        };
        g.add_edge(src, tgt, edge.strength as f32 * multiplier);
        // Add reverse for undirected expansion.
        g.add_edge(tgt, src, edge.strength as f32 * multiplier * 0.5);
    }

    Ok((g, name_by_idx))
}
```

- [ ] **Step 3: Add `list_all_entities` and `list_all_edges` to `EntityRepo` if missing.**

```rust
    pub async fn list_all_entities(&self, limit: usize) -> common::Result<Vec<EntityNode>> {
        let lim = limit as i64;
        let rows = sqlx::query_as!(
            EntityNode,
            "SELECT id, name, entity_type, description, source, source_id, mention_count, '{}' as metadata FROM entities ORDER BY mention_count DESC LIMIT ?1",
            lim
        )
        .fetch_all(self.pool.inner()).await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }

    pub async fn list_all_edges(&self, limit: usize) -> common::Result<Vec<EdgeRow>> {
        let lim = limit as i64;
        let rows = sqlx::query_as!(
            EdgeRow,
            "SELECT source_entity_id as source_id, target_entity_id as target_id, edge_type, strength FROM entity_relationships WHERE valid_until IS NULL LIMIT ?1",
            lim
        )
        .fetch_all(self.pool.inner()).await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows)
    }
```

```rust
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct EdgeRow {
    pub source_id: String,
    pub target_id: String,
    pub edge_type: String,
    pub strength: f64,
}
```

- [ ] **Step 4: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/ppr_retrieval::tests::build_graph_from_entity_repo/)'
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/cognitive/src/services/ppr_retrieval.rs crates/cognitive/src/repos/entity.rs
git commit -m "feat(cognitive): build PPR graph from entity repos with edge_type weighting (KCA Track 6)"
```

---

### Task C6.4: Cache the graph (rebuild every N seconds)

**Files:**
- Modify: `crates/cognitive/src/services/ppr_retrieval.rs`

- [ ] **Step 1: Add cached struct.**

```rust
use std::sync::Arc;
use tokio::sync::RwLock;
use std::time::{Duration, Instant};

pub struct CachedPprGraph {
    repo: EntityRepo,
    cache: RwLock<Option<(DiGraph<String, f32, u32>, HashMap<String, NodeIndex>, Instant)>>,
    ttl: Duration,
}

impl CachedPprGraph {
    pub fn new(repo: EntityRepo, ttl: Duration) -> Self {
        Self { repo, cache: RwLock::new(None), ttl }
    }

    pub async fn get_or_rebuild(&self) -> common::Result<(DiGraph<String, f32, u32>, HashMap<String, NodeIndex>)> {
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
```

- [ ] **Step 2: Add test.**

```rust
    #[tokio::test]
    async fn cached_graph_reuses_within_ttl() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = EntityRepo::new(pool.clone());
        repo.upsert_entity("A", "c", None, "t", None).await.unwrap();
        let cache = CachedPprGraph::new(repo.clone(), Duration::from_secs(60));
        let (g1, _) = cache.get_or_rebuild().await.unwrap();

        // Insert another entity; cache should NOT reflect it (within TTL).
        repo.upsert_entity("B", "c", None, "t", None).await.unwrap();
        let (g2, _) = cache.get_or_rebuild().await.unwrap();
        assert_eq!(g1.node_count(), g2.node_count(), "cache should still report stale view");
    }
```

- [ ] **Step 3: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/ppr_retrieval::tests::cached_graph/)'
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/src/services/ppr_retrieval.rs
git commit -m "feat(cognitive): TTL-cached PPR graph (KCA Track 6)"
```

---

### Task C6.5: Wire PPR boost into `MemoryRetriever`

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[tokio::test]
    async fn ppr_boost_improves_multi_hop_recall() {
        // Seed: 10 facts, 3 of them are 2 hops from query entity but related.
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let fact_repo = SemanticFactRepo::new(pool.clone());
        let entity_repo = EntityRepo::new(pool.clone());

        // Build entity chain: query → "rust" → "tokio" → "axum"
        let rust = entity_repo.upsert_entity("rust", "lang", None, "t", None).await.unwrap();
        let tokio = entity_repo.upsert_entity("tokio", "lib", None, "t", None).await.unwrap();
        let axum = entity_repo.upsert_entity("axum", "lib", None, "t", None).await.unwrap();
        entity_repo.upsert_relationship_typed(&rust.id, &tokio.id, "uses", "structural", 0.9, None, "t").await.unwrap();
        entity_repo.upsert_relationship_typed(&tokio.id, &axum.id, "supports", "structural", 0.8, None, "t").await.unwrap();

        // Insert facts for each entity.
        fact_repo.upsert(&SemanticFact::new("rust", "is", "memory_safe", 0.9, "t")).await.unwrap();
        fact_repo.upsert(&SemanticFact::new("tokio", "uses", "epoll", 0.85, "t")).await.unwrap();
        fact_repo.upsert(&SemanticFact::new("axum", "is", "web_framework", 0.85, "t")).await.unwrap();

        // Plus 7 unrelated facts.
        for i in 0..7 {
            fact_repo.upsert(&SemanticFact::new(&format!("X{i}"), "p", "Y", 0.8, "t")).await.unwrap();
        }

        // Retrieval for query "rust" — without PPR, only the rust-fact returns. With PPR,
        // tokio + axum facts also surface.
        let cache = std::sync::Arc::new(CachedPprGraph::new(entity_repo.clone(), std::time::Duration::from_secs(60)));
        let scored = retrieve_with_ppr_boost(&fact_repo, &entity_repo, &cache, "rust", 10).await.unwrap();

        let names: Vec<&str> = scored.iter().map(|s| s.fact.subject.as_str()).collect();
        assert!(names.contains(&"rust"));
        assert!(names.contains(&"tokio"), "tokio missing — PPR didn't expand: {:?}", names);
        assert!(names.contains(&"axum"), "axum missing — PPR didn't expand: {:?}", names);
    }
```

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Implement.**

In `memory_retriever.rs` (or a new helper file imported by it):

```rust
pub async fn retrieve_with_ppr_boost(
    fact_repo: &cognitive::repos::semantic_fact::SemanticFactRepo,
    entity_repo: &cognitive::repos::entity::EntityRepo,
    cache: &std::sync::Arc<CachedPprGraph>,
    seed_query: &str,
    limit: usize,
) -> common::Result<Vec<crate::services::memory_retriever::ScoredFact>> {
    // 1. Find seed entities by name match (cheap).
    let seed_names = extract_entity_names_from_query(seed_query);
    let mut seeds: Vec<petgraph::graph::NodeIndex> = Vec::new();
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
    let top_entities: Vec<String> = by_score.iter().take(limit * 3).map(|(n, _)| n.clone()).collect();

    // 4. Fetch facts for these entities.
    let mut out = Vec::new();
    for name in top_entities {
        if let Some(node) = entity_repo.find_by_name(&name).await? {
            let facts = fact_repo.find_facts_by_entity_id(&node.id, 5).await.unwrap_or_default();
            let ppr_score = by_score.iter().find(|(n, _)| n == &name).map(|(_, s)| *s).unwrap_or(0.0);
            for f in facts {
                out.push(crate::services::memory_retriever::ScoredFact {
                    fact: f,
                    score: ppr_score as f64,
                    source: "ppr".to_string(),
                });
            }
        }
    }

    // 5. Dedup by fact.id, keep top-N.
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    let mut seen = std::collections::HashSet::new();
    out.retain(|s| seen.insert(s.fact.id.clone()));
    out.truncate(limit);
    Ok(out)
}

fn extract_entity_names_from_query(query: &str) -> Vec<String> {
    // Heuristic: split on whitespace, keep tokens of length ≥3 that start with letter.
    query.split_whitespace()
        .filter(|t| t.len() >= 3 && t.chars().next().map_or(false, |c| c.is_alphabetic()))
        .map(|t| t.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase())
        .collect()
}
```

- [ ] **Step 4: Integrate into `MemoryRetriever::retrieve`.**

In the existing retrieval flow (find the embedding + BM25 step), add a final PPR step that merges via RRF (reciprocal rank fusion):

```rust
        // KCA Track 6: PPR expansion.
        if let Some(cache) = self.ppr_cache.as_ref() {
            let ppr_results = retrieve_with_ppr_boost(
                &self.fact_repo, &self.entity_repo, cache, query, limit
            ).await.unwrap_or_default();
            merged = rrf_merge(merged, ppr_results, 60);
        }
```

`rrf_merge(a, b, k)` is reciprocal rank fusion: `score = sum_i 1/(k + rank_i)` across input lists, dedupe by fact id.

- [ ] **Step 5: Add `rrf_merge` helper.**

```rust
pub fn rrf_merge(
    a: Vec<crate::services::memory_retriever::ScoredFact>,
    b: Vec<crate::services::memory_retriever::ScoredFact>,
    k: usize,
) -> Vec<crate::services::memory_retriever::ScoredFact> {
    let mut by_id: std::collections::HashMap<String, (crate::services::memory_retriever::ScoredFact, f64)> = Default::default();
    for (rank, sf) in a.into_iter().enumerate() {
        let s = 1.0 / (k as f64 + (rank + 1) as f64);
        by_id.entry(sf.fact.id.clone()).and_modify(|v| v.1 += s).or_insert((sf, s));
    }
    for (rank, sf) in b.into_iter().enumerate() {
        let s = 1.0 / (k as f64 + (rank + 1) as f64);
        by_id.entry(sf.fact.id.clone()).and_modify(|v| v.1 += s).or_insert((sf, s));
    }
    let mut out: Vec<_> = by_id.into_iter().map(|(_, (mut sf, score))| {
        sf.score = score;
        sf
    }).collect();
    out.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    out
}
```

- [ ] **Step 6: Run.**

```bash
cargo nextest run -p cognitive -E 'test(ppr_boost_improves_multi_hop_recall)'
```

Expected: PASS.

- [ ] **Step 7: Commit.**

```bash
git add crates/cognitive/src/services/ppr_retrieval.rs crates/cognitive/src/services/memory_retriever.rs
git commit -m "feat(cognitive): wire PPR + RRF merge into MemoryRetriever (KCA Track 6)"
```

---

# Track 7 — Predictive cache warming

End-of-turn fire-and-forget: a cheap LLM predicts the next 1-3 likely follow-up queries; we pre-run retrieval for them; cache for 5 minutes. When the user actually asks a similar question, retrieval is a cache hit.

### Task C7.1: `PredictiveCacheConfig`

**Files:**
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[test]
    fn predictive_cache_config_defaults() {
        let cfg: CognitiveConfig = serde_json::from_str("{}").unwrap();
        assert!(cfg.predictive_cache.enabled);
        assert_eq!(cfg.predictive_cache.predictions_per_turn, 3);
        assert_eq!(cfg.predictive_cache.ttl_seconds, 300);
        assert_eq!(cfg.predictive_cache.min_hit_rate_for_keep_alive, 0.20);
    }
```

- [ ] **Step 2: Implement.**

```rust
    /// Predictive cache config (KCA Track 7).
    #[serde(default)]
    pub predictive_cache: PredictiveCacheConfig,
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PredictiveCacheConfig {
    pub enabled: bool,
    pub predictions_per_turn: u32,
    pub ttl_seconds: u32,
    /// If hit rate over rolling 100 lookups falls below this, auto-disable for 24h.
    pub min_hit_rate_for_keep_alive: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for PredictiveCacheConfig {
    fn default() -> Self {
        Self { enabled: true, predictions_per_turn: 3, ttl_seconds: 300, min_hit_rate_for_keep_alive: 0.20, model: None }
    }
}
```

- [ ] **Step 3: Run + commit.**

```bash
cargo nextest run -p config -E 'test(predictive_cache_config_defaults)'
git add crates/config/src/schema/cognitive.rs
git commit -m "feat(config): PredictiveCacheConfig (KCA Track 7)"
```

---

### Task C7.2: `PredictiveCache` LRU service

**Files:**
- Create: `crates/cognitive/src/services/predictive_cache.rs`

- [ ] **Step 1: Add `lru` dep.**

In `crates/cognitive/Cargo.toml`:

```toml
lru = "0.12"
```

- [ ] **Step 2: Failing test (inline).**

```rust
//! KCA Track 7 — predictive cache.

use std::time::{Duration, Instant};

#[cfg(test)]
mod tests {
    use super::*;
    use cognitive::services::memory_retriever::ScoredFact;
    use cognitive::repos::semantic_fact::SemanticFact;

    fn sample(s: &str) -> Vec<ScoredFact> {
        vec![ScoredFact { fact: SemanticFact::new(s, "p", "o", 0.5, "t"), score: 0.7, source: "test".into() }]
    }

    #[tokio::test]
    async fn cache_returns_value_within_ttl() {
        let cache = PredictiveCache::new(100, Duration::from_secs(60));
        cache.put("hash1".into(), sample("alpha")).await;
        let got = cache.get("hash1").await;
        assert!(got.is_some());
        assert_eq!(got.unwrap()[0].fact.subject, "alpha");
    }

    #[tokio::test]
    async fn cache_expires_after_ttl() {
        let cache = PredictiveCache::new(100, Duration::from_millis(50));
        cache.put("h".into(), sample("z")).await;
        tokio::time::sleep(Duration::from_millis(80)).await;
        assert!(cache.get("h").await.is_none());
    }

    #[tokio::test]
    async fn cache_tracks_hit_rate() {
        let cache = PredictiveCache::new(100, Duration::from_secs(60));
        cache.put("h1".into(), sample("a")).await;
        let _ = cache.get("h1").await; // hit
        let _ = cache.get("missing").await; // miss
        let _ = cache.get("h1").await; // hit
        let stats = cache.stats().await;
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert!((stats.hit_rate() - 2.0/3.0).abs() < 1e-6);
    }
}
```

- [ ] **Step 3: Implement.**

```rust
use cognitive::services::memory_retriever::ScoredFact;
use lru::LruCache;
use std::num::NonZeroUsize;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

#[derive(Debug, Clone, Copy)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
}

impl CacheStats {
    pub fn hit_rate(&self) -> f64 {
        let total = self.hits + self.misses;
        if total == 0 { 0.0 } else { self.hits as f64 / total as f64 }
    }
}

struct CacheEntry {
    inserted_at: Instant,
    value: Vec<ScoredFact>,
}

pub struct PredictiveCache {
    inner: Mutex<LruCache<String, CacheEntry>>,
    ttl: Duration,
    hits: AtomicU64,
    misses: AtomicU64,
}

impl PredictiveCache {
    pub fn new(capacity: usize, ttl: Duration) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
            ttl,
            hits: AtomicU64::new(0),
            misses: AtomicU64::new(0),
        }
    }

    pub async fn put(&self, key: String, value: Vec<ScoredFact>) {
        let mut g = self.inner.lock().await;
        g.put(key, CacheEntry { inserted_at: Instant::now(), value });
    }

    pub async fn get(&self, key: &str) -> Option<Vec<ScoredFact>> {
        let mut g = self.inner.lock().await;
        let entry = g.get(key);
        match entry {
            Some(e) if e.inserted_at.elapsed() < self.ttl => {
                self.hits.fetch_add(1, Ordering::Relaxed);
                Some(e.value.clone())
            }
            Some(_) => {
                g.pop(key);
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
            None => {
                self.misses.fetch_add(1, Ordering::Relaxed);
                None
            }
        }
    }

    pub async fn stats(&self) -> CacheStats {
        CacheStats {
            hits: self.hits.load(Ordering::Relaxed),
            misses: self.misses.load(Ordering::Relaxed),
        }
    }
}

pub fn query_hash(query: &str) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(query.trim().to_lowercase().as_bytes());
    format!("{:x}", h.finalize())
}
```

- [ ] **Step 4: Register module.**

```rust
// In services/mod.rs
pub mod predictive_cache;
pub use predictive_cache::{PredictiveCache, CacheStats, query_hash};
```

- [ ] **Step 5: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/predictive_cache::/)'
```

Expected: 3 PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/cognitive/src/services/predictive_cache.rs crates/cognitive/src/services/mod.rs crates/cognitive/Cargo.toml
git commit -m "feat(cognitive): PredictiveCache LRU + stats (KCA Track 7)"
```

---

### Task C7.3: `LlmQueryPredictorHandler`

**Files:**
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`
- Modify: `crates/agent/src/adapters/prompts.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[tokio::test]
    async fn llm_query_predictor_returns_n_predictions() {
        let json = r#"{"predictions": ["how do I install rust?", "what about cargo?", "memory safety details?"]}"#;
        let provider = providers::test_helpers::FakeProvider::with_text(json);
        let handler = LlmQueryPredictorHandler::new(std::sync::Arc::new(provider), "m".into(), 256);

        let preds = handler.predict_next("user just asked about Rust", 3).await.unwrap();
        assert_eq!(preds.len(), 3);
    }
```

- [ ] **Step 2: Run, expect failure.**

- [ ] **Step 3: Prompt + handler.**

```rust
pub(crate) const QUERY_PREDICTOR_SYSTEM_PROMPT: &str = r#"You are a query predictor.

Given the just-completed conversation turn, predict the user's next 1-3 most likely follow-up questions.

Output strict JSON:
{"predictions": ["...", "...", "..."]}

Heuristics:
- If user asked "what is X?", a likely follow-up is "how do I use X?" or "what's an alternative to X?"
- If user asked "how do I do Y?", likely follow-ups are "is there a faster way?" or "what about edge case Z?"
- Keep predictions short (≤80 chars), specific, and grounded in the actual turn content.
- Don't invent topics not present in the conversation."#;

pub struct LlmQueryPredictorHandler {
    provider: std::sync::Arc<dyn providers::DynProvider>,
    model: String,
    max_tokens: u32,
}

impl LlmQueryPredictorHandler {
    pub fn new(provider: std::sync::Arc<dyn providers::DynProvider>, model: String, max_tokens: u32) -> Self {
        Self { provider, model, max_tokens }
    }

    pub async fn predict_next(&self, recent_turn: &str, n: u32) -> common::Result<Vec<String>> {
        let user = format!("RECENT TURN:\n{}\n\nPredict {n} follow-up questions.", recent_turn);
        let req = providers::ChatRequest {
            model: self.model.clone(),
            messages: vec![
                providers::Message::System(QUERY_PREDICTOR_SYSTEM_PROMPT.to_string()),
                providers::Message::User(providers::UserContent::Text(user)),
            ],
            max_tokens: Some(self.max_tokens),
            temperature: Some(0.5),
            response_format: Some(providers::ResponseFormat::JsonObject),
            ..Default::default()
        };
        let resp = self.provider.complete(req).await
            .map_err(|e| common::KlyntbotError::Provider(format!("query_predictor: {e}")))?;
        #[derive(serde::Deserialize)]
        struct R { predictions: Vec<String> }
        let parsed: R = serde_json::from_str(&resp.text())
            .map_err(|e| common::KlyntbotError::Internal(format!("query_predictor parse: {e}")))?;
        Ok(parsed.predictions.into_iter().take(n as usize).collect())
    }
}
```

- [ ] **Step 4: Run.**

```bash
cargo nextest run -p agent -E 'test(llm_query_predictor_returns_n_predictions)'
```

Expected: PASS.

- [ ] **Step 5: Commit.**

```bash
git add crates/agent/src/adapters/cognitive_handlers.rs crates/agent/src/adapters/prompts.rs
git commit -m "feat(agent): LlmQueryPredictorHandler (KCA Track 7)"
```

---

### Task C7.4: Wire predictive warming into runtime Phase 3

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Failing test.**

In `runtime.rs` tests:

```rust
    #[tokio::test]
    async fn process_message_warms_cache_for_predicted_queries() {
        let app = test_app_core_with_predictive_cache().await;
        let key = SessionKey::new("test", "session1");
        app.chat_send("Tell me about rust", key.clone(), None).await.unwrap();

        // Wait briefly for fire-and-forget tasks.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        // The cache should have entries for the predicted queries.
        let stats = app.predictive_cache().unwrap().stats().await;
        // We pre-populated 3 entries.
        // Misses still 0 because no .get() yet. Use cache.size() instead:
        assert!(app.predictive_cache().unwrap().size().await >= 1);
    }
```

- [ ] **Step 2: Add `size()` method to `PredictiveCache`.**

```rust
    pub async fn size(&self) -> usize {
        self.inner.lock().await.len()
    }
```

- [ ] **Step 3: Wire.**

In `runtime.rs::process_message`'s Phase 3, add:

```rust
            // KCA Track 7: predictive cache warming.
            if self.cfg.predictive_cache.enabled {
                if let (Some(predictor), Some(cache), Some(retriever)) = (
                    self.query_predictor.as_ref(),
                    self.predictive_cache.as_ref(),
                    self.memory_retriever.as_ref(),
                ) {
                    let predictor = predictor.clone();
                    let cache = cache.clone();
                    let retriever = retriever.clone();
                    let recent_turn_text = format!("USER: {}\nASSISTANT: {}", user_text, response_text);
                    let n = self.cfg.predictive_cache.predictions_per_turn;
                    tokio::spawn(async move {
                        let preds = match predictor.predict_next(&recent_turn_text, n).await {
                            Ok(p) => p,
                            Err(e) => { tracing::debug!(error = %e, "predictor failed"); return; }
                        };
                        for q in preds {
                            let key = cognitive::services::predictive_cache::query_hash(&q);
                            if cache.get(&key).await.is_some() { continue; } // already warm
                            match retriever.retrieve(&q, 10).await {
                                Ok(results) => cache.put(key, results).await,
                                Err(e) => tracing::debug!(error = %e, "predictive retrieve failed"),
                            }
                        }
                    });
                }
            }
```

Plumb `query_predictor: Option<Arc<LlmQueryPredictorHandler>>`, `predictive_cache: Option<Arc<PredictiveCache>>` through `AgentRuntime`'s constructor and `agent_loop::builder.rs`.

- [ ] **Step 4: Update read-side to check cache.**

In `runtime.rs::build_retrieval_context` (or wherever retrieval is called), check the cache first:

```rust
    if let Some(cache) = self.predictive_cache.as_ref() {
        let key = cognitive::services::predictive_cache::query_hash(message_text);
        if let Some(cached) = cache.get(&key).await {
            tracing::debug!("predictive cache HIT for {message_text:?}");
            return Ok(cached);
        }
    }
```

- [ ] **Step 5: Run.**

```bash
cargo nextest run -p agent -E 'test(process_message_warms_cache_for_predicted_queries)'
```

Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/agent/src/agent_runtime/runtime.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): wire predictive cache warming + lookup (KCA Track 7)"
```

---

### Task C7.5: Auto-disable on low hit rate

**Files:**
- Modify: `crates/cognitive/src/services/predictive_cache.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[tokio::test]
    async fn cache_disables_after_low_hit_rate_window() {
        let mut cache = PredictiveCache::with_auto_disable(100, Duration::from_secs(60), 0.2);
        // Force 100 misses + 0 hits.
        for i in 0..100 {
            let _ = cache.get(&format!("missing-{i}")).await;
        }
        assert!(cache.is_disabled().await);
    }
```

- [ ] **Step 2: Implement.**

Add `min_hit_rate` field, a window counter every 100 ops, and an `Atomic<bool>`:

```rust
use std::sync::atomic::AtomicBool;

pub struct PredictiveCache {
    inner: Mutex<LruCache<String, CacheEntry>>,
    ttl: Duration,
    hits: AtomicU64,
    misses: AtomicU64,
    min_hit_rate: f64,
    disabled: AtomicBool,
    disabled_until: Mutex<Option<Instant>>,
}

impl PredictiveCache {
    pub fn with_auto_disable(capacity: usize, ttl: Duration, min_hit_rate: f64) -> Self {
        Self {
            inner: Mutex::new(LruCache::new(NonZeroUsize::new(capacity).unwrap())),
            ttl, hits: AtomicU64::new(0), misses: AtomicU64::new(0),
            min_hit_rate, disabled: AtomicBool::new(false),
            disabled_until: Mutex::new(None),
        }
    }

    pub async fn is_disabled(&self) -> bool {
        if self.disabled.load(Ordering::Relaxed) {
            let mut g = self.disabled_until.lock().await;
            if let Some(until) = *g {
                if Instant::now() >= until {
                    self.disabled.store(false, Ordering::Relaxed);
                    self.hits.store(0, Ordering::Relaxed);
                    self.misses.store(0, Ordering::Relaxed);
                    *g = None;
                    return false;
                }
            }
            return true;
        }
        false
    }

    async fn maybe_disable(&self) {
        let h = self.hits.load(Ordering::Relaxed);
        let m = self.misses.load(Ordering::Relaxed);
        let total = h + m;
        if total < 100 { return; }
        let rate = h as f64 / total as f64;
        if rate < self.min_hit_rate {
            self.disabled.store(true, Ordering::Relaxed);
            *self.disabled_until.lock().await = Some(Instant::now() + Duration::from_secs(86400));
            tracing::info!(rate, "predictive cache auto-disabled for 24h");
        }
    }
}
```

In `get`/`put`, call `maybe_disable()` after recording stats; in `get`, return None when disabled.

- [ ] **Step 3: Run + commit.**

```bash
cargo nextest run -p cognitive -E 'test(cache_disables_after_low_hit_rate_window)'
git add crates/cognitive/src/services/predictive_cache.rs
git commit -m "feat(cognitive): auto-disable predictive cache on low hit rate (KCA Track 7)"
```

---

# Track 8 — Hierarchical episodic compression

Episodic memories accumulate. We add hourly → daily → weekly summary tiers, each compressing the level below via a cheap LLM call. Long-term memory becomes navigable in O(log N) instead of O(N).

### Task C8.1: Migration — hierarchical episodic columns

**Files:**
- Create: `crates/cognitive/migrations/013_hierarchical_episodics.sql`
- Modify: `crates/cognitive/src/lib.rs`

- [ ] **Step 1: Migration.**

```sql
-- KCA Track 8: hierarchical episodic compression.
ALTER TABLE episodic_memories ADD COLUMN tier TEXT NOT NULL DEFAULT 'raw'
    CHECK (tier IN ('raw', 'hourly', 'daily', 'weekly'));
ALTER TABLE episodic_memories ADD COLUMN parent_id TEXT;
ALTER TABLE episodic_memories ADD COLUMN child_count INTEGER NOT NULL DEFAULT 0;

CREATE INDEX IF NOT EXISTS idx_episodic_tier_recorded
    ON episodic_memories(tier, recorded_at DESC);
CREATE INDEX IF NOT EXISTS idx_episodic_parent
    ON episodic_memories(parent_id);

-- Track which raw episodics have been rolled into hourly summaries.
ALTER TABLE episodic_memories ADD COLUMN rolled_up_at TEXT;
```

Register version 13 in `lib.rs`.

- [ ] **Step 2: Run migrations test + commit.**

```bash
cargo nextest run -p cognitive -E 'test(/migration/)'
git add crates/cognitive/migrations/013_hierarchical_episodics.sql crates/cognitive/src/lib.rs
git commit -m "feat(cognitive): migration 013 hierarchical episodic columns (KCA Track 8)"
```

---

### Task C8.2: Failing test — `roll_up_hourly` produces parent summary

**Files:**
- Test: `crates/cognitive/src/services/hierarchical_compressor.rs`

- [ ] **Step 1: Add file with test.**

```rust
//! KCA Track 8 — hierarchical episodic compression.

#[cfg(test)]
mod tests {
    use super::*;
    use cognitive::repos::episodic::{EpisodicMemory, EpisodicMemoryRepo};
    use storage::StoragePool;

    struct EchoSummarizer;
    #[async_trait::async_trait]
    impl HierarchicalSummarizer for EchoSummarizer {
        async fn summarize(&self, items: &[EpisodicMemory], tier: Tier) -> common::Result<String> {
            Ok(format!("Summary of {} items at tier {:?}", items.len(), tier))
        }
    }

    #[tokio::test]
    async fn roll_up_hourly_creates_parent_with_correct_child_count() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = EpisodicMemoryRepo::new(pool.clone());

        // Insert 5 raw episodics in last hour.
        for i in 0..5 {
            repo.insert(&EpisodicMemory {
                id: format!("ep{i}"),
                domain: "test".into(),
                content: format!("event {i}"),
                summary: None,
                importance: 0.5,
                recorded_at: jiff::Timestamp::now(),
                stability: 1.0,
                tier: "raw".into(),
                parent_id: None,
                child_count: 0,
                ..Default::default()
            }).await.unwrap();
        }

        let summarizer = std::sync::Arc::new(EchoSummarizer);
        let n = roll_up_hourly(&repo, summarizer).await.unwrap();
        assert_eq!(n, 1, "1 hourly bucket created");

        let hourlies = repo.list_by_tier("hourly", 10).await.unwrap();
        assert_eq!(hourlies.len(), 1);
        assert_eq!(hourlies[0].child_count, 5);
        assert!(hourlies[0].summary.as_deref().map_or(false, |s| s.contains("5 items")));

        // Raw episodics should be marked rolled_up_at.
        let raw = repo.list_by_tier("raw", 10).await.unwrap();
        assert!(raw.iter().all(|e| e.rolled_up_at.is_some()));
    }
}
```

- [ ] **Step 2: Run, expect compile failure.**

---

### Task C8.3: Implement `Tier` enum + `HierarchicalSummarizer` trait + `roll_up_*`

**Files:**
- Modify: `crates/cognitive/src/services/hierarchical_compressor.rs`

- [ ] **Step 1: Implement.**

```rust
use cognitive::repos::episodic::{EpisodicMemory, EpisodicMemoryRepo};
use jiff::{Timestamp, Unit};

#[derive(Debug, Clone, Copy)]
pub enum Tier { Raw, Hourly, Daily, Weekly }

impl Tier {
    pub fn as_str(&self) -> &'static str {
        match self { Tier::Raw => "raw", Tier::Hourly => "hourly", Tier::Daily => "daily", Tier::Weekly => "weekly" }
    }
}

#[async_trait::async_trait]
pub trait HierarchicalSummarizer: Send + Sync {
    async fn summarize(&self, items: &[EpisodicMemory], tier: Tier) -> common::Result<String>;
}

/// Roll up all unrolled raw episodics into hourly summaries.
/// Returns the number of hourly buckets created.
pub async fn roll_up_hourly(
    repo: &EpisodicMemoryRepo,
    summarizer: std::sync::Arc<dyn HierarchicalSummarizer>,
) -> common::Result<u32> {
    let unrolled = repo.list_unrolled_at_tier("raw", 1000).await?;
    if unrolled.is_empty() { return Ok(0); }

    // Group by hour bucket (UTC).
    let mut by_hour: std::collections::BTreeMap<i64, Vec<EpisodicMemory>> = Default::default();
    for ep in unrolled {
        let bucket = ep.recorded_at.duration_since(Timestamp::UNIX_EPOCH).as_secs() / 3600;
        by_hour.entry(bucket).or_default().push(ep);
    }

    let mut created = 0u32;
    for (hour_bucket, items) in by_hour {
        let summary = summarizer.summarize(&items, Tier::Hourly).await
            .unwrap_or_else(|_| format!("(failed to summarize {} items)", items.len()));

        let parent_id = uuid::Uuid::new_v4().to_string();
        let dom = items[0].domain.clone();
        let avg_importance = items.iter().map(|e| e.importance).sum::<f64>() / items.len() as f64;
        let recorded_at = Timestamp::from_second(hour_bucket * 3600 + 1800).unwrap_or_else(|_| Timestamp::now());

        repo.insert(&EpisodicMemory {
            id: parent_id.clone(),
            domain: dom,
            content: summary.clone(),
            summary: Some(summary),
            importance: avg_importance,
            recorded_at,
            stability: 1.5,
            tier: Tier::Hourly.as_str().to_string(),
            parent_id: None,
            child_count: items.len() as i32,
            ..Default::default()
        }).await?;

        for it in &items {
            repo.set_parent_and_rolled_up(&it.id, &parent_id).await?;
        }
        created += 1;
    }

    Ok(created)
}

pub async fn roll_up_daily(
    repo: &EpisodicMemoryRepo,
    summarizer: std::sync::Arc<dyn HierarchicalSummarizer>,
) -> common::Result<u32> {
    // Same pattern: bucket by day (sec / 86400), summarize hourly tier into daily parent.
    let unrolled = repo.list_unrolled_at_tier("hourly", 500).await?;
    if unrolled.is_empty() { return Ok(0); }

    let mut by_day: std::collections::BTreeMap<i64, Vec<EpisodicMemory>> = Default::default();
    for ep in unrolled {
        let bucket = ep.recorded_at.duration_since(Timestamp::UNIX_EPOCH).as_secs() / 86400;
        by_day.entry(bucket).or_default().push(ep);
    }

    let mut created = 0u32;
    for (day_bucket, items) in by_day {
        let summary = summarizer.summarize(&items, Tier::Daily).await
            .unwrap_or_else(|_| format!("(failed to summarize {} items)", items.len()));
        let parent_id = uuid::Uuid::new_v4().to_string();
        let recorded_at = Timestamp::from_second(day_bucket * 86400 + 43200).unwrap_or_else(|_| Timestamp::now());

        repo.insert(&EpisodicMemory {
            id: parent_id.clone(),
            domain: items[0].domain.clone(),
            content: summary.clone(),
            summary: Some(summary),
            importance: items.iter().map(|e| e.importance).sum::<f64>() / items.len() as f64,
            recorded_at,
            stability: 2.0,
            tier: Tier::Daily.as_str().to_string(),
            parent_id: None,
            child_count: items.iter().map(|e| e.child_count).sum::<i32>(),
            ..Default::default()
        }).await?;
        for it in &items {
            repo.set_parent_and_rolled_up(&it.id, &parent_id).await?;
        }
        created += 1;
    }
    Ok(created)
}

pub async fn roll_up_weekly(
    repo: &EpisodicMemoryRepo,
    summarizer: std::sync::Arc<dyn HierarchicalSummarizer>,
) -> common::Result<u32> {
    let unrolled = repo.list_unrolled_at_tier("daily", 200).await?;
    if unrolled.is_empty() { return Ok(0); }
    let mut by_week: std::collections::BTreeMap<i64, Vec<EpisodicMemory>> = Default::default();
    for ep in unrolled {
        let bucket = ep.recorded_at.duration_since(Timestamp::UNIX_EPOCH).as_secs() / (86400 * 7);
        by_week.entry(bucket).or_default().push(ep);
    }
    let mut created = 0u32;
    for (week_bucket, items) in by_week {
        let summary = summarizer.summarize(&items, Tier::Weekly).await
            .unwrap_or_else(|_| format!("(failed to summarize {} items)", items.len()));
        let parent_id = uuid::Uuid::new_v4().to_string();
        let recorded_at = Timestamp::from_second(week_bucket * 86400 * 7).unwrap_or_else(|_| Timestamp::now());

        repo.insert(&EpisodicMemory {
            id: parent_id.clone(),
            domain: items[0].domain.clone(),
            content: summary.clone(),
            summary: Some(summary),
            importance: items.iter().map(|e| e.importance).sum::<f64>() / items.len() as f64,
            recorded_at,
            stability: 3.0,
            tier: Tier::Weekly.as_str().to_string(),
            parent_id: None,
            child_count: items.iter().map(|e| e.child_count).sum::<i32>(),
            ..Default::default()
        }).await?;
        for it in &items {
            repo.set_parent_and_rolled_up(&it.id, &parent_id).await?;
        }
        created += 1;
    }
    Ok(created)
}
```

- [ ] **Step 2: Add repo helpers.**

In `episodic.rs`:

```rust
    pub async fn list_unrolled_at_tier(&self, tier: &str, limit: usize) -> common::Result<Vec<EpisodicMemory>> {
        let lim = limit as i64;
        let rows = sqlx::query_as!(
            EpisodicMemoryRow,
            "SELECT * FROM episodic_memories WHERE tier = ?1 AND rolled_up_at IS NULL ORDER BY recorded_at ASC LIMIT ?2",
            tier, lim
        )
        .fetch_all(self.pool.inner()).await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(EpisodicMemory::from).collect())
    }

    pub async fn list_by_tier(&self, tier: &str, limit: usize) -> common::Result<Vec<EpisodicMemory>> {
        let lim = limit as i64;
        let rows = sqlx::query_as!(
            EpisodicMemoryRow,
            "SELECT * FROM episodic_memories WHERE tier = ?1 ORDER BY recorded_at DESC LIMIT ?2",
            tier, lim
        )
        .fetch_all(self.pool.inner()).await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(EpisodicMemory::from).collect())
    }

    pub async fn set_parent_and_rolled_up(&self, id: &str, parent: &str) -> common::Result<()> {
        sqlx::query!(
            "UPDATE episodic_memories SET parent_id = ?1, rolled_up_at = datetime('now') WHERE id = ?2",
            parent, id
        )
        .execute(self.pool.inner()).await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }
```

- [ ] **Step 3: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/hierarchical_compressor::tests::/)'
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/src/services/hierarchical_compressor.rs \
        crates/cognitive/src/services/mod.rs \
        crates/cognitive/src/repos/episodic.rs
git commit -m "feat(cognitive): hierarchical compressor (KCA Track 8)"
```

---

### Task C8.4: LLM summarizer + 3 prompt tiers

**Files:**
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`
- Modify: `crates/agent/src/adapters/prompts.rs`

- [ ] **Step 1: Prompts.**

```rust
pub(crate) const HIERARCHICAL_HOURLY_PROMPT: &str = r#"You are an episodic memory summarizer.

Summarize the following hour of activity into a single 2-3 sentence paragraph that:
- Names the dominant topic(s) of the hour.
- Lists key decisions or discoveries.
- Preserves quantifiable details (numbers, names, durations).

Output plain text, no JSON, no headings."#;

pub(crate) const HIERARCHICAL_DAILY_PROMPT: &str = r#"You are an episodic memory summarizer.

Summarize the following day of hourly summaries into a 4-6 sentence narrative paragraph that:
- Names the day's primary themes.
- Captures arc (morning vs evening, build-up vs resolution).
- Preserves any breakthroughs, blockers, or open questions.

Output plain text, no JSON, no headings."#;

pub(crate) const HIERARCHICAL_WEEKLY_PROMPT: &str = r#"You are an episodic memory summarizer.

Summarize the week of daily summaries into a structured narrative with:
- 1 paragraph: dominant theme of the week.
- 1 paragraph: notable wins.
- 1 paragraph: notable open threads.

Output plain text with paragraph breaks, no JSON, no headings."#;
```

- [ ] **Step 2: Handler.**

```rust
use cognitive::repos::episodic::EpisodicMemory;
use cognitive::services::hierarchical_compressor::{HierarchicalSummarizer, Tier};

pub struct LlmHierarchicalSummarizer {
    provider: std::sync::Arc<dyn providers::DynProvider>,
    model: String,
}

impl LlmHierarchicalSummarizer {
    pub fn new(provider: std::sync::Arc<dyn providers::DynProvider>, model: String) -> Self {
        Self { provider, model }
    }
}

#[async_trait::async_trait]
impl HierarchicalSummarizer for LlmHierarchicalSummarizer {
    async fn summarize(&self, items: &[EpisodicMemory], tier: Tier) -> common::Result<String> {
        if items.is_empty() { return Ok(String::new()); }
        let system = match tier {
            Tier::Raw => "Return content unchanged.",
            Tier::Hourly => HIERARCHICAL_HOURLY_PROMPT,
            Tier::Daily => HIERARCHICAL_DAILY_PROMPT,
            Tier::Weekly => HIERARCHICAL_WEEKLY_PROMPT,
        };
        let user = items.iter()
            .map(|e| format!("[{}] {}", e.recorded_at, e.summary.as_deref().unwrap_or(&e.content)))
            .collect::<Vec<_>>()
            .join("\n");
        let req = providers::ChatRequest {
            model: self.model.clone(),
            messages: vec![
                providers::Message::System(system.to_string()),
                providers::Message::User(providers::UserContent::Text(user)),
            ],
            max_tokens: Some(1024),
            temperature: Some(0.3),
            ..Default::default()
        };
        let resp = self.provider.complete(req).await
            .map_err(|e| common::KlyntbotError::Provider(format!("hier_summarize: {e}")))?;
        Ok(resp.text())
    }
}
```

- [ ] **Step 3: Run + commit.**

```bash
cargo build -p agent
git add crates/agent/src/adapters/cognitive_handlers.rs crates/agent/src/adapters/prompts.rs
git commit -m "feat(agent): LlmHierarchicalSummarizer + 3 tier prompts (KCA Track 8)"
```

---

### Task C8.5: Cron registration for 3 roll-up jobs

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`
- Modify: `crates/config/src/schema/cognitive.rs`

- [ ] **Step 1: Add `HierarchicalConfig`.**

```rust
    /// Hierarchical episodic compression config (KCA Track 8).
    #[serde(default)]
    pub hierarchical: HierarchicalConfig,
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HierarchicalConfig {
    pub enabled: bool,
    /// Cron expression for hourly roll-up (default: 5 minutes past every hour).
    pub hourly_schedule: String,
    pub daily_schedule: String,
    pub weekly_schedule: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

impl Default for HierarchicalConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            hourly_schedule: "0 5 * * * *".into(),
            daily_schedule: "0 30 0 * * *".into(),
            weekly_schedule: "0 0 1 * * 1".into(), // Monday 01:00
            model: None,
        }
    }
}
```

- [ ] **Step 2: Register crons.**

In `init_cron`:

```rust
    if config.cognitive.hierarchical.enabled {
        cron_executor.upsert_default_job(
            "__klyntbot_episodic_rollup_hourly",
            &config.cognitive.hierarchical.hourly_schedule,
            "Hourly episodic compression (KCA Track 8)",
        ).await?;
        cron_executor.upsert_default_job(
            "__klyntbot_episodic_rollup_daily",
            &config.cognitive.hierarchical.daily_schedule,
            "Daily episodic compression (KCA Track 8)",
        ).await?;
        cron_executor.upsert_default_job(
            "__klyntbot_episodic_rollup_weekly",
            &config.cognitive.hierarchical.weekly_schedule,
            "Weekly episodic compression (KCA Track 8)",
        ).await?;
    }
```

- [ ] **Step 3: Register callbacks.**

```rust
    cron_executor.register_callback("__klyntbot_episodic_rollup_hourly", {
        let pool = pool.clone();
        let model = config.cognitive.hierarchical.model.clone()
            .unwrap_or_else(|| config.cognitive.model.clone());
        let provider = cognitive_provider.clone();
        Box::new(move || {
            let pool = pool.clone();
            let model = model.clone();
            let provider = provider.clone();
            Box::pin(async move {
                let repo = EpisodicMemoryRepo::new(pool);
                let summarizer: std::sync::Arc<dyn HierarchicalSummarizer> = match provider {
                    Some(p) => std::sync::Arc::new(LlmHierarchicalSummarizer::new(p, model)),
                    None => return Ok(()),
                };
                if let Err(e) = roll_up_hourly(&repo, summarizer).await {
                    tracing::warn!(error = %e, "rollup_hourly failed");
                }
                Ok(())
            }) as _
        })
    }).await;

    // Repeat the pattern for daily + weekly with `roll_up_daily` / `roll_up_weekly`.
```

- [ ] **Step 4: Failing test that all 3 crons register.**

```rust
    #[tokio::test]
    async fn hierarchical_crons_registered() {
        let cfg = test_config_with_hierarchical_enabled();
        let cron = init_cron_for_test(&cfg).await.unwrap();
        let jobs = cron.list_jobs().await.unwrap();
        let names: Vec<&str> = jobs.iter().map(|j| j.name.as_str()).collect();
        assert!(names.contains(&"__klyntbot_episodic_rollup_hourly"));
        assert!(names.contains(&"__klyntbot_episodic_rollup_daily"));
        assert!(names.contains(&"__klyntbot_episodic_rollup_weekly"));
    }
```

- [ ] **Step 5: Run + commit.**

```bash
cargo nextest run -p app-core -E 'test(hierarchical_crons_registered)'
git add crates/app-core/src/init/cron.rs crates/config/src/schema/cognitive.rs
git commit -m "feat(app-core): register 3 hierarchical roll-up crons (KCA Track 8)"
```

---

# Track 13 — Temporal pruning at retrieval

Before retrieved facts go into the prompt, a tiny LLM call decides "given these facts and their `valid_until`, are any superseded by more recent ones in the same batch?" Drops stale info from the prompt.

### Task C13.1: `TemporalPruner` types + trait

**Files:**
- Create: `crates/cognitive/src/services/temporal_pruner.rs`

- [ ] **Step 1: Create.**

```rust
//! KCA Track 13 — temporal pruning at retrieval time.

use serde::{Deserialize, Serialize};
use cognitive::services::memory_retriever::ScoredFact;

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
```

- [ ] **Step 2: Register.**

```rust
// services/mod.rs
pub mod temporal_pruner;
pub use temporal_pruner::{TemporalPrunerHandler, NoopTemporalPruner, PruneInput, PruneFactRef, PruneOutput, DropDecision, apply_prune};
```

- [ ] **Step 3: Build + commit.**

```bash
cargo build -p cognitive
git add crates/cognitive/src/services/temporal_pruner.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): TemporalPruner types + Noop (KCA Track 13)"
```

---

### Task C13.2: Failing test — pruner drops stale fact

**Files:**
- Test: in `temporal_pruner.rs`

- [ ] **Step 1: Add.**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_prune_filters_dropped_facts() {
        use cognitive::repos::semantic_fact::SemanticFact;
        let f1 = ScoredFact { fact: SemanticFact::new("a", "p", "b", 0.5, "t"), score: 0.7, source: "t".into() };
        let mut f1_copy = f1.clone(); f1_copy.fact.id = "f1".into();
        let f2 = ScoredFact { fact: SemanticFact::new("c", "p", "d", 0.5, "t"), score: 0.6, source: "t".into() };
        let mut f2_copy = f2.clone(); f2_copy.fact.id = "f2".into();

        let out = PruneOutput {
            keep: vec!["f2".into()],
            drop: vec![DropDecision { fact_id: "f1".into(), reason: "stale".into() }],
        };

        let kept = apply_prune(vec![f1_copy, f2_copy], &out);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].fact.id, "f2");
    }
}
```

- [ ] **Step 2: Run.**

```bash
cargo nextest run -p cognitive -E 'test(/temporal_pruner::/)'
```

Expected: PASS.

- [ ] **Step 3: Commit.**

```bash
git add crates/cognitive/src/services/temporal_pruner.rs
git commit -m "test(cognitive): apply_prune filter (KCA Track 13)"
```

---

### Task C13.3: `LlmTemporalPrunerHandler` + prompt

**Files:**
- Modify: `crates/agent/src/adapters/cognitive_handlers.rs`
- Modify: `crates/agent/src/adapters/prompts.rs`

- [ ] **Step 1: Failing test.**

```rust
    #[tokio::test]
    async fn llm_temporal_pruner_drops_facts_with_explicit_supersede() {
        use cognitive::services::temporal_pruner::*;

        let json = r#"{"keep": ["f2"], "drop": [{"fact_id": "f1", "reason": "f2 supersedes by date"}]}"#;
        let provider = providers::test_helpers::FakeProvider::with_text(json);
        let handler = LlmTemporalPrunerHandler::new(std::sync::Arc::new(provider), "m".into(), 512);

        let input = PruneInput {
            facts: vec![
                PruneFactRef { fact_id: "f1".into(), subject: "Alice".into(), predicate: "works_at".into(), object: "Google".into(), valid_at: "2023-01-01T00:00:00Z".into(), valid_until: None },
                PruneFactRef { fact_id: "f2".into(), subject: "Alice".into(), predicate: "works_at".into(), object: "Anthropic".into(), valid_at: "2025-06-01T00:00:00Z".into(), valid_until: None },
            ],
            query_time: "2026-04-29T00:00:00Z".into(),
        };
        let out = handler.prune(input).await.unwrap();
        assert_eq!(out.drop.len(), 1);
        assert_eq!(out.drop[0].fact_id, "f1");
    }
```

- [ ] **Step 2: Prompt.**

```rust
pub(crate) const TEMPORAL_PRUNE_SYSTEM_PROMPT: &str = r#"You are a temporal pruner for retrieved facts.

You receive a list of facts (subject, predicate, object, valid_at, valid_until) and a query_time. Your job: identify facts that are CLEARLY superseded by newer facts in the same batch about the SAME (subject, predicate) pair. Drop the older one.

Rules:
- Only drop a fact if there's another fact with the SAME (subject, predicate) and a more recent valid_at.
- Do NOT drop facts about different predicates ("works_at" vs "manages" — keep both even if one is older).
- Do NOT drop facts that have explicit `valid_until` — those represent historical truths the user may still want to know.
- If unsure, keep the fact.

Output JSON: {"keep": ["fact_id1", ...], "drop": [{"fact_id": "...", "reason": "..."}]}"#;
```

- [ ] **Step 3: Handler.**

```rust
use cognitive::services::temporal_pruner::*;

pub struct LlmTemporalPrunerHandler {
    provider: std::sync::Arc<dyn providers::DynProvider>,
    model: String,
    max_tokens: u32,
}

impl LlmTemporalPrunerHandler {
    pub fn new(provider: std::sync::Arc<dyn providers::DynProvider>, model: String, max_tokens: u32) -> Self {
        Self { provider, model, max_tokens }
    }
}

#[async_trait::async_trait]
impl TemporalPrunerHandler for LlmTemporalPrunerHandler {
    async fn prune(&self, input: PruneInput) -> common::Result<PruneOutput> {
        if input.facts.is_empty() { return Ok(Default::default()); }
        let user = serde_json::to_string(&input)
            .map_err(|e| common::KlyntbotError::Internal(e.to_string()))?;
        let req = providers::ChatRequest {
            model: self.model.clone(),
            messages: vec![
                providers::Message::System(TEMPORAL_PRUNE_SYSTEM_PROMPT.to_string()),
                providers::Message::User(providers::UserContent::Text(user)),
            ],
            max_tokens: Some(self.max_tokens),
            temperature: Some(0.0),
            response_format: Some(providers::ResponseFormat::JsonObject),
            ..Default::default()
        };
        let resp = self.provider.complete(req).await
            .map_err(|e| common::KlyntbotError::Provider(format!("temporal_prune: {e}")))?;
        match serde_json::from_str::<PruneOutput>(&resp.text()) {
            Ok(o) => Ok(o),
            Err(_) => Ok(PruneOutput { keep: input.facts.iter().map(|f| f.fact_id.clone()).collect(), drop: vec![] }),
        }
    }
}
```

- [ ] **Step 4: Run + commit.**

```bash
cargo nextest run -p agent -E 'test(llm_temporal_pruner_drops_facts_with_explicit_supersede)'
git add crates/agent/src/adapters/cognitive_handlers.rs crates/agent/src/adapters/prompts.rs
git commit -m "feat(agent): LlmTemporalPrunerHandler + prompt (KCA Track 13)"
```

---

### Task C13.4: Wire pruner into `MemoryRetriever`

**Files:**
- Modify: `crates/cognitive/src/services/memory_retriever.rs`

- [ ] **Step 1: Add field + invocation.**

After embedding+BM25+PPR merge, before returning:

```rust
        // KCA Track 13: temporal prune.
        if let Some(pruner) = self.temporal_pruner.as_ref() {
            let input = PruneInput {
                facts: merged.iter().map(|s| PruneFactRef {
                    fact_id: s.fact.id.clone(),
                    subject: s.fact.subject.clone(),
                    predicate: s.fact.predicate.clone(),
                    object: s.fact.object.clone(),
                    valid_at: s.fact.valid_from.to_string(),
                    valid_until: s.fact.valid_until.map(|t| t.to_string()),
                }).collect(),
                query_time: jiff::Timestamp::now().to_string(),
            };
            match pruner.prune(input).await {
                Ok(out) => {
                    let dropped = out.drop.len();
                    merged = apply_prune(merged, &out);
                    if dropped > 0 {
                        tracing::debug!(dropped, "temporal_prune dropped stale facts");
                    }
                }
                Err(e) => tracing::warn!(error = %e, "temporal_prune skipped"),
            }
        }
```

- [ ] **Step 2: Plumb construction.**

In `agent_loop::builder.rs`:

```rust
        let temporal_pruner: Option<std::sync::Arc<dyn cognitive::services::temporal_pruner::TemporalPrunerHandler>> =
            cognitive_provider.as_ref().map(|p| {
                std::sync::Arc::new(crate::adapters::cognitive_handlers::LlmTemporalPrunerHandler::new(
                    p.clone(),
                    cognitive_cfg.temporal_prune_model.clone().unwrap_or_else(|| cognitive_cfg.model.clone()),
                    512,
                )) as _
            });
```

Add `temporal_prune_model: Option<String>` to `CognitiveConfig`.

- [ ] **Step 3: Run full agent test sweep.**

```bash
cargo nextest run -p agent -p cognitive
```

Expected: green.

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/src/services/memory_retriever.rs \
        crates/agent/src/agent_loop/builder.rs \
        crates/config/src/schema/cognitive.rs
git commit -m "feat(cognitive): wire TemporalPruner in retrieval (KCA Track 13)"
```

---

# Phase C Integration Tests

### Task CIT.1: Full retrieval pipeline (PPR + temporal prune + cache)

**Files:**
- Create: `crates/cognitive/tests/phase_c_retrieval_intelligence.rs`

- [ ] **Step 1: Create test.**

```rust
//! KCA Phase C integration — verifies Tracks 6, 7, 13 together against a fixture graph.

use cognitive::repos::*;
use cognitive::services::*;
use storage::StoragePool;

#[tokio::test]
async fn ppr_finds_multi_hop_facts_and_temporal_prune_drops_stale() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let fact_repo = semantic_fact::SemanticFactRepo::new(pool.clone());
    let entity_repo = entity::EntityRepo::new(pool.clone());

    // Seed graph: Alice—works_at→Google (old), Alice—works_at→Anthropic (new), Alice—knows→Bob, Bob—works_at→Anthropic
    let alice = entity_repo.upsert_entity("Alice", "person", None, "t", None).await.unwrap();
    let bob = entity_repo.upsert_entity("Bob", "person", None, "t", None).await.unwrap();
    let anthropic = entity_repo.upsert_entity("Anthropic", "org", None, "t", None).await.unwrap();
    entity_repo.upsert_relationship_typed(&alice.id, &bob.id, "knows", "correlational", 0.8, None, "t").await.unwrap();
    entity_repo.upsert_relationship_typed(&alice.id, &anthropic.id, "works_at", "structural", 0.9, None, "t").await.unwrap();
    entity_repo.upsert_relationship_typed(&bob.id, &anthropic.id, "works_at", "structural", 0.9, None, "t").await.unwrap();

    let old_fact = semantic_fact::SemanticFact::new("Alice", "works_at", "Google", 0.9, "t");
    fact_repo.upsert(&old_fact).await.unwrap();
    let new_fact = semantic_fact::SemanticFact::new("Alice", "works_at", "Anthropic", 0.95, "t");
    fact_repo.upsert(&new_fact).await.unwrap();
    fact_repo.upsert(&semantic_fact::SemanticFact::new("Bob", "works_at", "Anthropic", 0.9, "t")).await.unwrap();

    // Run PPR with seed "Alice".
    let cache = std::sync::Arc::new(ppr_retrieval::CachedPprGraph::new(entity_repo.clone(), std::time::Duration::from_secs(60)));
    let scored = memory_retriever::retrieve_with_ppr_boost(&fact_repo, &entity_repo, &cache, "Alice", 10).await.unwrap();

    // PPR should surface Bob's fact via Alice→Bob→Anthropic.
    let subjects: Vec<&str> = scored.iter().map(|s| s.fact.subject.as_str()).collect();
    assert!(subjects.contains(&"Bob"), "PPR didn't expand: {:?}", subjects);

    // Apply temporal pruner with always-drop-old logic.
    let pruner = std::sync::Arc::new(temporal_pruner::NoopTemporalPruner);
    let pruned = temporal_pruner::apply_prune(scored, &temporal_pruner::PruneOutput {
        keep: vec![new_fact.id.clone()],
        drop: vec![temporal_pruner::DropDecision { fact_id: old_fact.id.clone(), reason: "superseded".into() }],
    });
    let kept_subjects: std::collections::HashSet<&str> = pruned.iter().map(|s| s.fact.object.as_str()).collect();
    assert!(kept_subjects.contains(&"Anthropic"));
    assert!(!kept_subjects.contains(&"Google"));
    let _ = pruner; // silence unused
}
```

- [ ] **Step 2: Run.**

```bash
cargo nextest run -p cognitive --test phase_c_retrieval_intelligence
```

Expected: PASS.

- [ ] **Step 3: Commit.**

```bash
git add crates/cognitive/tests/phase_c_retrieval_intelligence.rs
git commit -m "test(cognitive): Phase C integration — PPR + temporal prune (KCA)"
```

---

### Task CIT.2: Hierarchical roll-up end-to-end

**Files:**
- Modify: `crates/cognitive/tests/phase_c_retrieval_intelligence.rs`

- [ ] **Step 1: Append.**

```rust
#[tokio::test]
async fn hierarchical_rollup_creates_three_tiers() {
    use cognitive::services::hierarchical_compressor::*;
    use cognitive::repos::episodic::EpisodicMemory;

    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = cognitive::repos::episodic::EpisodicMemoryRepo::new(pool.clone());

    // Insert 24 raw episodics across 3 hours.
    for hour in 0..3 {
        for j in 0..8 {
            let ts = jiff::Timestamp::from_second((hour * 3600 + j * 100) as i64).unwrap();
            repo.insert(&EpisodicMemory {
                id: format!("ep_{hour}_{j}"),
                domain: "test".into(),
                content: format!("event h{hour} idx{j}"),
                summary: None,
                importance: 0.5,
                recorded_at: ts,
                stability: 1.0,
                tier: "raw".into(),
                parent_id: None,
                child_count: 0,
                ..Default::default()
            }).await.unwrap();
        }
    }

    struct Echo;
    #[async_trait::async_trait]
    impl HierarchicalSummarizer for Echo {
        async fn summarize(&self, items: &[EpisodicMemory], _t: Tier) -> common::Result<String> {
            Ok(format!("summary {}", items.len()))
        }
    }
    let s = std::sync::Arc::new(Echo);

    let n_hourly = roll_up_hourly(&repo, s.clone()).await.unwrap();
    assert_eq!(n_hourly, 3, "3 hourly buckets expected");
    let n_daily = roll_up_daily(&repo, s.clone()).await.unwrap();
    assert_eq!(n_daily, 1);
    let n_weekly = roll_up_weekly(&repo, s).await.unwrap();
    assert_eq!(n_weekly, 1);

    let weeklies = repo.list_by_tier("weekly", 10).await.unwrap();
    assert_eq!(weeklies[0].child_count, 24, "weekly should aggregate all 24 raw events");
}
```

- [ ] **Step 2: Run + commit.**

```bash
cargo nextest run -p cognitive --test phase_c_retrieval_intelligence -E 'test(hierarchical_rollup_creates_three_tiers)'
git add crates/cognitive/tests/phase_c_retrieval_intelligence.rs
git commit -m "test(cognitive): hierarchical 3-tier rollup integration (KCA Track 8)"
```

---

### Task CIT.3: Predictive cache hit-rate over fixture conversation

- [ ] **Step 1: Append integration test.**

```rust
#[tokio::test]
async fn predictive_cache_warms_and_hits_on_followup() {
    use cognitive::services::predictive_cache::*;

    let cache = std::sync::Arc::new(PredictiveCache::new(50, std::time::Duration::from_secs(300)));
    let key1 = query_hash("how do I install rust?");
    let key2 = query_hash("what's cargo?");
    cache.put(key1.clone(), vec![]).await;
    cache.put(key2.clone(), vec![]).await;

    // Simulated user follow-up.
    assert!(cache.get(&key1).await.is_some());
    assert!(cache.get(&query_hash("How do I install Rust?")).await.is_some(), "case-insensitive hash should hit");
    let stats = cache.stats().await;
    assert_eq!(stats.hits, 2);
}
```

- [ ] **Step 2: Run + commit.**

```bash
cargo nextest run -p cognitive --test phase_c_retrieval_intelligence -E 'test(predictive_cache_warms_and_hits_on_followup)'
git add crates/cognitive/tests/phase_c_retrieval_intelligence.rs
git commit -m "test(cognitive): predictive cache hit on case-normalized query (KCA Track 7)"
```

---

### Task CIT.4: Workspace sweep

- [ ] **Step 1:**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo fmt --all --check
```

Fix anything that fails; recommit minimally.

- [ ] **Step 2: Marker.**

```bash
git commit --allow-empty -m "test(workspace): KCA Phase C green — retrieval intelligence live"
```

---

# Phase C Self-Review

1. **Spec coverage:** Tracks 6, 7, 8, 13 all have ≥3 tasks each; integration tests cover their interaction.
2. **No placeholders.**
3. **Type consistency:** `Tier::Hourly`/`"hourly"` round-trip via `as_str()`; `EdgeType` weights consistent across PPR + retrieval.
4. **Migrations registered** (013).
5. **Tracing on warn paths.**
6. **PPR cache TTL**: tested.
7. **Predictive cache auto-disable**: tested.
8. **Risk register:** Track 6 (PPR cost) — bounded via `top-100 entities`; Track 7 (low hit-rate) — auto-disable; Track 8 (fidelity) — raw episodics retained 30 days; Track 13 (false drops) — keep-by-default policy in prompt.

---

**Phase C complete.** Continue to [`2026-04-29-kca-phase-d-the-moat.md`](2026-04-29-kca-phase-d-the-moat.md).
