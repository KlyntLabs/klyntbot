# Relational Community Graph Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Louvain community detection over tree nodes with entity bridges, enabling cross-note synthesis via NoteTreeNavigator Path 4 and a 10-factor relevance scorer.

**Architecture:** Tree nodes (from Phase 1) become graph vertices. Edges are formed between tree nodes that share entities via `entity_tree_links`, weighted by `entity_relationships.strength`. Louvain discovers communities. A new `CommunityBuilder` event subscriber runs detection on note changes (debounced 5s). NoteTreeNavigator gains Path 4 (community traversal) using community summary embeddings. `GraphSearcher` is deleted.

**Tech Stack:** Rust (cognitive/context_engine/agent/storage/bus/common crates), petgraph (Louvain), LanceDB (community_embeddings), SQLite (communities, community_members), TypeScript/React (community card UI)

**Spec:** `docs/superpowers/specs/2026-03-28-community-graph-design.md`

---

### Task 1: SQLite Migration — communities + community_members Tables

**Files:**
- Create: `crates/cognitive/migrations/004_community_graph.sql`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Create migration SQL**

Create `crates/cognitive/migrations/004_community_graph.sql`:

```sql
-- Community graph tables for Phase 2 Cognitive Fabric
-- Communities are clusters of related tree nodes discovered by Louvain
-- over shared-entity edges.

CREATE TABLE IF NOT EXISTS communities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    summary TEXT NOT NULL,
    member_count INTEGER NOT NULL DEFAULT 0,
    modularity_score REAL,
    stability REAL NOT NULL DEFAULT 1.0,
    top_entities TEXT,
    representative_paths TEXT,
    source_note_count INTEGER DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE IF NOT EXISTS community_members (
    community_id TEXT NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    tree_node_id TEXT NOT NULL REFERENCES book_tree_nodes(id) ON DELETE CASCADE,
    membership_score REAL NOT NULL DEFAULT 0.0,
    joined_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (community_id, tree_node_id)
);

CREATE INDEX IF NOT EXISTS idx_community_members_node ON community_members(tree_node_id);
CREATE INDEX IF NOT EXISTS idx_communities_stability ON communities(stability);
```

- [ ] **Step 2: Register migration**

In `crates/cognitive/src/repos/mod.rs`, add to the `cognitive_migrations()` vec:

```rust
FeatureMigration {
    feature_name: "cognitive_community".to_string(),
    version: 1,
    description: "Community graph tables for Louvain community detection".to_string(),
    sql: include_str!("../../migrations/004_community_graph.sql").to_string(),
},
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p cognitive`

- [ ] **Step 4: Commit**

```bash
git add crates/cognitive/migrations/004_community_graph.sql crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add community graph SQLite tables (communities + community_members)"
```

---

### Task 2: CommunityRepo — SQLite CRUD

**Files:**
- Create: `crates/cognitive/src/repos/community.rs`
- Modify: `crates/cognitive/src/repos/mod.rs`

- [ ] **Step 1: Define CommunityRow and CommunityMemberRow structs**

Create `crates/cognitive/src/repos/community.rs`:

```rust
use common::Result;
use sqlx::SqlitePool;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommunityRow {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub member_count: i64,
    pub modularity_score: Option<f64>,
    pub stability: f64,
    pub top_entities: Option<String>,       // JSON array
    pub representative_paths: Option<String>, // JSON array
    pub source_note_count: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct CommunityMemberRow {
    pub community_id: String,
    pub tree_node_id: String,
    pub membership_score: f64,
    pub joined_at: String,
}
```

- [ ] **Step 2: Implement CommunityRepo**

