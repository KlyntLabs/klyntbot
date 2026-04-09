# Memory Retrieval Upgrade: Bridge Then Dissolve

**Date:** 2026-04-09
**Status:** Draft
**Approach:** Approach 2 — Bridge Then Dissolve (B→C)

## Problem Statement

Klyntbot's memory infrastructure (SQLite + LanceDB + FTS5) is architecturally sound, but the cognitive pipeline on top of it has four gaps that cripple retrieval quality:

1. **Entity/community embeddings disconnected from retrieval** — LanceDB entity_embeddings and community_embeddings exist but are only used by Fabric visualization and NoteTreeNavigator, never during fact retrieval.
2. **Conversation recalls siloed** — `conv_embeddings` and `cognitive_fact_embeddings` are completely separate vector stores. RecallCollector gates are very high (3+ messages, 2+ sessions). Conversations never promote to facts/entities.
3. **No relationship extraction from conversations** — `entity_relationships` table exists but no automatic relationship/edge extraction from chat turns.
4. **No `as_of` temporal queries** — Timestamps exist everywhere but there's no "what did the agent know at time T?" query path.

The root cause: **split memory**. Facts are clean but lack living context. Conversations are rich in context but not standardized. The richest data source (conversations) is wasted.

## Design Principles

1. **Retrieval never gets worse** — Each phase adds signals, never removes existing ones. Kill switches via feature flags or weight=0.0.
2. **Value-density filter before graph writes** — No conversation data enters the knowledge graph without passing the classifier. The single most important guardrail.
3. **Temporal append-only** — Facts are never mutated in place. Updates create new versions linked via `superseded_by`.
4. **Cost proportional to value** — Real-time extraction is cheap (piggybacked). Batch enrichment is triggered by value signals, not timers.
5. **Build A to unlock B** — Debugging/introspection foundation enables conversational temporal self-awareness.

## Architecture Overview

Three phases, one vision:

```
Phase 1: BRIDGE                    Phase 2: ENRICH                  Phase 3: DISSOLVE
---------------------              ---------------------            ---------------------
conv_embeddings --+                conv_embeddings --+              conv_embeddings
                  +-> Blended      ValueDensity -----+              (staging only)
fact_embeddings --+    Score       Classifier         |                    |
                                        |             +-> Graph      ValueDensity
entity scores -----> Retrieval     Entity Resolution  |    Aware     Classifier
  (wired in)          Formula      Typed Edges -------+    Retrieval      |
                                                                     v
temporal Layer 1 -> Fact Log       temporal Layer 2 -> Snapshots    Graph = Memory
(append-only)                      (derived state)                  Temporal L3
                                                                    (reasoning)
```

### New Components

| Component | Crate | Purpose |
|-----------|-------|---------|
| `CrossRetriever` | `cognitive` | Blends conv_embeddings results into fact retrieval scoring |
| `ValueDensityClassifier` | `cognitive` | Scores conversation turns for enrichment worthiness |
| `GraphEnricher` | `cognitive` | Extracts entities + relationships from conversations via batch LLM |
| `EntityResolver` | `cognitive` | Batch entity deduplication and merge |
| `TemporalIndex` | `cognitive` | Append-only fact log + snapshot reconstruction + temporal queries |
| `ConversationPromoter` | `cognitive` | Promotes high-value conversation data into the graph |
| `TemporalTool` | `tools` | Agent-facing tool for temporal reasoning queries |

---

## Phase 1: Bridge

**Goal:** Immediate retrieval quality boost by connecting the two siloed worlds. Zero changes to the knowledge graph write path.

### 1a. Cross-Retrieval Layer

**Where:** New module `crates/cognitive/src/services/cross_retrieval.rs`

After the existing two-pass scoring (base + community re-ranking), a third pass queries `conv_embeddings` for supporting conversation evidence:

- For top-K scored facts (configurable, default 15), query `conv_embeddings` for semantically similar conversation turns
- If conversation hits support a fact, boost its score
- New signal: `recall_support` (0.0-1.0), computed as `max(cosine_sim) * recency_decay`
- Conv search limited to 3 results per fact for performance

**Scoring integration:** Add `recall_support` as an 11th factor to `RelevanceWeights` in `decay.rs`. Default weight: `0.08` (taken proportionally from existing weights). The autotuner can tune this weight. Reforge observes its impact via retrieval precision metrics.

### 1b. Real-Time Entity Extraction (Cheap Path)

**Where:** Extend existing `ExtractionHandler` in `crates/cognitive/src/services/background.rs`

Piggyback on the existing LLM fact-extraction call. Extend the extraction prompt to also return:

```json
{
  "entities": [
    { "name": "Sarah Nguyen", "type": "person", "context": "API team lead" }
  ],
  "relations": [
    { "source": "Sarah Nguyen", "target": "API migration", "type": "leads", "strength": 0.5 }
  ]
}
```

Entities and relationships go to existing `entity.rs` repo (`upsert_entity`, `upsert_relationship`) with weak strength (0.3-0.5). No new infrastructure — richer extraction prompts feeding existing tables.

This is the cheap path. Entities are approximate, relationships are weak. Phase 2 refines them.

### 1c. Temporal Fact Log (Layer 1)

**Where:** New table `fact_changelog` in cognitive migrations

Append-only log of every fact mutation:

| Column | Type | Purpose |
|--------|------|---------|
| `id` | TEXT PK | UUID |
| `fact_id` | TEXT FK | Which fact changed |
| `operation` | TEXT | `created`, `updated`, `superseded`, `archived`, `confidence_changed` |
| `old_value` | TEXT (JSON) | Previous state (null for creates) |
| `new_value` | TEXT (JSON) | New state |
| `source` | TEXT | What triggered: `extraction`, `reforge`, `user`, `consolidation` |
| `recorded_at` | TEXT | Timestamp |

Wired into `SemanticFactRepo` mutations (upsert, update_convergence, archive) as after-write appends. No query API yet — reliable logging only.

### 1d. RecallCollector Threshold Adjustment

Lower gates from "3+ messages, 2+ sessions" to "2+ messages, 1+ session" in `recall_collector.rs`. Config change, not architectural — increases conversation signal flow into the fact promotion pipeline.

---

## Phase 2: Enrich

**Goal:** Build the value-density filter, upgrade graph quality through batch processing, establish temporal snapshots.

### 2a. Value-Density Classifier

**Where:** New module `crates/cognitive/src/services/value_density.rs`

Lightweight heuristic scoring function (no LLM) that classifies each conversation turn:

```
ValueDensity = weighted_sum(
    entity_signal:     0.30  -- named entities detected (NER-like heuristics)
    action_signal:     0.25  -- action verbs present (decide, plan, migrate, build, assign, cancel...)
    decision_signal:   0.25  -- decision markers ("let's go with", "I've decided", "we should", "changed my mind")
    novelty_signal:    0.20  -- references entities/topics not yet in the graph
)
```

**Output:** Score 0.0-1.0 with three tiers:
- **High (>0.7):** Triggers immediate batch enrichment
- **Medium (0.4-0.7):** Queued for next Reforge cycle
- **Low (<0.4):** Real-time cheap extraction only (Phase 1 path)

**Design choice:** Heuristic-based, not LLM-based. Must be fast (runs on every turn). Biased toward recall over precision — false positives are OK (batch enrichment filters further), false negatives are not.

### 2b. Batch Graph Enrichment (Smart Path)

**Where:** New module `crates/cognitive/src/services/graph_enricher.rs`

**Two trigger paths:**

1. **Immediate (high value-density >0.7):** Queue a targeted enrichment job. Runs within minutes, focused on that turn's entities + relationships.

2. **Nightly (Reforge Phase 6.5):** Processes all medium-density turns accumulated since last cycle. Full day's context for better entity resolution.

**Batch LLM call produces:**

```json
{
  "resolved_entities": [
    { "canonical": "Sarah Nguyen", "aliases": ["Sarah", "SN"], "type": "person", "description": "API team lead, backend" }
  ],
  "typed_relationships": [
    { "source": "Sarah Nguyen", "target": "API Migration v2", "type": "leads", "strength": 0.85, "evidence": "session_abc turn 12" }
  ],
  "entity_merges": [
    { "keep": "Sarah Nguyen", "merge": ["Sarah", "S. Nguyen"] }
  ]
}
```

**Writes to:**
- `entities` table — upsert with resolved descriptions, types
- `entity_relationships` — upgrade weak edges to typed/weighted, add new edges
- `entity.merge_entities()` — deduplicate using existing merge infrastructure

### 2c. Reforge Phase 6.5: Graph Consolidation

**Where:** New stage in `crates/cognitive/src/services/reforge/service.rs`, between Apply and Compact

**Steps:**

1. **Collect medium-density turns** from value-density queue since last cycle
2. **Batch entity resolution** — single LLM call with all pending entities for merge decisions
3. **Relationship refinement** — upgrade weak edges, add cross-session relationships
4. **Graph quality metrics** — compute and store:
   - Entity coverage (% of fact subjects that have entity records)
   - Relationship density (avg edges per entity)
   - Orphan rate (entities with 0 relationships)
   - Merge rate (entities merged this cycle)
