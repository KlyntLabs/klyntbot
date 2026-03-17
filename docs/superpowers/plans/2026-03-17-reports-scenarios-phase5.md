# Reports & Scenario Reasoning (Phase 5) Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add data-driven report generation (Weekly Review, Project Retrospective, Financial Health, Knowledge Growth) as a skill reference in the existing ReactiveEngine, and add generalized scenario reasoning ("what if…") via graph-neighborhood-driven planning prompts.

**Architecture:** Reports are NOT new engines — they're skill reference templates that the ReactiveEngine executes via its existing ReAct loop (tool calls per section, then synthesis). Scenario reasoning is a planning prompt template injected when hypothetical queries are detected at complexity ≥ 3, using `EntityRepo::get_neighborhood()` for cascading effect analysis. Both features leverage the existing `build_planning_prompt` → `ExecutionParams::with_planning_prompt` → `ReactiveEngine` pipeline.

**Tech Stack:** Rust (skill-system, agent runtime, config), Markdown (skill references), existing SQLite tools (tasks, finance, productivity, notes, cognitive graph).

**Spec reference:** `docs/superpowers/specs/2026-03-16-mirofish-integration-architecture.md` §6 (Reports as Skill) and §7 (Scenario Reasoning).

**Key dependency:** Phase 3's `TemporalService::change_summary()` already exists in `crates/cognitive/src/services/temporal.rs` — the Knowledge Growth report uses it directly.

---

## File Structure

### New files

| File | Responsibility |
|------|---------------|
| `skills/task-management/references/reports.md` | Report templates: Weekly Review Report, Project Retrospective, Knowledge Growth. Contains trigger phrases, section outlines with tool queries, and output format guidance. |
| `skills/finance-management/references/financial-health.md` | Financial Health report template. Lives in finance-management because it primarily uses finance tools and that skill already has `can_delegate_to: [task-management]` for cross-domain. |
| `crates/agent/src/agent_runtime/scenario.rs` | `build_scenario_prompt()` function — constructs the 5-step scenario reasoning template from the spec. Kept separate from `runtime.rs` for clarity. |
| `crates/config/src/schema/scenario.rs` | `ScenarioConfig` struct with `max_graph_depth: u32` (default 2). |

### Modified files