```rust
pub struct CommunityRepo {
    pool: SqlitePool,
}

impl CommunityRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert_community(&self, community: &CommunityRow) -> Result<()> {
        sqlx::query(
            "INSERT INTO communities (id, name, summary, member_count, modularity_score, stability, top_entities, representative_paths, source_note_count, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(id) DO UPDATE SET
               name = ?2, summary = ?3, member_count = ?4, modularity_score = ?5,
               stability = ?6, top_entities = ?7, representative_paths = ?8,
               source_note_count = ?9, updated_at = ?11"
        )
        .bind(&community.id).bind(&community.name).bind(&community.summary)
        .bind(community.member_count).bind(community.modularity_score)
        .bind(community.stability).bind(&community.top_entities)
        .bind(&community.representative_paths).bind(community.source_note_count)
        .bind(&community.created_at).bind(&community.updated_at)
        .execute(&self.pool).await?;
        Ok(())
    }

    pub async fn set_members(
        &self,
        community_id: &str,
        members: &[(String, f64)], // (tree_node_id, membership_score)
    ) -> Result<()> {
        // Clear old members
        sqlx::query("DELETE FROM community_members WHERE community_id = ?1")
            .bind(community_id)
            .execute(&self.pool).await?;
        // Insert new
        for (node_id, score) in members {
            sqlx::query(
                "INSERT INTO community_members (community_id, tree_node_id, membership_score)
                 VALUES (?1, ?2, ?3)"
            )
            .bind(community_id).bind(node_id).bind(score)
            .execute(&self.pool).await?;
        }
        Ok(())
    }

    pub async fn get_community(&self, id: &str) -> Result<Option<CommunityRow>> {
        let row = sqlx::query_as::<_, CommunityRow>(
            "SELECT * FROM communities WHERE id = ?1"
        ).bind(id).fetch_optional(&self.pool).await?;
        Ok(row)
    }

    pub async fn list_active_communities(&self) -> Result<Vec<CommunityRow>> {
        let rows = sqlx::query_as::<_, CommunityRow>(
            "SELECT * FROM communities WHERE stability >= 0.3 ORDER BY member_count DESC"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_members(&self, community_id: &str) -> Result<Vec<CommunityMemberRow>> {
        let rows = sqlx::query_as::<_, CommunityMemberRow>(
            "SELECT * FROM community_members WHERE community_id = ?1 ORDER BY membership_score DESC"
        ).bind(community_id).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn get_communities_for_node(&self, tree_node_id: &str) -> Result<Vec<CommunityRow>> {
        let rows = sqlx::query_as::<_, CommunityRow>(
            "SELECT c.* FROM communities c
             JOIN community_members cm ON cm.community_id = c.id
             WHERE cm.tree_node_id = ?1 AND c.stability >= 0.3
             ORDER BY cm.membership_score DESC"
        ).bind(tree_node_id).fetch_all(&self.pool).await?;
        Ok(rows)
    }

    pub async fn prune_weak_communities(&self, min_stability: f64) -> Result<Vec<String>> {
        let pruned = sqlx::query_scalar::<_, String>(
            "SELECT id FROM communities WHERE stability < ?1"
        ).bind(min_stability).fetch_all(&self.pool).await?;

        if !pruned.is_empty() {
            sqlx::query("DELETE FROM communities WHERE stability < ?1")
                .bind(min_stability)
                .execute(&self.pool).await?;
        }
        Ok(pruned)
    }

    /// Load all entity-shared edges between tree nodes for graph construction.
    /// Returns (node_a, node_b, weight) where weight = count(shared_entities) * avg(strength).
    pub async fn load_shared_entity_edges(&self) -> Result<Vec<(String, String, f64)>> {
        let rows = sqlx::query_as::<_, (String, String, f64)>(
            "SELECT a.tree_node_id AS node_a, b.tree_node_id AS node_b,
                    COUNT(DISTINCT a.entity_id) * COALESCE(AVG(er.strength), 0.5) AS weight
             FROM entity_tree_links a
             JOIN entity_tree_links b ON a.entity_id = b.entity_id AND a.tree_node_id < b.tree_node_id
             LEFT JOIN entity_relationships er
               ON (er.source_entity_id = a.entity_id OR er.target_entity_id = a.entity_id)
             GROUP BY a.tree_node_id, b.tree_node_id
             HAVING COUNT(DISTINCT a.entity_id) >= 1"
        ).fetch_all(&self.pool).await?;
        Ok(rows)
    }
}
```

- [ ] **Step 3: Add module declaration and export**

In `crates/cognitive/src/repos/mod.rs`, add:

```rust
pub mod community;
pub use community::{CommunityRepo, CommunityRow, CommunityMemberRow};
```

- [ ] **Step 4: Build and verify**

Run: `cargo build -p cognitive`

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/src/repos/community.rs crates/cognitive/src/repos/mod.rs
git commit -m "feat(cognitive): add CommunityRepo with CRUD + shared-entity edge loader"
```

---

### Task 3: LanceDB community_embeddings Table

**Files:**
- Modify: `crates/storage/src/vector_store/schemas.rs`
- Modify: `crates/storage/src/vector_store/mod.rs`
- Create: `crates/storage/src/vector_store/community.rs`

- [ ] **Step 1: Add schema function**

In `crates/storage/src/vector_store/schemas.rs`, add:

```rust
pub(crate) fn community_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),
        Field::new("member_count", DataType::Utf8, false),
        Field::new("source_note_count", DataType::Utf8, false),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}
