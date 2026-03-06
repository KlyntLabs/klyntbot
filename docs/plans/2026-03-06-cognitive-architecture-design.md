# Cognitive Architecture & Proactive Intelligence

**Date:** 2026-03-06
**Status:** Approved
**Scope:** Core platform upgrade — unified memory system + proactive coaching

## Problem

The AI currently has no persistent, cross-domain model of the user. Each feature (productivity, tasks, finance) stores data in isolation. The AI can answer questions about each domain but cannot reason across them. Memory infrastructure exists (`user_profile`, `agent_adaptations`, `behavioral_patterns`) but nothing writes to it automatically.

The system is reactive — it analyzes data only when the user asks. Real productivity improvement requires a proactive AI that continuously observes, learns, predicts, and intervenes.

## Goals

1. Build a unified cognitive memory system that learns across all domains
2. Enable cross-domain reasoning (connect productivity patterns with task performance, finance habits, learning progress)
3. Transform the AI from reactive analytics to proactive optimization
4. Create a compounding improvement loop: observe → intervene → measure → reflect → improve
5. Design for extensibility — future domains (notes, learning) plug in without architectural changes

## Non-Goals

- Building the notes or learning features themselves (future work)
- Replacing the existing productivity tracking pipeline (it stays, emits events)
- Building a general-purpose knowledge graph database (we use SQLite + LanceDB)

## Architecture Overview

Three new layers, built bottom-up:

```
Layer B: Proactive Intelligence Engine (reasons + intervenes)
         reads from ↑
Layer A: Cognitive Memory System (unified user model)
         fed by ↑
Layer C: Feature Integration Bus (cross-domain events)
         receives from ↑
[productivity] [tasks] [finance] [notes*] [learning*]  (* = future)
```

### Crate placement

```
L1: bus (upgrade)         — DomainEvent enum, DomainEventEmitter
L2: storage (upgrade)     — semantic_facts, episodic_memories, procedural_rules tables
L3: cognitive (NEW)       — memory lifecycle, user model, FSRS decay, extraction, consolidation, reflection
L3: context_engine (upgrade) — CognitiveContextSource replaces scattered sources
L4: feature-coaching (NEW) — signal accumulator, coaching reasoner, intervention router, feedback tracker
L4: feature-* (upgrade)   — emit DomainEvents, implement DomainObserver
L5: agent (upgrade)        — wire CognitiveContextSource, self-editing memory tools
```

## Layer C: Feature Integration Bus

### DomainEvent enum (in `crates/bus`)

Every feature emits domain events. The cognitive layer subscribes.

```rust
pub enum DomainEvent {
    // Productivity
    ActivitySessionCompleted { summary: SessionSummary, date: String },
    FocusSessionEnded { duration_secs: i64, quality: f64, interruptions: i32 },
    DistractionDetected { app: String, duration_secs: i64, context: String },
    ProductivityScoreComputed { date: String, score: f64, breakdown: ScoreBreakdown },

    // Tasks
    TaskCreated { task_id: String, project: Option<String>, estimate: Option<i64> },
    TaskCompleted { task_id: String, actual_duration: Option<i64>, estimate: Option<i64> },
    TaskDeferred { task_id: String, times_deferred: i32, reason: Option<String> },
    GoalProgress { objective_id: String, progress: f64, target: f64 },

    // Finance
    TransactionRecorded { category: String, amount: f64, is_over_budget: bool },
    BudgetAlert { category: String, spent: f64, limit: f64 },
    SpendingPatternDetected { pattern: String, confidence: f64 },

    // Notes (future)
    NoteCreated { topic: String, linked_to: Vec<String> },
    KnowledgeGapDetected { topic: String, context: String },

    // Learning (future)
    ReviewCompleted { topic: String, retention: f64, difficulty: f64 },
    LearningStreak { days: i32, topic: String },
    RetentionDecaying { topic: String, predicted_retention: f64 },

    // Cross-domain
    UserStatedFact { fact: String, domain: String },
    UserCorrectedAI { original: String, correction: String },

    // Coaching feedback loop
    CoachingFeedback { intervention_id: String, response: FeedbackResponse },
}
```

