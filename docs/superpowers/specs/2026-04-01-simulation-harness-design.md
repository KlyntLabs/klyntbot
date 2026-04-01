# Klyntbot Simulation Harness

> Deterministic, accelerated simulation of 6-24 months of real-user activity to validate long-term effectiveness of memory retention, RAG optimization, autotuner convergence, personalization, and all cognitive subsystems.

## Goals

1. Run 1 year of simulated user activity in under 10 minutes wall time.
2. Produce quantitative benchmarks across 14 metrics covering memory, retrieval, behavioral, and system convergence.
3. Deterministic and reproducible — same seed + scenario = identical results.
4. Zero production code changes — epoch-driven batch simulation drives subsystems directly.
5. CI-friendly — test binary returns pass/fail based on checkpoint assertions and regression detection.

## Architecture

### Crate Structure

```
crates/simulator/                    # New L4 library crate
├── src/
│   ├── lib.rs                       # Public API: SimulationHarness, run_scenario()
│   ├── harness.rs                   # SimulationHarness — orchestrates the full run
│   ├── epoch.rs                     # SimulatedEpoch — discrete time controller
│   ├── persona/
│   │   ├── mod.rs                   # PersonaRunner, LifecyclePhase state machine
│   │   ├── message_gen.rs           # Message generation from persona + phase + topic pool
│   │   └── types.rs                 # Persona, Phase, TopicPool, AnnotatedMessage
│   ├── providers/
│   │   ├── mod.rs
│   │   ├── scripted.rs              # ScriptedProvider (impl LlmProvider) — scenario JSON
│   │   └── cognitive.rs             # CognitiveLlmBridge — real LLM for extraction/consolidation
│   ├── metrics/
│   │   ├── mod.rs                   # MetricCollector — accumulates all 14 metrics per epoch
│   │   ├── memory.rs                # Knowledge retention, retrieval precision/recall
│   │   ├── behavioral.rs            # Correction rate, task completion, token efficiency
│   │   ├── system.rs                # Autotuner convergence, community stability, brain versions
│   │   └── ground_truth.rs          # GroundTruthVerifier — checkpoint assertions
│   ├── actions.rs                   # SimulatedToolAction executor (repo calls + events)
│   ├── report.rs                    # ReportGenerator — JSON output
│   ├── scenario.rs                  # Scenario loader (TOML files)
│   └── templates.rs                 # Per-topic parameterized message templates
│
tests/simulation/
├── main.rs                          # Test binary entry
├── scenarios/                       # Scenario definition files
│   ├── software_engineer_12mo.toml
│   ├── finance_focused_6mo.toml
│   └── onboarding_stress_test.toml
└── common.rs                        # Re-exports from tests/common/
```

**Dependency position**: L4 (alongside `autotuner`, `tools`). Depends on: `common`, `config`, `storage`, `bus`, `cognitive`, `context_engine`, `scheduling`, `autotuner`, `session`, `skill-system`. Does NOT depend on `agent` or `app-core`.

**Key principle**: The simulator replaces `AgentLoop` as the orchestrator. It calls the same subsystem functions that AgentLoop calls, but controls time, uses scripted LLM responses for agent turns, and instruments everything.

### LLM Strategy: Hybrid

- **ScriptedProvider** (`impl LlmProvider`): Returns pre-defined responses by scenario JSON. Used for all agent-facing LLM calls (intent classification heuristics run without LLM in shadow mode). Zero cost, deterministic.
- **CognitiveLlmBridge**: Real LLM (configurable model, default Claude Haiku) injected only into:
  - `cognitive::extraction::LlmExtractionHandler` — fact extraction from messages
  - `cognitive::consolidation::LlmConsolidationHandler` — memory consolidation decisions
  - `mirror::LlmNarrativeHandler` — weekly narrative generation
  - `autotuner::generator` — trial variant generation
- **Configuration**: `SimulationConfig` fields: `cognitive_llm_model` (default: `"claude-haiku-4-5-20251001"`), `cognitive_temperature` (default: `0.3`), `max_cognitive_calls_per_day` (default: `12`).
- **Fallback**: If a cognitive LLM call fails, fall back to `HeuristicExtractionHandler` / `HeuristicConsolidationHandler` (already exist in the codebase).

## SimulatedEpoch: Discrete Time Controller

### Time Model