```

- [ ] **Step 2: Register table in VectorStore::connect()**

In `crates/storage/src/vector_store/mod.rs`, after the last `ensure_table` call, add:

```rust
store
    .ensure_table("community_embeddings", schemas::community_embedding_schema())
    .await?;
```

- [ ] **Step 3: Create community.rs with search/upsert helpers**

Create `crates/storage/src/vector_store/community.rs` following the `tree_node.rs` pattern:

```rust
use crate::vector_store::VectorStore;
use common::Result;

pub struct CommunitySearchResult {
    pub community_id: String,
    pub member_count: String,
    pub source_note_count: String,
    pub score: f64,
}

impl VectorStore {
    pub async fn upsert_community_embedding(
        &self,
        community_id: &str,
        embedding: &[f32],
        member_count: &str,
        source_note_count: &str,
    ) -> Result<()> {
        self.upsert_embedding(
            "community_embeddings",
            community_id,
            embedding,
            &[
                ("member_count", member_count),
                ("source_note_count", source_note_count),
            ],
        )
        .await
    }

    pub async fn search_community_embeddings(
        &self,
        query_vector: &[f32],
        limit: usize,
        min_similarity: f64,
    ) -> Result<Vec<CommunitySearchResult>> {
        // Follow tree_node.rs pattern: open table, nearest_to, collect, score
        // Return results filtered by min_similarity, sorted desc
        todo!() // Implementer fills this following tree_node.rs pattern exactly
    }

    pub async fn delete_community_embedding(&self, community_id: &str) -> Result<()> {
        let filter = format!(
            "id = '{}'",
            crate::vector_store::sanitize_predicate_value(community_id)
        );
        self.delete_by_filter("community_embeddings", &filter).await
    }
}
```

Note: The `search_community_embeddings` method should follow the exact same pattern as `search_tree_node_embeddings` in `tree_node.rs` — open table, `nearest_to(query_vector).limit(limit)`, collect batches, extract `id` and `_distance`, compute `score = 1.0 - distance`, filter by `min_similarity`, sort descending. Read `tree_node.rs` for the complete implementation.

- [ ] **Step 4: Add module declaration**

In `crates/storage/src/vector_store/mod.rs`, add:

```rust
mod community;
pub use community::CommunitySearchResult;
```

- [ ] **Step 5: Build and verify**

Run: `cargo build -p storage`

- [ ] **Step 6: Commit**

```bash
git add crates/storage/src/vector_store/
git commit -m "feat(storage): add community_embeddings LanceDB table and helpers"
```

---

### Task 4: Bus Layer Extensions — Community Events

**Files:**
- Modify: `crates/bus/src/context_updates.rs`

- [ ] **Step 1: Add 3 new variants to ContextUpdateReason**

In `crates/bus/src/context_updates.rs`, add to the `ContextUpdateReason` enum (before `Custom`):

```rust
CommunityDiscovered,
CommunityUpdated,
CommunityWeakened,
```

Update the `as_str()` method with:

```rust
Self::CommunityDiscovered => "community_discovered",
Self::CommunityUpdated => "community_updated",
Self::CommunityWeakened => "community_weakened",
```

- [ ] **Step 2: Build and verify**

Run: `cargo build -p bus`

- [ ] **Step 3: Commit**

```bash
git add crates/bus/src/context_updates.rs
git commit -m "feat(bus): add CommunityDiscovered/Updated/Weakened context update reasons"
```

---

### Task 5: Extend Scoring Model (8-Factor → 10-Factor)

**Files:**
- Modify: `crates/cognitive/src/services/decay.rs`
- Modify: `crates/cognitive/src/services/retrieval.rs`

- [ ] **Step 1: Add 2 new fields to RelevanceWeights**

In `crates/cognitive/src/services/decay.rs`, add to `RelevanceWeights`:

```rust
pub community: f64,
pub cross_note: f64,
```

Update `Default`:

```rust
impl Default for RelevanceWeights {
    fn default() -> Self {
        Self {
            semantic: 0.20,
            retrievability: 0.10,
            importance: 0.08,
            frequency: 0.05,
            situation: 0.15,
            temporal: 0.02,
            hierarchy: 0.10,
            path_coherence: 0.05,
            community: 0.15,
            cross_note: 0.10,
        }
    }
}
```

- [ ] **Step 2: Extend relevance_score function**

Add two new parameters: `community_score: f64` and `cross_note_boost: f64`. Add to weighted sum.

- [ ] **Step 3: Fix all existing callers**

Add `0.0, 0.0` (neutral community, neutral cross_note) to all existing call sites. Main callers in `crates/cognitive/src/services/retrieval.rs`.

- [ ] **Step 4: Add tests**

Add tests for the 10-factor scorer, verifying backward compat and correct weighting.

- [ ] **Step 5: Build workspace and run tests**

Run: `cargo build --workspace && cargo nextest run -p cognitive -E 'test(relevance_score)'`

- [ ] **Step 6: Commit**

```bash
git add crates/cognitive/src/services/decay.rs crates/cognitive/src/services/retrieval.rs
git commit -m "feat(cognitive): extend relevance scorer from 8-factor to 10-factor (community + cross_note)"
```

---

### Task 6: Autotuner Parameter Expansion (24D → 28D)

**Files:**
- Modify: `crates/common/src/autotuner.rs`
- Modify: `crates/autotuner/src/generator.rs`

- [ ] **Step 1: Add 4 new fields to TrialParams**

In `crates/common/src/autotuner.rs`, add after Phase 4 fields:

```rust
// Phase 5: Community graph (4 params)
pub relevance_weight_community: Option<f64>,
pub relevance_weight_cross_note: Option<f64>,
pub community_top_k: Option<usize>,
pub community_min_similarity: Option<f64>,
```

- [ ] **Step 2: Add bounds to generator prompt**

In `crates/autotuner/src/generator.rs`, add 4 rows to the bounds table:

```
| relevance_weight_community    | 0.00  | 0.30  | 0.01  | community_membership weight in 10-factor model |
| relevance_weight_cross_note   | 0.00  | 0.20  | 0.01  | cross_note_boost weight |
| community_top_k               | 3     | 15    | 1     | top-k for community_embeddings search |
| community_min_similarity      | 0.30  | 0.70  | 0.05  | min cosine similarity for communities |
```

- [ ] **Step 3: Build and verify**

Run: `cargo build -p common -p autotuner`

- [ ] **Step 4: Commit**

```bash
git add crates/common/src/autotuner.rs crates/autotuner/src/generator.rs
git commit -m "feat(autotuner): expand search space from 24D to 28D for community graph"
```

---

### Task 7: Louvain Community Detection Algorithm

**Files:**
- Modify: `crates/cognitive/Cargo.toml` (add petgraph dependency)
- Create: `crates/cognitive/src/services/louvain.rs`
- Modify: `crates/cognitive/src/services/mod.rs`

- [ ] **Step 1: Add petgraph dependency**

In `crates/cognitive/Cargo.toml`, add under `[dependencies]`:

```toml
petgraph = "0.7"
```

- [ ] **Step 2: Implement Louvain**

Create `crates/cognitive/src/services/louvain.rs`:

Implement the Louvain modularity optimization algorithm:

```rust
use petgraph::graph::{NodeIndex, UnGraph};
use std::collections::HashMap;

