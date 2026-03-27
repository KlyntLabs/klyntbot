# Hierarchical Note RAG — NoteTreeNavigator Design Spec

**Date:** 2026-03-27
**Phase:** Phase 1 of Cognitive Fabric (3-phase roadmap)
**Scope:** Activate hierarchical note retrieval via tree node embeddings, replacing flat 500-char blob embedding
**Effort:** ~4 weeks

## Problem Statement

Notes in Klyntbot are embedded as a single 500-character truncated blob (`NoteEmbeddingAdapter`). When a user asks "summarize section 3 of Project X and relate it to habit Y," the system returns a disconnected snippet with no structural context. There is no hierarchy, no section-level navigation — retrieval cannot distinguish between different parts of a note.

This is daily friction across chat, InsightForge synthesis, coaching interventions, and focus session debriefs.

## Decision Record

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Primary pain point | Notes are flat (not entity graph or community synthesis) | Notes are the core daily-use feature; solving structural retrieval first creates primitives Layer 2/3 reuse |
| Embedding strategy | New `tree_node_embeddings` table (Option A) | Clean break from flat blobs; each tree node is an atomic unit like semantic fact triples |
| Retrieval approach | New `NoteTreeNavigator` (Option B) | Purpose-built for Cognitive Fabric vision; cleaner long-term than retrofitting BookRAGSearcher |
| BookRAGSearcher | Delete entirely (Option A) | Dead code is technical debt; Layer 2 builds fresh graph reasoning with community semantics |
| Parsing strategy | Heuristic-first, no LLM | Zero latency, zero token cost, deterministic; note `body` is markdown, `parse_markdown_to_tree()` already exists |

## Existing Infrastructure (already built, needs activation)

| Component | Status | Location |
|-----------|--------|----------|
| `book_tree_nodes` table + FTS5 triggers | Created, not fully populated | `cognitive/migrations/002_book_index_tables.sql` |
| `entity_tree_links` table | Created | `cognitive/migrations/002_book_index_tables.sql` |
| `SqliteBookTreeRepo` (insert, get, subtree, FTS) | Fully implemented | `cognitive/src/repos/book_tree.rs` |
| `SqliteGTLinkRepo` (link, batch, subtree traversal) | Fully implemented | `cognitive/src/repos/gt_link.rs` |
| `parse_markdown_to_tree()` | Fully implemented | `cognitive/src/repos/markdown_parser.rs` |
| `EmbeddingEngine` (384-dim fastembed) | Fully implemented | `tools/src/embedding/embedding_engine.rs` |
| `ContextUpdateQueue` + `LiveContextRefresher` | Fully implemented | `bus/src/context_updates.rs`, `agent/src/execution/live_context_refresher.rs` |

## What Gets Deleted

- `BookRAGSearcher` (`context_engine/src/insight_forge/bookrag_searcher.rs`)
- `RetrievalPlanner` + all operators (Extract, SelectByEntity, GraphReasoning, TextRanker, SkylineRanker, Reduce)
- Registration wiring in agent builder
- `note_embeddings` LanceDB table (deprecated once all notes have tree node embeddings and A/B metrics confirm ≥70% structural recall@3; drop the table after 2 weeks of stable operation)

---

## Section 1: Data Model & Embedding Pipeline

### New LanceDB table: `tree_node_embeddings`

```
tree_node_embeddings.lance/
  id: Utf8                              -- tree node ID (matches book_tree_nodes.id)
  vector: FixedSizeList<Float32, 384>   -- 384-dim fastembed embedding
  note_id: Utf8                         -- parent note ID (predicate filtering)
  level: Utf8                           -- "0"-"7" (hierarchy-aware scoring)
  source_type: Utf8                     -- "note" (extensible for future sources)
  updated_at: Utf8                      -- ISO 8601 timestamp
```

### Existing `book_tree_nodes` table (no schema changes)

Already has: `id`, `source_id`, `source_type`, `parent_id`, `level`, `title`, `content`, `created_at` + FTS5 auto-sync triggers.

### Embedding text composition

```rust
fn compose_node_text(node: &TreeNode) -> String {
    match node.level {
        0 => node.title.clone(),                           // Root: note title only
        1..=6 => format!("{}\n{}", node.title, node.content_preview(300)),  // Heading + child preview
        7 => node.content_preview(300),                    // Bullet pseudo-section: content only
        _ => node.content_preview(300),
    }
}
```

Each node gets its own 384-dim embedding. A note with 5 headings produces 6 embeddings (1 root + 5 sections) instead of 1 truncated blob.

