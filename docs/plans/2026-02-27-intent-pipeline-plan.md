# Intent Pipeline Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the current Orchestrator/EngineDispatch/Pipeline with a unified IntentPipeline that auto-decides Direct vs Reactive vs Planned execution based on structured complexity analysis.

**Architecture:** New `intent_pipeline/` module inside the `agent` crate replaces 9 files. Keeps `ExecutionCore`, `plan_executor`, `plan_step_generator` unchanged. Adds `visibility` and `task_id` columns to `plans` table. Adds `orchestrator` config section. Adds `execute` action to `TodoTool`.

**Tech Stack:** Rust, sqlx (SQLite), tokio, serde, chrono. Tests use `cargo nextest` with ephemeral in-memory SQLite.

**Design doc:** `docs/plans/2026-02-27-intent-pipeline-design.md`

---

## Task 1: Schema Migration — Add `visibility` and `task_id` to Plans

**Files:**
- Create: `crates/storage/migrations/004_intent_pipeline.sql`
- Modify: `crates/storage/src/rows/plan.rs:10-42`
- Modify: `crates/storage/src/repos/plan.rs` (upsert, list, get queries)
- Test: `crates/storage/src/repos/plan.rs` (inline tests)

**Step 1: Write the migration**

Create `crates/storage/migrations/004_intent_pipeline.sql`:

```sql
-- Intent Pipeline: plan visibility, task linkage, enhanced strategy recording

ALTER TABLE plans ADD COLUMN visibility TEXT NOT NULL DEFAULT 'transparent';
ALTER TABLE plans ADD COLUMN task_id TEXT REFERENCES todos(id) ON DELETE SET NULL;

CREATE INDEX idx_plans_visibility ON plans(visibility);
CREATE INDEX idx_plans_task_id ON plans(task_id);

ALTER TABLE strategy_records ADD COLUMN complexity_signals TEXT NOT NULL DEFAULT '{}';
ALTER TABLE strategy_records ADD COLUMN execution_mode TEXT;
```

**Step 2: Update `PlanRow` to include new fields**

Modify `crates/storage/src/rows/plan.rs:10-23` — add `visibility` and `task_id` fields to `PlanRow`:

```rust
pub struct PlanRow {
    pub id: uuid::Uuid,
    pub session_key: String,
    pub goal_id: Option<uuid::Uuid>,
    pub title: String,
    pub description: String,
    pub status: String,
    pub current_step_index: i32,
    pub iteration_limit: i32,
    pub backtrack_history: serde_json::Value,
    pub visibility: String,                    // NEW: "silent"|"on_failure"|"transparent"
    pub task_id: Option<String>,               // NEW: FK to todos.id
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

**Step 3: Update PlanRepo SQL queries**

In `crates/storage/src/repos/plan.rs`:
- `create()` (line 22): Add `visibility` and `task_id` to INSERT column list and bind params
- `upsert()` (line 48): Add `visibility` and `task_id` to ON CONFLICT UPDATE SET list
- `get()` (line 81): No change needed (uses `SELECT *`)
- `list()` (line 90): Add optional `visibility` filter param. Default behavior: exclude `'silent'` plans. Add `visibility: Option<&str>` parameter
- `update()` (line 118): Add `visibility` and `task_id` to UPDATE SET list

**Step 4: Write test for new fields**

Add to the inline `#[cfg(test)] mod tests` in `crates/storage/src/repos/plan.rs`:

```rust
#[tokio::test]
async fn plan_visibility_and_task_id() {
    let pool = crate::StoragePool::connect_in_memory().await.unwrap();
    let repo = PlanRepo::new(pool.inner().clone());

    let mut row = test_plan_row();
    row.visibility = "on_failure".to_string();
    row.task_id = Some("task-123".to_string());

    let created = repo.create(&row).await.unwrap();
    assert_eq!(created.visibility, "on_failure");
    assert_eq!(created.task_id.as_deref(), Some("task-123"));

    // list excludes silent by default
    let mut silent_row = test_plan_row();
    silent_row.id = uuid::Uuid::new_v4();
    silent_row.visibility = "silent".to_string();
    repo.create(&silent_row).await.unwrap();

    let visible = repo.list(None, None, None, None).await.unwrap(); // default visibility filter
    assert!(visible.iter().all(|p| p.visibility != "silent"));

    let all = repo.list(None, None, None, Some("all")).await.unwrap();
    assert!(all.len() > visible.len());
}
```

**Step 5: Run tests**

Run: `cargo nextest run -p storage -E 'test(plan_visibility)'`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/storage/migrations/004_intent_pipeline.sql crates/storage/src/rows/plan.rs crates/storage/src/repos/plan.rs
git commit -m "feat(storage): add plan visibility, task_id columns, migration 004"
```

---

## Task 2: Update Plan Domain Types — Add `visibility` and `task_id`

**Files:**
- Modify: `crates/plan/src/types.rs:11-25`
- Modify: `crates/plan/src/conversions.rs`
- Test: `crates/plan/src/types.rs` (inline tests)

**Step 1: Add `PlanVisibility` enum and new fields to `Plan`**

In `crates/plan/src/types.rs`, add the enum and modify `Plan`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum PlanVisibility {
    Silent,
    OnFailure,
    #[default]
    Transparent,
}

pub struct Plan {
    pub id: Uuid,
    pub session_key: String,
    pub goal_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub status: PlanStatus,
    pub steps: Vec<PlanStep>,
    pub current_step_index: usize,
    pub iteration_limit: usize,
    pub backtrack_history: Vec<BacktrackEntry>,
    pub visibility: PlanVisibility,          // NEW
    pub task_id: Option<String>,             // NEW
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}
```

**Step 2: Add conversion helpers**

In `crates/plan/src/conversions.rs`, add string converters and update `plan_to_row` / `row_to_plan`:

```rust
pub fn visibility_to_str(v: &PlanVisibility) -> &'static str {
    match v {
        PlanVisibility::Silent => "silent",
        PlanVisibility::OnFailure => "on_failure",
        PlanVisibility::Transparent => "transparent",
    }
}

pub fn str_to_visibility(s: &str) -> PlanVisibility {
    match s {
        "silent" => PlanVisibility::Silent,
        "on_failure" => PlanVisibility::OnFailure,
        _ => PlanVisibility::Transparent,
    }
}
```

Update `plan_to_row()` to map `visibility` and `task_id`. Update `row_to_plan()` to parse them back.

**Step 3: Write test**

```rust
#[test]
fn plan_visibility_roundtrip() {
    assert_eq!(visibility_to_str(&PlanVisibility::Silent), "silent");
    assert_eq!(str_to_visibility("on_failure"), PlanVisibility::OnFailure);
    assert_eq!(str_to_visibility("unknown"), PlanVisibility::Transparent);
}
```

**Step 4: Run tests**

Run: `cargo nextest run -p plan`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/plan/
git commit -m "feat(plan): add PlanVisibility enum, task_id field to Plan domain type"
```

---

## Task 3: Config — Add `orchestrator` Section

**Files:**
- Create: `crates/config/src/schema/orchestrator.rs`
- Modify: `crates/config/src/schema/core.rs:77-135` (add field to `Config`)
- Modify: `crates/config/src/schema/mod.rs` (add module)
- Modify: `crates/config/src/schema/todo.rs` (add auto-plan fields to `TodoConfig`)
- Test: `crates/config/src/schema/orchestrator.rs` (inline tests)

**Step 1: Create orchestrator config schema**

Create `crates/config/src/schema/orchestrator.rs`:

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorConfig {
    #[serde(default = "default_heuristic_threshold")]
    pub heuristic_confidence_threshold: f32,
    #[serde(default = "default_classifier_timeout")]
    pub llm_classifier_timeout: u64,
    pub llm_classifier_model: Option<String>,
    #[serde(default = "default_visibility")]
    pub default_plan_visibility: String,
    #[serde(default = "default_complexity_threshold")]
    pub plan_complexity_threshold: u8,
    #[serde(default = "default_max_escalations")]
    pub max_escalations: u32,
}

fn default_heuristic_threshold() -> f32 { 0.85 }
fn default_classifier_timeout() -> u64 { 2000 }
fn default_visibility() -> String { "on_failure".to_string() }
fn default_complexity_threshold() -> u8 { 3 }
fn default_max_escalations() -> u32 { 1 }

impl Default for OrchestratorConfig {
    fn default() -> Self {
        Self {
            heuristic_confidence_threshold: 0.85,
            llm_classifier_timeout: 2000,
            llm_classifier_model: None,
            default_plan_visibility: "on_failure".to_string(),
            plan_complexity_threshold: 3,
            max_escalations: 1,
        }
    }
}
```

**Step 2: Add to `Config` struct**

In `crates/config/src/schema/core.rs:77-135`, add:

```rust
pub orchestrator: OrchestratorConfig,
```

**Step 3: Add auto-plan fields to `TodoConfig`**

In the `TodoConfig` struct (find in `crates/config/src/schema/todo.rs`), add:

```rust
#[serde(default)]
pub auto_plan_suggestion: bool,        // default: true
#[serde(default)]
pub auto_plan_on_focus: bool,          // default: false
#[serde(default = "default_plan_complexity_threshold")]
pub plan_complexity_threshold: u8,     // default: 3
```

**Step 4: Register module**

In `crates/config/src/schema/mod.rs`, add:

```rust
pub mod orchestrator;
pub use orchestrator::OrchestratorConfig;
```

**Step 5: Write test**

```rust
#[test]
fn orchestrator_config_defaults() {
    let config: OrchestratorConfig = serde_json::from_str("{}").unwrap();
    assert_eq!(config.heuristic_confidence_threshold, 0.85);
    assert_eq!(config.plan_complexity_threshold, 3);
    assert_eq!(config.default_plan_visibility, "on_failure");
    assert_eq!(config.max_escalations, 1);
}
```

**Step 6: Run tests**

Run: `cargo nextest run -p config`
Expected: PASS

**Step 7: Commit**

```bash
git add crates/config/
git commit -m "feat(config): add orchestrator config section and todo auto-plan fields"
```

---

## Task 4: Core Types — `ExecutionMode`, `ComplexitySignals`, `IntentAnalysis`

**Files:**
- Create: `crates/agent/src/intent_pipeline/types.rs`
- Create: `crates/agent/src/intent_pipeline/mod.rs` (start with just `pub mod types;`)
- Modify: `crates/agent/src/lib.rs` (add `pub mod intent_pipeline;`)
- Test: `crates/agent/src/intent_pipeline/types.rs` (inline tests)

**Step 1: Write failing test for complexity scoring**

Create `crates/agent/src/intent_pipeline/types.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_complexity_is_direct() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 0,
            has_sequential_deps: false,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(signals.complexity_score(), 0);
    }

    #[test]
    fn high_complexity_triggers_planned() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 4,
            has_sequential_deps: true,
            failure_risk: FailureRisk::High,
            requires_state_tracking: true,
            requires_retries: true,
        };
        assert!(signals.complexity_score() >= 3);
    }

    #[test]
    fn sequential_deps_alone_score_2() {
        let signals = ComplexitySignals {
            estimated_tool_calls: 1,
            has_sequential_deps: true,
            failure_risk: FailureRisk::Low,
            requires_state_tracking: false,
            requires_retries: false,
        };
        assert_eq!(signals.complexity_score(), 2);
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(zero_complexity)'`
Expected: FAIL — types don't exist yet

**Step 3: Implement the types**

Fill in `crates/agent/src/intent_pipeline/types.rs`:

```rust
use plan::PlanVisibility;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq)]
pub enum ExecutionMode {
    Direct,
    Reactive { max_iterations: u32 },
    Planned { visibility: PlanVisibility, max_steps: u8 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum FailureRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComplexitySignals {
    pub estimated_tool_calls: u8,
    pub has_sequential_deps: bool,
    pub failure_risk: FailureRisk,
    pub requires_state_tracking: bool,
    pub requires_retries: bool,
}

impl ComplexitySignals {
    pub fn complexity_score(&self) -> u8 {
        let mut score: u8 = 0;
        if self.estimated_tool_calls >= 3 {
            score += 2;
        } else if self.estimated_tool_calls >= 2 {
            score += 1;
        }
        if self.has_sequential_deps {
            score += 2;
        }
        if self.failure_risk >= FailureRisk::Medium {
            score += 1;
        }
        if self.requires_state_tracking {
            score += 1;
        }
        if self.requires_retries {
            score += 1;
        }
        score
    }
}

#[derive(Debug, Clone)]
pub enum AnalysisSource {
    Heuristic,
    LlmClassifier,
    MidExecutionEscalation,
}

#[derive(Debug, Clone)]
pub struct IntentAnalysis {
    pub mode: ExecutionMode,
    pub signals: ComplexitySignals,
    pub confidence: f32,
    pub source: AnalysisSource,
    pub reasoning: String,
}
```

Create `crates/agent/src/intent_pipeline/mod.rs`:

```rust
pub mod types;

pub use types::{
    AnalysisSource, ComplexitySignals, ExecutionMode, FailureRisk, IntentAnalysis,
};
```

Add `pub mod intent_pipeline;` to `crates/agent/src/lib.rs`.

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(complexity)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/ crates/agent/src/lib.rs
git commit -m "feat(agent): add intent_pipeline core types — ExecutionMode, ComplexitySignals"
```

---

## Task 5: Enhanced Heuristics — Complexity-Aware Classification

**Files:**
- Create: `crates/agent/src/intent_pipeline/heuristics.rs`
- Modify: `crates/agent/src/intent_pipeline/mod.rs` (add `pub mod heuristics;`)
- Test: `crates/agent/src/intent_pipeline/heuristics.rs` (inline tests)

**Step 1: Write failing tests**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_is_direct() {
        let result = analyze_heuristic("hello");
        assert!(result.is_some());
        assert!(matches!(result.unwrap().mode, ExecutionMode::Direct));
    }

    #[test]
    fn task_crud_is_reactive() {
        let result = analyze_heuristic("create a task to buy groceries");
        assert!(result.is_some());
        assert!(matches!(result.unwrap().mode, ExecutionMode::Reactive { .. }));
    }

    #[test]
    fn sequential_multi_step_is_none() {
        let result = analyze_heuristic("first search for flights to Tokyo, then compare prices, and book the cheapest one");
        // Ambiguous — should defer to LLM
        assert!(result.is_none());
    }

    #[test]
    fn simple_search_is_reactive() {
        let result = analyze_heuristic("search for tasks about database migration");
        assert!(result.is_some());
        assert!(matches!(result.unwrap().mode, ExecutionMode::Reactive { .. }));
    }

    #[test]
    fn what_is_question_is_direct() {
        let result = analyze_heuristic("what is the capital of France?");
        assert!(result.is_some());
        assert!(matches!(result.unwrap().mode, ExecutionMode::Direct));
    }
}
```

**Step 2: Implement enhanced heuristics**

Create `crates/agent/src/intent_pipeline/heuristics.rs` with `analyze_heuristic(message: &str) -> Option<IntentAnalysis>`.

Key differences from current `orchestrator/heuristics.rs`:
- Returns `IntentAnalysis` (with `ComplexitySignals`) instead of `ExecutionStrategy`
- Uses structural analysis functions: `count_tool_indicators()`, `detect_sequential_language()`, `assess_failure_keywords()`, `detect_state_requirements()`, `detect_retry_indicators()`
- Preserves greeting detection, task management override, and conflict-based deferral
- Scores complexity 0-5 and maps to `ExecutionMode`

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(greeting_is_direct)' --no-capture`
Expected: PASS (all 5 tests)

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/heuristics.rs crates/agent/src/intent_pipeline/mod.rs
git commit -m "feat(agent): add complexity-aware heuristic classifier"
```

---

## Task 6: LLM Classifier — Structured ComplexitySignals Output

**Files:**
- Create: `crates/agent/src/intent_pipeline/classifier.rs`
- Modify: `crates/agent/src/intent_pipeline/mod.rs`
- Test: `crates/agent/src/intent_pipeline/classifier.rs` (inline tests)

**Step 1: Write failing test with mock provider**

Test that the classifier parses structured JSON into `IntentAnalysis`:

```rust
#[tokio::test]
async fn parses_structured_classification() {
    let response = r#"{"mode":"planned","estimated_tool_calls":5,"has_sequential_deps":true,"failure_risk":"high","requires_state_tracking":true,"requires_retries":false,"confidence":0.9,"reasoning":"Multi-step booking"}"#;
    let provider = mock_provider_returning(response);
    let classifier = IntentClassifier::new(provider, Duration::from_secs(2));
    let result = classifier.classify("book a flight", &["web_search", "web_fetch"], &params, None).await.unwrap();
    assert!(matches!(result.mode, ExecutionMode::Planned { .. }));
    assert_eq!(result.signals.estimated_tool_calls, 5);
    assert!(result.signals.has_sequential_deps);
}
```

**Step 2: Implement `IntentClassifier`**

Create `crates/agent/src/intent_pipeline/classifier.rs`:
- `IntentClassifier` struct (holds `provider: DynProvider`, `timeout: Duration`)
- `classify()` method: builds structured prompt asking for JSON with all `ComplexitySignals` fields + mode + confidence + reasoning
- Parses JSON response, maps mode string to `ExecutionMode`, builds `ComplexitySignals`
- Fallback on timeout/error: `Reactive { max_iterations: 10 }` with confidence 0.5
- Uses `extract_json_object()` helper (find first `{` to last `}`)

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(parses_structured)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/classifier.rs crates/agent/src/intent_pipeline/mod.rs
git commit -m "feat(agent): add LLM classifier with structured ComplexitySignals output"
```

---

## Task 7: IntentAnalyzer — Two-Stage Classification