/// Result of Louvain community detection.
pub struct CommunityAssignment {
    /// Map from node ID (String) to community ID (usize)
    pub assignments: HashMap<String, usize>,
    /// Total modularity score
    pub modularity: f64,
    /// Number of communities found
    pub community_count: usize,
}

/// Run Louvain community detection on a weighted undirected graph.
///
/// Input: `edges` is a list of (node_a_id, node_b_id, weight).
/// Returns community assignments for each node.
pub fn detect_communities(edges: &[(String, String, f64)]) -> CommunityAssignment {
    // Build petgraph
    let mut graph = UnGraph::<String, f64>::new_undirected();
    let mut node_map: HashMap<String, NodeIndex> = HashMap::new();
    let mut id_map: HashMap<NodeIndex, String> = HashMap::new();

    for (a, b, w) in edges {
        let na = *node_map.entry(a.clone()).or_insert_with(|| {
            let idx = graph.add_node(a.clone());
            id_map.insert(idx, a.clone());
            idx
        });
        let nb = *node_map.entry(b.clone()).or_insert_with(|| {
            let idx = graph.add_node(b.clone());
            id_map.insert(idx, b.clone());
            idx
        });
        graph.add_edge(na, nb, *w);
    }

    if graph.node_count() == 0 {
        return CommunityAssignment {
            assignments: HashMap::new(),
            modularity: 0.0,
            community_count: 0,
        };
    }

    // Louvain Phase 1: local moves
    let total_weight: f64 = graph.edge_weights().sum::<f64>() * 2.0;
    if total_weight == 0.0 {
        // No edges — each node is its own community
        let assignments: HashMap<String, usize> = node_map
            .into_iter()
            .enumerate()
            .map(|(i, (id, _))| (id, i))
            .collect();
        let count = assignments.len();
        return CommunityAssignment { assignments, modularity: 0.0, community_count: count };
    }

    // Initialize: each node in its own community
    let mut community: HashMap<NodeIndex, usize> = graph
        .node_indices()
        .enumerate()
        .map(|(i, n)| (n, i))
        .collect();
    let mut next_community_id = graph.node_count();

    let mut improved = true;
    while improved {
        improved = false;
        for node in graph.node_indices() {
            let current_comm = community[&node];

            // Calculate modularity gain for moving to each neighbor's community
            let mut best_comm = current_comm;
            let mut best_gain = 0.0_f64;

            let node_weight: f64 = graph.edges(node).map(|e| *e.weight()).sum();

            let neighbor_comms: Vec<usize> = graph
                .neighbors(node)
                .map(|n| community[&n])
                .collect();

            for &target_comm in &neighbor_comms {
                if target_comm == current_comm {
                    continue;
                }

                // Sum of edge weights from node to target community
                let edge_to_target: f64 = graph.edges(node)
                    .filter(|e| community[&e.target()] == target_comm || community[&e.source()] == target_comm)
                    .map(|e| *e.weight())
                    .sum();

                // Sum of weights inside target community
                let comm_weight: f64 = graph.node_indices()
                    .filter(|&n| community[&n] == target_comm)
                    .flat_map(|n| graph.edges(n).map(|e| *e.weight()))
                    .sum::<f64>();

                let gain = edge_to_target - (node_weight * comm_weight) / total_weight;

                if gain > best_gain {
                    best_gain = gain;
                    best_comm = target_comm;
                }
            }

            if best_comm != current_comm {
                community.insert(node, best_comm);
                improved = true;
            }
        }
    }

    // Renumber communities to 0..N
    let mut comm_renumber: HashMap<usize, usize> = HashMap::new();
    let mut next_id = 0;
    let assignments: HashMap<String, usize> = graph
        .node_indices()
        .map(|n| {
            let comm = community[&n];
            let new_comm = *comm_renumber.entry(comm).or_insert_with(|| {
                let id = next_id;
                next_id += 1;
                id
            });
            (id_map[&n].clone(), new_comm)
        })
        .collect();

    // Calculate modularity
    let modularity = calculate_modularity(&graph, &community, total_weight);

    CommunityAssignment {
        assignments,
        modularity,
        community_count: next_id,
    }
}