```rust
pub struct SimulatedEpoch {
    current: DateTime<Utc>,
    start: DateTime<Utc>,
    end: DateTime<Utc>,
    step: EpochStep,
}

pub enum EpochStep {
    Hours(u32),    // Fine-grained (e.g., 4h steps)
    Day,           // Default: 1 simulated day per tick
    Week,          // Fast-forward for long simulations
}
```

`advance()` returns an `EpochPlan` containing all actions for the tick:

```rust
pub struct EpochPlan {
    pub simulated_now: DateTime<Utc>,
    pub messages_to_send: Vec<AnnotatedMessage>,
    pub cron_triggers: Vec<CronTrigger>,
    pub checkpoints: Vec<Checkpoint>,
}

pub enum CronTrigger {
    CognitiveReflection,     // Monday 9am
    AtomDecay,               // Daily 3am
    AutotunerNightly,        // Daily 2am
    MirrorWeeklyNarrative,   // Sunday 10am
    MirrorCleanup,           // Sunday 4am
    AnalyticsCleanup,        // Daily
    MemoryMaintenance,       // Configurable interval
    CrossDomainInsight,      // Daily 2am
}
```

### Discrete Event Loop

Each tick executes this fixed sequence:

```
for each epoch tick:
  1. epoch.advance() → EpochPlan

  2. PRE-MESSAGE CRON PHASE
     For each CronTrigger due in this window:
       ├─ AtomDecay → call run_decay_cycle() with backdated timestamps
       ├─ AutotunerNightly → call nightly_cycle.run_evaluation_and_promotion()
       ├─ AnalyticsCleanup → call repos.cleanup_analytics() with simulated cutoffs
       └─ MemoryMaintenance → call compaction service

  3. MESSAGE PHASE
     For each AnnotatedMessage in plan:
       a. Insert as SessionMessage with simulated timestamp
       b. If message has tool_actions:
          ├─ CreateTask → TaskRepo::insert() + publish TaskCreated
          ├─ CreateNote → NoteRepo::insert() + publish NoteContentChanged
          ├─ RecordTransaction → TransactionRepo::insert() + publish TransactionRecorded
          ├─ StartFocus → publish FocusSessionStarted
          ├─ CompleteTask → TaskRepo::complete() + publish TaskCompleted
          └─ etc.
       c. Drive cognitive pipeline:
          - event_to_observation(message)
          - evaluate_salience() → Extract/Accumulate/Discard
          - extraction.extract_facts_batch() [REAL LLM if configured]
          - consolidation.decide_batch() [REAL LLM if configured]
          - Record to SemanticFactRepo + EpisodicMemoryRepo
       d. Drive context engine (for retrieval metrics):
          - memory_retriever.retrieve(message.content, 10)
          - Record which facts were retrieved
       e. If message is a correction:
          - Publish DomainEvent::UserCorrectedAI
       f. Collect per-message metrics

  3½. CONTEXT UPDATE DRAIN
     - context_update_queue.drain() — consume all pending updates
       generated by cognitive pipeline + tool actions during this tick.
       Simulates LiveContextRefresher mid-ReAct behavior in production.
       Updates are logged for metric collection but not injected into
       a running agent (since we drive subsystems directly).

  4. POST-MESSAGE CRON PHASE
     For each remaining CronTrigger:
       ├─ CognitiveReflection → call run_weekly_reflection()
       ├─ MirrorWeeklyNarrative → call facade.generate_weekly_narrative()
       └─ CrossDomainInsight → call cross_domain handler

  5. CHECKPOINT PHASE
     For each Checkpoint due at this epoch:
       - Run ground truth assertions via GroundTruthVerifier
       - Record pass/fail + actual values

  6. METRIC SNAPSHOT
     - MetricCollector.snapshot(epoch.current) → append to timeline
```

**Pre/post cron split rationale**: Decay and autotuner run before messages so the day's messages experience updated parameters. Reflection and narratives run after messages so they include the day's activity.

**DB timestamp strategy**: All repo operations use `simulated_now` explicitly. Structs that normally call `Utc::now()` in constructors (e.g., `SemanticFact::new()`) are constructed manually with the simulated timestamp. No production code needs patching.

## Persona System

### Persona Definition (TOML)