**Files:**
- Create: `crates/agent/src/intent_pipeline/analyzer.rs`
- Modify: `crates/agent/src/intent_pipeline/mod.rs`
- Test: `crates/agent/src/intent_pipeline/analyzer.rs` (inline tests)

**Step 1: Write test for two-stage flow**

```rust
#[tokio::test]
async fn greeting_bypasses_llm() {
    let analyzer = IntentAnalyzer::new(mock_provider(), "model", &OrchestratorConfig::default());
    let result = analyzer.analyze("hello", &[]).await;
    assert!(matches!(result.mode, ExecutionMode::Direct));
    assert!(matches!(result.source, AnalysisSource::Heuristic));
    // LLM should NOT have been called (mock would panic if it was)
}

#[tokio::test]
async fn ambiguous_uses_llm() {
    let response = r#"{"mode":"reactive","estimated_tool_calls":2,"has_sequential_deps":false,"failure_risk":"low","requires_state_tracking":false,"requires_retries":false,"confidence":0.8,"reasoning":"Needs search"}"#;
    let analyzer = IntentAnalyzer::new(mock_provider_returning(response), "model", &OrchestratorConfig::default());
    let result = analyzer.analyze("find and summarize the latest news about AI", &["web_search"]).await;
    assert!(matches!(result.mode, ExecutionMode::Reactive { .. }));
    assert!(matches!(result.source, AnalysisSource::LlmClassifier));
}
```

**Step 2: Implement `IntentAnalyzer`**

```rust
pub struct IntentAnalyzer {
    classifier: IntentClassifier,
    classifier_params: ChatParams,
    strategy_repo: Option<storage::StrategyRepo>,
    config: OrchestratorConfig,
}

impl IntentAnalyzer {
    pub fn new(provider: DynProvider, model: &str, config: &OrchestratorConfig) -> Self { ... }

    pub fn with_strategy_repo(mut self, repo: storage::StrategyRepo) -> Self { ... }

    pub async fn analyze(&self, message: &str, tool_names: &[&str]) -> IntentAnalysis {
        // Stage 1: Heuristics
        if let Some(analysis) = analyze_heuristic(message) {
            if analysis.confidence >= self.config.heuristic_confidence_threshold {
                return analysis;
            }
        }
        // Stage 2: LLM classifier
        let strategy_context = self.build_strategy_context().await;
        match self.classifier.classify(message, tool_names, &self.classifier_params, strategy_context.as_deref()).await {
            Ok(result) => {
                if result.confidence < 0.5 {
                    // Low confidence fallback
                    return IntentAnalysis {
                        mode: ExecutionMode::Reactive { max_iterations: 10 },
                        ..result
                    };
                }
                result
            }
            Err(_) => IntentAnalysis::fallback(),
        }
    }
}
```

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(greeting_bypasses|ambiguous_uses)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/analyzer.rs crates/agent/src/intent_pipeline/mod.rs
git commit -m "feat(agent): add IntentAnalyzer with two-stage classification"
```

---

## Task 8: ExecutionEngine Trait + EscalationContext

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/mod.rs`
- Create: `crates/agent/src/intent_pipeline/escalation.rs`
- Modify: `crates/agent/src/intent_pipeline/mod.rs`
- Test: inline in `escalation.rs`

**Step 1: Define the unified engine trait**

Create `crates/agent/src/intent_pipeline/engines/mod.rs`:

```rust
use async_trait::async_trait;
use crate::execution::{ExecutionCore, ExecutionParams, ReasoningTrace};
use providers::Usage;
use tools::RoutingContext;
use super::escalation::EscalationContext;

pub mod direct;
pub mod reactive;
pub mod planned;

pub enum EngineResult {
    Complete {
        content: String,
        usage: Usage,
        iterations: u32,
        traces: Vec<ReasoningTrace>,
        tool_name: Option<String>,
    },
    Escalate {
        reason: String,
        carried_context: EscalationContext,
        usage: Usage,
    },
}

#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    async fn execute(
        &self,
        messages: Vec<providers::Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> common::Result<EngineResult>;

    fn mode(&self) -> &str;
}
```

**Step 2: Define EscalationContext**

Create `crates/agent/src/intent_pipeline/escalation.rs`:

```rust
use providers::Message;

#[derive(Debug, Clone)]
pub struct CompletedStep {
    pub description: String,
    pub tool_name: String,
    pub result: String,
}

#[derive(Debug, Clone)]
pub struct EscalationContext {
    pub messages: Vec<Message>,
    pub completed_work: Vec<CompletedStep>,
    pub original_message: String,
}
```

**Step 3: Run compilation check**

Run: `cargo build -p agent 2>&1 | head -20`
Expected: Compiles (no tests yet for this task — types only)

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/ crates/agent/src/intent_pipeline/escalation.rs crates/agent/src/intent_pipeline/mod.rs
git commit -m "feat(agent): add ExecutionEngine trait and EscalationContext types"
```

---

## Task 9: DirectEngine (New)

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/direct.rs`
- Test: inline tests

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn direct_returns_response() {
    let core = test_execution_core("Hello! How can I help?");
    let engine = DirectEngine::new(Arc::new(core));
    let result = engine.execute(vec![], &[], &default_params(), &test_ctx(), None).await.unwrap();
    assert!(matches!(result, EngineResult::Complete { .. }));
}
```

**Step 2: Implement DirectEngine**

Port logic from `crates/agent/src/execution/direct.rs` but return `EngineResult` instead of `DirectOutcome`. On tool calls from LLM → return `EngineResult::Escalate` instead of `DirectOutcome::EscalateToToolAssisted`.

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(direct_returns)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/direct.rs
git commit -m "feat(agent): add DirectEngine to intent_pipeline"
```

---

## Task 10: ReactiveEngine with Mid-Execution Escalation

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/reactive.rs`
- Test: inline tests

**Step 1: Write failing tests**

```rust
#[tokio::test]
async fn reactive_completes_simple_tool_use() {
    // Mock provider returns tool call then final response
    let core = test_core_with_tool_then_response();
    let engine = ReactiveEngine::new(Arc::new(core), 10);
    let result = engine.execute(msgs, &tools, &params, &ctx, None).await.unwrap();
    assert!(matches!(result, EngineResult::Complete { .. }));
}