### Write pipeline: NoteTreeBuilder

New subscriber on `DomainEventBus`, triggered by `DomainEvent::NoteContentChanged`:

```
NoteTool::save / note_create / note_update
  → DomainEvent::NoteContentChanged
  → NoteTreeBuilder:
      1. parse_markdown_to_tree(note.body) → Vec<TreeNode>
      2. SqliteBookTreeRepo::delete_by_source(note_id) → clear old nodes
      3. SqliteBookTreeRepo::insert_nodes(tree_nodes)
      4. For each node:
         compose_node_text(node) → EmbeddingEngine::embed_async()
         → VectorStore::upsert("tree_node_embeddings", node.id, vector, extra_fields)
      5. SqliteGTLinkRepo::link_batch(entity_mentions)
      6. ContextUpdateQueue::push(NoteStructureChanged, affected_node_summaries)
```

Replaces the current `NoteEmbeddingHandler::embed_note()` single-blob approach.

### One-time migration

Background job at startup (behind feature flag): scan all notes with `embedding_updated_at IS NULL` or outdated → parse → embed nodes → populate `tree_node_embeddings`. Incremental, non-blocking, limit=100 per batch.

---

## Section 2: NoteTreeNavigator — Retrieval Engine

### Role

Implements `DomainSearcher` trait. Registered in InsightForge's multi-domain fan-out. Replaces both the deleted `BookRAGSearcher` and the never-implemented `NoteSearcher`.

### Two-path retrieval

```
NoteTreeNavigator::search(query, context)
  │
  ├── QueryClassifier (heuristic, no LLM, ~microseconds):
  │     Keywords: "section", "part", "chapter", "heading", "in my note about"
  │     Multi-entity: query mentions 2+ entities in different notes
  │     RetrievalContext: active_view == NoteEditor → bias structural
  │     Returns: SimpleQuery | HierarchicalQuery
  │
  ├── Path 1: SimpleQuery (80% of queries)
  │     Vector search on tree_node_embeddings:
  │       embed query → VectorStore::search(top_k, min_similarity)
  │       → Load ancestor chain: SqliteBookTreeRepo::get_ancestors(node_id)
  │       → Return nodes WITH path context
  │
  └── Path 2: HierarchicalQuery (20% of queries)
        Tree traversal with entity-guided drill-down:
          1. Vector search on tree_node_embeddings (top_k=10, coarse)
          2. Identify candidate notes from results
          3. Per candidate: load subtree, score each node by:
             - Vector similarity
             - Entity overlap with query entities
             - FTS5 match score (book_tree_nodes_fts)
          4. Select best path through tree (root → most relevant leaf)
          5. Merge paths across notes via RRF (k=60)
          6. Return ranked paths with full structural context
```

### Result type

```rust
struct TreeSearchResult {
    node: TreeNode,
    path: Vec<PathSegment>,  // root → ... → this node
    score: f64,
    linked_entities: Vec<EntityId>,
}

struct PathSegment {
    node_id: String,
    title: String,
    level: u8,
}
```

### Integration points

| Component | Change |
|-----------|--------|
| InsightForge | Register `NoteTreeNavigator` as `DomainSearcher` |
| RRF merge | Tree results merge with cognitive facts + conversation recall via same k=60 RRF |
| UnifiedMemoryService | No changes |
| QueryRewriter | Extend `RetrievalContext` with `hierarchical_intent: bool` |
| Memory prefetch | No changes — compatible with concurrent prefetch |

---

## Section 3: Scoring Model & Autotuner

### Extended relevance scorer (6-factor → 8-factor)

```
score = w_semantic       * cosine_similarity          // 0.25 (was 0.30)
      + w_retrievability * retrievability              // 0.15 (was 0.20)
      + w_importance     * importance                  // 0.10 (was 0.15)
      + w_frequency      * frequency_score             // 0.05 (was 0.10)
      + w_situation      * situational_boost           // 0.20 (was 0.25)
      + w_temporal       * temporal_recency            // 0.05 (unchanged)
      + w_hierarchy      * hierarchy_score             // 0.10 (NEW)
      + w_path_coherence * path_coherence              // 0.10 (NEW)
```

### hierarchy_score

Biases retrieval toward the correct tree depth based on query intent:

```rust
fn hierarchy_score(node_level: u8, query_type: QueryType) -> f64 {
    match query_type {
        Summary => 1.0 - (node_level as f64 / MAX_LEVEL as f64),  // favor roots
        Detail  => node_level as f64 / MAX_LEVEL as f64,           // favor leaves
        Neutral => 0.5,
    }
}
```