```toml
[persona]
name = "software_engineer_vn"
timezone = "Asia/Ho_Chi_Minh"
language = "en"
seed = 42
messages_per_day = { onboarding = 8, routine = 5, power_user = 7, shift = 4 }

[persona.profile]
known_facts = [
    { subject = "user", predicate = "works_as", object = "software engineer" },
    { subject = "user", predicate = "prefers_language", object = "Rust" },
    { subject = "user", predicate = "manages_project", object = "Klynt API rewrite" },
    { subject = "user", predicate = "tracks_currency", object = "VND" },
]

[persona.phases]
[persona.phases.onboarding]
duration_days = 14
correction_rate = 0.25
topic_weights = { tasks = 0.4, notes = 0.3, finance = 0.1, chat = 0.2 }
new_fact_introduction_rate = 0.6
tool_action_rate = 0.5

[persona.phases.routine]
duration_days = 75
correction_rate = 0.10
topic_weights = { tasks = 0.3, notes = 0.2, finance = 0.2, productivity = 0.2, chat = 0.1 }
new_fact_introduction_rate = 0.15
tool_action_rate = 0.7

[persona.phases.power_user]
duration_days = 90
correction_rate = 0.05
topic_weights = { tasks = 0.2, notes = 0.15, finance = 0.15, productivity = 0.15, automation = 0.15, insights = 0.1, chat = 0.1 }
new_fact_introduction_rate = 0.05
tool_action_rate = 0.8

[persona.phases.behavior_shift]
duration_days = 90
correction_rate = 0.12
shift_description = "User switches from backend to ML focus"
new_facts = [
    { subject = "user", predicate = "learning", object = "PyTorch" },
    { subject = "user", predicate = "project_focus", object = "ML pipeline" },
]
topic_weights = { tasks = 0.2, notes = 0.3, finance = 0.1, learning = 0.25, chat = 0.15 }
new_fact_introduction_rate = 0.4
tool_action_rate = 0.6
```

### Lifecycle State Machine

```rust
pub struct PersonaRunner {
    persona: Persona,
    current_phase: LifecyclePhase,
    day_in_phase: u32,
    rng: StdRng,                          // Seeded for determinism
    introduced_facts: HashSet<String>,     // Track what's been told to the system
    topic_history: VecDeque<String>,       // Prevent repetition (window of 20)
}

pub enum LifecyclePhase {
    Onboarding,
    Routine,
    PowerUser,
    BehaviorShift,
}
```

`PersonaRunner::generate_day(simulated_date) -> Vec<AnnotatedMessage>`:
1. Check phase transition (`day_in_phase >= phase.duration_days`).
2. Sample `message_count` from `messages_per_day ± 2` (seeded RNG).
3. For each message:
   - Sample topic from `topic_weights` distribution.
   - If `rng < new_fact_introduction_rate`: pick unintroduced fact, weave into message via template.
   - If `rng < tool_action_rate`: generate matching `SimulatedToolAction`.
   - If `rng < correction_rate`: mark as correction message.
   - Attach `GroundTruthAnnotation` if introducing a known fact.
4. Deduplicate topics via sliding window.

### Message Templates

Parameterized templates per topic — no LLM needed for message generation:

```rust
const TASK_TEMPLATES: &[&str] = &[
    "Create a task: {action} for {project}, due {due_date}",
    "Mark {existing_task} as done",
    "What's left on {project}?",
];

const NOTE_TEMPLATES: &[&str] = &[
    "Create a note about {topic}: {content}",
    "Update my {topic} note with: {new_info}",
];

const CORRECTION_TEMPLATES: &[&str] = &[
    "No, I meant {correct_value}, not {wrong_value}",
    "Actually, I prefer {correct_value}",
];
```

### AnnotatedMessage

```rust
pub struct AnnotatedMessage {
    pub content: String,
    pub phase: LifecyclePhase,
    pub simulated_at: DateTime<Utc>,
    pub ground_truth: Option<GroundTruthAnnotation>,
    pub tool_actions: Vec<SimulatedToolAction>,
    pub is_correction: bool,
    pub topic: String,
}

pub struct GroundTruthAnnotation {
    pub introduces_fact: Option<FactTriple>,
    pub relevant_facts: Vec<String>,       // Fact IDs expected in retrieval
    pub expected_skill: Option<String>,     // Expected skill router selection
}

pub enum SimulatedToolAction {
    CreateTask { title: String, due_offset_days: Option<i32>, project: Option<String> },
    CompleteTask { task_ref: String },
    CreateNote { title: String, content: String },
    UpdateNote { note_ref: String, new_content: String },
    RecordTransaction { amount: f64, category: String, description: String },
    StartFocus { task_ref: Option<String>, duration_mins: u32 },
    CreateObjective { title: String, project: Option<String>, due_offset_days: Option<i32> },
    RecordProductivityEvent { event_type: String, duration_mins: Option<u32> },
}
```

