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
| Note created/updated | NoteEmbeddingHandler | `note_embeddings` |
| Activity ingested | ActivityIngestionService | `activity_embeddings` |
| Work context clustered | ContextInferenceEngine | `work_context_embeddings` |
| Flashcard created | LearningTool | `flashcard_embeddings` |
| Knowledge entity extracted | KnowledgeGraphService | `entity_embeddings` |
| Note insight generated | InsightForge | `insight_embeddings` |

### LanceDB Vector Store

10 tables, all using `FixedSizeList<Float32, 384>`:

```
lance/
  cognitive_fact_embeddings.lance/   -- Semantic facts (subject+predicate+object)
  conv_embeddings.lance/             -- Conversation messages (content_preview + full_content)
  task_embeddings.lance/             -- Task titles + descriptions
  note_embeddings.lance/             -- Note content
  activity_embeddings.lance/         -- Activity log entries
  work_context_embeddings.lance/     -- Work context clusters
  flashcard_embeddings.lance/        -- Flashcard content (front/back)
  entity_embeddings.lance/           -- Knowledge graph entities
  insight_embeddings.lance/          -- Note insights
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
Relevance scoring (6-factor):
  score = w_semantic     * cosine_similarity(query_vec, fact_vec)
        + w_retrievability * retrievability(elapsed_days, stability)
        + w_importance    * fact.importance
        + w_frequency     * frequency_score(access_count)
        + w_situation     * situational_boost(fact, context)
        + w_temporal      * temporal_recency(age_days)
     |
     v
Ranked + filtered facts --> formatted as context section
```

**Default weights:** semantic=0.30, retrievability=0.20, importance=0.15, frequency=0.10, situation=0.25, temporal=0.05.

**Autotuner influence:** All weights, `vector_top_k`, and `min_similarity` are tunable parameters in the autotuner's 19-dimensional search space.

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
     +---> NoteSearcher    --> note_embeddings
     +---> TaskSearcher    --> task_embeddings
     +---> FinanceSearcher --> finance data
     +---> GraphSearcher   --> entity_embeddings
     |
     v
Per-session circuit breaker (trips after N failures within a session; falls back to plain memory retrieval)
     |
     v
Merged + ranked results
```

## Chunking Strategy

Klyntbot does **not** use traditional document chunking. Instead:

- **Semantic facts** are atomic triples (subject, predicate, object) — already minimal units
- **Conversation messages** are stored per-message (not chunked)
- **Notes** are embedded as full content (not chunked into passages)
- **Tasks** are embedded as title + description

This is appropriate because:
1. Semantic facts are extracted as atomic knowledge units by the consolidation pipeline
2. The embedding model's 384-dim space handles short-to-medium text well
3. The context budget allocator handles length constraints

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
BackgroundConsolidationService promotes memory
     |
     v
ContextUpdateQueue::push(ContextUpdate {
    priority: High,
    reason: "MemoryPromoted",
    content: "User prefers dark mode (confidence: 0.92)"
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
     |     <context_update reason="MemoryPromoted">
     |       User prefers dark mode (confidence: 0.92)
     |     </context_update>
     +-- Emit AgentEvent::ContextReassembled
     |
     v
Iteration 2: LLM now has access to newly promoted memory
```

**Budget protection:** Standard updates use max 80% of remaining context capacity (20% reserved for response). High-priority updates can use 90%.

**Dedup:** 30-second window on (reason, content) pairs prevents spam.

**Pause mode:** `ExecutionParams.pause_context_updates = true` freezes context for deterministic testing.

## FTS5 Full-Text Search (Complementary)

SQLite FTS5 virtual tables complement vector search for exact-match queries:

| Table | Indexed Content |
|-------|----------------|
| `semantic_facts_fts` | subject, predicate, object of semantic facts |
| `episodic_memories_fts` | content of episodic memories |
| `procedural_rules_fts` | condition, action of procedural rules |
| `note_fts5` | note title + content |

FTS5 triggers automatically maintain sync with base tables via INSERT/UPDATE/DELETE triggers.

## Conversation Recall (Separate from RAG)

In addition to semantic memory RAG, conversation history is available through:

1. **Recent history** (uncompressed): Last N messages from session, injected as conversation context
2. **Compressed history**: Older messages summarized by `HistoryCompressor` → `SummaryProvider`
3. **Conversation embeddings**: `conv_embeddings` table in LanceDB for cross-session recall

The `conv_embeddings` table stores: `session_key`, `role`, `content_preview`, `full_content` — enabling semantic search across all past conversations.
