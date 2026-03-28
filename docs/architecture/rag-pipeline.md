# RAG Pipeline

## Overview

Klyntbot implements a multi-dimensional Retrieval-Augmented Generation (RAG) pipeline that integrates semantic memory retrieval, conversation recall, query rewriting, and live context injection into the LLM's reasoning loop.

```
User Message
     |
     v
+----+-----+
| Query    |
| Rewriter |----> RetrievalContext (active task, user situation,
+----+-----+      active view, correction context)
     |
     v
Enriched Query
     |
     +--concurrent---> Memory Retrieval (LanceDB vector search)
     |                 InsightForge (multi-domain search)
     |
     v
Retrieved Facts + Insights
     |
     v
+----+---------+
| Relevance    |
| Scorer       |----> 6-factor weighted scoring
+----+---------+
     |
     v
Ranked Context
     |
     v
+----+---------+
| Token Budget |
| Allocator    |----> Waterfall allocation by priority
+----+---------+
     |
     v
Assembled Context --> LLM
     |
     v
(During ReAct loop)
     |
     +--per iteration--> LiveContextRefresher
                         (inject newly promoted memories)
```

## Indexing Pipeline

### Embedding Generation

All embeddings use a 384-dimensional vector space via ONNX fastembed model.

**Embedding triggers:**

| Trigger | Source | Target LanceDB Table |
|---------|--------|---------------------|
| Semantic fact created/updated | BackgroundConsolidationService | `cognitive_fact_embeddings` |
| Chat message sent | AgentLoop (fire-and-forget) | `conv_embeddings` |
| Task created/updated | TaskTool | `task_embeddings` |
| Note tree node created/updated | NoteTreeBuilder (Phase 1) | `tree_node_embeddings` |
| Community formed/updated | CommunityBuilder (Phase 2) | `community_embeddings` |
| Activity ingested | ActivityIngestionService | `activity_embeddings` |
| Work context clustered | ContextInferenceEngine | `work_context_embeddings` |
| Flashcard created | LearningTool | `flashcard_embeddings` |
| Knowledge entity extracted | KnowledgeGraphService | `entity_embeddings` |
| Note insight generated | InsightForge | `insight_embeddings` |

> **Deprecated:** `NoteEmbeddingHandler` → `note_embeddings` (flat 500-char blob). Replaced by `NoteTreeBuilder` which embeds each tree node individually into `tree_node_embeddings`. Legacy table retained during migration; dropped once all notes have tree node embeddings.

### LanceDB Vector Store

12 tables, all using `FixedSizeList<Float32, 384>`:

```
lance/
  cognitive_fact_embeddings.lance/   -- Semantic facts (subject+predicate+object)
  conv_embeddings.lance/             -- Conversation messages (content_preview + full_content)
  task_embeddings.lance/             -- Task titles + descriptions
  tree_node_embeddings.lance/        -- Note tree nodes (per-section, hierarchical)
  community_embeddings.lance/        -- Community summaries (cross-note clusters)
  activity_embeddings.lance/         -- Activity log entries
  work_context_embeddings.lance/     -- Work context clusters
  flashcard_embeddings.lance/        -- Flashcard content (front/back)
  entity_embeddings.lance/           -- Knowledge graph entities
  insight_embeddings.lance/          -- Note insights
  note_embeddings.lance/             -- [DEPRECATED] Flat note content (replaced by tree_node_embeddings)
  todo_embeddings.lance/             -- Legacy todo items
```

**ANN indexes:** Created in background task after startup (requires 256+ rows). Non-fatal on failure.

**Predicate sanitization:** `sanitize_predicate_value()` prevents SQL injection in vector search predicates.

## Retrieval Pipeline

### Query Rewriting

Before retrieval, the raw user message is enriched by `QueryRewriter`:

```
QueryRewriter::rewrite(message, RetrievalContext)
     |
     +-- RetrievalContext includes:
     |     - active_task (current task focus)
     |     - user_situation (inferred from activity)
     |     - active_view (current desktop page)
     |     - correction_context (exclude rejected topic terms)
     |
     +-- Returns RewriteResult { rewritten_query, source }
     |
     +-- source: RewriteSource (Original, ContextEnriched, CorrectionAdjusted)
```

When `CorrectionContext` is set (user corrected the AI), key terms from the rejected topic are excluded from the retrieval query to prevent re-retrieval of irrelevant context.

