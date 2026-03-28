# Relational Community Graph — Phase 2 Cognitive Fabric Design Spec

**Date:** 2026-03-28
**Phase:** Phase 2 of Cognitive Fabric (3-phase roadmap)
**Scope:** Community detection over tree nodes with entity bridges, community-aware retrieval, cross-note synthesis
**Effort:** ~4-5 weeks
**Prerequisite:** Phase 1 (Hierarchical Note RAG) — shipped 2026-03-28

## Problem Statement

Phase 1 gave notes structural retrieval via tree nodes. But retrieval is still note-scoped — when a user asks "summarize my thoughts on caffeine," the system finds the right section within a single note but cannot connect related sections across different notebooks. The entity graph exists (`entities`, `entity_relationships`, `entity_tree_links`) but is only used for FTS5 name lookup + 1-hop BFS via `GraphSearcher`. There is no community detection, no cross-note synthesis, and `entity_embeddings` is unpopulated.

Users feel: "The AI remembers each note better, but still can't connect the dots across different notebooks."

Phase 2 transforms this into: "The AI automatically connects things I never explicitly linked."

## Decision Record

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Primary deliverable | Cross-note synthesis (A) | Biggest daily RAG impact; gap analysis and visual navigator deferred to Phase 2 bonus / Phase 3 |
| Community freshness | Event-driven incremental (B) | Preserves live-context superpower; full Louvain re-run is sub-200ms at personal scale |
| Graph vertex type | Tree-node-centric with entity bridges (C) | Communities are "clusters of related note sections" — directly answers user questions; entities act as invisible glue |
| Detection algorithm | Louvain on shared-entity weighted graph (A) | Handles weighted edges natively, discovers natural boundaries, hierarchical resolution for Phase 3 |

## Existing Infrastructure (from Phase 1 + cognitive layer)

| Component | Status | Location |
|-----------|--------|----------|
| `book_tree_nodes` + `tree_node_embeddings` | Phase 1 — active | `cognitive/migrations/002_*`, `storage/vector_store/tree_node.rs` |
| `entity_tree_links` | Active (GT-Link) | `cognitive/migrations/002_*`, `cognitive/repos/gt_link.rs` |
| `entities` + `entity_relationships` | Active (frequency-based strength) | `cognitive/migrations/001_*`, `cognitive/repos/entity.rs` |
| `entity_embeddings` LanceDB table | Schema exists, **never written to** | `storage/vector_store/schemas.rs` |
| `GraphSearcher` | Active (FTS5 + 1-hop BFS) | `agent/src/domain_searchers/graph_searcher.rs` |
| `NoteTreeNavigator` (3-path) | Phase 1 — active | `context_engine/insight_forge/note_tree_navigator.rs` |
| `NoteTreeBuilder` subscriber | Phase 1 — active | `agent/src/adapters/note_tree_builder.rs` |
| `ContextUpdateQueue` + `LiveContextRefresher` | Active | `bus/context_updates.rs`, `agent/execution/live_context_refresher.rs` |
| 8-factor scorer + 24D autotuner | Phase 1 — active | `cognitive/services/decay.rs`, `common/autotuner.rs` |

## What Gets Deleted

- `GraphSearcher` (`agent/src/domain_searchers/graph_searcher.rs`) — replaced by NoteTreeNavigator Path 4
- Registration wiring in agent builder for GraphSearcher

## What Gets Activated

- `entity_embeddings` LanceDB table — populated for the first time (tree node embeddings with community membership flag)

---

## Section 1: Graph Construction & Community Detection

### Input graph

```
Vertices: all book_tree_nodes

Edges: tree_node_A ↔ tree_node_B if they share at least one entity
  Built via:
    SELECT DISTINCT a.tree_node_id AS node_a, b.tree_node_id AS node_b
    FROM entity_tree_links a
    JOIN entity_tree_links b ON a.entity_id = b.entity_id
    WHERE a.tree_node_id < b.tree_node_id

Edge weight: count(shared_entities) × avg(entity_relationships.strength)
  For each pair: shared entities linked to both via entity_tree_links
  avg_strength from entity_relationships between those entities
  weight = count × avg_strength
```

### Louvain algorithm