5. **Feed metrics to Reforge synthesis prompt** — next cycle's LLM sees graph health

### 2d. Temporal Layer 2: Derived State (Snapshots)

**Where:** Extend `crates/cognitive/src/services/temporal.rs`

**Nightly snapshot** (triggered by Reforge, after Graph Consolidation):

| Column | Type | Purpose |
|--------|------|---------|
| `id` | TEXT PK | UUID |
| `snapshot_at` | TEXT | Timestamp |
| `fact_count` | INTEGER | Total active facts |
| `entity_count` | INTEGER | Total entities |
| `relationship_count` | INTEGER | Total edges |
| `domain_summary` | TEXT (JSON) | Per-domain fact counts and avg confidence |
| `top_entities` | TEXT (JSON) | Top-20 entities by mention count |
| `graph_metrics` | TEXT (JSON) | Coverage, density, orphan rate |

**On-demand reconstruction** — new method `TemporalIndex::facts_as_of(timestamp)`:

```sql
SELECT * FROM semantic_facts
WHERE recorded_at <= ?1
  AND (valid_until IS NULL OR valid_until > ?1)
  AND (superseded_at IS NULL OR superseded_at > ?1)
```

Gives "what facts were active at time T" using existing bi-temporal columns. Combined with `fact_changelog` from Phase 1, full state reconstruction at any point.

---

## Phase 3: Dissolve

**Goal:** Conversation recall store becomes staging area. Knowledge graph becomes primary memory. Temporal reasoning goes conversational.

### 3a. Conversation Promoter

**Where:** New module `crates/cognitive/src/services/conversation_promoter.rs`

The `conv_embeddings` store shifts from permanent silo to **staging area** with a promotion lifecycle:

```
Conversation Turn
      |
      v
  conv_embeddings (staging)
      |
      +--- Low density (<0.4) --> Stays in staging, ages out via 138-day decay
      |
      +--- Medium density (0.4-0.7) --> Queued for Reforge batch --> promoted to graph
      |
      +--- High density (>0.7) --> Immediate promotion to graph
                                         |
                                         v
                                    entities + relationships + facts
                                    (with source_conversation_id link)
```

**After promotion:** Conversation embedding stays in `conv_embeddings` but gets a `promoted_at` timestamp. Cross-retrieval (Phase 1) deprioritizes already-promoted entries to avoid double-counting.

**Provenance chain:** Every promoted fact/entity/relationship carries `source_conversation_id` linking back to the originating turn. Enables:
- "Where did I learn this?" — trace fact to conversation
- Temporal reasoning — "when did I first discuss this?" uses conversation timestamp
- Confidence calibration — facts from multiple conversations get higher convergence

### 3b. Graph-Aware Retrieval

**Where:** Modify `crates/cognitive/src/services/retrieval.rs` — new retrieval mode alongside existing paths

**Strategy: Graph-first with vector fallback**

```
Query arrives
    |
    +--- 1. Entity extraction from query (heuristic, no LLM)
    |         "What was Sarah working on before the pivot?"
    |         --> entities: ["Sarah"], temporal: "before pivot"
    |
    +--- 2. Graph neighborhood traversal
    |         Sarah -> [leads -> API Migration, works_with -> Backend Team, ...]
    |         Expand 1-2 hops, collect attached facts
    |
    +--- 3. Temporal filter (if temporal markers detected)
    |         "before pivot" -> resolve to timestamp via fact_changelog
    |         Filter facts to those active before that timestamp
    |
    +--- 4. Vector search (existing path) for non-graph results
    |
    +--- 5. Merge: Graph results (boosted) + Vector results (existing scoring)
              Graph results get graph_path_boost based on hop distance
              1-hop: 0.15, 2-hop: 0.08
```

**Scoring integration:** Add `graph_path_boost` as 12th factor to `RelevanceWeights`. Autotuner tunes its weight. Reforge observes retrieval precision with graph vs without.

**Fallback guarantee:** If entity extraction finds nothing or graph traversal returns <3 results, falls back entirely to existing vector+BM25 path. Retrieval never degrades.

### 3c. Temporal Layer 3: Reasoning Queries

**Where:** Extend `crates/cognitive/src/services/temporal.rs` with high-level query API