### path_coherence

Rewards nodes whose siblings also scored well — the entire section is relevant, not just an isolated paragraph:

```rust
fn path_coherence(node: &TreeNode, all_scores: &HashMap<NodeId, f64>) -> f64 {
    let sibling_scores: Vec<f64> = node.sibling_ids()
        .filter_map(|id| all_scores.get(&id))
        .collect();
    if sibling_scores.is_empty() { return 0.5; }
    sibling_scores.iter().sum::<f64>() / sibling_scores.len() as f64
}
```

### Backward compatibility

For non-note results (cognitive facts, conversation recall): `w_hierarchy = 0`, `w_path_coherence = 0.5` (neutral). The scorer degrades to original 6-factor behavior.

### Autotuner expansion (19D → 23D)

| Parameter | Range | Default | Purpose |
|-----------|-------|---------|---------|
| `w_hierarchy` | 0.0–0.25 | 0.10 | Weight for hierarchy_score |
| `w_path_coherence` | 0.0–0.20 | 0.10 | Weight for path_coherence |
| `tree_top_k` | 5–30 | 15 | Top-k for tree_node_embeddings search |
| `tree_min_similarity` | 0.3–0.7 | 0.50 | Min cosine similarity for tree nodes |

Shadow trials use existing `RwLock<Option<TrialParams>>` pattern on `NoteTreeNavigator`.

---

## Section 4: Live Context Injection + UX

### Live injection on note edit

```
Note edited during active ReAct loop
  → DomainEvent::NoteContentChanged { note_id, changed_sections }
  → NoteTreeBuilder:
      1. Diff new tree against existing book_tree_nodes (incremental, not full re-parse)
      2. Re-embed only changed/added nodes
      3. ContextUpdateQueue::push(ContextUpdate {
           priority: Normal (High if note is linked to active_task),
           reason: "NoteStructureChanged",
           content: "Updated: [Health > Sleep > Coffee Effects] — new content added"
         })
  → Next ReAct iteration: LiveContextRefresher injects as Message::ContextUpdate
```

30-second dedup window prevents rapid-save flooding. No changes to `ContextUpdateQueue` or `LiveContextRefresher` APIs.

### Desktop UI: structure path (Phase 1 — minimal)

**Chat breadcrumb:** When a response references tree node content, metadata includes the path. Rendered as a clickable breadcrumb:
```
📄 Health > Sleep > Coffee Effects
```
Clicking opens the note editor scrolled to that section.

**InsightForge collapsible tree:** In synthesis/gap-analysis panels, display a collapsible mini-tree for matched nodes:
```
▼ Health (notebook)
  ▼ Sleep (section)
    ● Coffee Effects ← matched (0.87)
    ○ Blue Light
  ▼ Exercise (section)
    ● Morning Routine ← matched (0.72)
```

No Cytoscape graph or mind-map — those belong in Phase 3 (Knowledge Fabric Explorer).

### Frontend data contract

```typescript
interface TreePathRef {
  noteId: string;
  noteName: string;
  path: PathSegment[];
  nodeId: string;
}

interface PathSegment {
  nodeId: string;
  title: string;
  level: number;
}
```

---

## A/B Testing & Validation

| Metric | Measurement | Target |
|--------|-------------|--------|
| Structural recall@3 | % of note queries where correct section in top 3 | ≥ 70% (vs ~40% baseline) |
| Path accuracy | % of returned paths matching user's intended location | ≥ 80% |
| Latency delta | Additional ms for tree retrieval vs flat blob | < 50ms p95 |
| User satisfaction | `enrichment_feedback` positive rate on note queries | +25% vs baseline |
| Autotuner convergence | Shadow trials find stable weights | < 20 sessions |

Measured via existing `enrichment_feedback` + new `tree_retrieval_accuracy` analytics event.

---

## Phase 2/3 Hooks (out of scope, designed for)

This Phase 1 design creates structural primitives that Layer 2 (Relational Community Graph) and Layer 3 (Unified Fabric Retriever) will reuse:

- **Tree nodes as graph vertices:** Each `book_tree_nodes` row + `entity_tree_links` becomes a vertex in the community graph. Community detection runs over these links.
- **`tree_node_embeddings` as community inputs:** Community summary embeddings are derived from member tree node embeddings.
- **`NoteTreeNavigator` → `FabricRetriever`:** The two-path architecture (simple vector / hierarchical traversal) extends naturally to include community traversal as a third path.
- **8-factor scorer → Fabric scorer:** Community-related weights (community_radius, graph_traverse_depth) add to the 23D autotuner space.
