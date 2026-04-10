# Cognitive Memory System

Klyntbot's cognitive layer gives the agent persistent, evolving memory — not just conversation history, but structured knowledge that decays, consolidates, and strengthens over time like human memory. It implements bi-temporal semantic facts with FSRS5 spaced-repetition decay, a 12-factor relevance scoring system, episodic event memories, procedural rules learned from reflection, and a self-reflection subsystem (mirror) that monitors the agent's own behavior.

This is the system that makes Klyntbot a learning agent rather than a stateless chatbot.

## Table of Contents

- [Architecture Overview](#architecture-overview)
- [Memory Types](#memory-types)
- [FSRS5 Decay Algorithm](#fsrs5-decay-algorithm)
- [12-Factor Relevance Scoring](#12-factor-relevance-scoring)
- [Memory Retrieval](#memory-retrieval)
- [Extraction Pipeline](#extraction-pipeline)
- [Consolidation](#consolidation)
- [Background Services](#background-services)
- [Mirror — Self-Reflection Layer](#mirror--self-reflection-layer)
- [Reforge — Strategy Reflection](#reforge--strategy-reflection)
- [Knowledge Graph & Temporal Versioning](#knowledge-graph--temporal-versioning)
- [Storage Schema](#storage-schema)
- [Key Files](#key-files)

---

## Architecture Overview

```
                    User Message
                         │
                         ▼
              ┌─────────────────────┐
              │   Agent Runtime      │
              │  (execute loop)      │
              └────────┬────────────┘
                       │ retrieval request
                       ▼
              ┌─────────────────────┐
              │ UnifiedMemoryService │◀──── 12-factor scoring
              │  (retrieval)         │◀──── FSRS5 decay
              └────────┬────────────┘◀──── semantic embedding
                       │
         ┌─────────────┼─────────────┐
         ▼             ▼             ▼
   ┌───────────┐ ┌──────────┐ ┌──────────────┐
   │ Semantic   │ │ Episodic │ │ Procedural   │
   │ Facts      │ │ Memories │ │ Rules        │
   │ (bi-temp.) │ │ (events) │ │ (reflection) │
   └───────────┘ └──────────┘ └──────────────┘
         ▲             ▲             ▲
         │             │             │
   ┌───────────────────────────────────────┐
   │         Extraction Pipeline            │
   │    (from conversations & events)       │
   └───────────────────────────────────────┘
         ▲                           ▲
         │                           │
   ┌───────────┐            ┌──────────────┐
   │ DomainEvent│            │ Consolidation │
   │ Bus        │            │ Service       │
   └───────────┘            └──────────────┘
         ▲
         │
   ┌───────────┐  ┌──────────────┐  ┌────────────┐
   │ Mirror     │  │ Reforge      │  │ Background │
   │ (self-     │  │ (strategy    │  │ Services   │
   │ reflection)│  │  reflection) │  │ (cron)     │
   └───────────┘  └──────────────┘  └────────────┘
```

**Location**: `crates/cognitive/src/`

The cognitive system operates on three time scales:
1. **Per-request** — Memory retrieval during context assembly (milliseconds)
2. **Per-conversation** — Fact extraction after each interaction (seconds, fire-and-forget)
3. **Background** — Consolidation, decay updates, mirror analysis, reforge (minutes/hours/weekly)

---

## Memory Types

### Semantic Facts (Bi-Temporal)

The core knowledge unit. Each fact is a subject-predicate-object triple with two temporal dimensions:

```rust
pub struct SemanticFact {
    pub id: String,
    pub domain: String,          // identity, energy, work, finance, learning, preferences, general
    pub subject: String,
    pub predicate: String,
    pub object: String,

    pub confidence: f64,         // 0.0–1.0
    pub source: String,

    // Validity timeline (when the fact was true in the real world)
    pub valid_from: String,      // ISO8601
    pub valid_until: Option<String>,

    // Recording timeline (when the system learned about it)
    pub recorded_at: String,
    pub superseded_at: Option<String>,
    pub superseded_by: Option<String>,

    // FSRS5 decay parameters
    pub stability: f64,          // Days until 90% recall probability
    pub last_accessed: Option<String>,
    pub access_count: i64,
    pub convergence_score: f64,  // Incremented when independent sources confirm

    pub project_id: Option<String>,
    pub memory_type: String,     // Default: "fact"
    pub scope_type: String,      // system, squad, persona
    pub scope_id: Option<String>,
}
```

**Bi-temporal design**: Facts track both *when they were true* (`valid_from`/`valid_until`) and *when the system recorded them* (`recorded_at`/`superseded_at`). This lets the system distinguish between "the user's favorite editor changed" (validity) and "we learned about this change" (recording). Supersession chains (`superseded_by`) link fact versions.

**Domains**: `identity` (who the user is), `energy` (daily energy patterns), `work` (projects, skills), `finance` (spending patterns), `learning` (knowledge gaps), `preferences` (tool/style preferences), `general` (everything else).

### Episodic Memories

Time-bound events — things that happened, not permanent facts:

```rust
pub struct EpisodicMemory {
    pub id: String,
    pub domain: String,
    pub content: String,
    pub summary: Option<String>,
    pub importance: f64,           // 0.0–1.0

    pub occurred_at: String,       // When the event happened
    pub recorded_at: String,       // When the system logged it

    pub stability: f64,            // FSRS5 stability
    pub last_accessed: Option<String>,
    pub access_count: i64,

    pub project_id: Option<String>,
    pub scope_type: String,
    pub scope_id: Option<String>,
}
```

Examples: "User had a productive 3-hour focus session on the API redesign", "User expressed frustration about the deployment pipeline", "User completed their first Monte Carlo simulation".

### Procedural Rules

Meta-cognitive rules learned from patterns in the agent's behavior:

```rust
pub struct ProceduralRule {
    pub id: String,
    pub domain: String,
    pub rule_text: String,
    pub confidence: f64,
    pub source: String,
    pub signal_count: i64,     // How many signals support this rule
    pub created_at: String,
    pub updated_at: String,
    pub active: bool,
    pub project_id: Option<String>,
    pub scope_type: String,
    pub scope_id: Option<String>,
}
```

Examples: "When the user asks about tasks, always check for overdue items first", "Use bullet points for finance summaries — the user corrected paragraph format twice".

---

## FSRS5 Decay Algorithm

**Location**: `crates/cognitive/src/services/decay.rs`

Klyntbot uses an FSRS-inspired (Free Spaced Repetition System) decay model for memory retrievability. Every semantic fact and episodic memory has a **stability** parameter that controls how quickly it fades.

### Retrievability

The probability of successful recall decays exponentially with time:

```
R = exp(ln(0.9) * elapsed_days / stability)
```

| Elapsed Time | R (stability=10d) | R (stability=30d) |
|-------------|-------------------|-------------------|
| 0 days | 1.00 | 1.00 |
| 5 days | 0.95 | 0.98 |
| 10 days | 0.90 | 0.97 |
| 20 days | 0.81 | 0.93 |
| 30 days | 0.73 | 0.90 |
| 60 days | 0.53 | 0.81 |
| 90 days | 0.39 | 0.73 |

**Interpretation**: At `elapsed_days = stability`, recall probability is exactly 90%. A fact with stability=30 means it takes 30 days to drop to 90% recall. Higher stability = slower decay = more durable memory.

### Stability Update

When a memory is successfully retrieved (used in a response), its stability increases:

```rust
pub fn update_stability(current: f64, success: bool, max_stability: f64) -> f64 {
    if success {
        (current + ln(1.0 + current).max(0.1)).min(max_stability)
    } else {
        current  // Unchanged on failed retrieval
    }
}
```

**Properties**:
- **Logarithmic growth**: `ln(1 + stability)` means early retrievals boost stability significantly, but returns diminish — a fact already at stability=20 gains much less than one at stability=2
- **Always positive**: `.max(0.1)` ensures even the most stable facts get some boost
- **Capped**: `max_stability` (default 30 days) prevents any single fact from dominating rankings indefinitely
- **Asymmetric**: Failed retrieval doesn't decrease stability — the fact just continues to decay naturally

### Why FSRS5 Over Simpler Decay

Simple time-based decay (e.g., `score *= 0.95^days`) treats all memories equally. FSRS5 models the observation that:
1. Memories retrieved more often become more durable (stability increases)
2. The benefit of repetition diminishes (logarithmic, not linear)
3. There's a natural ceiling to memory durability (max stability cap)

This means frequently-useful facts naturally float to the top while rarely-accessed facts gracefully fade — without any manual curation.

---

## 12-Factor Relevance Scoring

**Location**: `crates/cognitive/src/services/decay.rs`

When retrieving memories for a query, raw embedding similarity alone is insufficient. Klyntbot uses a weighted combination of 12 factors:

```rust
pub struct RelevanceWeights {
    pub semantic: f64,           // 0.20 — Embedding similarity to query
    pub retrievability: f64,     // 0.10 — FSRS5 decay factor
    pub importance: f64,         // 0.08 — Intrinsic importance score
    pub frequency: f64,          // 0.05 — Access count (usage history)
    pub situation: f64,          // 0.15 — Context match (active project, time of day)
    pub temporal: f64,           // 0.02 — Raw recency
    pub hierarchy: f64,          // 0.10 — Position in note/task tree
    pub path_coherence: f64,     // 0.02 — Path consistency in knowledge graph
    pub community: f64,          // 0.15 — External knowledge source confirmation
    pub cross_note: f64,         // 0.05 — Cross-linking between notes
    pub recall_support: f64,     // 0.08 — Supporting evidence strength
    pub graph_path_boost: f64,   // 0.06 — Knowledge graph traversal bonus
}
```

### Factor Breakdown

```
Relevance Score (0.0 – 1.0)
│
├── Content Signals (35%)
│   ├── semantic:    0.20  ← Embedding cosine similarity
│   ├── community:   0.15  ← External source confirmation
│   └─────────────────────
│
├── Memory Dynamics (18%)
│   ├── retrievability: 0.10  ← FSRS5 decay
│   └── importance:     0.08  ← Extraction-time importance
│
├── Structural Signals (23%)
│   ├── situation:     0.15  ← Active project/time match
│   ├── hierarchy:     0.10  ← Note/task tree depth
│   └─────────────────────
│
├── Graph Signals (13%)
│   ├── graph_path:   0.06  ← Knowledge graph traversal
│   ├── cross_note:   0.05  ← Inter-note links
│   └── path_coherence: 0.02 ← Path consistency
│
└── Usage Signals (11%)
    ├── recall_support: 0.08  ← Supporting evidence
    ├── frequency:      0.05  ← Historical access count
    └── temporal:       0.02  ← Raw recency
```

### Design Rationale

- **Semantic (20%)** is the strongest single factor but intentionally not dominant — pure embedding similarity misses context
- **Situation (15%)** is high because "relevant to what you're doing right now" often matters more than abstract similarity
- **Community (15%)** rewards facts confirmed by external sources (higher confidence)
- **Temporal (2%)** is deliberately low — FSRS5 retrievability already handles recency properly; raw recency would double-count it
- **The weights are configurable** via `RelevanceWeights` — different depth modes or domains could tune these differently

---

## Memory Retrieval

**Location**: `crates/cognitive/src/services/memory_retriever.rs`

The `UnifiedMemoryService` orchestrates retrieval:

```
Query
  │
  ├── Embed query → vector
  │
  ├── Semantic search (LanceDB) → candidate facts
  │
  ├── For each candidate:
  │   ├── Compute FSRS5 retrievability
  │   ├── Compute all 12 relevance factors
  │   └── Weighted combination → final score
  │
  ├── Sort by score, take top N
  │
  └── Return ranked memories for context injection
```

**Retrieval limit**: Configurable (default 5 facts). Respects the token budget allocated by the [Context Engine](context-engine.md).

**Scope filtering**: Memories are scoped (`system`, `squad`, `persona`). Retrieval respects the active scope — persona-scoped memories only appear when that persona is active.

---

## Extraction Pipeline

**Location**: `crates/cognitive/src/services/extraction.rs`

After each conversation turn, the extraction handler processes the exchange to create new memories:

```
Conversation Turn
  │
  ▼
ExtractionHandler
  │
  ├── Entity extraction ─── Named entities, relationships
  │
  ├── Fact extraction ────── Subject-predicate-object triples
  │   ├── Domain classification
  │   ├── Confidence scoring
  │   ├── Temporal bounds (valid_from/valid_until)
  │   └── Supersession detection (does this update an existing fact?)
  │
  ├── Episodic capture ──── Notable events with importance scoring
  │
  └── Pattern detection ─── Signals for procedural rule learning
```

Extraction runs asynchronously (fire-and-forget via `tokio::spawn`) so it never delays the response to the user. The extracted facts are stored with initial stability values and begin decaying immediately.

### Supersession

When a new fact contradicts an existing one (e.g., "user's favorite language is Python" → "user's favorite language is Rust"), the old fact is **superseded** rather than deleted:

1. Old fact gets `superseded_at` timestamp and `superseded_by` pointing to new fact
2. New fact gets `valid_from` set to extraction time
3. The supersession chain is preserved for temporal queries ("what did we know about X at time T?")

---

## Consolidation

**Location**: `crates/cognitive/src/services/consolidation.rs`

The consolidation service periodically merges and deduplicates the fact store:

1. **Duplicate detection** — Facts with similar subject+predicate+object are identified
2. **Confidence merging** — When multiple sources confirm the same fact, `convergence_score` increases
3. **Stale cleanup** — Facts with near-zero retrievability and no recent access are archived
4. **Temporal chain maintenance** — Supersession chains are validated and repaired

Consolidation runs as a background service, not during request processing.

---

## Background Services

| Service | Purpose | Schedule |
|---------|---------|----------|
| `BackgroundConsolidationService` | Fact merging, dedup, cleanup | Periodic (fire-and-forget) |
| `DecayService` | Updates stability after retrievals | Per-retrieval |
| `SessionMemoryService` | Per-session scratchpad maintenance | Per-session |
| `ConversationRecallService` | Dense retrieval of past conversations | On-demand |
| `TemporalService` | Fact version history, supersession chains | On write |

---

## Mirror — Self-Reflection Layer

**Location**: `crates/cognitive/src/mirror/`

The mirror system gives Klyntbot awareness of its own behavior patterns. It watches domain events reactively and generates insights about the agent's performance.

### Architecture

```
DomainEventBus
  │
  ├── RoutingMirrorSubscriber ─── Hourly routing snapshots + drift detection
  ├── MetaRuleDetector ────────── Correction streaks → pending rule proposals
  ├── ConfigArchiver ──────────── Autotuner promotions → brain version timeline
  └── TrialPreviewSubscriber ──── 4h early trial evaluation + kill/continue
  │
  ▼
MirrorEngine::start()
  │
  └── Returns (MirrorFacade, Vec<JoinHandle>, CancellationToken)
```

### Subscribers

| Subscriber | Watches | Produces |
|------------|---------|----------|
| `RoutingMirrorSubscriber` | All routing decisions | Hourly snapshots, drift alerts |
| `MetaRuleDetector` | User corrections | Pending rule proposals (after correction streaks) |
| `ConfigArchiver` | Autotuner promotions | Brain version timeline |
| `TrialPreviewSubscriber` | A/B trial results | Early evaluation signals (kill/continue at 4h) |

### MirrorFacade

Public API for the mirror system:

- **State queries**: `get_state()`, `get_narratives()`, `get_routing_history()`, `get_brain_versions()`, `get_meta_rules()`
- **User actions**: approve/dismiss rules, kill/continue trials, revert brain versions
- **Narrative generation**: weekly self-reflection narratives
- **Conversational responses**: "how have I been doing?" → structured mirror response

Wired with optional `EpisodicMemoryRepo` (cross-feature memory ripple) and `Arc<DomainEventBus>` (auto-note on trial kill).

### MCP Tool

`MirrorTool` (multi-action, read-only) is registered post-init:
- `get_state` — current mirror state
- `get_narratives` — generated coaching narratives
- `get_routing_history` — routing decision timeline
- `get_brain_versions` — config change history
- `get_meta_rules` — learned behavioral rules

### Cron Jobs

| Job | Schedule | Purpose |
|-----|----------|---------|
| `JOB_MIRROR_WEEKLY_NARRATIVE` | Sunday 10am UTC | Generate weekly self-reflection narrative |
| `JOB_MIRROR_CLEANUP` | Sunday 4am UTC | Clean snapshots, snippets, trial previews older than 90 days |

---

## Reforge — Strategy Reflection

**Location**: `crates/cognitive/src/services/reforge/`

Reforge is the nightly self-improvement cycle. It reviews the agent's strategy files (skills), collects behavioral feedback, and generates targeted improvement suggestions.

### Pipeline

```
┌──────────┐    ┌──────────┐    ┌─────────────┐    ┌──────────┐
│ Collect   │───▶│ Review   │───▶│ Synthesize  │───▶│ Apply    │
│ signals   │    │ skills   │    │ suggestions │    │ rewrites │
└──────────┘    └──────────┘    └─────────────┘    └──────────┘
```

### Phases

1. **Collect** — Load behavioral signals from:
   - Tool failure logs
   - User corrections (`UserCorrectedAI` events)
   - Retrieval precision metrics
   - Routing accuracy data

2. **Review** — Analyze each skill file (`skills/*.md`) against collected signals:
   - Which tools are underused?
   - Which instructions led to corrections?
   - Where is retrieval precision low?

3. **Synthesize** — Generate improvement suggestions via LLM:
   - Targeted skill rewrites
   - New procedural rules
   - Tool configuration changes

4. **Apply** — High-confidence suggestions are persisted to `reforge_suggestions` table for cross-session continuity. Applied suggestions update skill files.

Triggered by cron (nightly), orchestrated by `ReforgeService::run_reforge()`.

---

## Knowledge Graph & Temporal Versioning

### Knowledge Atoms

`KnowledgeAtomRepo` stores atomic knowledge units that form a graph:
- Nodes: facts, notes, tasks, concepts
- Edges: relationships (supports, contradicts, elaborates, supersedes)
- Used by the learning system for flashcard generation and by the graph visualization in the desktop UI

### Temporal Queries

The bi-temporal design enables powerful temporal queries:

```
"What did we know about the user's work context on March 15?"
  → Filter: recorded_at <= March 15, NOT superseded_at <= March 15
  → Returns: facts as they were known at that point in time

"When did the user's preferred editor change?"
  → Query supersession chain for subject="preferred_editor"
  → Returns: timeline of valid_from/valid_until transitions
```

### Persona Facts

`PersonaRepo` stores user-specific facts and preferences, scoped by persona:
- Identity facts (name, role, expertise)
- Communication preferences
- Knowledge domain mapping

---

## Storage Schema

**Location**: `crates/cognitive/migrations/`

### Core Tables

| Table | Purpose |
|-------|---------|
| `semantic_facts` | Bi-temporal fact triples with FSRS5 parameters |
| `episodic_memories` | Time-bound event memories |
| `procedural_rules` | Learned behavioral rules |
| `knowledge_atoms` | Atomic knowledge graph nodes |
| `flashcards` | Spaced repetition cards (FSRS5 scheduling) |
| `persona_facts` | Per-persona user knowledge |

### Mirror Tables (003_mirror_tables.sql)

| Table | Purpose |
|-------|---------|
| `routing_snapshots` | Hourly routing decision records |
| `trend_narratives` | Weekly self-reflection narratives |
| `snippets` | Behavioral pattern snippets |
| `meta_rules` | Learned meta-cognitive rules |
| `brain_versions` | Config/strategy version history |
| `trial_previews` | A/B trial early evaluation data |

### Vector Store

Embeddings stored in LanceDB (`{data_dir}/lance/`):
- Semantic fact embeddings (384-dim via fastembed)
- Used for similarity search during retrieval
- Separate from SQLite relational data

---

## Key Files

| File | Purpose |
|------|---------|
| `crates/cognitive/src/types.rs` | SemanticFact, EpisodicMemory, ProceduralRule structs |
| `crates/cognitive/src/services/decay.rs` | FSRS5 retrievability + stability update + 12-factor scoring |
| `crates/cognitive/src/services/memory_retriever.rs` | UnifiedMemoryService — ranked retrieval |
| `crates/cognitive/src/services/extraction.rs` | Fact extraction from conversations |
| `crates/cognitive/src/services/consolidation.rs` | Fact merging, dedup, cleanup |
| `crates/cognitive/src/services/conversation_recall.rs` | Past conversation retrieval |
| `crates/cognitive/src/services/background.rs` | BackgroundConsolidationService |
| `crates/cognitive/src/services/reforge/` | Nightly strategy reflection pipeline |
| `crates/cognitive/src/mirror/engine.rs` | MirrorEngine — subscriber orchestration |
| `crates/cognitive/src/mirror/repo.rs` | Mirror storage (snapshots, narratives, rules) |
| `crates/cognitive/src/mirror/types.rs` | Mirror data types |
| `crates/cognitive/src/mirror/subscribers/` | Event subscribers (routing, meta-rule, config, trial) |
| `crates/cognitive/src/repos/` | SemanticFactRepo, FlashcardRepo, PersonaRepo, etc. |
| `crates/cognitive/src/search/` | Semantic fact search |
| `crates/cognitive/migrations/` | SQL migration files |

---

*Related docs: [Agent Runtime](agent-runtime.md) | [Context Engine](context-engine.md) | [Core Infrastructure](core-infrastructure.md) | [Features](features.md)*
