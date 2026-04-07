# Memory Unification — Design Spec

> **Goal:** Unify Klynt's 11 fragmented memory subsystems into a coherent cognitive pipeline where knowledge flows between all sources, retrieval uses the full 10-factor scoring formula, and a configurable intelligence depth controls LLM cost.

## Executive Summary

Klynt has 11 memory subsystems but only 5 real connections between them (out of 55 possible pairs). Session insights die with the session. Note atoms and semantic facts are parallel stores. Coaching patterns are computed then discarded. 3 of 10 retrieval scoring factors are permanently 0.0, wasting 35% of the formula's capacity.

This spec designs a **Unified Cognitive Pipeline** that connects all memory types through a 3-stage architecture (Collectors → Consolidator → Writers), adds a **Deep Mode** for full-LLM processing, wires the 3 dead scoring factors with meaningful signals (**knowledge depth**, **co-activation strength**, **multi-source convergence**), and fixes lifecycle gaps (independent compaction, unbounded table cleanup, retrieval→autotuner feedback).

## Architecture

### Pipeline Overview

```
Domain Events (bus)
    │
    ▼
┌─────────────────────────────────────────────┐
│  STAGE 1: COLLECTORS (event-driven, async)  │
│                                             │
│  ChatTurnCollector ──┐                      │
│  SessionCollector ───┤                      │
│  AtomCollector ──────┼──→ UnifiedQueue      │
│  CoachingCollector ──┤    (mpsc channel)    │
│  RecallCollector ────┘                      │
└─────────────────────────────────────────────┘
    │
    ▼ (3-second batch window)
┌─────────────────────────────────────────────┐
│  STAGE 2: CONSOLIDATOR (batched, single)    │
│                                             │
│  Group signals by subject/domain overlap    │
│  Compute multi-source convergence score     │
│  Standard Mode: heuristic thresholds        │
│  Deep Mode: single LLM call per batch       │
│  Output: PromotionOps                       │
└─────────────────────────────────────────────┘
    │
    ▼
┌─────────────────────────────────────────────┐
│  STAGE 3: WRITERS (existing repos)          │
│                                             │
│  SemanticFactRepo + LanceDB embeddings      │
│  ProceduralRuleRepo                         │
│  EpisodicMemoryRepo                         │
│  CoActivationRepo (new)                     │
└─────────────────────────────────────────────┘
```

### Design Principles

- **Event-driven collection, batched consolidation** — Collectors are small, independent subscribers. The consolidator sees the full picture across sources before making decisions.
- **Heuristic-first, LLM for ambiguity** (Standard Mode) — Signal thresholds and overlap scoring handle high-confidence decisions. LLM is called only in Deep Mode or when heuristics aren't confident.
- **Background only** — The pipeline runs entirely in background services. Chat responsiveness is never affected by intelligence mode.
- **No infinite loops** — Collectors subscribe to upstream events only. The consolidator's writes (new facts, rules) do NOT trigger collectors. The pipeline is strictly one-directional.

---

## Deep Mode Configuration

### Config Schema

```rust
// crates/config/src/schema/cognitive.rs
pub struct CognitiveConfig {
    /// Intelligence processing depth for the cognitive pipeline.
    /// "standard" = heuristic-first, LLM only for ambiguity.
    /// "deep" = full LLM consolidation per batch.
    #[serde(default)]
    pub intelligence_mode: IntelligenceMode,
    // ... existing fields unchanged
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum IntelligenceMode {
    #[default]
    Standard,
    Deep,
}
```

### Behavior

- Stored in `config.json` under `cognitive.intelligenceMode`.
- Exposed in Settings UI as a toggle ("Standard" / "Deep").
- Read by the Stage 2 consolidator at each batch cycle — mode switches take effect on the next batch, no restart needed.
- **Affects only background pipeline (Stage 2).** Chat turn processing, context assembly, and retrieval scoring are identical in both modes.

### What Each Mode Changes

| Pipeline Stage | Standard Mode | Deep Mode |
|----------------|--------------|-----------|
| Stage 1: Collectors | Identical | Identical |
| Stage 2: Signal grouping | Identical | Identical |
| Stage 2: Promotion decisions | Heuristic thresholds | Single LLM call per batch |
| Stage 2: Cross-referencing | Keyword overlap only | LLM sees full batch context, can synthesize cross-source insights |
| Stage 3: Writers | Identical | Identical |