### Semantic Memory Retrieval

```
ContextEngine::retrieve_memory(query, profile)
     |
     v
EmbeddingEngine::embed_async(query) --> query_vector: Vec<f32>
     |
     v
VectorStore::search(table="cognitive_fact_embeddings",
                    query_vector,
                    top_k=vector_top_k,      -- default from config/autotuner
                    min_similarity)           -- default from config/autotuner
     |
     v
Vec<SemanticFact> candidates
     |
     v
Relevance scoring (10-factor):
  score = w_semantic       * cosine_similarity(query_vec, fact_vec)
        + w_retrievability * retrievability(elapsed_days, stability)
        + w_importance     * fact.importance
        + w_frequency      * frequency_score(access_count)
        + w_situation      * situational_boost(fact, context)
        + w_temporal       * temporal_recency(age_days)
        + w_hierarchy      * hierarchy_score(node_level, query_type)
        + w_path_coherence * path_coherence(node, all_scores)
        + w_community      * community_membership(node, community)
        + w_cross_note     * cross_note_boost(source_note_count)
     |
     v
Ranked + filtered facts --> formatted as context section
```

**Default weights (Phase 2):** semantic=0.20, retrievability=0.10, importance=0.08, frequency=0.05, situation=0.15, temporal=0.02, hierarchy=0.10, path_coherence=0.05, community=0.15, cross_note=0.10.

**Phase 1 additions (hierarchy + path_coherence):**
- `hierarchy_score` — biases retrieval toward correct tree depth (favor roots for summaries, leaves for details)
- `path_coherence` — rewards nodes whose siblings also scored well (the entire section is relevant, not just an isolated paragraph)

**Phase 2 additions (community + cross_note):**
- `community_membership` — `membership_score × community_stability` for tree nodes in a matched community. Recency boost: +0.15 if community updated within last 60 seconds
- `cross_note_boost` — `log2(source_note_count)` clamped to [0, 1]. Communities spanning 4+ notes get maximum boost

**Backward compatibility:** For non-note results (cognitive facts, conversation recall): `w_hierarchy = 0`, `w_path_coherence = 0.5` (neutral), `w_community = 0.0`, `w_cross_note = 0.0`. Scorer degrades to original 6-factor behavior.

**Autotuner influence:** All weights, `vector_top_k`, and `min_similarity` are tunable parameters in the autotuner's 28-dimensional search space (expanded from 19D → 24D in Phase 1, 24D → 28D in Phase 2).

### Retrievability Decay

Two decay models used in different contexts:

**Memory relevance (exponential):**
```
retrievability(elapsed_days, stability) = exp(ln(0.9) * elapsed_days / stability)
```

**FSRS-5 card scheduling (power law):**
```
retrievability(elapsed_days, stability) = 1 / (1 + elapsed_days / (9 * stability))
```

**Temporal recency:**
```
temporal_recency_score(age_days) = max(1 / (1 + age_days / 30), 0.1)
```

### InsightForge (Multi-Domain RAG)

`InsightForge` performs multi-dimensional retrieval across different data domains:

```
InsightForge::search(query)
     |
     v
LlmDecomposer::decompose(query)
     |
     +-- Decomposes into sub-queries per domain
     |   (falls back to HeuristicDecomposer on LLM failure)
     |
     v
Parallel domain searches:
     |
     +---> NoteTreeNavigator --> tree_node_embeddings + community_embeddings
     +---> TaskSearcher      --> task_embeddings
     +---> FinanceSearcher   --> finance data
     |
     v
Per-session circuit breaker (trips after N failures within a session; falls back to plain memory retrieval)
     |
     v
Merged + ranked results (RRF k=60)
```

> **Deleted searchers:** `BookRAGSearcher` (Phase 1) and `GraphSearcher` (Phase 2) replaced by `NoteTreeNavigator` which handles all note retrieval (tree nodes, hierarchical traversal, and community-aware graph reasoning).

### NoteTreeNavigator (4-Path Retrieval)

`NoteTreeNavigator` implements `DomainSearcher` and provides hierarchical note retrieval with community awareness:

