# BookRAG-Style Architecture for Klyntbot

**Date**: 2026-03-17
**Status**: Approved
**Paper**: [BookRAG: A Hierarchical Structure-aware Index-based Approach for RAG on Complex Documents](https://arxiv.org/abs/2512.03413) (arXiv 2512.03413)

## Problem

Klyntbot's current RAG system retrieves facts, conversation history, and domain results (notes, tasks, finance) via flat vector search + BM25 + FSRS scoring, merged through Reciprocal Rank Fusion in InsightForge. This works well for simple lookups but has three gaps:

1. **No hierarchical awareness** — notes have heading structure, tasks have project/subtask nesting, but retrieval treats all content as flat chunks. Cross-section reasoning ("how does Section 2.1 relate to Section 3.4?") is impossible.
2. **No entity-aware navigation** — the existing `EntityRepo` stores entities and relationships, but retrieval doesn't use entity-to-location mapping to jump from a query entity to the relevant document sections.
3. **Static retrieval workflow** — every query gets the same vector+BM25+RRF treatment regardless of complexity. A simple "what's the deadline?" and a complex "compare my finance goals with project progress" both use identical retrieval paths.

BookRAG (arXiv 2512.03413) solves these with three innovations: a hierarchical BookIndex `B = (T, G, M)`, gradient-based entity resolution, and an agent-based retrieval planner inspired by Information Foraging Theory. This spec adapts those ideas to Klyntbot's personal-agent domain.

## Design Decisions

Three key architectural decisions were evaluated and confirmed:

1. **Entity graph storage**: Hybrid approach — entities extracted from notes/tasks are stored in the existing `EntityRepo` graph, with a new GT-Link junction table mapping entities to tree nodes. Cognitive memory (SemanticFact, EpisodicMemory, ProceduralRule) remains untouched.
2. **Tree construction sources**: Markdown heading hierarchy from notes + project/task/subtask nesting from tasks + skill sections. No PDF ingestion (out of scope for a personal agent).
3. **Retrieval planner integration**: The BookRAG planner becomes a new `DomainSearcher` inside InsightForge. Results participate in existing RRF merge alongside notes, tasks, graph, and finance searchers. Zero changes to `ContextEngine` or `AgentRuntime`.

## Architecture Overview

```
User Query
    |
    v
SkillRouter -> IntentAnalyzer -> ContextEngine -> ExecutionRouter
                                      |
                    +-----------------+-----------------+
                    |                 |                 |
              BudgetAllocator   MemoryRetrieval   HistoryCompressor
                                      |
                    +-----------------+-----------------+
                    |                 |                 |
              UnifiedMemory     InsightForge      ContextSources
              (cognitive)            |
                    +----------------+----------------+
                    |                |                |
              NoteSearcher    TaskSearcher    BookRAGSearcher  <-- NEW
              GraphSearcher   FinanceSearcher       |
                                              RetrievalPlanner
                                                    |
                                         +----------+----------+
                                         |          |          |
                                    Classify    Generate    Execute
                                     Query       Plan      Operators
                                                    |
                                              BookIndex (T, G, M)
```

## Component 1: BookIndex `B = (T, G, M)`

The BookIndex is a triplet of three complementary structures that together enable hierarchical, entity-aware document navigation.

### 1.1 Tree `T = (N, E_T)` — Hierarchical Document Structure

The tree mirrors the logical hierarchy of notes (markdown headings) and tasks (project nesting).

**Data type:**

```rust
pub struct TreeNode {
    pub id: String,                     // UUID
    pub parent_id: Option<String>,      // NULL for roots
    pub node_type: TreeNodeType,
    pub content: String,                // Actual text content
    pub title: Option<String>,          // Section title if applicable
    pub level: u32,                     // Depth in hierarchy (0 = root)
    pub source_type: SourceType,        // Note, Task, Skill
    pub source_id: String,              // Original note/task/skill ID
    pub position: u32,                  // Order among siblings
    pub metadata: Option<String>,       // JSON: code language, table schema, etc.
}

pub enum TreeNodeType {
    Section,    // Heading / project / area
    Text,       // Paragraph / task description
    Table,      // Markdown table
    Code,       // Fenced code block
    Task,       // Task item (leaf in task tree)
    ListItem,   // Bullet/numbered list item
}

pub enum SourceType {
    Note,
    Task,
    Skill,
}
```

**Storage:** SQLite table `book_tree_nodes` with indexes on `parent_id`, `source_type + source_id`, and `level`. FTS5 table `book_tree_nodes_fts` for keyword search within nodes.

```sql
CREATE TABLE book_tree_nodes (
    id TEXT PRIMARY KEY,
    parent_id TEXT REFERENCES book_tree_nodes(id),
    node_type TEXT NOT NULL,
    content TEXT NOT NULL,
    title TEXT,
    level INTEGER NOT NULL DEFAULT 0,
    source_type TEXT NOT NULL,
    source_id TEXT NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    metadata TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_tree_nodes_parent ON book_tree_nodes(parent_id);
CREATE INDEX idx_tree_nodes_source ON book_tree_nodes(source_type, source_id);
CREATE INDEX idx_tree_nodes_level ON book_tree_nodes(level);

CREATE VIRTUAL TABLE book_tree_nodes_fts USING fts5(
    title, content,
    content='book_tree_nodes',
    content_rowid='rowid',
    tokenize='porter'
);
```

**Tree construction:**

- **Notes**: Parse markdown into blocks (headings, paragraphs, code fences, tables). Heading depth (`#` = level 1, `##` = level 2) determines hierarchy. Non-heading blocks become children of the nearest preceding heading.
- **Tasks**: Area (level 0) -> Project (level 1) -> Task (level 2) -> Subtask (level 3). Task descriptions become leaf Text nodes.
- **Skills**: Built once at boot from skill YAML sections. Rarely changes.

### 1.2 Graph `G = (V, E_G)` — Entity Relations

Reuses the existing `EntityRepo` with `EntityRow` and `RelationshipRow`. No schema changes needed.

Entities are extracted from tree node content during indexing and resolved through gradient-based entity resolution (Component 2). Relationships between co-occurring entities within a node are extracted and stored via `upsert_relationship`.

Existing capabilities reused as-is:
- `get_neighborhood(entity_id, depth)` — 1-2 hop graph expansion
- `find_path(from, to, max_depth)` — BFS path finding
- `merge_entities(source_id, target_id)` — transactional consolidation
- `get_related_entities(entity_id, rel_type)` — typed relationship traversal

### 1.3 GT-Link `M: V -> P(N)` — Entity-to-Node Mapping

A junction table that bidirectionally maps entities to the tree nodes where they appear.

```sql
CREATE TABLE entity_tree_links (
    entity_id TEXT NOT NULL REFERENCES entities(id) ON DELETE CASCADE,
    tree_node_id TEXT NOT NULL REFERENCES book_tree_nodes(id) ON DELETE CASCADE,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    PRIMARY KEY (entity_id, tree_node_id)
);

CREATE INDEX idx_entity_tree_links_node ON entity_tree_links(tree_node_id);
```

Enables two critical operations:
- `get_linked_nodes(entity_id)` -> all tree nodes where entity appears
- `get_entities_in_subtree(node_id)` -> all entities mentioned in a section/subtree

### 1.4 BookIndex Orchestrator

```rust
pub struct BookIndex {
    tree_repo: BookTreeRepo,
    entity_repo: EntityRepo,
    gt_link_repo: GTLinkRepo,
    vector_store: VectorStore,
    embedder: Arc<dyn EmbeddingEngine>,
}

impl BookIndex {
    // Construction
    pub async fn build_from_note(&self, note_id: &str, content: &str) -> Result<()>;
    pub async fn build_from_task_hierarchy(&self, project_id: &str) -> Result<()>;
    pub async fn rebuild_all(&self) -> Result<()>;
    pub fn has_content(&self) -> bool;  // Cached check

    // Tree navigation
    pub async fn get_subtree(&self, node_id: &str) -> Result<Vec<TreeNode>>;
    pub async fn get_children(&self, node_id: &str) -> Result<Vec<TreeNode>>;
    pub async fn get_path_to_root(&self, node_id: &str) -> Result<Vec<TreeNode>>;
    pub async fn get_root_sections(&self, source_type: SourceType) -> Result<Vec<TreeNode>>;

    // GT-Link navigation
    pub async fn get_linked_nodes(&self, entity_id: &str) -> Result<Vec<TreeNode>>;
    pub async fn get_entities_in_subtree(&self, node_id: &str) -> Result<Vec<EntityRow>>;
    pub async fn get_subtree_roots_linked(&self, entity_id: &str, depth: u32) -> Result<Vec<TreeNode>>;

    // Entity extraction + linking (called during build)
    async fn extract_and_link_entities(&self, node: &TreeNode) -> Result<()>;

    // Maintenance
    pub async fn delete_source_tree(&self, source_type: SourceType, source_id: &str) -> Result<()>;
}
```

## Component 2: Gradient-based Entity Resolution (Algorithm 1)

Enhancement to the existing `EntityRepo` that replaces simple name matching with BookRAG's gradient-based alias detection.

### Algorithm

For each new entity `v_n` extracted during indexing:

1. Embed `v_n` (name + description) using the existing `EmbeddingEngine`
2. Vector search `top_k` candidates from entity embeddings in LanceDB
3. Sort candidates by cosine similarity descending
4. **Gradient walk**: iterate sorted scores; while `score[i] > score[i-1] / g`, add to selection set; break on sharp drop
5. Decision:
   - All candidates passed gradient check (no drop) -> **Case A: new entity** (uniformly low similarity)
   - Gradient found, 1 match -> **merge** via existing `merge_entities()`
   - Gradient found, multiple matches -> LLM disambiguation (optional, falls back to highest score)

### API

```rust
impl EntityRepo {
    pub async fn resolve_entity(
        &self,
        new_entity: &NewEntity,
        vector_store: &VectorStore,
        embedder: &dyn EmbeddingEngine,
        config: &EntityResolutionConfig,
    ) -> Result<EntityRow>;
}

pub struct EntityResolutionConfig {
    pub top_k: usize,                   // default: 10
    pub gradient_threshold: f64,         // g = 0.6 (BookRAG default)
    pub min_similarity: f64,             // below this, always new entity (default: 0.3)
    pub use_llm_disambiguation: bool,    // default: false (merge to highest score)
}
```

### Helper

```rust
/// Detect the gradient drop point in a sorted score list.
/// Returns the index where the sharp drop begins, or None if no gradient found.
fn detect_gradient(scores: &[f64], g: f64) -> Option<usize> {
    if scores.is_empty() { return None; }
    let mut prev = scores[0];
    for (i, &score) in scores.iter().enumerate().skip(1) {
        if score < prev / g {
            return Some(i);
        }
        prev = score;
    }
    None // All scores passed — no gradient (Case A)
}
```

### New LanceDB Table

```rust
// entity_embeddings table schema
fn entity_embedding_schema() -> Schema {
    Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        vector_field(),  // FixedSizeList<Float32, 384>
        Field::new("name", DataType::Utf8, false),
        Field::new("entity_type", DataType::Utf8, false),
        Field::new("description", DataType::Utf8, true),
        Field::new("updated_at", DataType::Utf8, false),
    ])
}
```

### When Resolution Runs

- During `BookIndex::build_from_note()` and `build_from_task_hierarchy()` after entity extraction
- In background consolidation when new entities arrive from domain events
- Not on the query hot path (resolution is an indexing-time operation)

## Component 3: Operator Library

Composable retrieval operators that transform a working set of tree nodes through a pipeline.

### Core Trait

```rust
#[async_trait]
pub trait Operator: Send + Sync {
    fn name(&self) -> &str;
    fn operator_type(&self) -> OperatorType;
    async fn execute(&self, ctx: &mut OperatorContext) -> Result<()>;
}

pub enum OperatorType {
    Formulator,
    Selector,
    Reasoner,
    Synthesizer,
}
```

### OperatorContext (Mutable Pipeline State)

```rust
pub struct OperatorContext {
    // Query
    pub query: String,
    pub sub_queries: Vec<String>,
    pub extracted_entities: Vec<EntityRow>,

    // Working set (refined by operators)
    pub working_set: Vec<ScoredNode>,

    // Synthesis
    pub partial_answers: Vec<String>,
    pub final_answer: Option<String>,

    // Resources
    pub book_index: Arc<BookIndex>,
    pub provider: Arc<dyn LlmProvider>,

    // Safety
    pub max_nodes: usize,        // default: 50
    pub token_budget: usize,     // for Map/Reduce LLM calls
}

pub struct ScoredNode {
    pub node: TreeNode,
    pub graph_score: f64,
    pub text_score: f64,
    pub combined: f64,
}
```

### Operator Catalog

| Operator | Type | LLM? | Description |
|----------|------|------|-------------|
| `Decompose` | Formulator | Yes | Break query into sub-queries -> `ctx.sub_queries` |
| `Extract` | Formulator | Yes (light) | Extract entities from query, link to graph -> `ctx.extracted_entities` |
| `FilterModal` | Selector | No | Filter `working_set` by `TreeNodeType` |
| `FilterRange` | Selector | No | Filter by source_id or level range |
| `SelectByEntity` | Selector | No | GT-Link: entity -> linked nodes -> subtree roots -> descendants |
| `SelectBySection` | Selector | Yes | LLM picks relevant sections from root children, expand subtrees |
| `GraphReasoning` | Reasoner | No | PageRank on entity subgraph -> map scores to nodes via GT-Link |
| `TextRanker` | Reasoner | No | Embed query, score each node by cosine similarity |
| `SkylineRanker` | Reasoner | No | Pareto frontier on (graph_score, text_score) |
| `SubQueryExecutor` | Composite | Varies | Run SingleHop pipeline per sub-query (for MultiHop) |
| `Map` | Synthesizer | Yes | Per-node LLM call for partial answer |
| `Reduce` | Synthesizer | Yes | Aggregate partial answers into final response |

### PageRank (In-process, Small Subgraphs)

```rust
/// Personalized PageRank on a subgraph seeded from query entities.
/// Operates on small subgraphs (typically 20-50 entities).
pub fn pagerank(
    entities: &[EntityRow],
    relationships: &[RelationshipRow],
    seed_ids: &[String],
    damping: f64,       // 0.85
    iterations: u32,    // 20
) -> HashMap<String, f64>
```

Standard iterative PageRank with personalized teleportation to seed entities. ~30 lines of code, no external graph library needed.

### Skyline (Pareto Frontier) Ranker

```rust
/// Retain only non-dominated nodes across (graph_score, text_score).
/// Node A dominates B if A >= B on all dimensions with at least one strict >.
pub fn skyline_filter(nodes: &mut Vec<ScoredNode>) {
    nodes.retain(|candidate| {
        !nodes.iter().any(|other| dominates(other, candidate))
    });
}
```

Typically reduces ~50 candidates to 7-10 non-dominated nodes, matching BookRAG's empirical working set size.

## Component 4: Retrieval Planner (Agent-based Planning)

The planner classifies queries and generates tailored operator pipelines, inspired by Information Foraging Theory.

### Query Classification

```rust
pub enum QueryCategory {
    SingleHop,          // Direct lookup: "What's the deadline for Project Alpha?"
    MultiHop,           // Cross-reference: "How do my finance goals relate to work projects?"
    GlobalAggregation,  // Filter + aggregate: "How many tasks are overdue across all projects?"
    PassThrough,        // Not suitable for BookIndex (chitchat, greetings)
}
```

Classification is heuristic-first (keyword detection), with LLM fallback for ambiguous cases:

- Aggregation keywords (count, how many, total, list all, sum) -> Global
- Comparison/relation keywords (compare, differ, relate, between, how does X affect Y) -> MultiHop
- Short, single-entity queries -> SingleHop
- Very short / greeting patterns -> PassThrough

### Plan Generation

```rust
impl RetrievalPlanner {
    pub async fn plan(&self, query: &str) -> Result<RetrievalPlan>;

    fn generate_plan(&self, query: &str, category: QueryCategory) -> Vec<Box<dyn Operator>> {
        match category {
            SingleHop => vec![
                Extract, SelectByEntity, GraphReasoning, TextRanker, SkylineRanker, Reduce
            ],
            MultiHop => vec![
                Decompose, SubQueryExecutor(single_hop_pipeline), Map, Reduce
            ],
            GlobalAggregation => vec![
                FilterModal, FilterRange, Map, Reduce
            ],
            PassThrough => vec![],
        }
    }
}
```

SingleHop plan detail: `Extract` attempts entity linking first. If no entities found, falls back to `SelectBySection` (LLM picks relevant sections). Both paths then proceed through `GraphReasoning || TextRanker -> SkylineRanker -> Reduce`.

### RetrievalPlan

```rust
pub struct RetrievalPlan {
    pub category: QueryCategory,
    pub operators: Vec<Box<dyn Operator>>,
}
```

## Component 5: InsightForge Integration

The BookRAG planner is exposed as a `DomainSearcher` that plugs directly into InsightForge's RRF merge pipeline.

### BookRAGSearcher

```rust
pub struct BookRAGSearcher {
    planner: Arc<RetrievalPlanner>,
}

#[async_trait]
impl DomainSearcher for BookRAGSearcher {
    fn domain_name(&self) -> &str { "book_index" }

    async fn search(&self, query: &str, limit: usize) -> Vec<MemoryEntry> {
        // 1. Plan: classify query + generate operators
        let plan = self.planner.plan(query).await;
        if plan.category == PassThrough { return vec![]; }

        // 2. Execute: run operator pipeline
        let mut ctx = OperatorContext::new(query, &self.planner.book_index);
        for op in &plan.operators {
            op.execute(&mut ctx).await?;
        }

        // 3. Convert: ScoredNode -> MemoryEntry
        ctx.working_set.iter()
            .take(limit)
            .map(|node| MemoryEntry {
                id: node.node.id.clone(),
                content: node.node.content.clone(),
                score: node.combined,
                source: MemorySource::Domain { name: "book_index".into() },
                raw_score: node.combined,
            })
            .collect()
    }
}
```

### Wiring (in `agent_loop/builder.rs`)

```rust
// After existing searcher registration:
let book_index = Arc::new(BookIndex::new(tree_repo, entity_repo, gt_link_repo, vector_store, embedder));
let planner = Arc::new(RetrievalPlanner::new(provider.clone(), book_index.clone()));
let bookrag_searcher = Arc::new(BookRAGSearcher::new(planner));

forge.add_searcher(bookrag_searcher);
```

No changes to `ContextEngine`, `AgentRuntime`, `MemoryRetriever`, or any other existing component. BookRAG results blend into the existing RRF merge at `k=60.0` alongside notes, tasks, graph, and finance results.

### Activation Gate

BookRAGSearcher returns empty results when BookIndex has no content (e.g., first run before any notes/tasks are indexed). The cached `has_content()` check makes this zero-overhead.

## Component 6: Incremental Tree Construction

Tree builds are triggered by domain events, following the same pattern as `BackgroundConsolidationService`.

### Event-driven Updates

```rust
// In BackgroundConsolidationService or a new BookIndexUpdater:
match event {
    DomainEvent::NoteUpdated { note_id, content } => {
        book_index.delete_source_tree(SourceType::Note, &note_id).await?;
        book_index.build_from_note(&note_id, &content).await?;
    }
    DomainEvent::NoteDeleted { note_id } => {
        book_index.delete_source_tree(SourceType::Note, &note_id).await?;
    }
    DomainEvent::TaskCreated { project_id, .. } |
    DomainEvent::TaskUpdated { project_id, .. } => {
        book_index.build_from_task_hierarchy(&project_id).await?;
    }
}
```

### Markdown Parser

Simple heading-based tree construction:

1. Split markdown into blocks (headings, paragraphs, code fences, tables)
2. Heading depth (`#` = level 1, `##` = level 2) determines hierarchy
3. Non-heading blocks become children of the nearest preceding heading
4. Content blocks without a preceding heading attach to a synthetic root node

### Entity Extraction During Build

After tree nodes are inserted:

1. For each leaf node (Text, Code, Table, Task), extract entities via LLM or heuristic NER
2. For each extracted entity, run `resolve_entity()` (gradient-based ER)
3. Create GT-Link entry: `entity_id -> tree_node_id`
4. Extract relationships between co-occurring entities in the same node

Entity extraction runs in background (spawned task) so note/task save remains fast.

### Skill Tree (Static)

Built once at boot from compiled skill YAML sections. Rebuilt only when skills change (rare, requires restart).

## Storage Summary

### New SQLite Tables

| Table | Purpose |
|-------|---------|
| `book_tree_nodes` | Hierarchical tree nodes |
| `book_tree_nodes_fts` | FTS5 for keyword search in nodes |
| `entity_tree_links` | GT-Link junction (entity <-> tree node) |

### New LanceDB Table

| Table | Purpose |
|-------|---------|
| `entity_embeddings` | 384-dim vectors for entity names, used in gradient-based ER |

### Existing Tables Reused (No Changes)

| Table | Component |
|-------|-----------|
| `entities` | Graph `G` vertices |
| `relationships` | Graph `G` edges |
| `cognitive_fact_embeddings` | Cognitive memory vectors |
| `conv_embeddings` | Conversation recall vectors |

## Module Structure

```
crates/context_engine/src/
    book_index/
        mod.rs              -- BookIndex orchestrator
        tree.rs             -- TreeNode, BookTreeRepo, markdown parser
        gt_link.rs          -- GTLinkRepo, entity-to-node mapping
        entity_resolution.rs -- Gradient-based ER (Algorithm 1)
        types.rs            -- TreeNodeType, SourceType, ScoredNode, configs
    operators/
        mod.rs              -- Operator trait, OperatorType, OperatorContext, pipeline executor
        formulator.rs       -- Decompose, Extract
        selector.rs         -- FilterModal, FilterRange, SelectByEntity, SelectBySection
        reasoner.rs         -- GraphReasoning (PageRank), TextRanker, SkylineRanker
        synthesizer.rs      -- Map, Reduce, SubQueryExecutor
    retrieval_planner/
        mod.rs              -- RetrievalPlanner
        classifier.rs       -- QueryCategory, heuristic + LLM classification
        plan.rs             -- RetrievalPlan, plan generation per category
    insight_forge/
        bookrag_searcher.rs -- NEW: BookRAGSearcher (DomainSearcher impl)
        mod.rs              -- EXISTING: add bookrag_searcher to module
        ... (existing files unchanged)
```

## What's NOT Changing

- `ContextEngine` assembly pipeline
- `AgentRuntime` execution flow
- `UnifiedMemoryService` / cognitive memory scoring (FSRS, decay, situational boost)
- `HistoryCompressor` / `BudgetAllocator`
- All existing `ContextSource` implementations
- `SkillRouter` / `IntentAnalyzer`
- Existing `DomainSearcher` implementations (NoteSearcher, TaskSearcher, GraphSearcher, FinanceSearcher)
- `VectorStore` CRUD operations
- Config schema (new fields added, no breaking changes)

## Config Additions

```json
{
  "cognitive": {
    "bookIndex": {
      "enabled": true,
      "entityResolution": {
        "topK": 10,
        "gradientThreshold": 0.6,
        "minSimilarity": 0.3,
        "useLlmDisambiguation": false
      },
      "retrieval": {
        "maxNodes": 50,
        "pagerankDamping": 0.85,
        "pagerankIterations": 20
      }
    }
  }
}
```

## Risks & Mitigations

| Risk | Mitigation |
|------|-----------|
| LLM calls in operators (Extract, Decompose, Map, Reduce) add latency | Heuristic-first classification; operators are optional and skip on PassThrough; per-source timeout in InsightForge (800ms default) |
| Entity extraction during indexing is slow for large notes | Run in background task; note save returns immediately |
| Gradient-based ER may merge entities that shouldn't be merged | Conservative threshold (g=0.6); min_similarity floor (0.3); entity merge is reversible (split operation can be added later) |
| Small subgraph PageRank may not converge for disconnected components | Seed-based personalized PageRank handles disconnected components naturally (isolated nodes get low scores) |
| Tree rebuilds on every note edit | Delete-then-rebuild is fast for typical note sizes (30-50 nodes); SQLite handles this in <10ms |

## Success Criteria

1. **BookIndex builds** from notes and tasks without errors
2. **Gradient-based ER** merges obvious aliases ("LLM" <-> "Large Language Model") while keeping distinct entities separate
3. **SingleHop queries** return relevant tree nodes for entity-linked lookups
4. **MultiHop queries** decompose and synthesize across multiple sections
5. **RRF integration** — BookRAG results blend naturally with existing domain searchers
6. **No regression** — existing cognitive memory, conversation recall, and InsightForge behavior unchanged
7. **Performance** — BookRAGSearcher completes within InsightForge's per-source timeout (800ms) for typical queries