| Query | Implementation | Example |
|-------|---------------|---------|
| `facts_as_of(timestamp)` | Bi-temporal SQL filter (Phase 2) | "What did I know on March 1st?" |
| `first_mention(entity)` | Min `recorded_at` from `fact_changelog` WHERE subject matches | "When did I first mention Sarah?" |
| `change_history(entity)` | `fact_changelog` filtered by entity, ordered by time | "How has the API plan evolved?" |
| `competing_truths(subject, predicate)` | Active + superseded facts for same S-P pair | "What were the options we considered?" |
| `knowledge_diff(t1, t2)` | `facts_as_of(t1)` symmetric diff `facts_as_of(t2)` | "What changed between last week and now?" |
| `decision_points()` | Facts where `superseded_by IS NOT NULL`, grouped by subject | "What decisions have I reversed?" |

**Exposed as a tool:** New `TemporalTool` (multi-action, read-only) registered in the agent's tool registry. The agent calls these during conversation to answer temporal questions naturally.

### 3d. Reforge Feedback Loop Closure

Reforge synthesis prompt gets the full picture:

- Session scratchpads (existing)
- Episodic memories (existing)
- Retrieval precision metrics (existing)
- Graph quality metrics (Phase 2)
- Cross-retrieval blend effectiveness (Phase 1)
- Value-density distribution (Phase 2)
- Temporal diff since last cycle (Phase 3): "12 facts created, 3 superseded, 2 entities merged"
- Autotuner trial results for `recall_support` + `graph_path_boost` weights

Full visibility into memory system performance including graph health and temporal evolution. Can suggest targeted improvements: "entity coverage in domain X is low, increase extraction focus there."

---

## Testing & Validation Strategy

### Per-Phase Validation Gates

| Phase | Gate | Metric | Threshold |
|-------|------|--------|-----------|
| **Phase 1** | Cross-retrieval improves relevance | Retrieval precision (existing feedback repo) | +5% over baseline or no regression |
| **Phase 1** | Entity extraction doesn't break latency | P95 extraction time | <2x current |
| **Phase 1** | Fact changelog captures all mutations | Count(changelog) vs Count(fact writes) | 100% coverage |
| **Phase 2** | Value-density classifier biases toward recall | Manual review of 50 turns | <10% false negative rate |
| **Phase 2** | Entity resolution reduces duplicates | Orphan rate + merge rate from graph metrics | Orphan rate decreasing week-over-week |
| **Phase 2** | `facts_as_of` returns correct state | Test with known mutation sequences | 100% correctness on synthetic data |
| **Phase 3** | Graph-first retrieval matches or beats vector-only | A/B on retrieval precision | No regression + qualitative improvement on entity-centric queries |
| **Phase 3** | Temporal queries return accurate results | Integration tests with planted timelines | All 6 query types correct |

### Test Architecture

**Unit tests** (inline `#[cfg(test)]`):
- `ValueDensityClassifier` — score known turns, verify tier assignment
- `CrossRetriever` — mock conv_embeddings results, verify boost calculation
- `TemporalIndex::facts_as_of` — insert facts with known timestamps, verify reconstruction
- `fact_changelog` writes — verify all mutation paths emit changelog entries

**Integration tests** (in `tests/integration/cognitive.rs`):
- End-to-end: inject conversation turns -> verify entities -> verify relationships -> verify retrieval improvement
- Temporal round-trip: create facts, supersede them, query `facts_as_of` at various timestamps
- Reforge Phase 6.5: run cycle with accumulated medium-density turns, verify graph quality metrics

**Regression safety:**
- Existing retrieval tests continue passing unchanged
- New `recall_support` and `graph_path_boost` weights default to 0.0 until explicitly enabled
- Existing behavior preserved until Phase 1/3 are activated

### Observability

Per project non-goal (no structured observability), use existing patterns:
- `tracing::info!` for phase transitions, enrichment triggers, graph metrics
- `PipelineEvent` SSE stream for UI transparency on enrichment activity
- Graph quality metrics in SQLite (queryable via MirrorTool or TemporalTool)
- Reforge cycle narrative includes graph consolidation summary

---

## Affected Crates

| Crate | Changes |
|-------|---------|
| `cognitive` | Cross-retrieval, value-density, graph enricher, entity resolver, conversation promoter, temporal index, Reforge Phase 6.5 |
| `storage` | `fact_changelog` migration (SQLite), `knowledge_snapshots` migration (SQLite), `promoted_at` field on conv_embeddings (LanceDB schema) |
| `tools` | New `TemporalTool` (multi-action, read-only) |
| `agent` | Register TemporalTool, wire graph-aware retrieval |
| `config` | New weights (`recall_support`, `graph_path_boost`), value-density thresholds |

## Non-Goals

- Replacing LanceDB or SQLite with a different storage engine
- Adding structured observability (OpenTelemetry, Prometheus)
- Multi-user or server-based deployment considerations
- UI changes to Fabric/Brain View (separate spec)