fn calculate_modularity(
    graph: &UnGraph<String, f64>,
    community: &HashMap<NodeIndex, usize>,
    total_weight: f64,
) -> f64 {
    let mut q = 0.0;
    for edge in graph.edge_references() {
        let (a, b) = (edge.source(), edge.target());
        if community[&a] == community[&b] {
            let w = *edge.weight();
            let ka: f64 = graph.edges(a).map(|e| *e.weight()).sum();
            let kb: f64 = graph.edges(b).map(|e| *e.weight()).sum();
            q += w - (ka * kb) / total_weight;
        }
    }
    q / total_weight
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_graph() {
        let result = detect_communities(&[]);
        assert_eq!(result.community_count, 0);
        assert!(result.assignments.is_empty());
    }

    #[test]
    fn test_single_edge() {
        let edges = vec![("a".into(), "b".into(), 1.0)];
        let result = detect_communities(&edges);
        assert!(result.community_count >= 1);
        // a and b should be in the same community
        assert_eq!(result.assignments["a"], result.assignments["b"]);
    }

    #[test]
    fn test_two_clusters() {
        let edges = vec![
            // Cluster 1: tight
            ("a".into(), "b".into(), 1.0),
            ("b".into(), "c".into(), 1.0),
            ("a".into(), "c".into(), 1.0),
            // Cluster 2: tight
            ("d".into(), "e".into(), 1.0),
            ("e".into(), "f".into(), 1.0),
            ("d".into(), "f".into(), 1.0),
            // Weak bridge between clusters
            ("c".into(), "d".into(), 0.1),
        ];
        let result = detect_communities(&edges);
        // Should find 2 communities
        assert!(result.community_count >= 2);
        // a, b, c should be together
        assert_eq!(result.assignments["a"], result.assignments["b"]);
        assert_eq!(result.assignments["b"], result.assignments["c"]);
        // d, e, f should be together
        assert_eq!(result.assignments["d"], result.assignments["e"]);
        assert_eq!(result.assignments["e"], result.assignments["f"]);
        // The two clusters should be different
        assert_ne!(result.assignments["a"], result.assignments["d"]);
    }

    #[test]
    fn test_modularity_positive() {
        let edges = vec![
            ("a".into(), "b".into(), 1.0),
            ("b".into(), "c".into(), 1.0),
            ("a".into(), "c".into(), 1.0),
        ];
        let result = detect_communities(&edges);
        assert!(result.modularity >= 0.0);
    }
}
```

- [ ] **Step 3: Add module declaration**

In `crates/cognitive/src/services/mod.rs`, add:

```rust
pub mod louvain;
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p cognitive -E 'test(louvain)'`
Expected: All 4 tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/cognitive/Cargo.toml crates/cognitive/src/services/louvain.rs crates/cognitive/src/services/mod.rs
git commit -m "feat(cognitive): implement Louvain community detection over petgraph"
```

