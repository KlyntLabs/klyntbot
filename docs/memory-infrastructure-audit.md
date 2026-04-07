# Memory Infrastructure Audit

> Generated 2026-04-07. Comprehensive analysis of all memory subsystems, their connections, and integration gaps.

## The Problem

Klynt has **11 distinct memory subsystems** that were built incrementally. Many operate in isolation — data flows one-way or not at all between them. The Brain View graph is empty because it's disconnected from the fact extraction pipeline. Session insights evaporate. Coaching patterns are computed but discarded. Three of the 10 retrieval scoring factors are permanently 0.0.

---

## All Memory Subsystems

| # | Subsystem | Storage | Indexing | Status |
|---|-----------|---------|----------|--------|
| 1 | **Semantic Facts** | SQLite `semantic_facts` | LanceDB vectors + BM25 FTS5 | Working |
| 2 | **Episodic Memories** | SQLite `episodic_memories` | FTS5 only | Working |
| 3 | **Procedural Rules** | SQLite `procedural_rules` | FTS5 only | Working |
| 4 | **Knowledge Atoms** | SQLite `knowledge_atoms` | No indexing | Partial |
| 5 | **Conversation Recalls** | LanceDB `conv_embeddings` | Vector search | Working |
| 6 | **Session Memory** | SQLite `session_memory` | None | Dead-end |
| 7 | **Brain/Knowledge Graph** | SQLite `entity_tree_links` + `note_links` + `communities` | LanceDB `tree_node_embeddings` | Empty |
| 8 | **FSRS Spaced Repetition** | On `semantic_facts.stability` + `flashcards.*` | Embedded in scoring | Partial |
| 9 | **User Model** | Derived from semantic_facts | Computed on-demand | Working |
| 10 | **Coaching Patterns** | Not persisted | Computed and discarded | Dead-end |
| 11 | **Autotuner Experiments** | SQLite trials table | None | Working |

---

## Connection Matrix

```
                    Facts  Episodic  Rules  Atoms  Recalls  Session  Graph  FSRS  UserModel  Coaching  Autotuner
Semantic Facts       ---    <--ref    ---    ---     ---      ---     ---    <->     -->        ---       <--
Episodic Memories   -->ref   ---     -->ref   ---     ---      ---     ---    ---     ---        ---       ---
Procedural Rules     ---     ---      ---     ---     ---      ---     ---    ---     ---        ---       ---
Knowledge Atoms      ---     ---      ---     ---     ---      ---    -->?    ---     ---        ---       ---
Conversation Recall  ---     ---      ---     ---     ---      ---     ---    ---     ---        ---       ---
Session Memory       ---     ---      ---     ---     ---      ---     ---    ---     ---        ---       ---
Brain/Graph          ---     ---      ---    <--?     ---      ---     ---    ---     ---        ---       ---
FSRS                <->      ---      ---     ---     ---      ---     ---    ---     ---        ---       ---
User Model          <--      ---      ---     ---     ---      ---     ---    ---     ---        ---       ---
Coaching             ---     ---      ---     ---     ---      ---     ---    ---     ---        ---       ---
Autotuner           -->      ---      ---     ---     ---      ---     ---    ---     ---        ---       ---

Legend: --> feeds into, <-- fed by, <-> bidirectional, ---  no connection, ? weak/broken
```

**Only 5 real connections exist out of 55 possible pairs.** Most subsystems are islands.

---

## Integration Gaps (Ranked by Impact)

### CRITICAL

| # | Gap | What Happens | Impact |
|---|-----|-------------|--------|
| 1 | **Session memory is a dead-end** | `SessionMemoryService` writes per-session scratchpads, but nothing reads them for long-term storage. Session insights (task progress, errors, learnings) are never promoted to episodic memories or semantic facts. | Every session's accumulated context is lost when it ends. The LLM rediscovers the same things next session. |
| 2 | **Brain View disconnected from facts** | The Brain View graph is built from `note_links` (wiki-links) and `entity_tree_links` (atom entities). Semantic facts — the richest knowledge store — have NO representation in the graph. | The brain visualization shows an empty graph even though the system has hundreds of facts. Users see "0 connections" and think the system doesn't work. |
| 3 | **Atoms and facts are parallel stores** | `knowledge_atoms` (extracted from notes) and `semantic_facts` (extracted from conversations/events) store overlapping knowledge independently. Atoms have an optional `semantic_fact_id` field but it's rarely populated. | Knowledge fragmentation. The same fact "Jayden is a software engineer" could exist as an atom AND a semantic fact, with neither reinforcing the other. |