Implemented in Rust using `petgraph::Graph<(), f64, Undirected>` (~100 lines):
1. Initialize: each node in its own community
2. Phase 1 (local moves): for each node, try moving to each neighbor's community, accept if modularity gain > 0
3. Phase 2 (aggregation): collapse communities into super-nodes, rebuild graph
4. Repeat until no improvement

Returns `Vec<(NodeId, CommunityId)>` — community assignment for each tree node.

### CommunityBuilder event subscriber

```
DomainEvent::NoteContentChanged (after NoteTreeBuilder finishes)
  → CommunityBuilder (debounce 5s, full Louvain re-run):
      1. Load graph from entity_tree_links + entity_relationships
         (diff-aware: cache previous edges, rebuild only affected note's edges,
          full re-run if change >10%)
      2. Build petgraph weighted undirected graph
      3. Run Louvain → community assignments
      4. Diff against previous assignments in community_members
      5. For changed/new communities:
         a. Compute membership_score per member:
            avg(hierarchy_score + path_coherence) × shared_entity_strength
         b. Compose summary from top-5 members (by membership_score)
         c. Derive representative_paths: top-3 tree node paths
         d. Derive top_entities: most frequent entity names across members
         e. Communities with ≥5 members: LLM-generate short name (fire-and-forget)
            Communities with <5 members: name = top entity names joined
         f. Embed summary → upsert community_embeddings
         g. Upsert communities + community_members in SQLite
         h. Update stability (increases on persistence, decays on member loss)
      6. Emit events:
         - CommunityDiscovered (new, ≥3 members, stability > 0.3)
         - CommunityUpdated (>20% membership change)
         - CommunityWeakened (stability < 0.3, auto-prune)
      7. Push ContextUpdateQueue with rich payloads:
         "CommunityUpdated: 'Sleep Optimization' (5 sections from 3 notebooks)
          strengthened — new section from Health > Sleep > Caffeine Effects.
          Top entities: caffeine, melatonin, circadian rhythm.
          Stability: 0.87 (+0.12)"
```

### Community naming

- < 5 members: name = top entity names joined (e.g., "caffeine, melatonin, sleep")
- ≥ 5 members: LLM generates short descriptive name (single API call, fire-and-forget)
- Name updated when membership changes > 30%

---

## Section 2: Storage Schema

### SQLite: `communities` table

```sql
CREATE TABLE communities (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    summary TEXT NOT NULL,
    member_count INTEGER NOT NULL DEFAULT 0,
    modularity_score REAL,
    stability REAL NOT NULL DEFAULT 1.0,
    top_entities TEXT,                      -- JSON array of top entity names
    representative_paths TEXT,              -- JSON array of top-3 tree node path strings
    source_note_count INTEGER DEFAULT 0,    -- distinct notes contributing members
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);
```

### SQLite: `community_members` junction table

```sql
CREATE TABLE community_members (
    community_id TEXT NOT NULL REFERENCES communities(id) ON DELETE CASCADE,
    tree_node_id TEXT NOT NULL REFERENCES book_tree_nodes(id) ON DELETE CASCADE,
    membership_score REAL NOT NULL DEFAULT 0.0,
    joined_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (community_id, tree_node_id)
);

CREATE INDEX idx_community_members_node ON community_members(tree_node_id);
```

### LanceDB: `community_embeddings` table

```
community_embeddings.lance/
  id: Utf8                              -- community ID
  vector: FixedSizeList<Float32, 384>   -- embedded from community summary
  member_count: Utf8                    -- for predicate filtering
  source_note_count: Utf8               -- for predicate filtering
  updated_at: Utf8
```

### Key design choices

- **`representative_paths`** — JSON array of top-3 tree node path strings (e.g., `["Health > Sleep > Caffeine Effects", "Journal > Circadian"]`). Computed from top membership_score members. Makes live injection and community cards concrete.
- **`source_note_count`** — distinct notes contributing members. Cross-note communities are more interesting. Fed into scorer as `cross_note_boost`.
- **`membership_score`** — `avg(hierarchy_score + path_coherence) × shared_entity_strength`. Higher = more central/important. Used for ranking within Path 4 results.
- **`stability`** — FSRS-inspired. Increases on persistence across Louvain re-runs. Decays on member loss. Communities with stability < 0.3 auto-pruned. `CommunityWeakened` event emitted on prune.
- **Cascade delete** — tree node deletion cleans up community membership automatically. Communities with 0 members pruned.
- **Migration** — New migration in `cognitive` crate (bump version). Pre-release: direct schema creation.