Features emit via `DomainEventEmitter` (lightweight, no knowledge of cognitive layer). New features only need to define their event variants and emit them.

## Layer A: Cognitive Memory System (`crates/cognitive`)

### Memory types (CoALA + LangMem taxonomy)

| Type | What | Storage | Examples |
|------|------|---------|----------|
| **Working** | Current context window | In-context tokens | Current conversation + retrieved memories |
| **Episodic** | Records of specific events | SQLite + LanceDB | "User had a 92-score day on Feb 28" |
| **Semantic** | Facts about the user | SQLite (structured) + LanceDB (embeddings) | "User's peak hours are 10am-12pm" |
| **Procedural** | Learned behavioral rules | SQLite | "Don't nudge about social media on Fridays" |

### Semantic memory: bi-temporal fact store (Zep pattern)

```sql
CREATE TABLE semantic_facts (
    id              TEXT PRIMARY KEY,
    domain          TEXT NOT NULL,     -- 'productivity', 'finance', 'learning', etc.
    subject         TEXT NOT NULL,     -- entity: 'user', 'project:klyntbot'
    predicate       TEXT NOT NULL,     -- relationship: 'peak_hours', 'avoids'
    object          TEXT NOT NULL,     -- value: '10am-12pm', 'design_tasks'
    confidence      REAL NOT NULL,     -- 0.0-1.0
    source          TEXT NOT NULL,     -- 'observed', 'inferred', 'user_stated', 'reflected'

    -- Bi-temporal (Zep): never hard-delete, always supersede
    valid_from      TEXT NOT NULL,     -- when this became true in reality
    valid_until     TEXT,              -- NULL = still true
    recorded_at     TEXT NOT NULL,     -- when the system learned this
    superseded_at   TEXT,              -- when a newer fact replaced this
    superseded_by   TEXT,              -- ID of the replacing fact

    -- FSRS decay
    stability       REAL DEFAULT 1.0, -- increases with successful retrieval
    last_accessed   TEXT,
    access_count    INTEGER DEFAULT 0
);
```

### Structured user model (semantic memory, queryable)

```
UserModel
  identity    : name, roles, relationships
  energy      : peak_hours, fatigue_curve, break_patterns
  work        : estimation_accuracy, task_avoidance, context_switch_cost
  finance     : spending_patterns, budget_habits
  learning    : strengths, gaps, retention_rates
  preferences : tools, communication_style, scheduling
```

This is the structured core that heuristics and UI can query directly. Populated by extraction + consolidation + reflection.

### Memory lifecycle

```
EXTRACT → CONSOLIDATE → STORE → RETRIEVE → DECAY → REFLECT
```

**Extract:** LLM identifies salient facts from domain events and conversations.