### HIGH

| # | Gap | What Happens | Impact |
|---|-----|-------------|--------|
| 4 | **Coaching patterns are compute-and-discard** | `feature-coaching` detects behavioral patterns (study_streak, learning_momentum, retention_decay, domain_gap) but only emits event summaries. No storage, no rule creation. | High-signal patterns like "user always studies at 10am" or "user's retention drops on Fridays" evaporate. They could be procedural rules but aren't. |
| 5 | **3 of 10 scoring factors are always 0.0** | `hierarchy_score`, `community_score`, and `cross_note_boost` are passed as 0.0 in every retrieval call. That's 35% of the scoring formula's intended weight contributing nothing. | Retrieval quality is degraded. The autotuner optimizes weights for factors that are always zero. |
| 6 | **Conversation recalls are siloed** | `ConversationRecallService` stores message embeddings in LanceDB with time-decay search. These never feed into episodic memories or semantic facts. Rich conversational context is ephemeral. | After the recall decay window (~138 days), conversation insights are permanently lost even if they contained important facts. |

### MEDIUM

| # | Gap | What Happens | Impact |
|---|-----|-------------|--------|
| 7 | **FSRS weight training is dead code** | `fsrs_parameters` table exists with default weights. `review_log` captures flashcard review history. But no training algorithm reads the log to personalize weights. | FSRS works but with generic weights. Could be 20-40% more accurate with personalized parameters based on user's actual forgetting curve. |
| 8 | **Flashcard ↔ fact no cross-reinforcement** | Reviewing a flashcard updates the flashcard's FSRS state but doesn't touch the corresponding semantic fact's stability. | User explicitly reviews "Newton's 2nd Law" flashcard 50 times, but the semantic fact stays at stability=1.0. The system doesn't learn that the user knows this well. |
| 9 | **Compaction not independent of reflection** | `run_compaction()` only runs inside `run_weekly_reflection()`. If reflection is skipped (< 8 episodes), no compaction happens. | Semantic facts can exceed the 10K budget for days/weeks. Stale episodic memories and rules accumulate between Mondays. |
| 10 | **No retrieval → autotuner feedback** | Autotuner optimizes scoring weights via shadow trials, but has no signal about whether retrieved facts were actually useful to the LLM. | Autotuner optimizes in the dark. No ground truth about retrieval quality. |

---

## FSRS Analysis

| Memory Type | FSRS Fields | Updates | Used in Scoring | Verdict |
|-------------|-------------|---------|-----------------|---------|
| **Flashcards** | stability, difficulty, due_at, lapses, state | On review (full FSRS-5) | Indirectly via embeddings | **FULLY WORKING** |
| **Semantic Facts** | stability only | On retrieval (ln-based growth) | Yes, weight 0.2 in 10-factor | **LIGHTWEIGHT, WORKING** |
| **Knowledge Atoms** | stability, difficulty, retention_pct | Write-only (copied from flashcard) | Never read | **DEAD CODE** |
| **Persona Accuracy** | stability, difficulty | On debate end | Never read | **LOGGING ONLY** |
| **FSRS Weights** | 19-parameter array | Never trained | N/A | **DEAD CODE** |

---

## Brain View Analysis

The Brain View (visible in screenshot as an empty graph with 1 isolated node) requires THREE data sources to show connections:

| Source | Requirement | Status | Why Empty |
|--------|------------|--------|-----------|
| **Wiki-links** | Notes must contain `[[Title]]` syntax | No links in notes | Users don't use wiki-link syntax |
| **Entity extraction** | Atoms must be extracted from notes AND linked to tree nodes | `entity_tree_links` empty | Atom extraction may not have run, or entities not linked |
| **Community detection** | Louvain needs shared-entity edges between tree nodes | 0 communities | No entity_tree_links = no edges = no communities |