---

## Stage 1: Collectors

### Common Signal Type

```rust
// crates/cognitive/src/pipeline/signal.rs

pub struct CognitiveSignal {
    /// Which collector produced this signal.
    pub source: SignalSource,
    /// The knowledge content (natural language).
    pub content: String,
    /// Knowledge domain: "identity", "productivity", "learning", "finance", "general".
    pub domain: String,
    /// Collector's confidence in this signal (0.0-1.0).
    pub confidence: f64,
    /// Additional metadata for cross-referencing in the consolidator.
    pub context: SignalContext,
    /// When this signal was produced.
    pub timestamp: DateTime<Utc>,
}

pub enum SignalSource {
    /// Existing chat-turn extraction (ChatTurnCompleted events).
    ChatTurn,
    /// Session scratchpad promotion on session end.
    SessionEnd,
    /// Cross-note atom reinforcement.
    AtomReinforcement,
    /// Behavioral pattern from coaching.
    CoachingPattern,
    /// High-value conversation recall cluster.
    ConversationRecall,
    /// Explicit user-stated fact via record_fact tool.
    UserStatedFact,
}

pub struct SignalContext {
    /// Session this signal originated from.
    pub session_key: Option<String>,
    /// Known related semantic fact IDs (for convergence detection).
    pub related_fact_ids: Vec<String>,
    /// Known related atom IDs.
    pub related_atom_ids: Vec<String>,
    /// How many independent data sources confirm this signal.
    pub source_count: u32,
    /// Original text snippets for LLM context in Deep Mode.
    pub raw_observations: Vec<String>,
}
```

### Collector Specifications

#### ChatTurnCollector

Replaces the current `event_to_observation()` → extraction path inside `BackgroundConsolidationService`.

- **Subscribes to:** `ChatTurnCompleted { session_key, user_message }`
- **Behavior:** Loads last 6 messages (3 turns) from session history (same as current enriched extraction). Converts to `CognitiveSignal` with `source: ChatTurn`.
- **Confidence:** Inherits from existing extraction pipeline.
- **Change from current:** Instead of directly calling `ExtractionHandler.extract_facts_batch()`, the collector normalizes the observation into `CognitiveSignal` and sends it to the unified queue. Extraction happens in Stage 2.

#### SessionCollector

New service. Bridges the session memory dead-end.

- **Subscribes to:** `SessionEnded { session_key }` (new domain event)
- **Behavior:** Reads the `session_memory` scratchpad for this session. In Standard Mode, extracts key sentences using heuristic keyword detection (looks for "learned", "decided", "error", "fixed", "important", "remember", "preference"). In Deep Mode, the raw scratchpad is sent as `raw_observations` for LLM extraction in Stage 2.
- **Confidence:** Standard: 0.5-0.8 based on keyword match strength. Deep: delegated to consolidator LLM.
- **New event required:** `SessionEnded` — published by `AppCore` when a session is closed (window close, timeout, explicit end). If a session has no scratchpad (< 3 turns, no `session_memory` row), no signal is emitted.

#### AtomCollector

New service. Bridges the atom→fact gap.

- **Subscribes to:** `AtomReinforced { atom_id, subject, domain, reinforcement_count, salience }` (new domain event)
- **Behavior:** When an atom is reinforced by cross-note detection (salience boosted), and its `reinforcement_count >= 2`, emit a signal. The signal's content is the atom's subject + source context. Also populates `related_atom_ids` for convergence tracking.
- **Confidence:** `min(0.5 + reinforcement_count * 0.15, 0.95)` — 2 reinforcements = 0.80, 3 = 0.95.
- **New event required:** `AtomReinforced` — published by `AtomExtractionService` when `REINFORCEMENT_BOOST` is applied to an existing atom.

#### CoachingCollector

New service. Bridges coaching patterns to procedural rules.

- **Subscribes to:** `CoachingPatternDetected` (existing domain event from `feature-coaching`)
- **Behavior:** Converts the detected pattern into a rule-candidate signal. The pattern's `description` becomes the signal's `content`. The pattern's `domain` maps directly. Pattern types that are prescriptive (e.g., "afternoon_energy_drop" → "Take breaks in the afternoon when energy drops") are formatted as behavioral guidelines.
- **Confidence:** Inherits the coaching pattern's confidence (0.3-0.95 depending on signal count).