#[tokio::test]
async fn reactive_escalates_on_complexity() {
    // Mock provider keeps returning tool calls, never a final response
    let core = test_core_always_tools();
    let engine = ReactiveEngine::new(Arc::new(core), 5);
    let result = engine.execute(msgs, &tools, &params, &ctx, None).await.unwrap();
    match result {
        EngineResult::Escalate { carried_context, .. } => {
            assert!(!carried_context.completed_work.is_empty());
        }
        _ => panic!("Expected escalation"),
    }
}
```

**Step 2: Implement ReactiveEngine**

Port from `crates/agent/src/execution/react_plus.rs` with these changes:
- Returns `EngineResult` instead of `ReactOutcome`
- On escalation: builds `EscalationContext` with `completed_work` populated from tool results gathered so far
- Tracks `completed_work: Vec<CompletedStep>` during the loop — on each `ToolsExecuted`, push a `CompletedStep` with the tool name and result
- Preserves: duplicate detection, fabrication handling, reflection modes

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(reactive_completes|reactive_escalates)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/reactive.rs
git commit -m "feat(agent): add ReactiveEngine with mid-execution escalation and context preservation"
```

---

## Task 11: PlannedEngine — Unified Plan Generate + Execute

**Files:**
- Create: `crates/agent/src/intent_pipeline/engines/planned.rs`
- Test: inline tests

**Step 1: Write failing tests**

```rust
#[tokio::test]
async fn planned_generates_and_executes() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let engine = PlannedEngine::new(
        Arc::new(test_core()),
        PlanRepo::new(pool.inner().clone()),
        mock_provider_with_steps(),
        "model".to_string(),
        PlanVisibility::Transparent,
    );
    let result = engine.execute(msgs, &tools, &params, &ctx, None).await.unwrap();
    assert!(matches!(result, EngineResult::Complete { .. }));
}

#[tokio::test]
async fn planned_accepts_escalation_context() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let engine = PlannedEngine::new(/* ... */);
    let prior_work = vec![CompletedStep {
        description: "Searched flights".into(),
        tool_name: "web_search".into(),
        result: "Found 5 results".into(),
    }];
    let escalation = EscalationContext {
        messages: vec![],
        completed_work: prior_work,
        original_message: "book cheapest flight".into(),
    };
    let result = engine.execute_with_prior_work(escalation, &tools, &params, &ctx, None).await.unwrap();
    // Verify completed_work was pre-filled as completed steps
    assert!(matches!(result, EngineResult::Complete { .. }));
}
```

**Step 2: Implement PlannedEngine**

Unifies `crates/agent/src/execution/plan_generate.rs` and `crates/agent/src/plan_runner.rs`:

```rust
pub struct PlannedEngine {
    core: Arc<ExecutionCore>,
    plan_repo: storage::PlanRepo,
    provider: DynProvider,
    model: String,
    default_visibility: PlanVisibility,
}

impl PlannedEngine {
    /// Fresh execution — generates steps from scratch
    async fn execute_fresh(&self, messages, tools, params, ctx, event_tx) -> Result<EngineResult>

    /// Escalation takeover — accepts prior work as pre-filled completed steps
    pub async fn execute_with_prior_work(&self, escalation: EscalationContext, ...) -> Result<EngineResult>
}
```

Key behaviors:
- Uses `generate_plan_steps()` and `plan_executor::run_step()` (both kept unchanged)
- Full retry + backtracking support (ported from `plan_runner.rs`)
- `execute_with_prior_work()`: generates steps with LLM awareness of completed work, pre-fills completed steps
- Plans are saved with the configured `visibility`
- Falls back to ReactiveEngine on empty step generation (same as current PlanGenerateEngine)

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(planned_generates|planned_accepts)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/planned.rs
git commit -m "feat(agent): add PlannedEngine — unified plan generate + execute with escalation support"
```

---

## Task 12: ExecutionRouter — Strategy-to-Engine Dispatch with Escalation

**Files:**
- Create: `crates/agent/src/intent_pipeline/router.rs`
- Modify: `crates/agent/src/intent_pipeline/mod.rs`
- Test: inline tests

**Step 1: Write failing tests**

```rust
#[tokio::test]
async fn routes_direct_to_direct_engine() { ... }

#[tokio::test]
async fn routes_reactive_to_reactive_engine() { ... }

#[tokio::test]
async fn handles_escalation_from_reactive_to_planned() { ... }

#[tokio::test]
async fn respects_max_escalation_limit() { ... }
```

**Step 2: Implement ExecutionRouter**

```rust
pub struct ExecutionRouter {
    direct: DirectEngine,
    reactive: ReactiveEngine,
    planned: Option<PlannedEngine>,
    max_escalations: u32,
}

impl ExecutionRouter {
    pub async fn execute(
        &self,
        mode: ExecutionMode,
        messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
        event_tx: Option<Sender<AgentEvent>>,
    ) -> Result<RouterResult>
}
```

Logic:
- Match on `ExecutionMode` → call appropriate engine
- On `EngineResult::Escalate` and `escalation_count < max_escalations` → route to next engine with `EscalationContext`
- `Direct → Reactive` escalation: just re-execute with tools
- `Reactive → Planned` escalation: call `planned.execute_with_prior_work()`
- Track `escalation_count` and `final_mode`

**Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(routes_direct|routes_reactive|handles_escalation|respects_max)'`
Expected: PASS

**Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/router.rs crates/agent/src/intent_pipeline/mod.rs
git commit -m "feat(agent): add ExecutionRouter with escalation chain handling"
```

---

## Task 13: IntentPipeline — Full Pipeline Replacement

**Files:**
- Create: `crates/agent/src/intent_pipeline/pipeline.rs` (or expand `mod.rs`)
- Modify: `crates/agent/src/intent_pipeline/mod.rs`
- Test: inline tests

**Step 1: Write failing test**

```rust
#[tokio::test]
async fn pipeline_processes_greeting() {
    let pipeline = test_pipeline();
    let result = pipeline.process_message("hello", vec![], &[], &[], &ctx, None, None).await.unwrap();
    assert_eq!(result.classification.source, AnalysisSource::Heuristic);
    assert!(!result.content.is_empty());
}
```

**Step 2: Implement IntentPipeline**

```rust
pub struct IntentPipeline {
    analyzer: IntentAnalyzer,
    context_engine: ContextEngine,
    router: ExecutionRouter,
    validator: ResponseValidator,
    cost_tracker: Arc<CostTracker>,
    config: PipelineConfig,
    strategy_repo: Option<storage::StrategyRepo>,
}
```

Same 6-step flow as current `AgentPipeline::process_message()`:
1. `analyzer.analyze()` → `IntentAnalysis`
2. `context_engine.assemble()` → needs `ExecutionStrategy` mapping (see Step 3)
3. `router.execute()` → `RouterResult`
4. `validator.validate()`
5. `cost_tracker.record()`
6. `strategy_repo.create()` (with new `complexity_signals` and `execution_mode` fields)

**Step 3: Bridge `ExecutionMode` → `ExecutionStrategy` for ContextEngine**

The `ContextEngine` still expects `ExecutionStrategy`. Add a conversion:

```rust
impl From<&ExecutionMode> for ExecutionStrategy {
    fn from(mode: &ExecutionMode) -> Self {
        match mode {
            ExecutionMode::Direct => ExecutionStrategy::DirectResponse,
            ExecutionMode::Reactive { max_iterations } => ExecutionStrategy::ToolAssisted { max_iterations: *max_iterations },
            ExecutionMode::Planned { .. } => ExecutionStrategy::AutonomousTask { max_iterations: 50 },
        }
    }
}
```

This keeps `ContextEngine` unchanged — it only cares about whether to allocate tool budget.

**Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(pipeline_processes)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/
git commit -m "feat(agent): add IntentPipeline — full pipeline replacement with intent analysis"
```

---

## Task 14: Wire IntentPipeline into AgentLoopBuilder

**Files:**
- Modify: `crates/agent/src/agent_loop/builder.rs:693-746`
- Modify: `crates/agent/src/agent_loop/mod.rs:85` (change pipeline type)
- Modify: `crates/agent/src/lib.rs` (update re-exports)

**Step 1: Update builder to construct IntentPipeline**

In `crates/agent/src/agent_loop/builder.rs`, replace the pipeline construction block (lines 693-746) with:

```rust
// Build IntentPipeline (replaces AgentPipeline + Orchestrator + EngineDispatch)
let execution_core = Arc::new(ExecutionCore::new(provider.clone(), Arc::clone(&tool_registry)));

let direct_engine = intent_pipeline::engines::direct::DirectEngine::new(Arc::clone(&execution_core));
let reactive_engine = intent_pipeline::engines::reactive::ReactiveEngine::new(Arc::clone(&execution_core), 10);

let planned_engine = stored_plan_repo.as_ref().map(|repo| {
    intent_pipeline::engines::planned::PlannedEngine::new(
        Arc::clone(&execution_core),
        repo.clone(),
        provider.clone(),
        config.agents.defaults.model.clone(),
        plan::str_to_visibility(&config.orchestrator.default_plan_visibility),
    )
});

let router = intent_pipeline::router::ExecutionRouter::new(
    direct_engine, reactive_engine, planned_engine,
).with_max_escalations(config.orchestrator.max_escalations);

let analyzer = intent_pipeline::analyzer::IntentAnalyzer::new(
    provider.clone(),
    &config.agents.defaults.model,
    &config.orchestrator,
).with_strategy_repo(repos.strategies.clone());

let pipeline = Arc::new(intent_pipeline::IntentPipeline::new(
    analyzer, context_engine, router, cost_tracker, pipeline_config,
).with_strategy_repo(repos.strategies.clone()));
```

**Step 2: Update AgentLoop struct**

In `crates/agent/src/agent_loop/mod.rs:85`, change:
```rust
// Old:
pub(crate) pipeline: Arc<crate::pipeline::AgentPipeline>,
// New:
pub(crate) pipeline: Arc<crate::intent_pipeline::IntentPipeline>,
```

Update `run_pipeline()` to call the new pipeline's `process_message()`.

**Step 3: Verify compilation**

Run: `cargo build -p agent`
Expected: Compiles (may have warnings about unused old modules)

**Step 4: Run all existing tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass (old orchestrator/pipeline tests may need updating — see Task 15)

**Step 5: Commit**

```bash
git add crates/agent/src/agent_loop/ crates/agent/src/lib.rs
git commit -m "feat(agent): wire IntentPipeline into AgentLoopBuilder, replace AgentPipeline"
```

---

## Task 15: Update Existing Tests + Delete Old Modules

**Files:**
- Delete: `crates/agent/src/orchestrator/` (directory)
- Delete: `crates/agent/src/execution/dispatch.rs`
- Delete: `crates/agent/src/execution/direct.rs`
- Delete: `crates/agent/src/execution/react_plus.rs`
- Delete: `crates/agent/src/execution/plan_generate.rs`
- Delete: `crates/agent/src/pipeline.rs`
- Delete: `crates/agent/src/plan_runner.rs`
- Modify: `crates/agent/src/execution/mod.rs` (remove deleted re-exports)
- Modify: `crates/agent/src/lib.rs` (remove old module declarations)
- Modify: `tests/orchestrator_e2e.rs` (update to use IntentPipeline)

**Step 1: Remove old module declarations**

In `crates/agent/src/execution/mod.rs`, remove:
```rust
pub mod direct;
pub mod dispatch;
pub mod plan_generate;
pub mod react_plus;
```
Keep: `pub mod core;`, `pub mod scratchpad;`, `pub mod types;`