```
NoteTreeNavigator::search(query, context)
  │
  ├── QueryClassifier (heuristic, no LLM):
  │     Structural: "section", "heading", "in my note about"
  │     Community: "across my notes", "connect", "everything about"
  │     RetrievalContext: active_view, active_task, hierarchical_intent
  │     Returns: Simple | Hierarchical | Hybrid | Community
  │
  ├── Path 1: SimpleQuery (~50% of queries)
  │     Vector search on tree_node_embeddings
  │     → Load ancestor chain for path context
  │
  ├── Path 2: HierarchicalQuery (~15% of queries)
  │     Tree traversal with entity-guided drill-down:
  │       coarse vector search → per-note subtree scoring
  │       → best path through tree → RRF merge across notes
  │
  ├── Path 3: HybridQuery (~20% of queries)
  │     Fused simple + hierarchical when cross-domain context exists
  │     (active_task + hierarchical_intent, or mixed note/external entities)
  │
  └── Path 4: CommunityQuery (~15% of queries, Phase 2)
        Vector search on community_embeddings
        → Load community members by membership_score
        → Format with community card metadata
        → RRF merge with Paths 1-3
```

**Proactive community triggering:** When query mentions one entity but `active_task` or recent notes belong to a known community → Path 4 triggers automatically. Soft confidence threshold `community_intent_confidence` (autotunable, default 0.65).

## Chunking Strategy

Klyntbot does **not** use traditional document chunking. Instead:

- **Semantic facts** are atomic triples (subject, predicate, object) — already minimal units
- **Conversation messages** are stored per-message (not chunked)
- **Notes** are embedded per tree node (heading sections, bullet groups) — each node is an atomic unit with structural context
- **Tasks** are embedded as title + description
- **Communities** are embedded from summary text derived from top member nodes

This is appropriate because:
1. Semantic facts are extracted as atomic knowledge units by the consolidation pipeline
2. Tree nodes provide natural section-level granularity for notes (a note with 5 headings → 6 embeddings instead of 1 truncated blob)
3. The embedding model's 384-dim space handles short-to-medium text well
4. The context budget allocator handles length constraints

## Memory Prefetch Optimization

Memory retrieval is the most expensive step in context assembly. The runtime overlaps it with intent classification:

```
process_message()
     |
     +--tokio::spawn--> prefetch_memory(initial_profile)
     |                       |
     |                  embed query
     |                  vector search
     |                  relevance scoring
     |                       |
     +--sequential----> IntentAnalyzer::analyze()
     |                       |
     |                  heuristic (fast)
     |                  LLM classifier (if needed)
     |                       |
     +--await both------+----+
     |
     +-- Profile match? (95% of the time: yes)
     |     Yes --> use prefetched result
     |     No  --> prefetch_handle.abort(); fresh retrieval with correct profile
     |            (orchestration override changed the profile mid-pipeline)
     |
     v
ContextEngine::assemble_with_prefetched()
```

## Live Context Injection (During ReAct Loop)

Unlike traditional RAG where retrieval happens once before generation, Klyntbot supports **mid-execution context injection**:

```
Iteration 1: LLM reasons + calls tools
     |
Producers push context updates:
     |
     +-- BackgroundConsolidationService: MemoryPromoted
     +-- NoteTreeBuilder: NoteStructureChanged (Phase 1)
     +-- CommunityBuilder: CommunityDiscovered/Updated/Weakened (Phase 2)
     |
     v
ContextUpdateQueue::push(ContextUpdate {
    priority: High,
    reason: "MemoryPromoted" | "NoteStructureChanged" | "CommunityUpdated",
    content: rich_injection_payload
})
     |
     v
Iteration 2 boundary:
LiveContextRefresher::inject_pending()
     |
     +-- Drain queue
     +-- Sort by priority
     +-- Calculate remaining budget (80% for Normal, 90% for High)
     +-- Inject as Message::ContextUpdate:
     |     <context_update reason="...">
     |       payload content
     |     </context_update>
     +-- Emit AgentEvent::ContextReassembled
     |
     v
Iteration 2: LLM now has access to updated context
```

### ContextUpdateReason Variants

| Reason | Producer | Priority | Payload |
|--------|----------|----------|---------|
| `MemoryPromoted` | BackgroundConsolidation | High | Fact content + confidence |
| `NoteStructureChanged` | NoteTreeBuilder | Normal (High if linked to active_task) | Path + content preview + linked entities |
| `CommunityDiscovered` | CommunityBuilder | Normal | Community name + member count + top entities |
| `CommunityUpdated` | CommunityBuilder | Normal (High if shares entities with active_task) | Community name + source_note_count + representative_paths + stability trend |
| `CommunityWeakened` | CommunityBuilder | Normal | Community name + stability |