#### RecallCollector

New service. Bridges conversation recalls to episodic memories.

- **Subscribes to:** Periodic timer (every 6 hours)
- **Behavior:** Queries LanceDB `conv_embeddings` for messages from the past 24 hours. Groups by semantic similarity (cosine > 0.85) into clusters. Clusters with 3+ messages from 2+ sessions are promoted as episodic memory candidates. The cluster summary (highest-similarity message) becomes the signal's `content`. Dedup: tracks last-processed timestamp to avoid re-promoting the same cluster across 6-hour cycles.
- **Confidence:** `min(0.4 + cluster_size * 0.1, 0.9)` — 3 messages = 0.7, 5 = 0.9.

---

## Stage 2: Consolidator

### Batch Processing

The consolidator extends the existing `BackgroundConsolidationService` with a unified queue input. The current 3-second batch window is reused.

```rust
// crates/cognitive/src/pipeline/consolidator.rs

pub struct UnifiedConsolidator {
    signal_rx: mpsc::Receiver<CognitiveSignal>,
    config: Arc<Config>,
    repos: CognitiveRepos,
    extraction_handler: Arc<dyn ExtractionHandler>,
    consolidation_handler: Arc<dyn ConsolidationHandler>,
    embedder: Option<Arc<dyn SemanticFactEmbedder>>,
}
```

### Processing Steps

**Step 1: Group signals by subject/domain.**

Signals are grouped using `word_overlap_ratio()` (existing Jaccard + stemming function from `ProceduralRuleRepo`, threshold > 0.6). Signals about "Jayden learning Rust" and "user studies Rust programming" group together.

```rust
pub struct KnowledgeCluster {
    /// All signals that were grouped together.
    pub signals: Vec<CognitiveSignal>,
    /// Best subject text (from highest-confidence signal).
    pub merged_subject: String,
    /// Domain (most common among signals, or "general" if mixed).
    pub domain: String,
    /// How many distinct SignalSource types contributed.
    pub source_diversity: u32,
    /// Convergence score: source_diversity / 5.0 (5 = total source types).
    pub convergence_score: f64,
    /// Highest confidence among grouped signals.
    pub max_confidence: f64,
    /// All raw observations merged (for Deep Mode LLM context).
    pub combined_observations: Vec<String>,
}
```

**Step 2: Compute convergence score.**

```
convergence_score = distinct_source_count / 5.0
```

- 1 source (chat only) → 0.2
- 2 sources (chat + notes) → 0.4
- 3 sources (chat + notes + coaching) → 0.6
- 4 sources → 0.8
- 5 sources → 1.0

This score is stored on any created/updated semantic fact as `convergence_score` and used directly as the `cross_note_boost` factor in retrieval scoring.

**Step 3: Decide promotions.**

#### Standard Mode (heuristic)

| Target | Condition |
|--------|-----------|
| **Semantic Fact** | `max_confidence >= 0.6` OR `convergence_score >= 0.4` |
| **Procedural Rule** | `max_confidence >= 0.7` AND any signal has `source: CoachingPattern` |
| **Episodic Memory** | `max_confidence >= 0.5` AND no fact/rule match (catch-all) |
| **Co-activation update** | Always — every cluster updates co-activation for member facts |

For semantic facts, the consolidator calls the existing `ExtractionHandler.extract_facts_batch()` with the cluster's combined observations. This produces SPO triples. Then the existing `ConsolidationHandler.decide_batch()` handles dedup (Add/Update/Delete/Noop).

For procedural rules, the consolidator uses the existing `ProceduralRuleRepo.find_similar()` for dedup. If a similar rule exists, reinforce it (`increment_signal_count`). Otherwise, create a new rule.

#### Deep Mode (LLM)

Single LLM call per batch. All clusters are formatted into a structured prompt:

```
You are analyzing knowledge signals from multiple sources about a user.
Each cluster represents signals about the same topic from different origins.

For each cluster, decide:
1. FACT: Extract as a semantic fact (subject-predicate-object triple)
2. RULE: Extract as a procedural rule (behavioral guideline)
3. EPISODE: Store as an episodic memory (event record)
4. SKIP: Not significant enough to store

Clusters:
[Cluster 1: convergence=0.6, sources=[ChatTurn, AtomReinforcement, CoachingPattern]]
  - "User mentioned learning Rust in conversation"
  - "Atom 'Rust programming' reinforced across 3 notes"
  - "Pattern: learning_momentum_review_strong detected"

[Cluster 2: convergence=0.2, sources=[SessionEnd]]
  - "Session scratchpad: fixed a bug in the auth middleware, decided to refactor later"

Respond as JSON array of decisions.
```