### Determinism

`PersonaRunner` takes `seed: u64` from the scenario. Same seed + scenario = identical message sequence. Critical for CI regression testing.

## Metric Collection & Ground Truth

### 14 Metrics in 3 Tiers

**Tier 1 — Annotation-based (hard correctness)**

| # | Metric | Measurement | Ground truth |
|---|--------|------------|-------------|
| 1 | Knowledge retention | Query `SemanticFactRepo::list_active()` at checkpoint; check persona's `known_facts` exist unsuperseded | `persona.known_facts` |
| 2 | Retrieval precision | Per-message: `|retrieved ∩ relevant| / |retrieved|` | `ground_truth.relevant_facts` |
| 3 | Retrieval recall | Per-message: `|retrieved ∩ relevant| / |relevant|` | Same annotation |
| 4 | Fact extraction accuracy | After cognitive pipeline, check if `introduces_fact` was extracted within 1 epoch | `introduces_fact` tag |
| 5 | Contradiction detection rate | During behavior_shift, check if `ContradictionDetected` fires for conflicting facts | Shift `new_facts` vs existing |
| 6 | Correction rate reduction | Corrections/messages per epoch; must decrease onboarding → power_user | Phase `correction_rate` trajectory |

**Tier 2 — Relative improvement (trend-based, baseline = month 1)**

| # | Metric | Measurement |
|---|--------|------------|
| 7 | Token efficiency | Tokens per message from usage records (lower = better) |
| 8 | Personalization score | `fact_coverage × 0.4 + retrieval_precision × 0.3 + correction_rate_inverse × 0.3` |
| 9 | Task completion rate | Completed / created tasks per epoch |
| 10 | Routing stability | Skill router selection matches persona's intended topic |
| 11 | Insight usefulness | Cross-domain insights referencing facts from ≥2 persona topics |

**Tier 3 — System convergence**

| # | Metric | Target |
|---|--------|--------|
| 12 | Autotuner promotion success | Non-reverted promotions / total ≥ 0.7 by month 6 |
| 13 | Community stability | Avg community `stability` field monotonically increasing after month 2 |
| 14 | Brain version velocity | Versions/month decreasing after month 3 |

### MetricCollector

```rust
pub struct MetricCollector {
    timeline: Vec<MetricSnapshot>,
    checkpoints: Vec<CheckpointResult>,
    baselines: Option<BaselineMetrics>,   // Computed after month 1
}

pub struct MetricSnapshot {
    pub epoch: DateTime<Utc>,
    pub knowledge_retention: f64,
    pub retrieval_precision: f64,
    pub retrieval_recall: f64,
    pub fact_extraction_accuracy: f64,
    pub contradiction_detection_rate: f64,
    pub correction_rate: f64,
    pub token_efficiency: f64,
    pub personalization_score: f64,
    pub task_completion_rate: f64,
    pub routing_stability: f64,
    pub insight_usefulness: f64,
    pub autotuner_promotion_success: f64,
    pub community_stability: f64,
    pub brain_version_velocity: u32,
    pub wall_time_per_epoch_ms: f64,  // Simulator performance debugging
}
```

### GroundTruthVerifier

```rust
pub enum CheckpointAssertion {
    FactExists { subject: String, predicate: String, object: String, min_confidence: f64 },
    FactSuperseded { subject: String, predicate: String, old_object: String },
    MetricAbove { metric: MetricName, threshold: f64 },
    MetricImproved { metric: MetricName, min_improvement_pct: f64 },
}
```

Checkpoints defined in scenario TOML:

```toml
[[checkpoints]]
at_day = 30
assertions = [
    { type = "fact_exists", subject = "user", predicate = "works_as", object = "software engineer", min_confidence = 0.7 },
    { type = "metric_above", metric = "knowledge_retention", threshold = 0.6 },
]

[[checkpoints]]
at_day = 180
assertions = [
    { type = "fact_superseded", subject = "user", predicate = "project_focus", old_object = "Klynt API rewrite" },
    { type = "metric_improved", metric = "personalization_score", min_improvement_pct = 30.0 },
]
```