### Rich Injection Payloads

Payloads include actionable context so the LLM can act on new information without a separate retrieval call:

```
NoteStructureChanged: "In Health > Sleep > Coffee Effects, you added:
  'Caffeine after 2pm reduces deep sleep by 18%'
  (linked entities: Habit:Evening Wind-down, Metric:Sleep Quality)"

CommunityUpdated: "'Sleep Optimization' (5 sections from 3 notebooks) strengthened
  — new section from Health > Sleep > Caffeine Effects.
  Top entities: caffeine, melatonin, circadian rhythm. Stability: 0.87 (+0.12)"
```

**Budget protection:** Standard updates use max 80% of remaining context capacity (20% reserved for response). High-priority updates can use 90%.

**Dedup:** 30-second window on (reason, content) pairs prevents spam.

**Pause mode:** `ExecutionParams.pause_context_updates = true` freezes context for deterministic testing.

## Hierarchical Note Indexing (Phase 1)

Notes are indexed as tree structures rather than flat blobs. `NoteTreeBuilder` subscribes to `DomainEvent::NoteContentChanged` and builds a tree of heading sections:

```
NoteContentChanged
  → NoteTreeBuilder:
      1. Parse note: Tiptap JSON primary, markdown fallback
      2. Clear old tree nodes for this note
      3. Insert new tree nodes (book_tree_nodes table)
      4. Embed each node → tree_node_embeddings (LanceDB)
      5. Link entity mentions → entity_tree_links
      6. Push ContextUpdateQueue (NoteStructureChanged)
```

**Tree node embedding text:** Root nodes embed title only, heading nodes embed title + 300-char content preview, leaf pseudo-sections (bullets) embed content only. Each node gets its own 384-dim embedding.

**Storage:** `book_tree_nodes` (SQLite, with FTS5 auto-sync triggers) + `tree_node_embeddings` (LanceDB). Entity references stored in `entity_tree_links`.

## Community Detection (Phase 2)

Louvain community detection over tree nodes connected by shared entities:

```
NoteContentChanged (after NoteTreeBuilder)
  → CommunityBuilder (debounce 5s):
      1. Build weighted graph:
         Vertices = book_tree_nodes
         Edges = shared entities via entity_tree_links
         Weight = count(shared_entities) × avg(entity_relationships.strength)
      2. Run Louvain algorithm (petgraph)
      3. Diff against previous community assignments
      4. For changed/new communities:
         - Compute membership_score per member
         - Compose summary, representative_paths, top_entities
         - Embed summary → community_embeddings (LanceDB)
         - Upsert communities + community_members (SQLite)
      5. Emit events: CommunityDiscovered | CommunityUpdated | CommunityWeakened
      6. Push ContextUpdateQueue with rich payloads
```

**Storage:** `communities` + `community_members` (SQLite) + `community_embeddings` (LanceDB).

**Stability:** FSRS-inspired score. Increases on persistence across Louvain re-runs, decays on member loss. Communities with `stability < 0.3` auto-pruned.

## FTS5 Full-Text Search (Complementary)

SQLite FTS5 virtual tables complement vector search for exact-match queries:

| Table | Indexed Content |
|-------|----------------|
| `semantic_facts_fts` | subject, predicate, object of semantic facts |
| `episodic_memories_fts` | content of episodic memories |
| `procedural_rules_fts` | condition, action of procedural rules |
| `note_fts5` | note title + content |
| `book_tree_nodes_fts` | tree node title + content (auto-synced via triggers) |

FTS5 triggers automatically maintain sync with base tables via INSERT/UPDATE/DELETE triggers.

## Conversation Recall (Separate from RAG)

In addition to semantic memory RAG, conversation history is available through:

1. **Recent history** (uncompressed): Last N messages from session, injected as conversation context
2. **Compressed history**: Older messages summarized by `HistoryCompressor` → `SummaryProvider`
3. **Conversation embeddings**: `conv_embeddings` table in LanceDB for cross-session recall

The `conv_embeddings` table stores: `session_key`, `role`, `content_preview`, `full_content` — enabling semantic search across all past conversations.