The LLM can cross-reference clusters — seeing that Rust appears in both conversation and notes, combined with a strong review pattern, produces a higher-confidence fact than any single source would.

**Step 4: Output PromotionOps.**

```rust
pub enum PromotionOp {
    CreateFact { fact: SemanticFact, convergence: f64 },
    UpdateFact { id: String, new_confidence: f64 },
    SupersedeFact { old_id: String, new_fact: SemanticFact },
    CreateRule { rule: ProceduralRule },
    ReinforceRule { id: String },
    CreateEpisode { memory: EpisodicMemory },
    UpdateCoActivation { fact_ids: Vec<String> },
}
```

---

## The 3 Scoring Factors

### Factor 1: `hierarchy_score` → Knowledge Depth (weight 0.10)

Measures how well-supported a fact is by related facts in the same domain.

**Computation:** At retrieval time, count active facts that share the same `subject` or have keyword overlap with this fact in the same `domain`.

```rust
fn knowledge_depth(fact: &SemanticFact, repo: &SemanticFactRepo) -> f64 {
    let related_count = repo.count_related(&fact.subject, &fact.domain);
    // Logarithmic normalization: 0=0.0, 1=0.42, 3=0.77, 5+=~1.0
    (related_count as f64).ln_1p() / 3.0_f64.ln_1p()
}
```

**New SQL query (`count_related`):**
```sql
SELECT COUNT(*) FROM semantic_facts
WHERE domain = ?1
  AND subject = ?2
  AND id != ?3
  AND superseded_at IS NULL
```

**Cost:** One SQL query per unique subject in retrieval batch. Cached per-fact with 60-second TTL.

### Factor 2: `community_score` → Co-Activation Strength (weight 0.15)

Measures how often this fact gets retrieved alongside other facts — implicit "neural pathways."

**New table:**
```sql
CREATE TABLE co_activation (
    fact_id_a TEXT NOT NULL,
    fact_id_b TEXT NOT NULL,
    strength  REAL NOT NULL DEFAULT 1.0,
    last_fired TEXT NOT NULL,
    PRIMARY KEY (fact_id_a, fact_id_b)
);
CREATE INDEX idx_co_activation_a ON co_activation(fact_id_a);
CREATE INDEX idx_co_activation_b ON co_activation(fact_id_b);
```

**Update mechanism:** After every retrieval that returns 2+ facts, emit `UpdateCoActivation { fact_ids }`. The writer increments `strength` for every pair:
```sql
INSERT INTO co_activation (fact_id_a, fact_id_b, strength, last_fired)
VALUES (?1, ?2, 1.0, ?3)
ON CONFLICT (fact_id_a, fact_id_b) DO UPDATE SET
    strength = strength + 1.0,
    last_fired = ?3
```

**Decay:** During weekly compaction, all strengths multiplied by 0.95. Pairs with strength < 0.1 are deleted.

**Scoring:** Two-pass retrieval:
1. First pass: retrieve top-30 candidates (existing logic, `community_score = 0.0`).
2. Query co-activation strengths among the top-30.
3. Compute per-fact co-activation score:
   ```rust
   fn co_activation_score(fact_id: &str, peer_ids: &[String], repo: &CoActivationRepo) -> f64 {
       let total = repo.sum_strength_with_peers(fact_id, peer_ids);
       // Sigmoid: 0 co-activations=0.18, 3=0.50, 6=0.82, 10+=~1.0
       1.0 / (1.0 + (-0.5 * (total - 3.0)).exp())
   }
   ```
4. Re-rank top-30 with updated scores. Return top-N.

**Cost:** One SQL query per retrieval (~1ms). No LLM cost.

### Factor 3: `cross_note_boost` → Multi-Source Convergence (weight 0.10)

Measures how many independent sources confirmed this fact.

**New column:**
```sql
ALTER TABLE semantic_facts ADD COLUMN convergence_score REAL NOT NULL DEFAULT 0.0;
```

**Set by consolidator** at fact creation/update time based on the `KnowledgeCluster.convergence_score`. No computation at retrieval time — just read `fact.convergence_score`.