---

## Section 3: NoteTreeNavigator Path 4 + Scorer Update

### Path 4: Community Traversal

```
NoteTreeNavigator::search(query, context)
  │
  ├── QueryClassifier (extended):
  │     Existing: "section", "in my note", etc.
  │     NEW community signals:
  │       - Multi-entity: query references 2+ concepts from different notes
  │       - Cross-note: "across my notes", "connect", "related to",
  │         "how does X relate to Y", "community", "cluster"
  │       - Broad synthesis: "summarize my thoughts on", "everything about"
  │       - Proactive: active_task or recent notes belong to a known community
  │         → trigger Path 4 even without explicit community language
  │     Returns: Simple | Hierarchical | Hybrid | Community
  │
  ├── Paths 1-3: unchanged from Phase 1
  │
  └── Path 4: CommunityQuery (~15% of queries)
        1. Embed query → vector search on community_embeddings
           (top_k = community_top_k, min_similarity = community_min_sim)
        2. For top matching communities:
           a. Load community_members (sorted by membership_score desc)
           b. For top N members: load tree node + build path
           c. Include community metadata (name, source_note_count, top_entities,
              representative_paths, stability)
        3. Format results with community card:
           MemoryEntry {
             content: "[Community: Sleep Optimization (4 notebooks)]
                       Health > Sleep > Caffeine Effects: content...",
             source: "community",
             score: community_similarity × cross_note_boost,
             metadata: {
               community_id, community_name, member_count, source_note_count,
               representative_paths, stability, top_entities,
               community_card: { name, source_note_count, paths, stability_trend }
             }
           }
        4. RRF merge with Paths 1-3 results
```

### Hybrid + Community fusion

When both hierarchical AND community intent detected:
- Run Path 2 + Path 4 concurrently via `tokio::join!`
- RRF merge both result sets
- Dedup by `node_id`

### Proactive community triggering

When query mentions one entity but `active_task` or recent notes belong to a known community → automatically trigger Path 4. The user asks "how does caffeine affect me?" while focused on productivity → AI pulls the full Sleep Optimization community without explicit "connect" language.

### Extended scorer: 8-factor → 10-factor

```
score = w_semantic       * cosine_similarity          // 0.20 (was 0.25)
      + w_retrievability * retrievability              // 0.10 (was 0.15)
      + w_importance     * importance                  // 0.08 (was 0.10)
      + w_frequency      * frequency_score             // 0.05 (unchanged)
      + w_situation      * situational_boost           // 0.15 (was 0.20)
      + w_temporal       * temporal_recency            // 0.02 (was 0.05)
      + w_hierarchy      * hierarchy_score             // 0.10 (unchanged)
      + w_path_coherence * path_coherence              // 0.05 (was 0.10)
      + w_community      * community_membership        // 0.15 (NEW)
      + w_cross_note     * cross_note_boost            // 0.10 (NEW)
```

**New factors:**

- **`community_membership`** — For tree nodes in a matched community: `membership_score × community_stability`. Non-community results: 0.0. Recency boost: +0.15 if community updated within last 60 seconds.
- **`cross_note_boost`** — `log2(source_note_count)` clamped to [0, 1]. Communities spanning 4+ notes get maximum boost. Single-note results: 0.0.

### Autotuner expansion: 24D → 28D

| Parameter | Range | Default | Purpose |
|-----------|-------|---------|---------|
| `w_community` | 0.0–0.30 | 0.15 | community_membership weight |
| `w_cross_note` | 0.0–0.20 | 0.10 | cross_note_boost weight |
| `community_top_k` | 3–15 | 5 | top-k for community_embeddings search |
| `community_min_similarity` | 0.3–0.7 | 0.45 | min cosine similarity for communities |

### GraphSearcher replacement

`GraphSearcher` (FTS5 name + 1-hop BFS) deleted. Path 4 replaces it entirely with community-aware vector search + entity bridge traversal. Same pattern as BookRAGSearcher deletion in Phase 1.

