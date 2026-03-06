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

**Retrieve (FSRS-scored, state-aware):**
```
retrievability = exp(ln(0.9) * elapsed_days / stability)
relevance = semantic_similarity * 0.3
           + retrievability * 0.2
           + importance * 0.15
           + access_frequency_score * 0.1
           + situational_boost * 0.25
```
Stability increases each time a memory is retrieved and leads to a good outcome.

**State-aware retrieval (`situational_boost`):** Retrieval does not rely only on semantic similarity. The current `UserSituation` biases which memories surface:
- If `deadline_pressure` is High/Critical → boost memories related to the at-risk tasks
- If `energy_level` is Low/Depleted → boost memories about break patterns and recovery strategies
- If `distraction_risk` is high → boost memories about effective focus interventions
- If `task_avoidance_detected` → boost memories about that task type and past avoidance resolution
- Domain-specific: if budget alert active → boost finance-related behavioral memories

This ensures retrieval is contextually relevant, not just semantically similar.

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

### Guardrail 1: Event filtering before LLM extraction

Not every DomainEvent should trigger LLM memory extraction. A lightweight heuristic **salience filter** runs before any LLM call:

```rust
pub enum SalienceVerdict {
    Extract,       // Send to LLM extraction (anomaly, user-stated fact, threshold crossing)
    Accumulate,    // Add to signal buffer for pattern detection, skip LLM
    Discard,       // Routine event, no action needed
}
```

Criteria for `Extract`:
- Contains explicit user-stated fact (`UserStatedFact`, `UserCorrectedAI`)
- Crosses a statistical threshold vs personal baseline (e.g., distraction rate > 1.5x average)
- Is a notable anomaly (first occurrence of a new pattern)
- Represents a milestone (task completed, goal hit, budget exceeded)

Criteria for `Accumulate`:
- Routine activity events (app switches, normal productivity ticks)
- Events that might form a pattern when aggregated but are not significant alone

Criteria for `Discard`:
- Duplicate or near-duplicate of recent event
- Below minimum duration/impact threshold

This prevents the hot path from becoming an LLM bottleneck. The vast majority of events are `Accumulate` or `Discard`.

### Guardrail 2: Pattern validation for reflection

Reflection must not update the user model from isolated observations. Patterns are only promoted to semantic or procedural memory after statistical validation:

- **Minimum signal count:** >= 5 occurrences of the same pattern type
- **Minimum time span:** Pattern observed across >= 3 distinct days
- **Confidence threshold:** Statistical significance >= 0.7 (computed from signal consistency)
- **Contradiction check:** New pattern must not contradict a higher-confidence existing fact without explicit user confirmation

This prevents hallucinated patterns from single anomalous days or coincidental correlations.

### Guardrail 3: Memory compaction strategy

Bi-temporal storage avoids hard deletion, so semantic facts grow indefinitely. A background compaction job runs weekly (after reflection):

1. **Merge redundant chains:** If fact A was superseded by B, which was superseded by C, and A is older than 90 days, merge the chain — keep C as active, archive A and B to a `semantic_facts_archive` table
2. **Archive cold facts:** Facts with `valid_until` set and `retrievability < 0.1` (effectively forgotten) are moved to archive
3. **Compact episodic memories:** Episodic memories older than 90 days with low access count are summarized by LLM into a single consolidated memory, then archived
4. **Size budget:** If active semantic facts exceed a configurable limit (default: 10,000), trigger aggressive compaction on lowest-retrievability facts

Archive tables have identical schema — data is never lost, just moved to cold storage that is not searched during normal retrieval.

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

### UserSituation — Derived World Model Layer

Instead of the coaching engine reasoning directly over raw memories, an intermediate **UserSituation** struct is computed from cognitive memory + real-time signals. This makes reasoning more stable and explainable:

```rust
pub struct UserSituation {
    // Energy & focus
    pub energy_level: EnergyLevel,        // High / Medium / Low / Depleted
    pub focus_state: FocusState,          // DeepWork / LightWork / Distracted / Idle / Break
    pub minutes_since_break: i64,
    pub session_quality_trend: Trend,     // Improving / Stable / Declining

    // Task pressure
    pub deadline_pressure: PressureLevel, // None / Low / Medium / High / Critical
    pub overdue_tasks: Vec<TaskRef>,
    pub approaching_deadlines: Vec<(TaskRef, Duration)>,
    pub current_task_inference: Option<TaskInference>,

    // Behavioral state
    pub distraction_risk: f64,            // 0.0-1.0, based on time-of-day patterns
    pub context_switch_rate: f64,         // switches/hour vs personal baseline
    pub task_avoidance_detected: Option<TaskRef>,

    // Cross-domain
    pub active_goals_at_risk: Vec<GoalRef>,
    pub budget_alerts: Vec<BudgetAlert>,
    pub learning_decay_alerts: Vec<LearningAlert>,

    // Meta
    pub coaching_receptivity: f64,        // 0.0-1.0, learned from feedback history
    pub last_intervention_ago: Duration,
    pub computed_at: DateTime<Utc>,
}
```