## Report Generation

```rust
pub struct SimulationReport {
    pub scenario: String,
    pub persona: String,
    pub simulated_days: u32,
    pub wall_time_secs: f64,
    pub seed: u64,
    pub metric_timeline: Vec<MetricSnapshot>,
    pub checkpoints: Vec<CheckpointResult>,
    pub summary: ReportSummary,
}

pub struct ReportSummary {
    pub total_messages: u32,
    pub total_facts_extracted: u32,
    pub total_facts_superseded: u32,
    pub total_brain_versions: u32,
    pub total_autotuner_promotions: u32,
    pub total_autotuner_reverts: u32,
    pub final_metrics: MetricSnapshot,
    pub baseline_metrics: MetricSnapshot,
    pub improvement_pct: HashMap<String, f64>,
    pub checkpoint_pass_rate: f64,
    pub regression_alerts: Vec<RegressionAlert>,
}
```

Output: `target/simulation/{scenario}_{timestamp}.json` by default.

Supports `SIMULATION_OUTPUT_DIR` env var to override output location (e.g., for exporting to an HTML dashboard renderer).

CI assertion: `checkpoint_pass_rate == 1.0 && regression_alerts.is_empty()`.

## Reuse Map

### Existing code reused 100% as-is

| Component | Source crate | Used for |
|-----------|-------------|----------|
| `StoragePool::connect_in_memory()` | `storage` | All simulation state |
| `StoragePool::run_feature_migrations()` | `storage` | Cognitive, tasks, notes, finance tables |
| `SemanticFactRepo` | `cognitive` | Fact CRUD + ground truth queries |
| `EpisodicMemoryRepo` | `cognitive` | Episode storage |
| `MirrorRepo` | `cognitive` | Brain versions, routing snapshots |
| `TrialRepo` | `storage` | Autotuner trial storage |
| `TaskRepo`, `NoteRepo`, `TransactionRepo` | `storage` + feature crates | Domain entity storage |
| `HeuristicExtractionHandler` | `cognitive` | Fallback extraction |
| `run_decay_cycle()` | `cognitive` | FSRS decay |
| `NightlyCycle` | `autotuner` | Autotuner evaluation |
| `ConstraintEvaluator` | `autotuner` | Promotion constraint checks |
| `relevance_score()` | `cognitive` | 10-factor scoring |
| `detect_communities()` | `cognitive` | Community rebuild |
| `DomainEventBus` | `bus` | Event routing |
| `ContextUpdateQueue` | `bus` | Live context queue |
| `deterministic_embedding()` | `tests/common/` | Reproducible vectors |
| `MockEmbeddingHandler` | `tests/common/` | Embedding without fastembed |

### New code required

| Component | Purpose |
|-----------|---------|
| `SimulationHarness` | Orchestrates the epoch loop |
| `SimulatedEpoch` | Discrete time controller |
| `PersonaRunner` | Lifecycle state machine + message generation |
| `ScriptedProvider` | `impl LlmProvider` for scripted agent responses |
| `CognitiveLlmBridge` | Real LLM wrapper for extraction/consolidation |
| `MetricCollector` | 14-metric accumulator with baseline tracking |
| `GroundTruthVerifier` | Checkpoint assertion engine |
| `ReportGenerator` | JSON report output |
| `Scenario` loader | TOML parsing |
| Message templates | Per-topic parameterized strings |
| `SimulatedToolAction` executor | Repo calls + domain event publishing |

## Test Binary Usage

```bash
# Run all simulation scenarios
cargo nextest run -p klyntbot --test simulation

# Run a specific scenario
cargo nextest run -p klyntbot --test simulation -- software_engineer_12mo

# Run with real cognitive LLM (slower, more realistic)
SIMULATION_COGNITIVE_LLM=claude-haiku-4-5-20251001 \
  cargo nextest run -p klyntbot --test simulation

# Fast mode: heuristic-only extraction (no LLM, pure infrastructure test)
SIMULATION_COGNITIVE_LLM=heuristic \
  cargo nextest run -p klyntbot --test simulation
```

## Non-Goals

- Testing LLM response quality (use eval harnesses for that).
- Testing UI/desktop integration (separate concern).
- Testing channel adapters (Telegram, Discord, etc.).
- Real-time simulation (we want acceleration, not wall-clock parity).
- Multi-user simulation (single user per scenario; run multiple scenarios for coverage).