---

### Task 8: CommunityBuilder Event Subscriber

**Files:**
- Create: `crates/agent/src/adapters/community_builder.rs`
- Modify: `crates/agent/src/adapters/mod.rs`

- [ ] **Step 1: Read existing patterns**

Read these files for exact subscriber patterns:
- `crates/agent/src/adapters/note_tree_builder.rs` — the Phase 1 subscriber pattern
- `crates/cognitive/src/services/louvain.rs` — the `detect_communities` function
- `crates/cognitive/src/repos/community.rs` — `CommunityRepo` CRUD
- `crates/bus/src/context_updates.rs` — `ContextUpdateQueue`, `ContextUpdate`, new community reasons

- [ ] **Step 2: Create CommunityBuilder**

Create `crates/agent/src/adapters/community_builder.rs`:

The subscriber should:

1. **Struct fields:**
   - `community_repo: CommunityRepo`
   - `vector_store: Arc<VectorStore>`
   - `embedder: Arc<dyn TextEmbedder>`
   - `context_update_queue: Option<Arc<ContextUpdateQueue>>`
   - `tree_repo: Arc<dyn BookTreeRepo>`
   - `debounce_duration: Duration` (default 5s)

2. **`run` method:** Same pattern as `NoteTreeBuilder::run`:
   - `tokio::select!` with `shutdown.cancelled()` and `rx.recv()`
   - Listen for `DomainEvent::NoteContentChanged`
   - Debounce: collect note_ids for 5 seconds before processing
   - Call `rebuild_communities()` after debounce window

3. **`rebuild_communities` method (pub, for backfill):**
   - Load shared-entity edges from `community_repo.load_shared_entity_edges()`
   - Run `louvain::detect_communities(&edges)`
   - Group assignments into communities
   - For each community:
     a. Load member tree nodes from `tree_repo`
     b. Compute `membership_score` = avg phase-1 scorer factors × entity strength
     c. Compose summary from top-5 members (reuse `compose_node_text` logic)
     d. Derive `representative_paths` from top-3 members
     e. Derive `top_entities` from shared entities across members
     f. Name: join top entity names for <5 members; for ≥5 members, fire-and-forget LLM call
     g. Embed summary → `vector_store.upsert_community_embedding()`
     h. Upsert community + set_members via `community_repo`
   - Diff against previous run:
     - New communities (≥3 members) → push `CommunityDiscovered`
     - Changed (>20% membership change) → push `CommunityUpdated`
     - Stability < 0.3 → prune + push `CommunityWeakened`
   - Rich payload format for context updates

4. **Debounce implementation:** Use `tokio::time::sleep` + event collection in a `Vec<String>` (note IDs). When 5s passes with no new events, process. Reset timer on each new event.

- [ ] **Step 3: Add module declaration**

In `crates/agent/src/adapters/mod.rs`, add:

```rust
pub mod community_builder;
```

- [ ] **Step 4: Build**

Run: `cargo build -p agent`

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/adapters/community_builder.rs crates/agent/src/adapters/mod.rs
git commit -m "feat(agent): add CommunityBuilder event subscriber with Louvain detection"
```

---

### Task 9: NoteTreeNavigator Path 4 — Community Traversal

**Files:**
- Modify: `crates/context_engine/src/insight_forge/note_tree_navigator.rs`

- [ ] **Step 1: Read current NoteTreeNavigator**

Read the full file to understand:
- `QueryType` enum (currently 3 variants)
- `classify_query()` function
- `TreeNodeEmbeddingSearch` trait
- The 3 existing search paths
- `DomainSearcher::search()` dispatch

- [ ] **Step 2: Add CommunityEmbeddingSearch trait**

Add a new trait for community vector search (dependency inversion — context_engine can't import agent):

```rust
#[async_trait]
pub trait CommunityEmbeddingSearch: Send + Sync {
    async fn search_communities(
        &self,
        query_embedding: &[f32],
        limit: usize,
        min_similarity: f64,
    ) -> common::Result<Vec<CommunityHit>>;
}