The situation is recomputed every 60 seconds (or on significant DomainEvent). The coaching engine reasons over `UserSituation`, not raw memories — this provides a stable, explainable input to the LLM.

### Signal Accumulator

Subscribes to DomainEvent bus. Maintains a rolling SituationBuffer. Fires triggers when heuristic conditions are met (evaluated against `UserSituation`):

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

### Guardrail 4: Rate limiting + adaptive coaching tolerance

**Fixed limits (safety floor):**
- Maximum nudges per hour (configurable, default: 3)
- Cooldown after ignored suggestions (default: 30min for same trigger type)
- Exponential backoff on repeatedly dismissed intervention types
- Daily cap on total interventions (default: 10)
- Never interrupt during active focus sessions with low-priority nudges

**Adaptive tolerance (learned from `UserSituation.coaching_receptivity`):**

The system learns the user's interruption tolerance over time by tracking:
- Dismiss rate by time-of-day (user ignores nudges in the morning → reduce morning nudges)
- Dismiss rate by intervention type (user never acts on break reminders → reduce weight)
- Accept rate by channel (user responds to chat but ignores notifications → prefer chat)
- Overall receptivity trend (new users may be more tolerant; long-term users want fewer, higher-quality nudges)

`coaching_receptivity` (0.0-1.0) is stored as a procedural memory fact and updated during weekly reflection. It modulates the intervention router:
- `receptivity < 0.3` → only dashboard cards regardless of coaching_intensity
- `receptivity 0.3-0.6` → cards + occasional chat, never notifications
- `receptivity > 0.6` → full channel routing per coaching_intensity setting

This means the system naturally quiets down for users who don't engage with nudges, and becomes more active for users who find them helpful.

### Feedback Tracker

Three feedback channels:

1. **Explicit:** User clicks helpful / dismiss / stop-suggesting
2. **Behavioral:** Did user change behavior within 2 minutes of nudge?
3. **Outcome:** Did the strategy improve metrics over days/weeks?

All feedback emitted as `DomainEvent::CoachingFeedback` → cognitive layer stores as episodic memory → weekly reflection uses this to adjust procedural memory.

### Guardrail 5: Intervention effectiveness tracking

Beyond individual feedback, the system tracks **long-term strategy effectiveness**:

```sql
CREATE TABLE coaching_strategies (
    id              TEXT PRIMARY KEY,
    strategy_type   TEXT NOT NULL,     -- 'task_decomposition', 'break_reminder', 'schedule_shift', etc.
    domain          TEXT NOT NULL,     -- which domain this strategy targets
    times_used      INTEGER DEFAULT 0,
    times_accepted  INTEGER DEFAULT 0,
    times_led_to_improvement INTEGER DEFAULT 0,  -- behavioral change within 24h
    avg_improvement_magnitude REAL,    -- e.g., +12% productivity score after applying
    confidence      REAL DEFAULT 0.5,
    last_used       TEXT,
    created_at      TEXT NOT NULL
);
```

During weekly reflection, the LLM reviews strategy effectiveness:
- "Task decomposition was used 8 times, accepted 6 times, led to improvement 5 times → high effectiveness (0.83)"
- "Break reminders were used 12 times, accepted 2 times, led to improvement 1 time → low effectiveness (0.17) → reduce usage"

Effective strategies are prioritized by the coaching reasoner. Ineffective strategies are demoted or retired. This creates a natural selection of coaching approaches tailored to the individual user.

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
9. **Salience filtering before LLM:** Heuristic filter (Extract/Accumulate/Discard) prevents LLM bottleneck on hot path.
10. **Pattern validation:** Minimum 5 signals across 3+ days before promoting to semantic/procedural memory.
11. **Memory compaction:** Weekly archival of superseded chains + cold facts. Size budget prevents unbounded growth.
12. **UserSituation world model:** Derived intermediate layer between raw memories and coaching reasoning — stable, explainable.
13. **State-aware retrieval:** Memory retrieval biased by current situation, not just semantic similarity.
14. **Adaptive coaching tolerance:** Learned `coaching_receptivity` modulates intervention intensity over time.
15. **Strategy effectiveness tracking:** Long-term tracking of which coaching strategies actually improve user behavior.

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