In `crates/agent/src/lib.rs`, remove:
```rust
pub mod orchestrator;
// and old pipeline re-exports
```

**Step 2: Delete old files**

```bash
rm -rf crates/agent/src/orchestrator/
rm crates/agent/src/execution/dispatch.rs
rm crates/agent/src/execution/direct.rs
rm crates/agent/src/execution/react_plus.rs
rm crates/agent/src/execution/plan_generate.rs
rm crates/agent/src/pipeline.rs
rm crates/agent/src/plan_runner.rs
```

**Step 3: Fix compilation errors**

Any remaining references to old types (`ExecutionStrategy` used as parameters, `AgentPipeline` in tests, etc.) need updating. The `From<&ExecutionMode>` impl for `ExecutionStrategy` (Task 13) handles the ContextEngine bridge.

**Step 4: Update integration tests**

`tests/orchestrator_e2e.rs` — update to test `IntentPipeline` instead of `AgentPipeline`. Update imports and assertions.

**Step 5: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

**Step 6: Commit**

```bash
git add -A
git commit -m "refactor(agent): remove old orchestrator/pipeline/engines, update tests for IntentPipeline"
```

---

## Task 16: Task → Plan Bridge — TaskComplexitySignals + TodoTool `execute` Action

**Files:**
- Create: `crates/feature-todo/src/task_complexity.rs`
- Modify: `crates/feature-todo/src/tool/mod.rs:257-290` (add `execute` action)
- Create: `crates/feature-todo/src/tool/actions/execute.rs`
- Modify: `crates/feature-todo/src/tool/actions/add.rs:99` (add plan suggestion)
- Modify: `crates/feature-todo/src/tool/actions/update.rs:232-256` (add plan-on-focus)
- Test: `crates/feature-todo/src/task_complexity.rs` (inline tests)

**Step 1: Write failing tests for complexity scoring**

Create `crates/feature-todo/src/task_complexity.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_task_not_plan_worthy() {
        let signals = TaskComplexitySignals {
            has_dependencies: false,
            subtask_count: 0,
            dependency_depth: 0,
            estimated_minutes: Some(15),
            priority: 3,
        };
        assert!(!signals.is_plan_worthy(3));
    }

    #[test]
    fn complex_task_is_plan_worthy() {
        let signals = TaskComplexitySignals {
            has_dependencies: true,
            subtask_count: 5,
            dependency_depth: 3,
            estimated_minutes: Some(120),
            priority: 1,
        };
        assert!(signals.is_plan_worthy(3));
    }
}
```

**Step 2: Implement TaskComplexitySignals**

```rust
pub struct TaskComplexitySignals {
    pub has_dependencies: bool,
    pub subtask_count: u16,
    pub dependency_depth: u8,
    pub estimated_minutes: Option<u32>,
    pub priority: u8,
}

impl TaskComplexitySignals {
    pub fn complexity_score(&self) -> u8 {
        let mut score: u8 = 0;
        if self.has_dependencies { score += 2; }
        if self.subtask_count >= 3 { score += 1; }
        if self.dependency_depth >= 2 { score += 1; }
        if self.estimated_minutes.unwrap_or(0) >= 60 { score += 1; }
        if self.priority <= 2 { score += 1; }
        score
    }

    pub fn is_plan_worthy(&self, threshold: u8) -> bool {
        self.complexity_score() >= threshold
    }
}

pub async fn evaluate_task_complexity(repo: &TodoRepo, task_id: &str) -> TaskComplexitySignals {
    // Query deps, subtask count, etc. from repo
    ...
}
```

**Step 3: Add `execute` action to TodoTool**

In `crates/feature-todo/src/tool/mod.rs`, add `"execute"` to the match dispatch (around line 289):

```rust
"execute" => actions::execute::handle_execute(self, p).await,
```

Create `crates/feature-todo/src/tool/actions/execute.rs`:

```rust
pub(crate) async fn handle_execute(tool: &TodoTool, p: &ParamExtractor<'_>) -> Result<String> {
    let id = p.required_str("id")?;
    let task = load_full_todo(&tool.repo, id).await?;
    let signals = evaluate_task_complexity(&tool.repo, id).await;

    if signals.is_plan_worthy(3) { // threshold from config
        Ok(format!(
            "Task '{}' is complex (score: {}/5). It has {} subtasks and {} dependencies. \
             I recommend executing this as a plan. Say 'create a plan for task {}' to proceed.",
            task.title, signals.complexity_score(), signals.subtask_count,
            if signals.has_dependencies { "active" } else { "no" }, id
        ))
    } else {
        // Simple task — just mark as doing
        tool.repo.update(&storage::TodoPatch {
            id: id.to_string(),
            status: Some("doing".to_string()),
            ..Default::default()
        }).await?;
        Ok(format!("Started working on task '{}'. Marked as doing.", task.title))
    }
}
```

**Step 4: Add plan suggestion to `handle_add()`**

In `crates/feature-todo/src/tool/actions/add.rs`, after the enrichment block (around line 99), add:

```rust
// Plan suggestion for complex tasks
if let Some(config_threshold) = self.plan_complexity_threshold {
    let signals = evaluate_task_complexity(&self.repo, &created.id).await;
    if signals.is_plan_worthy(config_threshold) {
        result.push_str(&format!(
            "\n\nThis looks like a complex task (score: {}). Consider creating an execution plan for it.",
            signals.complexity_score()
        ));
    }
}
```

**Step 5: Run tests**