pub struct CommunityHit {
    pub community_id: String,
    pub member_count: usize,
    pub source_note_count: usize,
    pub score: f64,
}
```

- [ ] **Step 3: Add CommunityMemberLoader trait**

```rust
#[async_trait]
pub trait CommunityMemberLoader: Send + Sync {
    async fn get_community_members(
        &self,
        community_id: &str,
    ) -> common::Result<Vec<CommunityMember>>;

    async fn get_community_info(
        &self,
        community_id: &str,
    ) -> common::Result<Option<CommunityInfo>>;
}

pub struct CommunityMember {
    pub tree_node_id: String,
    pub membership_score: f64,
}

pub struct CommunityInfo {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub top_entities: Vec<String>,
    pub representative_paths: Vec<String>,
    pub source_note_count: usize,
    pub stability: f64,
}
```

- [ ] **Step 4: Extend QueryType + classify_query**

Add `Community` variant to `QueryType`.

Extend `classify_query()` with community keywords: "across my notes", "connect", "related to", "how does X relate to Y", "community", "cluster", "summarize my thoughts on", "everything about".

Add proactive triggering: if `active_task` exists and query mentions an entity in a known community → `Community`.

- [ ] **Step 5: Implement community_search method**

```rust
async fn community_search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
    // 1. Embed query
    // 2. Search community_embeddings via CommunityEmbeddingSearch
    // 3. For each top community:
    //    a. Load members via CommunityMemberLoader
    //    b. For top members: load tree node + build path (reuse existing)
    //    c. Format as MemoryEntry with community card metadata
    // 4. Return ranked results
}
```

- [ ] **Step 6: Wire Path 4 into DomainSearcher::search()**

In the `search()` method dispatch, add:

```rust
QueryType::Community => self.community_search(query, limit).await,
```

Also handle Hybrid + Community fusion: when both hierarchical and community intent, run both concurrently.

- [ ] **Step 7: Add tests for query classification**

Add tests:
- `test_classify_community_query` — "across my notes" → Community
- `test_classify_synthesis_query` — "summarize my thoughts on sleep" → Community
- `test_classify_connect_query` — "how does X relate to Y" → Community

- [ ] **Step 8: Build and test**

Run: `cargo build -p context_engine && cargo nextest run -p context_engine -E 'test(classify)'`

- [ ] **Step 9: Commit**

```bash
git add crates/context_engine/src/insight_forge/note_tree_navigator.rs
git commit -m "feat(context-engine): add NoteTreeNavigator Path 4 community traversal"
```

---

### Task 10: Community Search Adapter + Delete GraphSearcher + Rewire Builder

**Files:**
- Create: `crates/agent/src/adapters/community_search.rs`
- Delete: `crates/agent/src/domain_searchers/graph_searcher.rs`
- Modify: `crates/agent/src/domain_searchers/mod.rs`
- Modify: `crates/agent/src/adapters/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Create CommunitySearchAdapter**

Create `crates/agent/src/adapters/community_search.rs`:

Implements `CommunityEmbeddingSearch` and `CommunityMemberLoader` traits from `note_tree_navigator.rs`, bridging to concrete `VectorStore` + `CommunityRepo`.

- [ ] **Step 2: Delete GraphSearcher**

Remove `crates/agent/src/domain_searchers/graph_searcher.rs`.
Remove its module declaration from `crates/agent/src/domain_searchers/mod.rs`.

- [ ] **Step 3: Rewire builder.rs**

In `crates/agent/src/agent_loop/builder.rs`:

a. Remove GraphSearcher construction and `forge.add_searcher(Arc::new(GraphSearcher::new(...)))` (around L745-L748).

b. Create `CommunitySearchAdapter` and inject into `NoteTreeNavigator`:

```rust
// Community search adapter
let community_search = Arc::new(CommunitySearchAdapter::new(
    vector_store.clone(),
    community_repo.clone(),
));

// Update NoteTreeNavigator to accept community search
let note_tree_navigator = NoteTreeNavigator::new(
    tree_node_search,
    tree_repo.clone(),
)
.with_community_search(community_search);
```

c. Start `CommunityBuilder` subscriber:

```rust
let community_builder = Arc::new(CommunityBuilder::new(
    community_repo.clone(),
    vector_store.clone(),
    text_embedder.clone(),
    tree_repo.clone(),
    context_update_queue.clone(),
));
let comm_builder_rx = bus.subscribe();
let comm_builder_shutdown = shutdown_token.clone();
tokio::spawn(async move {
    community_builder.run(comm_builder_rx, comm_builder_shutdown).await;
});
```