**Consolidate (Mem0 pattern):** Compare each extracted fact against top-k semantically similar existing memories. LLM selects one of four operations:
- **ADD** — new fact, no existing match
- **UPDATE** — augments or refines existing fact
- **DELETE** — contradicts existing fact (mark superseded, don't hard-delete)
- **NOOP** — already known, no change needed

**Store:** Write to SQLite (structured) + LanceDB (embedded for retrieval).

**Retrieve (FSRS-scored):**
```
retrievability = exp(ln(0.9) * elapsed_days / stability)
relevance = semantic_similarity * 0.4
           + retrievability * 0.3
           + importance * 0.2
           + access_frequency_score * 0.1
```
Stability increases each time a memory is retrieved and leads to a good outcome.

**Decay:** Daily background job recalculates retrievability scores. Memories below threshold are compacted (merged or archived), never deleted.

**Reflect (Stanford Generative Agents pattern):** Weekly LLM self-review:
1. Load week's episodic memories + coaching outcomes
2. LLM identifies cross-domain patterns and strategy adjustments
3. Output: updated semantic facts, updated procedural rules, cross-domain insights
4. Run consolidation on each output
5. Store reflection itself as episodic memory

### Processing modes (LangMem pattern)

| Mode | When | What | Latency |
|------|------|------|---------|
| **Hot-path** | During agent invocation | Extract critical facts from user messages | ~200ms |
| **Background consolidation** | After conversation ends | Compare + ADD/UPDATE/DELETE/NOOP | None |
| **Background reflection** | Weekly | Cross-domain synthesis, strategy adjustment | None |
| **Background decay** | Daily | Recalculate retrievability, compact low-relevance | None |

### Implementation guardrail: salience filtering

Not every DomainEvent becomes stored memory. Events must cross an importance threshold:
- Represents a pattern (not a single occurrence)
- Contains explicit user-stated fact
- Crosses a statistical threshold vs baseline
- Is a notable anomaly

This prevents memory store bloat from high-frequency low-value events.

### Implementation guardrail: pattern confidence

Reflection only updates the user model or procedural rules after multiple observations (minimum 5 signals for the same pattern). Prevents premature behavioral inferences from isolated events.

### Public API

```rust
pub trait DomainObserver: Send + Sync {
    fn domain(&self) -> &str;
    fn observe(&self, event: &DomainEvent) -> Vec<Observation>;
}

pub trait CognitiveMemory: Send + Sync {
    async fn extract_and_store(&self, context: &ExtractionContext) -> Result<Vec<MemoryOp>>;
    async fn retrieve(&self, query: &str, domain: Option<&str>, limit: usize) -> Result<Vec<Memory>>;
    async fn user_model(&self) -> Result<UserModel>;
    async fn cross_domain_query(&self, query: &str) -> Result<Vec<Memory>>;
}

pub trait MemoryLifecycle: Send + Sync {
    async fn consolidate(&self) -> Result<ConsolidationReport>;
    async fn reflect(&self) -> Result<ReflectionReport>;
    async fn decay(&self) -> Result<DecayReport>;
}
```

## Layer B: Proactive Intelligence Engine (`crates/feature-coaching`)

### Signal Accumulator

Subscribes to DomainEvent bus. Maintains a rolling SituationBuffer. Fires triggers when heuristic conditions are met:

- distraction_streak >= 3 in 15min
- productive_ratio drops below personal baseline
- task_deadline < 24h && no activity on task
- focus_session_quality declining trend
- context_switches > 2x personal average
- budget_category > 80% of limit
- learning_retention < 70% threshold
- custom triggers from procedural memory

Triggers are heuristic (fast, no LLM). They gate when the coaching reasoner is invoked.

### Coaching Reasoner (LLM-powered)

When a trigger fires:

1. Load trigger context (what happened)
2. Load user model snapshot (from cognitive layer)
3. Retrieve relevant memories (FSRS-ranked)
4. Load procedural rules (learned preferences)
5. Load recent intervention history (avoid repetition)
6. LLM reasons → generates `CoachingDecision`:
   - `intervention: Option<Intervention>`
   - `confidence: f64`
   - `reasoning: String` (stored for reflection)
   - `observations: Vec<Observation>` (feed back to cognitive)

### Pattern Detection Layer (intermediate)

Between raw signals and coaching, a pattern detector aggregates signals into higher-level patterns:

- Raw: 5 distraction events after 3pm → Pattern: "afternoon energy drop"
- Raw: task X deferred 3 times → Pattern: "task avoidance on design work"
- Raw: high score on days with morning exercise → Pattern: "exercise-productivity correlation"

The coaching reasoner operates on patterns, not raw events. This provides better LLM reasoning and reduces noise.

### Intervention Router

Routes interventions to channels based on `confidence x coaching_intensity`:

| Channel | gentle | balanced | strict |
|---------|--------|----------|--------|
| Dashboard card | conf >= 0.3 | conf >= 0.3 | conf >= 0.2 |
| Chat message | conf >= 0.7 | conf >= 0.5 | conf >= 0.4 |
| Notification | -- | conf >= 0.7 | conf >= 0.5 |
| Overlay | -- | -- | conf >= 0.8 |

Config: `coaching_intensity: "gentle" | "balanced" | "strict"`

### Implementation guardrail: rate limiting

- Maximum nudges per hour (configurable, default: 3)
- Cooldown after ignored suggestions (default: 30min for same trigger type)
- Exponential backoff on repeatedly dismissed intervention types
- Daily cap on total interventions (default: 10)
- Never interrupt during active focus sessions with low-priority nudges

### Feedback Tracker

Three feedback channels:

1. **Explicit:** User clicks helpful / dismiss / stop-suggesting
2. **Behavioral:** Did user change behavior within 2 minutes of nudge?
3. **Outcome:** Did the strategy improve metrics over days/weeks?

All feedback emitted as `DomainEvent::CoachingFeedback` → cognitive layer stores as episodic memory → weekly reflection uses this to adjust procedural memory.

### Task-Activity Linking (hybrid infer + confirm)

System infers which task the user is working on from activity context (app, project, file paths, window titles) matched against task metadata. Surfaces lightweight confirmations ("Looks like you're working on 'API redesign' — correct?"). Learns from confirmations/corrections, increasing confidence over time.

### Task Performance Metrics

| Metric | What it enables |
|--------|----------------|
| Time spent vs estimate | Estimation accuracy coaching |
| Completion velocity | Predict when tasks/projects finish |
| Context switch cost | Quantify ramp-up time lost to switching |
| Task avoidance patterns | Detect procrastination on specific task types |
| Energy-task alignment | Match task types to optimal time windows |
| Deadline proximity behavior | Identify last-minute patterns |

## Migration Plan: What Gets Replaced

| Current | Replaced by |
|---------|------------|
| `user_profile` repo (unused) | `cognitive::UserModel` (auto-populated) |
| `agent_adaptations` repo (unused) | `cognitive::ProceduralMemory` |
| `behavioral_patterns` repo | `cognitive::EpisodicMemory` + reflection |
| `LearningService` (basic patterns) | `cognitive::MemoryLifecycle` + `coaching::SignalAccumulator` |
| `LearningContextSource` | `cognitive::CognitiveContextSource` |
| `InsightEngine` (heuristic-only) | `coaching::CoachingReasoner` (heuristics become triggers) |
| `NudgeService` (static rules) | `coaching::InterventionRouter` (adaptive) |

Existing heuristic code (InsightEngine checks, NudgeService rules) is not deleted — it becomes trigger conditions in the Signal Accumulator and Pattern Detector.

## Cross-Domain Reasoning Examples

| Signals | AI reasoning | Intervention |
|---------|-------------|-------------|
| Productivity: 3h YouTube. Tasks: "API redesign" untouched 3 days, due tomorrow. | Procrastinating on difficult task. | "The API redesign is due tomorrow. Want to break it into smaller pieces?" |
| Finance: food delivery 2x this week. Productivity: 12h/day coding. | Crunching, neglecting self-care. | "You've been coding 12h/day and ordering more food. Rest days correlate with your best scores." |
| Learning: calculus retention 40%. Tasks: "Math exam" in 5 days. | Exam approaching with declining retention. | "Calculus retention is at 40% with exam in 5 days. A 30-min review now targets your weakest cards." |

## Key Design Decisions

1. **Hybrid reasoning (C):** Heuristic triggers gate LLM calls. No LLM cost during quiet periods.
2. **Hybrid user model (C):** SQLite for structured queryable data + LanceDB for semantic memories.
3. **Hybrid task linking (C):** Infer from activity + lightweight user confirmation. Learns over time.
4. **Full closed-loop feedback (C):** Explicit + behavioral + outcome tracking + weekly self-reflection.
5. **All four intervention channels:** Dashboard cards, chat, notifications, overlays — gated by confidence x intensity.
6. **Bi-temporal facts (Zep):** Never hard-delete. Mark superseded. Preserve history of how user evolves.
7. **FSRS decay:** Memories that are used survive; unused ones fade. Stability increases with successful retrieval.
8. **New `cognitive` crate at L3:** Core infrastructure, not a feature. All features build on it.

## References

- MemGPT/Letta: OS-inspired memory management (core/recall/archival tiers, self-editing)
- Mem0: ADD/UPDATE/DELETE/NOOP consolidation framework, graph memory variant
- Zep/Graphiti: Bi-temporal knowledge graph, non-lossy fact updates
- LangMem: Semantic/episodic/procedural taxonomy, hot-path vs background processing
- Stanford Generative Agents: Reflection pattern (observations → higher-level insights)
- Reflexion: Verbal reinforcement learning through self-reflection
- CoALA: Cognitive architecture framework for language agents
- ODEI: Memory validation/governance layers
- FSRS: Spaced repetition stability model (Rust crate: `fsrs-rs`)