**Scoring:** Direct pass-through to `relevance_score()`.

### Updated Retrieval Call

```rust
// crates/cognitive/src/services/retrieval.rs — vector path
let depth = knowledge_depth_cache.get_or_compute(&fact, &repo);
let co_act = co_activation_score(&fact.id, &peer_ids, &co_activation_repo);

let score = relevance_score(
    similarity,
    r,
    fact.confidence,
    freq,
    situational_boost,
    temporal,
    depth,                    // was 0.0
    0.5,                      // path_coherence (unchanged for now)
    co_act,                   // was 0.0
    fact.convergence_score,   // was 0.0
    weights,
);
```

---

## Lifecycle & Compaction

### Independent Compaction Cron

**New cron job:** `JOB_COGNITIVE_COMPACTION` — daily at 3am UTC, separate from weekly reflection.

**Extracted function:** `run_compaction()` becomes a standalone public function (currently embedded in `run_weekly_reflection()`). Both the daily cron and the weekly reflection call it.

**Compaction targets:**

| Target | Action | Threshold |
|--------|--------|-----------|
| Semantic facts exceeding budget | Archive lowest-stability | If count > 10,000: archive facts with `stability < 0.1`, then lowest overall |
| Superseded facts | Move to `semantic_facts_archive` | `superseded_at` older than 90 days |
| Episodic memories | Delete low-access old entries | `access_count < 2` AND `occurred_at` older than 90 days |
| Procedural rules | Deactivate stale rules | `signal_count < 2` AND `updated_at` older than 90 days |
| Co-activation pairs | Decay strengths | Weekly (Sundays only): multiply all by 0.95, delete where `strength < 0.1` |
| `accumulated_observations` | Delete processed | `created_at` older than 7 days |
| `failed_observations` | Delete old failures | `created_at` older than 30 days |
| `session_memory` | Delete old sessions | `updated_at` older than 90 days |
| Conversation recalls | Prune old embeddings | Delegate to existing LanceDB maintenance job |

### Retrieval → Autotuner Feedback

**New table:**
```sql
CREATE TABLE retrieval_feedback (
    id TEXT PRIMARY KEY,
    retrieved_fact_ids TEXT NOT NULL,  -- JSON array
    referenced_fact_ids TEXT NOT NULL, -- JSON array (facts the LLM actually used)
    precision REAL NOT NULL,          -- referenced / retrieved
    session_key TEXT NOT NULL,
    created_at TEXT NOT NULL
);
```

**Detection mechanism:** After the LLM produces a response, heuristically scan for subject/predicate keywords from retrieved facts. If a retrieved fact's `subject` appears in the response text, mark it as "referenced." Compute `precision = referenced_count / retrieved_count`.

**Autotuner integration:** The autotuner reads `retrieval_feedback` during nightly evaluation. Weight configurations that produce higher average precision over the past 7 days are scored higher in trial evaluation.

### Flashcard → Fact Cross-Reinforcement

When a flashcard is reviewed via `flashcard_record_review()`, if the flashcard's knowledge atom has a `semantic_fact_id`, update the corresponding semantic fact:
- Increment `access_count`
- Update `stability` using `update_stability()`
- Update `last_accessed`

This bridges the flashcard FSRS system to semantic fact stability. A user who reviews "Newton's 2nd Law" 50 times also strengthens the corresponding semantic fact.

---

## New Domain Events

| Event | Published By | Consumed By |
|-------|-------------|-------------|
| `SessionEnded { session_key }` | `AppCore` on session close/timeout | `SessionCollector` |
| `AtomReinforced { atom_id, subject, domain, reinforcement_count, salience }` | `AtomExtractionService` on salience boost | `AtomCollector` |

Existing events consumed by new collectors:
- `ChatTurnCompleted` → `ChatTurnCollector`
- `CoachingPatternDetected` → `CoachingCollector`

---

## New Tables

| Table | Purpose |
|-------|---------|
| `co_activation` | Track co-retrieval strength between fact pairs |
| `retrieval_feedback` | Log retrieval precision for autotuner |

## Schema Changes

| Table | Change |
|-------|--------|
| `semantic_facts` | Add `convergence_score REAL NOT NULL DEFAULT 0.0` |

---

## Sub-Project Breakdown

