# Agentic Capabilities Design

**Date:** 2026-02-16
**Status:** Approved
**Implementation Effort:** ~3,800 LOC
**Timeline:** Phase 1 (Goal Engine + Planning Engine), then Phase 2 (Learning System)

---

## Executive Summary

This design adds three core agentic capabilities to klyntbot:

1. **Autonomous Goal Engine** — Agent suggests and manages strategic goals that span multiple projects
2. **Planning Engine (ReAct)** — Multi-step plan generation and execution with confidence-gated approval
3. **Learning System** — Adapts behavior based on outcomes, improves confidence calibration and tool selection

**Key Decisions:**
- **Implementation approach:** Hybrid (Goal + Planning together, Learning separately)
- **Goal model:** Meta-containers above projects (Strategic → Tactical → Operational)
- **Goal creation:** Suggestion-based (agent proposes, user approves)
- **Plan execution:** Confidence-gated (auto-execute if confident, request approval if uncertain)
- **Learning scope:** Balanced (patterns + inferred preferences, privacy-conscious summaries)

**Architecture approach:** Hybrid modularization
- **Goal** as separate crate (Layer 2) — reusable domain model
- **Planning** and **Learning** as modules in agent crate (Layer 5) — tightly coupled to orchestration

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Component Design](#2-component-design)
3. [Data Flow & Interactions](#3-data-flow--interactions)
4. [Error Handling & Edge Cases](#4-error-handling--edge-cases)
5. [Testing Strategy](#5-testing-strategy)
6. [Implementation Roadmap](#6-implementation-roadmap)

---

## 1. Architecture Overview

### 1.1 Crate & Module Structure

```
klyntbot/
├── crates/
│   ├── goal/                    ← NEW CRATE (Layer 2)
│   │   ├── src/
│   │   │   ├── lib.rs          # Re-exports
│   │   │   ├── types.rs        # Goal, GoalStatus, Metric, GoalProgress
│   │   │   ├── store.rs        # GoalStore (JSONL persistence)
│   │   │   └── suggestion.rs   # GoalSuggestionEngine
│   │   └── Cargo.toml
│   │
│   ├── tools/                   ← MODIFIED
│   │   ├── src/
│   │   │   ├── goal_tool.rs    # NEW - GoalTool + GoalHandler trait
│   │   │   └── ...
│   │
│   └── agent/                   ← MODIFIED
│       ├── src/
│       │   ├── planner.rs          # NEW - Planning engine (ReAct)
│       │   ├── learning.rs         # NEW - Learning engine
│       │   ├── goal_handler.rs     # NEW - Implements GoalHandler trait
│       │   ├── agent_loop.rs       # MODIFIED - Integrate planning
│       │   ├── context.rs          # MODIFIED - Inject active goals
│       │   └── ...
│
├── cli/                         ← MODIFIED
│   ├── src/
│   │   ├── goal_commands.rs    # NEW - CLI handlers
│   │   └── ...
```

### 1.2 Dependency Graph (Updated)

```
Layer 0: common
Layer 1: config, bus
Layer 2: providers, session, scheduling, calendar, goal (NEW)
Layer 3: tools (now depends on goal)
Layer 5: agent (now includes planner, learning, goal_handler)
Layer 6: cli
Layer 7: klyntbot (facade)
```

**Rationale:** `goal` at Layer 2 means tools can depend on it without circular dependencies, just like they depend on `config` and `session`.

### 1.3 Integration with Existing Systems

| Existing System | Integration Point | How |
|----------------|-------------------|-----|
| **TodoTool** | Goal assignment | Tasks link to goals via `goal_id` field |
| **ProjectTool** | Goal → Project | Projects become children of goals |
| **ContextBuilder** | System prompt | Inject active goals (like current todo context) |
| **ConfidenceEvaluator** | Plan approval | Reuse existing confidence dimensions |
| **MemoryStore** | Plan history | Store executed plans as memory entries |
| **SubagentManager** | Plan execution | Spawn subagents for complex plan steps |

---

## 2. Component Design

### 2.1 Goal Domain Model (`goal/src/types.rs`)

```rust
pub struct Goal {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub status: GoalStatus,
    pub priority: u8,                    // 1-5 (matches todo priority)
    pub target_date: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub metrics: Vec<Metric>,            // Progress indicators
    pub linked_project_ids: Vec<Uuid>,   // Projects contributing to this goal
    pub metadata: HashMap<String, String>, // Extensible
}

pub enum GoalStatus {
    Active,      // Currently pursuing
    Paused,      // Temporarily suspended
    Achieved,    // Completed successfully
    Abandoned,   // No longer pursuing
}

pub struct Metric {
    pub name: String,              // "Tasks completed", "Revenue generated"
    pub current: f64,              // Current value
    pub target: f64,               // Target value
    pub unit: String,              // "tasks", "$", "%"
}

pub struct GoalProgress {
    pub goal_id: Uuid,
    pub completion_percentage: f64,  // 0.0-100.0
    pub metrics: Vec<Metric>,
    pub summary: String,             // "3 of 5 projects completed"
}
```

### 2.2 GoalStore (`goal/src/store.rs`)

**Storage:** `~/.klyntbot/data/goals.jsonl` (append-only, compaction at >10MB)

```rust
pub struct GoalStore {
    goals: HashMap<Uuid, Goal>,
    file_path: PathBuf,
}

impl GoalStore {
    pub fn new(file_path: PathBuf) -> Result<Self>;
    pub fn load() -> Result<Self>;                    // Load from JSONL
    pub fn save(&self) -> Result<()>;                 // Append-only journal
    pub fn create(&mut self, goal: Goal) -> Result<Uuid>;
    pub fn get(&self, id: &Uuid) -> Option<&Goal>;
    pub fn update(&mut self, goal: Goal) -> Result<()>;
    pub fn list(&self, status: Option<GoalStatus>) -> Vec<&Goal>;
    pub fn delete(&mut self, id: &Uuid) -> Result<()>; // Mark as Abandoned
    pub fn calculate_progress(&self, id: &Uuid) -> Result<GoalProgress>;
}
```

### 2.3 GoalSuggestionEngine (`goal/src/suggestion.rs`)

```rust
pub struct GoalSuggestionEngine {
    pattern_threshold: usize,  // Suggest goal after N related tasks (default: 5)
}

pub struct GoalSuggestion {
    pub proposed_title: String,
    pub rationale: String,
    pub linked_items: Vec<Uuid>,  // Tasks/projects that triggered suggestion
    pub confidence: f64,          // 0.0-1.0
}

impl GoalSuggestionEngine {
    pub fn analyze_patterns(
        &self,
        todos: &[Todo],
        projects: &[Project],
    ) -> Vec<GoalSuggestion>;

    pub fn detect_goal_intent(&self, message: &str) -> Option<GoalSuggestion>;
}
```

**Pattern detection:**
- Tags: 5+ tasks with same tag → suggest goal
- Keywords: "launch", "build", "improve", "learn" → goal indicators
- Projects: Multiple related projects → umbrella goal
- Time span: Tasks created over >2 weeks with theme → goal candidate

### 2.4 Planning Engine (`agent/src/planner.rs`)

```rust
pub struct Planner {
    max_steps: usize,             // Max plan length (default: 15)
    llm_provider: DynProvider,
}

pub struct Plan {
    pub id: Uuid,
    pub goal_id: Option<Uuid>,    // Optional: can plan for tasks too
    pub steps: Vec<PlanStep>,
    pub current_step: usize,
    pub status: PlanStatus,
    pub confidence: f64,          // Overall plan confidence
    pub backtrack_history: Vec<BacktrackEntry>,
}

pub struct PlanStep {
    pub step_number: usize,
    pub action: String,           // "Search web for competitor pricing"
    pub rationale: String,        // Why this step?
    pub tool_calls: Vec<String>,  // Expected tools (optional)
    pub dependencies: Vec<usize>, // Which steps must complete first
    pub status: StepStatus,       // Pending, Executing, Completed, Failed
    pub result: Option<String>,   // Result after execution
    pub attempts: u8,             // Retry count
}

pub enum PlanStatus {
    Draft,       // Generated, not started
    Executing,   // In progress
    Completed,   // All steps done
    Failed,      // Unrecoverable failure
    Abandoned,   // User cancelled
}

impl Planner {
    pub async fn generate_plan(
        &self,
        goal: &Goal,
        context: &str,
    ) -> Result<Plan>;

    pub async fn execute_step(
        &self,
        plan: &mut Plan,
        agent_loop: &AgentLoop,
    ) -> Result<StepOutcome>;

    pub async fn backtrack(
        &self,
        plan: &mut Plan,
        reason: &str,
    ) -> Result<()>;  // Regenerate from failure point
}
```

**ReAct-style execution:**
1. **Thought:** "I need to understand competitor pricing"
2. **Action:** `web_search("competitor pricing SaaS")`
3. **Observation:** "Found 5 competitors, average $49/mo"
4. **Thought:** "Now I should analyze their features"
5. ... (continues until plan complete)

### 2.5 Learning Engine (`agent/src/learning.rs`)

**Storage:** `~/.klyntbot/data/outcomes.jsonl` (append-only, anonymized summaries)

```rust
pub struct LearningEngine {
    outcome_store: OutcomeStore,
    adaptation_config: AdaptationConfig,
}

pub struct Outcome {
    pub id: Uuid,
    pub task_description: String,
    pub plan_id: Option<Uuid>,
    pub tools_used: Vec<String>,
    pub confidence_initial: f64,
    pub success: bool,
    pub user_feedback: Option<Feedback>,
    pub duration_ms: u64,
    pub created_at: DateTime<Utc>,
}

pub struct Feedback {
    pub rating: FeedbackRating,  // Positive, Negative, Neutral
    pub corrections: Vec<String>, // What user changed after
    pub comment: Option<String>,
}

pub struct LearningInsight {
    pub insight_type: InsightType,
    pub description: String,
    pub confidence: f64,
}

pub enum InsightType {
    ConfidenceThreshold,  // "Lower threshold to 0.65 for web searches"
    ToolPreference,       // "User prefers web_fetch over web_search"
    ResponseStyle,        // "User prefers concise responses"
    TaskEnrichment,       // "Adjust priority inference for 'bug' keywords"
}

impl LearningEngine {
    pub async fn record_outcome(&mut self, outcome: Outcome) -> Result<()>;
    pub async fn analyze_patterns(&self) -> Vec<LearningInsight>;
    pub async fn update_confidence_threshold(
        &self,
        confidence_eval: &mut ConfidenceEvaluator,
    ) -> Result<()>;
    pub async fn suggest_enrichment_adjustments(&self) -> Vec<EnrichmentRule>;
}
```

---

## 3. Data Flow & Interactions

### 3.1 Goal Creation Flow (Suggestion-Based)

```
1. User creates 5 tasks tagged "fitness" over 2 weeks
   ↓
2. ContextBuilder includes recent todos in system prompt
   ↓
3. Agent loop detects pattern via GoalSuggestionEngine
   • Analyzes: 5 tasks with "fitness" tag
   • Confidence: 0.82
   • Proposal: "Create goal 'Improve Fitness'?"
   ↓
4. Agent calls ask_user tool with suggestion
   User: "Yes, create it"
   ↓
5. GoalHandler.create_goal() → GoalStore.create()
   • Persists to ~/.klyntbot/data/goals.jsonl
   • Links existing 5 tasks to goal (goal_id field)
   • Returns goal_id
   ↓
6. Agent responds: "Created goal [abc123] 'Improve Fitness'
   with 5 linked tasks. Want me to create a plan?"
```

### 3.2 Plan Generation & Execution Flow

```
1. User: "Create a plan to achieve the fitness goal"
   ↓
2. Planner.generate_plan(goal, context)
   • Fetches goal details from GoalStore
   • Fetches linked tasks/projects
   • Calls LLM with ReAct prompt
   • LLM generates 8-step plan
   • Calculates plan.confidence = 0.85
   ↓
3. Confidence check: 0.85 > 0.8 threshold
   → Auto-execute (show progress in real-time)
   ↓
4. Agent loop executes steps sequentially:

   Step 1/8: Research workout routines
     Action: web_search("beginner workout routines")
     Result: Found 5 routines, avg 30 min
     Status: ✓ Completed

   Step 2/8: Create workout schedule task
     Action: todo.add("Create workout schedule", P3, tomorrow)
     Result: Created task [def456]
     Status: ✓ Completed

   ... (continues for all 8 steps)
   ↓
5. LearningEngine.record_outcome()
   • Records: plan_id, tools_used, duration, success=true
   • Stores to outcomes.jsonl
   ↓
6. Agent responds: "✓ Plan completed. Created 3 new tasks,
   researched 5 workout routines. Goal is now 40% complete."
```

### 3.3 Learning Cycle (Background Process)

```
1. Every 24 hours (configurable): LearningEngine.analyze()
   ↓
2. Load last 100 outcomes from outcomes.jsonl
   ↓
3. Pattern Detection:
   • Confidence threshold: 20 outcomes with conf<0.7 succeeded
     → Insight: "Lower web_search threshold to 0.65"

   • Tool preference: User always picks web_fetch over search
     → Insight: "Prefer web_fetch for URL-based queries"

   • Response style: User trims responses 70% of the time
     → Insight: "User prefers concise responses"
   ↓
4. Apply Adaptations (with user approval):
   • Update confidence thresholds in config
   • Add preference hints to system prompt
   • Adjust enrichment rules in EnrichmentEngine
   ↓
5. Log changes to ~/.klyntbot/data/learning_history.jsonl
```

### 3.4 Integration with Existing Agent Loop

**Modified `agent_loop.rs::run_agent_loop()`:**

```rust
async fn run_agent_loop(&self, messages: Vec<Message>) -> Result<String> {
    // NEW: Check for goal suggestions every 5 messages
    if self.message_count % 5 == 0 {
        if let Some(suggestion) = self.goal_suggestion_engine.analyze_patterns(...) {
            // Inject system message: "Consider suggesting goal to user"
        }
    }

    // Existing: Build context (NOW includes active goals)
    let context = self.context_builder.build_system_prompt();

    // Existing: Run tool-calling loop (max 20 iterations)
    for iteration in 0..MAX_TOOL_ITERATIONS {
        let response = provider.chat(&messages, &tools).await?;

        // NEW: If LLM calls "goal.create_plan", generate plan
        if let Some(plan_request) = detect_plan_request(&response) {
            let plan = self.planner.generate_plan(...).await?;

            // NEW: Confidence-gated execution
            if plan.confidence > self.config.planning.auto_exec_threshold {
                // Execute immediately, stream progress
                execute_plan_with_streaming(&plan).await?;
            } else {
                // Present plan, wait for approval
                present_plan_for_approval(&plan).await?;
            }
        }

        // Existing: Execute tool calls
        let tool_results = execute_tools(&response.tool_calls).await?;

        // NEW: Record outcome for learning
        self.learning_engine.record_outcome(Outcome { ... }).await?;

        // Existing: Check for completion
        if no more tool calls { break; }
    }

    Ok(final_response)
}
```

---

## 4. Error Handling & Edge Cases

### 4.1 Error Types

**New error variants** (add to `common/src/error.rs`):

```rust
pub enum KlyntbotError {
    Goal(GoalError),
    Planning(PlanError),
    Learning(LearningError),
}

pub enum GoalError {
    NotFound(Uuid),
    AlreadyExists(String),
    InvalidMetric(String),
    StorageCorrupted,
}

pub enum PlanError {
    GenerationFailed(String),
    ExecutionStalled(usize),  // Stuck at step N
    BacktrackLimitReached,
    InvalidDependency(usize), // Circular dependency
}

pub enum LearningError {
    OutcomeStorageFailure,
    InsufficientData,
    AdaptationDisabled,
}
```

### 4.2 Plan Execution Failures

**Step fails after 3 retries:**
- Retry with exponential backoff (2s, 4s, 8s)
- If still failing: Attempt backtracking
- If backtracking fails: Mark plan as Failed, notify user

**Backtracking strategy:**
- Tool timeout → Retry with adjusted params
- Resource not found → Regenerate steps from failure point
- LLM confusion → Add clarification to context, regenerate
- User cancellation → Mark plan as Abandoned

**Circular dependency detection:**
- Build dependency graph before execution
- Use `petgraph::algo::is_cyclic_directed()`
- Reject plan if cycle detected

### 4.3 Goal State Inconsistencies

**Goal has linked projects, but projects deleted:**
- Validate links on `calculate_progress()`
- Auto-clean orphaned links (log warning)
- Recalculate progress based on valid projects only

### 4.4 Learning Engine Safeguards

**Insufficient data:**
- Require minimum 20 outcomes before adapting
- Require 10+ samples per insight type
- Log "Insufficient data" at INFO level

**Auto-apply restrictions:**
- Only apply adaptations with confidence > 0.85
- Never auto-apply without user approval in production
- Store pending adaptations for user review

### 4.5 JSONL Storage Corruption Recovery

**Corruption detected:**
1. Attempt recovery from `.bak` file
2. If backup exists: Restore from backup
3. If backup corrupted: Skip corrupted lines, load valid entries
4. Log all skipped entries at WARN level

---

## 5. Testing Strategy

### 5.1 Unit Tests (Per Crate)

**Goal Crate:** 15 tests
- Goal creation and retrieval
- Progress calculation (3/5 projects = 60%)
- JSONL persistence and reload
- Orphaned project cleanup
- Status transitions
- Metric calculations

**Planner Module:** 12 tests
- Plan generation (3-15 steps)
- Dependency validation (circular detection)
- Backtracking logic
- Confidence calculation
- Step execution with retries
- ReAct prompt formatting

**Learning Module:** 10 tests
- Outcome recording to JSONL
- Confidence threshold adaptation (20 successes → lower threshold)
- Insufficient data safeguard (<20 outcomes → no adaptation)
- Anonymization (no sensitive data)
- Pattern detection accuracy
- Insight generation

### 5.2 Integration Tests

**Goal + TodoTool:** 3 tests
- Link tasks to goals
- Progress reflects task completion
- Orphaned task cleanup

**Planner + AgentLoop:** 2 tests
- End-to-end plan execution with real tools
- Plan streaming with cancellation

**Learning Integration:** 2 tests
- Record outcomes from agent loop
- Adapt confidence thresholds based on patterns

### 5.3 End-to-End Scenarios

**Scenario 1: Full goal lifecycle** (5 steps)
1. User creates related tasks
2. Agent suggests goal (pattern detected)
3. User approves goal creation
4. Agent generates and executes plan
5. Goal reaches 100% completion

**Scenario 2: Plan failure and recovery** (4 steps)
1. Generate plan with failing step
2. Execute until failure
3. Backtrack and regenerate
4. Complete successfully after backtrack

### 5.4 Performance Tests

- Goal store handles 1,000 goals in <500ms
- Plan generation completes in <10s
- Learning analysis of 100 outcomes in <2s

### 5.5 Test Coverage Goals

| Component | Unit Coverage | Integration | E2E |
|-----------|--------------|-------------|-----|
| `goal` crate | 90%+ | 3 tests | 2 scenarios |
| `planner` module | 85%+ | 2 tests | 2 scenarios |
| `learning` module | 80%+ | 2 tests | 1 scenario |

**Total new tests:** ~50 tests (35 unit + 9 integration + 5 e2e)

---

## 6. Implementation Roadmap

### Phase 1: Goal Engine (Week 1-2)

**Deliverables:**
- [ ] `goal` crate (types, store, suggestion engine)
- [ ] `goal_tool` in tools crate
- [ ] `goal_handler` in agent crate
- [ ] CLI commands (`klyntbot goal create`, `list`, `show`, etc.)
- [ ] ContextBuilder integration (inject active goals)
- [ ] Unit tests + integration tests

**Lines of Code:** ~1,100 LOC

### Phase 2: Planning Engine (Week 3-4)

**Deliverables:**
- [ ] `planner` module in agent crate
- [ ] Plan generation (ReAct prompt)
- [ ] Plan execution with confidence gating
- [ ] Backtracking on failure
- [ ] Integration with agent loop
- [ ] Streaming progress UI
- [ ] Unit tests + integration tests

**Lines of Code:** ~1,200 LOC

### Phase 3: Learning System (Week 5-6)

**Deliverables:**
- [ ] `learning` module in agent crate
- [ ] Outcome recording (anonymized)
- [ ] Pattern analysis (confidence, tool preference, style)
- [ ] Adaptation engine (threshold updates, enrichment tuning)
- [ ] Background analysis service (24hr cycle)
- [ ] Unit tests + integration tests

**Lines of Code:** ~1,000 LOC

### Phase 4: Integration & Polish (Week 7)

**Deliverables:**
- [ ] End-to-end testing (5 scenarios)
- [ ] Performance optimization
- [ ] Documentation updates (CLAUDE.md, README)
- [ ] User guide for new features
- [ ] Migration guide (config changes)

**Lines of Code:** ~500 LOC (docs, integration glue)

---

## Total Effort Summary

| Phase | Component | LOC | Duration |
|-------|-----------|-----|----------|
| 1 | Goal Engine | 1,100 | 2 weeks |
| 2 | Planning Engine | 1,200 | 2 weeks |
| 3 | Learning System | 1,000 | 2 weeks |
| 4 | Integration | 500 | 1 week |
| **Total** | | **~3,800 LOC** | **7 weeks** |

---

## Appendix: Configuration Schema

**New config fields** (add to `config/src/schema/core.rs`):

```rust
pub struct Config {
    // Existing fields...
    pub goals: GoalsConfig,
    pub planning: PlanningConfig,
    pub learning: LearningConfig,
}

pub struct GoalsConfig {
    pub enabled: bool,
    pub suggestion_threshold: usize,  // Default: 5 tasks
}

pub struct PlanningConfig {
    pub enabled: bool,
    pub max_steps: usize,               // Default: 15
    pub auto_exec_threshold: f64,       // Default: 0.8
    pub backtrack_max_attempts: u8,     // Default: 3
}

pub struct LearningConfig {
    pub enabled: bool,
    pub auto_apply: bool,               // Default: false (require approval)
    pub analysis_interval_hours: u64,   // Default: 24
    pub min_outcomes_for_insight: usize, // Default: 20
}
```

**Example config** (`~/.klyntbot/config.json`):

```json
{
  "goals": {
    "enabled": true,
    "suggestionThreshold": 5
  },
  "planning": {
    "enabled": true,
    "maxSteps": 15,
    "autoExecThreshold": 0.8,
    "backtrackMaxAttempts": 3
  },
  "learning": {
    "enabled": true,
    "autoApply": false,
    "analysisIntervalHours": 24,
    "minOutcomesForInsight": 20
  }
}
```

---

## Appendix: File Structure Summary

**New files:**
```
crates/goal/src/lib.rs               (50 LOC)
crates/goal/src/types.rs             (200 LOC)
crates/goal/src/store.rs             (350 LOC)
crates/goal/src/suggestion.rs        (200 LOC)
crates/tools/src/goal_tool.rs        (300 LOC)
crates/agent/src/planner.rs          (800 LOC)
crates/agent/src/learning.rs         (700 LOC)
crates/agent/src/goal_handler.rs     (150 LOC)
cli/src/goal_commands.rs             (250 LOC)
tests/goal_integration.rs            (200 LOC)
tests/planner_integration.rs         (150 LOC)
tests/learning_integration.rs        (100 LOC)
tests/e2e_goal_lifecycle.rs          (200 LOC)
tests/e2e_plan_recovery.rs           (150 LOC)
```

**Modified files:**
```
crates/agent/src/agent_loop.rs       (+150 LOC)
crates/agent/src/context.rs          (+50 LOC)
crates/common/src/error.rs           (+80 LOC)
config/src/schema/core.rs            (+100 LOC)
cli/src/main.rs                      (+30 LOC)
```

**Total:** ~3,800 LOC new + ~410 LOC modifications = **~4,210 LOC**

---

**End of Design Document**