| File | Change |
|------|--------|
| `crates/skill-system/src/discovery.rs:44-64` | Add `include_skill_reference!("task-management", "reports")` and `include_skill_reference!("finance-management", "financial-health")` to `BUILTIN_SKILL_REFERENCES`. |
| `skills/task-management/SKILL.md` | Add "reports" triggers: `"weekly report"`, `"project retrospective"`, `"what did I learn"`, `"knowledge review"`. Add routing entry for reports reference. |
| `skills/finance-management/SKILL.md` | Add "financial health" triggers: `"financial health"`, `"money review"`, `"spending report"`. Add routing entry for financial-health reference. |
| `crates/agent/src/intent_pipeline/analysis.rs:497-499` | Add new branch before the existing hypothetical gate: if `has_hypothetical && has_any_domain_keyword && complexity >= 3` → return `Some(reactive_with_scenario_flag)`. |
| `crates/agent/src/agent_runtime/runtime.rs:401-440` | After the existing `build_planning_prompt` block, add scenario detection: if `analysis.signals.has_hypothetical` flag is set, substitute `build_scenario_prompt()` as the planning prompt. |
| `crates/agent/src/agent_runtime/mod.rs` | Add `mod scenario;` declaration. |
| `crates/agent/src/intent_pipeline/types.rs` | Add `has_hypothetical: bool` field to `ComplexitySignals`. `ComplexitySignals` has no `Default` derive — must add `#[derive(Default)]` (with `FailureRisk::Low` as default) or construct explicitly. |
| `crates/config/src/schema/mod.rs` | Add `pub mod scenario;` declaration. |
| `crates/config/src/schema/core.rs:91-197` | Add `pub scenario: scenario::ScenarioConfig` field to the root `Config` struct (the struct lives in `core.rs`, NOT `mod.rs`). |
| `crates/agent/src/intent_pipeline/types.rs:130-143` | Add `pub scenario_max_graph_depth: u32` field to `PipelineConfig` (the runtime uses `PipelineConfig`, not `Config` directly — there's no `Arc<RwLock<Config>>` on the runtime). |

---

## Task 1: Report Skill References (Markdown Templates)

**Files:**
- Create: `skills/task-management/references/reports.md`
- Create: `skills/finance-management/references/financial-health.md`

- [ ] **Step 1: Create the task-management reports reference**

```markdown
---
name: reports
description: Data-driven report generation — Weekly Review Report, Project Retrospective, Knowledge Growth
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-17"
  source: official
  tags: "report,review,retrospective,knowledge,summary"
  always: false
  triggers: "weekly report,project retrospective,what did I learn,knowledge review,knowledge growth,how did the project go,retrospective for"
  agent: task
---

## When to Use

- User asks for a **report** (data summary), NOT an interactive review
- Distinction: "weekly review" → interactive `weekly-review.md`; "weekly report" → this reference
- Automated cron trigger (e.g., "Every Sunday at 6pm, generate my weekly report")

## Report: Weekly Review Report

### Triggers
"weekly report", "week summary", "what happened this week"

### Sections (execute in order)

1. **Accomplishments**
   - Tool: `tasks list` with filter for completed this week
   - Present: count + top highlights

2. **In Progress**
   - Tool: `tasks list` with filter for status = in_progress
   - Present: grouped by project if possible

3. **Blockers & Overdue**
   - Tool: `tasks list` with filter for overdue or blocked
   - Present: each with days overdue and priority

4. **Patterns**
   - Tool: `productivity activity_summary` for the week
   - Tool: `productivity focus_sessions` if available
   - Present: total focused hours, peak days, trends vs last week

5. **Knowledge Growth**
   - Tool: `notes list` filtered to created/modified this week
   - If cognitive memory available: mention new facts learned count
   - Present: topics explored, notes created

6. **Next Week**
   - Tool: `tasks list` with filter for due next 7 days
   - Present: upcoming deadlines, suggested focus areas

### Output Format
Present as a clean markdown report with section headers (##).
End with a brief "Key takeaway" sentence.
Do NOT ask interactive questions — this is a passive report, not a review workflow.

---

## Report: Project Retrospective

### Triggers
"retrospective for {project}", "how did {project} go", "project retrospective"

### Sections

1. **Project Overview**
   - Tool: `project get` for the named project
   - Present: title, status, date range, completion %

2. **Task Analysis**
   - Tool: `tasks list` filtered by project
   - Present: completed vs total, overdue count, avg completion time

3. **OKR Progress** (if objectives exist)
   - Tool: `okr list` filtered by project
   - Present: each KR with current vs target

4. **Time Investment**
   - Tool: `productivity activity_summary` filtered by project tags
   - Present: total hours, focus distribution

5. **Lessons Learned**
   - Synthesize patterns from the data above
   - Present: what went well, what could improve, key decisions

### Output Format
Structured markdown. If the user doesn't name a project, list active projects and ask which one.

---

## Report: Knowledge Growth

### Triggers
"what did I learn", "knowledge review", "knowledge growth"

### Sections

1. **Notes Activity**
   - Tool: `notes list` filtered to recent period (default: 7 days)
   - Present: notes created, notes modified, word count growth

2. **Topics Explored**
   - Analyze note titles and tags for theme clustering
   - Present: main topics with note counts

3. **Memory Evolution**
   - If TemporalService available: use `change_summary` for the period
   - Present: new facts learned, facts updated, contradictions resolved

4. **Flashcard Performance** (if available)
   - Query flashcard review stats for the period
   - Present: cards reviewed, accuracy rate, mastery progress

### Output Format
Markdown report. Focus on growth narrative, not raw numbers.
```

- [ ] **Step 2: Create the finance-management financial-health reference**

```markdown
---
name: financial-health
description: Comprehensive financial health report — net worth, spending analysis, budgets, goals, and FIRE progress
license: MIT
metadata:
  author: klyntbot
  version: "1.0.0"
  updated-on: "2026-03-17"
  source: official
  tags: "finance,report,health,spending,budget,net-worth"
  always: false
  triggers: "financial health,money review,spending report,financial report,how's my money,financial summary"
  agent: finance
---

## When to Use

- User asks for a financial overview or health check
- Automated cron trigger (e.g., "Every month, generate my financial health report")

## Report: Financial Health

### Sections (execute in order)

1. **Net Worth Snapshot**
   - Tool: `finance net_worth`
   - Present: total net worth, breakdown by account type, change vs last period

2. **Spending Analysis**
   - Tool: `finance report_spending` for the period (default: last 30 days)
   - Present: total spend, top categories, comparison to budget

3. **Income Summary**
   - Tool: `finance report_income` for the period
   - Present: total income, sources, trend

4. **Budget Status**
   - Tool: `finance budget_status`
   - Present: each budget with used/remaining, over-budget alerts

5. **Goals Progress**
   - Tool: `finance goal_list`
   - Present: each goal with progress %, projected completion date

6. **FIRE Progress** (if user has FIRE goals configured)
   - Tool: `finance fire_status` if available
   - Present: FIRE number, current progress %, projected date

### Output Format
Structured markdown report with section headers.
Include specific dollar amounts and percentages.
End with 2-3 actionable recommendations based on the data.
Do NOT ask interactive questions — this is a passive report.
```

- [ ] **Step 3: Verify the reference files parse correctly**

Run:
```bash
# Check YAML frontmatter is valid by attempting to parse
head -14 skills/task-management/references/reports.md
head -14 skills/finance-management/references/financial-health.md
```
Expected: Valid YAML frontmatter with `name`, `description`, `triggers` fields.

- [ ] **Step 4: Commit**

```bash
git add skills/task-management/references/reports.md skills/finance-management/references/financial-health.md
git commit -m "feat(skills): add report templates for weekly, retrospective, financial health, and knowledge growth"
```

---

## Task 2: Register References in Skill System

**Files:**
- Modify: `crates/skill-system/src/discovery.rs:44-64`

- [ ] **Step 1: Write the failing test**

Add a test to `crates/skill-system/src/discovery.rs` (in the existing `mod tests` block):

```rust
#[test]
fn test_builtin_references_include_reports() {
    let ref_map = builtin_reference_map();
    assert!(
        ref_map.contains_key("builtin::task-management/references/reports.md"),
        "reports reference should be registered"
    );
    assert!(
        ref_map.contains_key("builtin::finance-management/references/financial-health.md"),
        "financial-health reference should be registered"
    );
}
```

- [ ] **Step 2: Add the references to BUILTIN_SKILL_REFERENCES (must come before test can compile)**

**Note:** The `include_skill_reference!` macro uses `include_str!` at compile time — the markdown files from Task 1 MUST exist on disk before this step. That's why Task 1 creates them first. The TDD "write test, see it fail" flow is blocked here because the test won't compile until both the files exist AND the references are registered. Instead: add references first, then add the test to lock them in.

- [ ] **Step 3: Add the test (now it compiles and passes)**

In `crates/skill-system/src/discovery.rs`, the references were added in Step 2 (after `retrospective` ~line 55 and after `portfolio-analysis` ~line 60):

```rust
    include_skill_reference!("task-management", "reports"),
    // ... and ...
    include_skill_reference!("finance-management", "financial-health"),
```

Now add the test to lock them in (in the `mod tests` block):

```rust
#[test]
fn test_builtin_references_include_reports() {
    let ref_map = builtin_reference_map();
    assert!(ref_map.contains_key("builtin::task-management/references/reports.md"));
    assert!(ref_map.contains_key("builtin::finance-management/references/financial-health.md"));
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `cargo nextest run -p skill-system -E 'test(builtin_references_include_reports)'`
Expected: PASS.

- [ ] **Step 5: Run full skill-system tests**

Run: `cargo nextest run -p skill-system`
Expected: All tests pass (including existing `test_builtin_source_produces_skills`, `test_catalog_prompt_xml`, etc.).

- [ ] **Step 6: Commit**

```bash
git add crates/skill-system/src/discovery.rs
git commit -m "feat(skill-system): register reports and financial-health references"
```

---

## Task 3: Update SKILL.md Triggers and Routing

**Files:**
- Modify: `skills/task-management/SKILL.md`
- Modify: `skills/finance-management/SKILL.md`

- [ ] **Step 1: Add report triggers to task-management SKILL.md**

Add these lines to the `triggers:` list (after `"weekly review"` around line 31):

```yaml
      - weekly report
      - week summary
      - what happened this week
      - project retrospective
      - how did the project go
      - what did I learn
      - knowledge review
      - knowledge growth
```

Add a routing entry to the `## Routing by Request Type` table:

```markdown
| "weekly report" / "week summary" | Data-driven report (references/reports.md) |
| "project retrospective" / "how did X go" | Project retro report (references/reports.md) |
| "what did I learn" / "knowledge growth" | Knowledge growth report (references/reports.md) |
```

Add the distinction note near the existing `weekly-review` routing:

```markdown
> **Report vs Review:** "weekly review" → interactive GTD workflow (weekly-review.md). "weekly report" → passive data summary (reports.md). If ambiguous, ask the user.
```

- [ ] **Step 2: Add financial-health triggers to finance-management SKILL.md**

Add these lines to the `triggers:` list:

```yaml
      - financial health
      - money review
      - spending report
      - financial summary
```

Add a routing entry to the decision flowchart or routing table:

```markdown
| "financial health" / "money review" | Financial health report (references/financial-health.md) |
```

- [ ] **Step 3: Run skill-system tests to verify YAML still parses**

Run: `cargo nextest run -p skill-system`
Expected: All tests pass. The YAML frontmatter changes don't break parsing.

- [ ] **Step 4: Commit**

```bash
git add skills/task-management/SKILL.md skills/finance-management/SKILL.md
git commit -m "feat(skills): add report trigger phrases and routing to skill definitions"
```

---

## Task 4: Scenario Config Schema

**Files:**
- Create: `crates/config/src/schema/scenario.rs`
- Modify: `crates/config/src/schema/mod.rs` (or wherever the root Config struct lives)

- [ ] **Step 1: Find the root Config struct and its schema module**

Run: `grep -n "pub struct Config" crates/config/src/schema.rs crates/config/src/schema/mod.rs crates/config/src/lib.rs 2>/dev/null`

This tells us exactly where to add the `scenario` field.

- [ ] **Step 2: Create scenario.rs config schema**

Create `crates/config/src/schema/scenario.rs`:

```rust
use serde::{Deserialize, Serialize};

/// Configuration for generalized scenario/what-if reasoning.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioConfig {
    /// Maximum graph neighborhood depth for cascading effect analysis.
    /// Higher depth = more entities explored but slower queries.
    #[serde(default = "default_max_graph_depth")]
    pub max_graph_depth: u32,
}

fn default_max_graph_depth() -> u32 {
    2
}

impl Default for ScenarioConfig {
    fn default() -> Self {
        Self {
            max_graph_depth: default_max_graph_depth(),
        }
    }
}
```

- [ ] **Step 3: Wire into root Config**

Two files to edit:

1. In `crates/config/src/schema/mod.rs`, add the module declaration:
```rust
pub mod scenario;
```

2. In `crates/config/src/schema/core.rs`, add to the `Config` struct (after the existing `launcher` field around line 192):
```rust
    #[serde(default)]
    pub scenario: scenario::ScenarioConfig,
```

And add the import at the top of `core.rs` if needed (or use the fully qualified path `scenario::ScenarioConfig`).

- [ ] **Step 4: Verify it compiles**

Run: `cargo check -p config`
Expected: Clean compilation. The `#[serde(default)]` ensures existing configs without `scenario` still deserialize.

- [ ] **Step 5: Write a deserialization test**

Add to `crates/config/src/schema/scenario.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = ScenarioConfig::default();
        assert_eq!(config.max_graph_depth, 2);
    }

    #[test]
    fn test_deserialize_empty_object() {
        let json = "{}";
        let config: ScenarioConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_graph_depth, 2);
    }

    #[test]
    fn test_deserialize_custom_depth() {
        let json = r#"{"maxGraphDepth": 3}"#;
        let config: ScenarioConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.max_graph_depth, 3);
    }
}
```

- [ ] **Step 6: Run tests**

Run: `cargo nextest run -p config`
Expected: All tests pass.

- [ ] **Step 7: Commit**

```bash
git add crates/config/src/schema/scenario.rs crates/config/src/schema/mod.rs
git commit -m "feat(config): add scenario config with max_graph_depth"
```

---

## Task 5: Add `has_hypothetical` Signal to Intent Analysis

**Files:**
- Modify: `crates/agent/src/intent_pipeline/types.rs`
- Modify: `crates/agent/src/intent_pipeline/analysis.rs:497-499`

- [ ] **Step 1: Find the ComplexitySignals struct**

Run: `grep -n "pub struct ComplexitySignals" crates/agent/src/intent_pipeline/types.rs`

- [ ] **Step 2: Write the failing test**

Add to `crates/agent/src/intent_pipeline/analysis.rs` tests:

```rust
#[test]
fn test_hypothetical_with_domain_keyword_returns_reactive() {
    // "what if I deprioritize the API migration" has hypothetical + task domain keywords
    let result = analyze_heuristic("what if I deprioritize the API migration project");
    assert!(result.is_some(), "hypothetical + domain should return Some");
    let analysis = result.unwrap();
    assert!(
        matches!(analysis.mode, ExecutionMode::Reactive { .. }),
        "should be Reactive mode"
    );
    assert!(
        analysis.signals.has_hypothetical,
        "has_hypothetical signal should be set"
    );
}

#[test]
fn test_simple_hypothetical_without_domain_defers_to_llm() {
    // "what if it rains tomorrow" — hypothetical but no domain keywords
    let result = analyze_heuristic("what if it rains tomorrow");
    assert!(result.is_none(), "hypothetical without domain should defer to LLM");
}
```

- [ ] **Step 3: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(hypothetical)'`
Expected: FAIL — `has_hypothetical` field doesn't exist on `ComplexitySignals` yet.

- [ ] **Step 4: Add `has_hypothetical` to ComplexitySignals**

In `crates/agent/src/intent_pipeline/types.rs`, add to `ComplexitySignals` (after `requires_retries` at line 74):

```rust
    /// Whether the message contains hypothetical framing ("what if", "suppose", etc.)
    pub has_hypothetical: bool,
```

**Important:** `ComplexitySignals` has no `Default` derive. Every construction site fills in all fields explicitly. You must add `has_hypothetical: false` to every existing construction site in `analysis.rs` (search for `ComplexitySignals {`). There are ~5-6 sites: `direct_analysis()`, `reactive_analysis()`, the various domain-match constructors, and `IntentAnalysis::fallback()`.

- [ ] **Step 5: Modify analyze_heuristic to set the signal and return Reactive**

In `crates/agent/src/intent_pipeline/analysis.rs`, replace the block at ~line 497-499:

```rust
    // If negated/hypothetical and has domain content, defer to LLM.
    if (has_negation || has_hypothetical) && has_any_domain_keyword(&msg, m) {
        return None;
    }
```

With:

```rust
    // Hypothetical + domain content → scenario reasoning (Reactive mode with flag).
    // This lets the runtime inject the scenario planning prompt.
    if has_hypothetical && has_any_domain_keyword(&msg, m) {
        return Some(IntentAnalysis {
            mode: ExecutionMode::Reactive { max_iterations: 8 },
            confidence: 0.80,
            source: AnalysisSource::Heuristic,
            reasoning: "Hypothetical scenario with domain context".to_string(),
            needs_orchestration: false,
            signals: ComplexitySignals {
                estimated_tool_calls: 3,
                has_sequential_deps: true,
                failure_risk: FailureRisk::Low,
                requires_state_tracking: false,
                requires_retries: false,
                has_hypothetical: true,
            },
        });
    }

    // Negation with domain content still defers to LLM (can't parse "don't create" correctly).
    if has_negation && has_any_domain_keyword(&msg, m) {
        return None;
    }
```

**Note:** `IntentAnalysis` requires `source: AnalysisSource` and `needs_orchestration: bool` fields — the old snippet was missing these. `ComplexitySignals` must be constructed with all fields explicitly (no `Default` impl).

- [ ] **Step 6: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(hypothetical)'`
Expected: PASS.

- [ ] **Step 7: Run full agent tests for regressions**

Run: `cargo nextest run -p agent`
Expected: All existing tests pass. The key change is that hypothetical+domain queries now return `Some(Reactive)` instead of `None` (LLM fallback). Any test that relied on the old behavior needs updating — check the test output and fix.

- [ ] **Step 8: Commit**

```bash
git add crates/agent/src/intent_pipeline/types.rs crates/agent/src/intent_pipeline/analysis.rs
git commit -m "feat(agent): detect hypothetical queries and route to Reactive with scenario flag"
```

---

## Task 6: Scenario Prompt Builder

**Files:**
- Create: `crates/agent/src/agent_runtime/scenario.rs`
- Modify: `crates/agent/src/agent_runtime/mod.rs`

- [ ] **Step 1: Write the test**

Create `crates/agent/src/agent_runtime/scenario.rs`:

```rust
/// Build the scenario reasoning planning prompt.
///
/// Injected when the intent analyzer detects hypothetical framing
/// (e.g., "what if I deprioritize Project X?"). The prompt guides the
/// ReAct engine through a 5-step reasoning process using graph neighborhoods.
pub fn build_scenario_prompt(
    user_message: &str,
    tools: &[serde_json::Value],
    max_graph_depth: u32,
) -> String {
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(|t| t["function"]["name"].as_str())
        .collect();

    format!(
        "The user is exploring a scenario. Structure your response using these steps:\n\
         \n\
         1. **Identify the change variable and current baseline** — use tools to get real data \
            about the current state of what the user wants to change.\n\
         2. **Trace first-order effects** — what changes directly as a result?\n\
         3. **Trace second-order effects** — use knowledge graph neighborhoods (max depth {max_graph_depth}) \
            to find connected entities. For each connected entity, query the relevant tool.\n\
         4. **Present best/worst/likely outcomes** with specific numbers where possible.\n\
         5. **Synthesize a recommendation** — what should the user do?\n\
         \n\
         User scenario: {user_message}\n\
         Available tools: [{tool_list}]\n\
         \n\
         Start by identifying the change variable and querying for baseline data.",
        max_graph_depth = max_graph_depth,
        user_message = user_message,
        tool_list = tool_names.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_prompt_contains_key_elements() {
        let tools = vec![serde_json::json!({
            "function": { "name": "tasks" }
        })];
        let prompt = build_scenario_prompt("what if I delay the launch", &tools, 2);

        assert!(prompt.contains("what if I delay the launch"));
        assert!(prompt.contains("tasks"));
        assert!(prompt.contains("max depth 2"));
        assert!(prompt.contains("first-order effects"));
        assert!(prompt.contains("second-order effects"));
        assert!(prompt.contains("recommendation"));
    }

    #[test]
    fn test_scenario_prompt_respects_depth() {
        let tools = vec![];
        let prompt = build_scenario_prompt("test", &tools, 3);
        assert!(prompt.contains("max depth 3"));
    }
}
```

- [ ] **Step 2: Add module declaration**

In `crates/agent/src/agent_runtime/mod.rs`, add:

```rust
pub mod scenario;
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p agent -E 'test(scenario_prompt)'`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_runtime/scenario.rs crates/agent/src/agent_runtime/mod.rs
git commit -m "feat(agent): add scenario reasoning prompt builder"
```

---

## Task 7: Wire Scenario Prompt into Runtime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:401-440`

- [ ] **Step 1: Add `scenario_max_graph_depth` to PipelineConfig**

The runtime's `self.config` is `PipelineConfig` (a plain value struct at `types.rs:130`), NOT `Arc<RwLock<Config>>`. It has no `scenario` field. Add the field to `PipelineConfig`:

In `crates/agent/src/intent_pipeline/types.rs`, add to `PipelineConfig`:
```rust
    /// Max graph depth for scenario reasoning (from Config.scenario.max_graph_depth).
    pub scenario_max_graph_depth: u32,
```

And in the `Default` impl for `PipelineConfig`, default to `2`.

Then in `app-core` where `PipelineConfig` is constructed from `Config`, wire it:
```rust
scenario_max_graph_depth: config.scenario.max_graph_depth,
```

Find this construction site with: `grep -rn "PipelineConfig" crates/app-core/`

- [ ] **Step 2: Write the test**

Add to the existing runtime tests in `crates/agent/src/agent_runtime/runtime.rs`:

```rust
#[test]
fn test_scenario_prompt_built_for_hypothetical_signals() {
    let tools = vec![serde_json::json!({
        "function": { "name": "tasks" }
    })];

    // Verify build_scenario_prompt produces the expected format
    let prompt = super::scenario::build_scenario_prompt("what if I quit", &tools, 2);
    assert!(prompt.contains("scenario"));
    assert!(prompt.contains("what if I quit"));
    assert!(prompt.contains("max depth 2"));
}
```

- [ ] **Step 3: Modify the planning prompt block in runtime.rs**

In `crates/agent/src/agent_runtime/runtime.rs`, modify the planning prompt logic at ~line 405-420. The current code:

```rust
let planning_prompt = match analysis.mode {
    crate::intent_pipeline::types::ExecutionMode::Reactive { .. }
        if analysis.signals.complexity_score() >= COT_COMPLEXITY_THRESHOLD =>
    {
        let prompt = build_planning_prompt(message, &filtered_tools);
        // ... event emission ...
        Some(prompt)
    }
    _ => None,
};
```

Replace with:

```rust
let planning_prompt = match analysis.mode {
    crate::intent_pipeline::types::ExecutionMode::Reactive { .. }
        if analysis.signals.has_hypothetical =>
    {
        // Scenario reasoning — use specialized prompt
        let prompt = scenario::build_scenario_prompt(
            message,
            &filtered_tools,
            self.config.scenario_max_graph_depth,
        );
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::PlanningStarted {
                    complexity_score: analysis.signals.complexity_score(),
                })
                .await;
        }
        Some(prompt)
    }
    crate::intent_pipeline::types::ExecutionMode::Reactive { .. }
        if analysis.signals.complexity_score() >= COT_COMPLEXITY_THRESHOLD =>
    {
        let prompt = build_planning_prompt(message, &filtered_tools);
        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(AgentEvent::PlanningStarted {
                    complexity_score: analysis.signals.complexity_score(),
                })
                .await;
        }
        Some(prompt)
    }
    _ => None,
};
```

**Important:** The scenario branch must come FIRST because hypothetical queries should always use the scenario prompt, even if their complexity score is below `COT_COMPLEXITY_THRESHOLD`. The config access is now a plain field read (`self.config.scenario_max_graph_depth`), not an async lock.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent`
Expected: All tests pass.

- [ ] **Step 5: Run clippy**

Run: `cargo clippy -p agent --all-targets --all-features`
Expected: No new warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): wire scenario prompt into runtime for hypothetical queries"
```

---

## Task 8: Integration Test — Report End-to-End

**Files:**
- Modify: existing agent test file or create a new integration test

- [ ] **Step 1: Verify skill routing picks up report triggers**

Add a test to `crates/skill-system/src/router.rs` (or existing router tests):

```rust
#[test]
fn test_report_triggers_route_to_task_management() {
    let builtin: Vec<(String, String)> = BUILTIN_SKILLS
        .iter()
        .map(|(n, c)| (n.to_string(), c.to_string()))
        .collect();
    let source = SkillSource::BuiltIn(builtin);
    let catalog = SkillCatalog::discover_sync(&[source]).unwrap();
    // SkillRouter::new takes &SkillCatalog to pre-tokenize descriptions
    let router = SkillRouter::new(&catalog);

    let pkg = router.select_orchestrator("weekly report", &catalog);
    assert_eq!(pkg.name, "task-management", "weekly report should route to task-management");

    let pkg = router.select_orchestrator("financial health", &catalog);
    assert_eq!(pkg.name, "finance-management", "financial health should route to finance-management");
}
```

- [ ] **Step 2: Verify hypothetical routing in intent analysis**

Add to `crates/agent/src/intent_pipeline/analysis.rs` tests:

```rust
#[test]
fn test_what_if_budget_routes_reactive() {
    let result = analyze_heuristic("what if I increase my savings by 500 per month");
    assert!(result.is_some());
    let analysis = result.unwrap();
    assert!(analysis.signals.has_hypothetical);
    assert!(matches!(analysis.mode, ExecutionMode::Reactive { .. }));
}

#[test]
fn test_what_if_task_routes_reactive() {
    let result = analyze_heuristic("what if I push the deadline back 2 weeks for the migration task");
    assert!(result.is_some());
    let analysis = result.unwrap();
    assert!(analysis.signals.has_hypothetical);
}
```

- [ ] **Step 3: Run all tests**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 4: Run clippy on full workspace**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings (per project convention).

- [ ] **Step 5: Commit**

```bash
git add -A
git commit -m "test(agent): add integration tests for report routing and scenario detection"
```

---

## Task 9: Final Verification

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: Clean build.

- [ ] **Step 2: Full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

- [ ] **Step 3: Doctest check**

Run: `cargo test --workspace --doc`
Expected: Pass.

- [ ] **Step 4: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: Zero warnings.

- [ ] **Step 5: Format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

---

## Cron Integration (No Code Changes Needed)

The existing cron system already handles report automation end-to-end:

1. User says: "Every Sunday at 6pm, generate my weekly report and send to Telegram"
2. The `automation` orchestrator skill creates a cron job via `CronService::add_job()` with `message: "generate my weekly report"` and `deliver: true`
3. When the timer fires, `execute_job_static()` calls the `on_job` callback which routes through `AgentRuntime::run("generate my weekly report")`
4. The `SkillRouter` matches "weekly report" → `task-management` skill → loads `reports.md` reference
5. The ReactiveEngine executes tool calls per section, synthesizes the report
6. The result is delivered to the configured channel (Telegram)

**No cron code changes needed.** The existing infrastructure handles this automatically once the skill references and triggers are in place.

---

## What's NOT in This Plan

- **No new `ReportEngine`** — reports execute through existing ReactiveEngine
- **No new streaming infrastructure** — PipelineEvent SSE already shows tool results incrementally
- **No graph tool creation** — scenario reasoning uses existing tools (tasks, finance, productivity) to query entities found via graph neighborhoods. The graph neighborhood query happens within the LLM's tool calls to `memory` or entity-related tools
- **No UI changes** — reports and scenarios are conversational features delivered through chat