- [ ] **Step 4: Build full workspace**

Run: `cargo build --workspace`

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 6: Run tests**

Run: `cargo nextest run --workspace`

- [ ] **Step 7: Commit**

```bash
git add -A
git commit -m "feat(agent): wire CommunityBuilder + Path 4, delete GraphSearcher"
```

---

### Task 11: Frontend — Community Card Component

**Files:**
- Create: `desktop-ui/src/features/chat/components/CommunityCard.tsx`
- Modify: `desktop-ui/src/features/chat/components/MessageList.tsx`
- Modify: `desktop-ui/src/shared/types/tree-path.ts`

- [ ] **Step 1: Define CommunityCard types**

In `desktop-ui/src/shared/types/tree-path.ts`, add:

```typescript
export interface CommunityCardData {
  communityId: string;
  name: string;
  sourceNoteCount: number;
  representativePaths: string[];
  stabilityTrend: number; // positive = strengthening
  topEntities: string[];
}
```

- [ ] **Step 2: Create CommunityCard component**

Create `desktop-ui/src/features/chat/components/CommunityCard.tsx`:

```tsx
import type { CommunityCardData } from "@shared/types/tree-path";
import { ipc } from "@shared/hooks/useIpc";

interface Props {
  community: CommunityCardData;
}

export function CommunityCard({ community }: Props) {
  const trend = community.stabilityTrend >= 0
    ? `+${community.stabilityTrend.toFixed(2)}`
    : community.stabilityTrend.toFixed(2);
  const trendColor = community.stabilityTrend >= 0 ? "text-success" : "text-destructive";

  return (
    <div className="mt-2 rounded-lg border border-border/50 bg-surface-raised/30 p-2.5 text-xs">
      <div className="flex items-center justify-between mb-1">
        <span className="font-medium text-foreground">
          {community.name}
        </span>
        <span className="text-muted">
          {community.sourceNoteCount} notebook{community.sourceNoteCount !== 1 ? "s" : ""}
          <span className={`ml-1.5 ${trendColor}`}>{trend}</span>
        </span>
      </div>
      <div className="space-y-0.5 text-muted">
        {community.representativePaths.slice(0, 3).map((path) => (
          <div key={path} className="truncate">{path}</div>
        ))}
      </div>
      <div className="flex gap-2 mt-1.5">
        <button
          type="button"
          onClick={() => ipc("navigate_to_community", { communityId: community.communityId })}
          className="text-brand hover:text-brand/80 transition-colors"
        >
          Jump to all sections
        </button>
      </div>
    </div>
  );
}
```

- [ ] **Step 3: Integrate into MessageList**

In `desktop-ui/src/features/chat/components/MessageList.tsx`, render `CommunityCard` when `message.metadata?.communityCard` exists.

- [ ] **Step 4: Build and lint**

Run: `cd desktop-ui && bun run build && bun run lint:fix`

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/
git commit -m "feat(ui): add CommunityCard component in chat messages"
```

---

### Task 12: Integration Test + Final Verification

**Files:**
- Modify existing test files as needed

- [ ] **Step 1: Write Louvain integration test**

Test that the full pipeline works: create edges → detect communities → verify community assignments.

- [ ] **Step 2: Write CommunityRepo test**

Test upsert, get_members, prune, load_shared_entity_edges.

- [ ] **Step 3: Run full test suite**

Run: `cargo nextest run --workspace`

- [ ] **Step 4: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`

- [ ] **Step 5: Run frontend tests**

Run: `cd desktop-ui && bun run test`

- [ ] **Step 6: Commit**

```bash
git add -A
git commit -m "test: add integration tests for community graph pipeline"
```

---

### Follow-up (not blocking Phase 2 core)

- **Community naming via LLM:** Fire-and-forget LLM call for communities ≥5 members. Can be added after core pipeline is functional.
- **Community Pulse badge:** Global tray + sidebar indicator. Requires Tauri tray API integration — separate UI task.
- **Sparkline component:** 7-day stability trend visualization using Recharts. Frontend-only, can be added to CommunityCard later.
- **Gap analysis in InsightForge:** Community-level gap detection. Layered on top of the core pipeline.
- **Mirror "Community Evolution":** Sunday cron addition. Requires mirror facade extension.
- **Coaching community context:** Extend coaching handler with community metadata.
- **community_engagement analytics event:** New analytics event for autotuner reward signal.
- **Soft confidence threshold:** `community_intent_confidence` in QueryClassifier for proactive triggering modulation.