### Sub-Project 1: Memory Bridge Layer

**Goal:** Connect all isolated subsystems through the unified cognitive pipeline.

**Components:**
1. `IntelligenceMode` config schema + Settings UI toggle
2. `CognitiveSignal` type and `SignalSource` enum
3. Unified queue (mpsc channel) in consolidated background service
4. `SessionCollector` + `SessionEnded` domain event
5. `AtomCollector` + `AtomReinforced` domain event
6. `CoachingCollector` (subscribes to existing `CoachingPatternDetected`)
7. `RecallCollector` (periodic LanceDB scan)
8. `ChatTurnCollector` (refactor existing extraction into collector shape)
9. Stage 2 consolidator with grouping, convergence scoring, heuristic + LLM decision paths
10. `convergence_score` column on `semantic_facts`
11. Stage 3 writers executing `PromotionOps`

**Testing:** Each collector testable in isolation with mock queue. Consolidator testable with synthetic signals. Integration test: emit events → verify facts/rules/episodes created.

### Sub-Project 2: Intelligent Scoring

**Goal:** Wire the 3 dead scoring factors and add retrieval feedback.

**Components:**
1. `count_related()` SQL query for knowledge depth
2. `co_activation` table, `CoActivationRepo`, indexes
3. Co-activation writer (processes `UpdateCoActivation` ops)
4. Two-pass retrieval (first pass → co-activation query → re-rank)
5. Wire all 3 factors into `retrieval.rs` (replace hardcoded 0.0s)
6. `retrieval_feedback` table and heuristic detection
7. Autotuner feedback reader integration

**Testing:** Unit tests for each scoring function. Integration test: create facts with known relationships → verify scoring order changes. Autotuner test: verify feedback table is read during evaluation.

### Sub-Project 3: Lifecycle & Polish

**Goal:** Independent compaction and cross-system reinforcement.

**Components:**
1. Extract `run_compaction()` into standalone function
2. `JOB_COGNITIVE_COMPACTION` daily cron (3am UTC)
3. Cleanup for `accumulated_observations`, `failed_observations`, `session_memory`
4. Co-activation decay (weekly 0.95x)
5. Flashcard → semantic fact cross-reinforcement

**Testing:** Compaction integration test: create stale data → run compaction → verify cleanup. Cross-reinforcement test: review flashcard → verify fact stability increased.

---

## What This Does NOT Include (YAGNI)

- **Brain View graph changes** — Retrieval quality is the priority. Graph visualization is a separate future project.
- **FSRS weight training** — The review_log data exists but personalizing weights is a separate optimization. Default weights work adequately.
- **Prompt suggestion / speculation** — Claude Code features not applicable to Klynt's architecture.
- **Magic notes / auto-updating notes** — Separate feature, not memory infrastructure.
- **Away/resume summary** — Nice-to-have, not a bridge gap.
- **path_coherence factor** — Stays at 0.5 (neutral). Requires a knowledge graph traversal algorithm that's out of scope.

---

## Success Criteria

After implementation:

1. **Session insights persist** — Send messages across 2 sessions. Facts extracted from session 1's scratchpad appear in session 2's retrieval.
2. **Atoms promote to facts** — Create 3 notes mentioning "Rust programming." The atom is reinforced → promoted to a semantic fact with convergence_score > 0.
3. **Coaching patterns become rules** — Trigger `afternoon_energy_drop` pattern. Verify a procedural rule appears in `CognitiveContextSource` output.
4. **Scoring factors are non-zero** — Retrieve facts and verify `hierarchy_score`, `community_score`, and `cross_note_boost` are computed (not 0.0).
5. **Co-activation forms** — Retrieve facts about "Rust" and "programming" repeatedly. Verify co-activation strength increases and affects re-ranking.
6. **Convergence rewards multi-source facts** — A fact confirmed by chat + notes + coaching retrieves higher than a single-source fact.
7. **Compaction runs independently** — Disable weekly reflection. Verify daily compaction still cleans stale data.
8. **Deep Mode uses LLM** — Toggle to Deep Mode. Verify consolidator makes LLM calls. Toggle back to Standard. Verify heuristic-only.
9. **Retrieval feedback logged** — After LLM response, verify `retrieval_feedback` table has entries with precision > 0.
10. **Flashcard reviews strengthen facts** — Review a flashcard linked to a semantic fact. Verify fact's stability increases.