**Key insight:** Semantic facts (the most populated knowledge store) are **invisible** to the Brain View. The graph only shows note-based relationships, not conversation-extracted knowledge.

---

## RAG Retrieval Pipeline (What Actually Works)

```
User Query
    |
    v
UnifiedMemoryService.retrieve()     ← RRF merge (k=60)
    |
    +---> Semantic Facts (vector + BM25 + 10-factor scoring)
    |         - Vector: LanceDB cosine similarity (min 0.55)
    |         - BM25: FTS5 boost (RRF-style)
    |         - 10-factor: semantic(0.3) + situation(0.25) + retrievability(0.2) + ...
    |         - 3 factors always 0.0: hierarchy, community, cross_note
    |
    +---> Conversation Recalls (vector + time-decay)
    |         - Vector: LanceDB cosine similarity
    |         - Time-decay: half-life 138 days
    |
    +---> Episodic Memories (BM25 FTS5 only, capped at 5)
    |
    v
Dedup (facts win over recalls) → RRF merge → Normalize [0,1]
    |
    v
Context Injection:
    - CognitiveContextSource (priority 60): user model + procedural rules
    - SessionMemoryContextSource (priority 88): per-session scratchpad
    - UnifiedMemoryService results: formatted memory text
    |
    v
LLM Prompt
```

---

## Memory Lifecycle Summary

| Type | Creation | Reinforcement | Decay | Cleanup | Budget |
|------|----------|--------------|-------|---------|--------|
| Semantic Facts | Event extraction or `record_fact` | Access: stability +ln(1+s), Reflection: consolidation | FSRS retrievability decay (passive) | Weekly: archive superseded >90d, prune <0.05 confidence >180d | 10K active |
| Episodic Memories | Direct insert or reflection output | Access count increment | None (age-based deletion only) | Weekly: delete >90d with access <2 | Unlimited |
| Procedural Rules | Reflection or MetaRule promotion | Signal count increment | None | Weekly: deactivate >90d with signals <2 | Unlimited |
| Knowledge Atoms | Note atom extraction (LLM) | Salience +0.1 on cross-note reinforcement | Daily atom_decay cron | Manual archive or decay-triggered | Unlimited |
| Conversation Recalls | Every stored message | None (immutable) | Time-decay on retrieval (138-day half-life) | Periodic LanceDB maintenance job | Unbounded until pruned |
| Session Memory | Every 3 chat turns | LLM re-summarization | None | **NEVER CLEANED UP** | 1 per session, overwritten |

---

## Unbounded Growth Concerns

| Table/Store | Growth | Cleanup | Risk |
|-------------|--------|---------|------|
| `accumulated_observations` | Grows every extraction batch | **None found** | Unbounded |
| `failed_observations` | Grows on extraction/consolidation errors | **None found** | Unbounded |
| `conv_embeddings` (LanceDB) | Every message stored | Periodic maintenance job | Medium (if job disabled) |
| `session_memory` | One row per session, never deleted | **None found** | Low (small rows) |
| `review_log` | Every flashcard review | **None found** | Low-medium |

---

## What This Means

The memory system is **architecturally rich but operationally fragmented**. Each subsystem works well in isolation, but the bridges between them are mostly missing:

1. **Notes ←×→ Facts**: Two parallel knowledge stores that don't talk
2. **Conversations ←×→ Long-term**: Session insights die with the session
3. **Brain View ←×→ Everything else**: The visualization is disconnected from the primary knowledge pipeline
4. **Learning ←×→ Memory**: Flashcard reviews don't reinforce semantic facts
5. **Patterns ←×→ Rules**: Coaching discovers patterns but can't encode them

The FSRS system is properly implemented for flashcards and partially for semantic fact scoring, but the weight training infrastructure (which would personalize the forgetting curve) is dead code.

The 10-factor relevance scoring formula allocates 35% of weight to three factors (`hierarchy`, `community`, `cross_note`) that are always 0.0, effectively leaving ~65% of the formula's design capacity in use.