Run: `cargo nextest run -p feature-todo -E 'test(plan_worthy)'`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/feature-todo/
git commit -m "feat(todo): add TaskComplexitySignals, execute action, plan suggestion on create"
```

---

## Task 17: Plan Visibility Auto-Cleanup Service

**Files:**
- Create: `crates/agent/src/intent_pipeline/visibility.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs` (start cleanup task)
- Modify: `crates/storage/src/repos/plan.rs` (add `delete_stale_silent_plans()`)
- Test: inline tests

**Step 1: Add repo method for stale plan cleanup**

In `crates/storage/src/repos/plan.rs`, add:

```rust
pub async fn delete_stale_plans(&self, silent_age_hours: i64, on_failure_age_hours: i64) -> Result<u64, StorageError> {
    let result = sqlx::query(
        "DELETE FROM plans WHERE
            (visibility = 'silent' AND status IN ('completed', 'failed', 'abandoned')
             AND completed_at < datetime('now', ?1))
            OR
            (visibility = 'on_failure' AND status = 'completed'
             AND completed_at < datetime('now', ?2))"
    )
    .bind(format!("-{} hours", silent_age_hours))
    .bind(format!("-{} hours", on_failure_age_hours))
    .execute(&self.pool).await?;
    Ok(result.rows_affected())
}
```

**Step 2: Create cleanup service**

Create `crates/agent/src/intent_pipeline/visibility.rs`:

```rust
pub async fn start_plan_cleanup_service(
    plan_repo: storage::PlanRepo,
    cancel: tokio_util::sync::CancellationToken,
) {
    let mut interval = tokio::time::interval(Duration::from_secs(3600)); // hourly
    loop {
        tokio::select! {
            _ = interval.tick() => {
                let _ = plan_repo.delete_stale_plans(24, 168).await; // 24h silent, 7d on_failure
            }
            _ = cancel.cancelled() => break,
        }
    }
}
```

**Step 3: Wire in builder**

In `crates/agent/src/agent_loop/builder.rs`, after the existing background service spawns, add:

```rust
if let Some(ref plan_repo) = stored_plan_repo {
    let repo = plan_repo.clone();
    let token = cancel_token.clone();
    tokio::spawn(async move {
        intent_pipeline::visibility::start_plan_cleanup_service(repo, token).await;
    });
}
```

**Step 4: Write test**

```rust
#[tokio::test]
async fn deletes_stale_silent_plans() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    let repo = PlanRepo::new(pool.inner().clone());
    // Create a silent completed plan with old completed_at
    // ...
    let deleted = repo.delete_stale_plans(0, 0).await.unwrap(); // age=0 → delete all completed
    assert_eq!(deleted, 1);
}
```

**Step 5: Run tests**

Run: `cargo nextest run -p storage -E 'test(stale_silent)'`
Expected: PASS

**Step 6: Commit**

```bash
git add crates/agent/src/intent_pipeline/visibility.rs crates/storage/src/repos/plan.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(agent): add plan visibility auto-cleanup background service"
```

---

## Task 18: Update Dashboard API for Plan Visibility

**Files:**
- Modify: `crates/dashboard/src/api/plans.rs:75-90` (list endpoint filtering)
- Modify: `crates/dashboard/frontend/src/lib/types.ts` (add visibility field)
- Modify: `crates/dashboard/frontend/src/app/pages/Plans.tsx` (visibility badge)

**Step 1: Update list endpoint**

In `crates/dashboard/src/api/plans.rs`, update the `list_plans` handler to accept a `visibility` query param and default to excluding `silent` plans.

**Step 2: Update TypeScript types**

In `crates/dashboard/frontend/src/lib/types.ts`, add to `Plan` interface:

```typescript
visibility: string;  // "silent" | "on_failure" | "transparent"
taskId: string | null;
```

**Step 3: Add visibility badge to Plans.tsx**

Show a small badge on plan cards indicating visibility mode (only for non-transparent).

**Step 4: Run build check**

Run: `cd crates/dashboard/frontend && npm run build`
Expected: Builds successfully

**Step 5: Commit**

```bash
git add crates/dashboard/
git commit -m "feat(dashboard): add plan visibility filtering and display"
```

---

## Task 19: Update CLAUDE.md and Final Integration Test

**Files:**
- Modify: `CLAUDE.md` (update architecture section)
- Modify: `tests/orchestrator_e2e.rs` (full pipeline e2e test)

**Step 1: Update CLAUDE.md architecture description**

Replace the orchestrator/pipeline sections with the new IntentPipeline architecture. Update the "Key patterns" section. Update the "Extension traits" table.

**Step 2: Write end-to-end integration test**

```rust
#[tokio::test]
async fn intent_pipeline_routes_greeting_directly() { ... }

#[tokio::test]
async fn intent_pipeline_routes_search_as_reactive() { ... }

#[tokio::test]
async fn intent_pipeline_auto_plans_complex_request() { ... }

#[tokio::test]
async fn intent_pipeline_escalates_reactive_to_planned() { ... }
```

**Step 3: Run full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings

Run: `cargo fmt --all --check`
Expected: No formatting issues

**Step 4: Commit**

```bash
git add CLAUDE.md tests/
git commit -m "docs: update CLAUDE.md for IntentPipeline, add e2e integration tests"
```

---

## Summary

| Task | What | Estimated Complexity |
|------|------|---------------------|
| 1 | Schema migration + PlanRow update | Small |
| 2 | Plan domain type updates | Small |
| 3 | Orchestrator config section | Small |
| 4 | Core types (ExecutionMode, ComplexitySignals) | Small |
| 5 | Enhanced heuristics | Medium |
| 6 | LLM classifier | Medium |
| 7 | IntentAnalyzer (two-stage) | Medium |
| 8 | Engine trait + EscalationContext | Small |
| 9 | DirectEngine | Small |
| 10 | ReactiveEngine with escalation | Large |
| 11 | PlannedEngine (unified) | Large |
| 12 | ExecutionRouter | Medium |
| 13 | IntentPipeline (full pipeline) | Large |
| 14 | Wire into AgentLoopBuilder | Medium |
| 15 | Delete old modules, fix tests | Medium |
| 16 | Task→Plan bridge (complexity + execute action) | Medium |
| 17 | Visibility auto-cleanup service | Small |
| 18 | Dashboard API + FE updates | Small |
| 19 | CLAUDE.md + e2e tests | Small |

**Total: 19 tasks.** Tasks 1-4 are foundational types. Tasks 5-13 are the core pipeline rewrite. Tasks 14-15 are integration. Tasks 16-19 are extensions.