### Backward compatibility

- Non-community results: `w_community = 0.0`, `w_cross_note = 0.0` — scorer degrades to Phase 1 behavior
- No communities exist (fresh install): Paths 1-3 carry the load, Path 4 returns empty
- `RelevanceWeights::default()` rebalances all 10 weights to sum to 1.0

---

## Section 4: Live Injection + Feature Integration

### New ContextUpdateReason variants

```rust
CommunityDiscovered,  // new community formed (≥3 members, stability > 0.3)
CommunityUpdated,     // existing community changed (>20% membership)
CommunityWeakened,    // community pruned (stability < 0.3)
```

### Rich injection payloads

Payloads include: community name, source_note_count, representative_paths, top_entities, stability trend. Gives the LLM actionable cross-note context without requiring a separate retrieval call.

Priority escalation: if affected community shares entities with `active_task`, upgrade to `High`.

### InsightForge integration

**Synthesis:** Community context injected into synthesis prompt — "This section belongs to the 'Sleep Optimization' community, which also includes sections from 3 other notebooks." LLM naturally produces cross-note connections.

**Gap analysis (bonus):** Runs at community level — load communities with `source_note_count > 1`, check pairwise entity overlap, flag thematic communities without shared bridges. Example: "Productivity and Health both reference 'focus' but have no direct entity bridge."

### Coaching integration

Interventions gain community context:
- `CommunityDiscovered` → "A new knowledge cluster formed around your recent notes"
- `CommunityUpdated` with drift > 30% → "Your X community is evolving rapidly"
- Gap between active_task community and another → proactive suggestion: "Your Productivity community lacks a bridge to Sleep Optimization — want me to suggest a connection?"

### Mirror weekly narrative

Sunday cron adds **"Community Evolution"** section:
- List active communities with stability trends
- Highlight strongest bridges (entity relationships driving community connections)
- Report pruned communities
- Report detected gaps between communities

### Desktop UI: Community Pulse + Community Card

**Community Pulse badge** (tray + chat header):
- Small badge showing community update count: "3 communities updated today"
- Subtle pulse animation on `CommunityDiscovered` / `CommunityUpdated`
- Click → mini Community Overview (list of communities with member counts)

**Community Card** (inline in chat responses):
```
[Sleep Optimization — 4 notebooks]
Health > Sleep > Caffeine Effects
Journal > Circadian Rhythm
→ Jump to all sections | Pin to focus
```
- Clickable paths → open note editor scrolled to section
- "Jump to all sections" → opens note editor with multiple AI highlights
- "Pin to focus" → links community to current focus session

### Analytics: community_engagement

New event `community_engagement` triggered when users:
- Click Community Card
- Jump to community sections
- Pin community to focus

Fed into autotuner as reward signal for `w_community` and `w_cross_note`. After ~2 weeks, the system learns when to prioritize community paths based on real interaction patterns.

---

## A/B Testing & Validation

| Metric | Measurement | Target |
|--------|-------------|--------|
| Cross-note recall@3 | % of cross-note queries where relevant sections from other notes in top 3 | ≥ 75% |
| Community precision | % of detected communities rated "meaningful" by user feedback | ≥ 80% |
| Cross-note synthesis satisfaction | `enrichment_feedback` positive rate on multi-note queries | +30% vs Phase 1 |
| Community engagement | Click rate on Community Cards | ≥ 15% |
| Latency delta | Additional ms for community detection + Path 4 | < 100ms p95 |
| Autotuner convergence | Shadow trials find stable community weights | < 25 sessions |
| Community stability | Average stability after 2 weeks | ≥ 0.7 |

---

## Phase 3 Hooks (out of scope, designed for)

- **Knowledge Fabric Explorer** — visual navigator using community graph. Communities as clusters, entity bridges as edges, interactive Cytoscape/D3 view.
- **Unified FabricRetriever** — merges NoteTreeNavigator (Paths 1-4) into a single retrieval interface with adaptive path selection.
- **Proactive community refresh** — background service that periodically re-evaluates community health and suggests notes that would strengthen weak communities.
- **Multi-resolution communities** — Louvain's hierarchical output enables zoom levels (macro → micro communities) for the visual navigator.
