# R8: Structured Chain-of-Thought Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add a structured planning phase inside the ReactiveEngine for complex tasks (complexity >= 5), with plan tracking and chat visibility.

**Architecture:** Planning is injected as a prompt via `ExecutionParams.planning_prompt` into the ReactiveEngine's existing ReAct loop. Iteration 1 becomes the "planning iteration" — the LLM generates a plan (parsed into `ExecutionPlan`) and optionally begins execution. Subsequent iterations track progress against the plan. No new execution mode — planning is a feature of Reactive mode.

**Tech Stack:** Rust, regex for plan parsing, existing Scratchpad/ReasoningTrace infrastructure.

---

### Task 1: Add PlanStep and ExecutionPlan structs to scratchpad

**Files:**
- Modify: `crates/agent/src/execution/scratchpad.rs:7-15` (extend ReasoningTrace)
- Modify: `crates/agent/src/execution/scratchpad.rs:17-21` (extend Scratchpad)
- Modify: `crates/agent/src/execution/mod.rs:8` (update re-exports)

**Step 1: Write the failing tests**

Add these tests at the end of the existing `mod tests` block in `scratchpad.rs`:

```rust
#[test]
fn plan_step_tracks_completion() {
    let mut step = PlanStep {
        index: 0,
        description: "Fetch tasks".to_string(),
        expected_tool: Some("task".to_string()),
        completed: false,
    };
    assert!(!step.completed);
    step.completed = true;
    assert!(step.completed);
}

#[test]
fn execution_plan_default_is_empty() {
    let plan = ExecutionPlan::default();
    assert!(plan.steps.is_empty());
    assert!(plan.raw_text.is_empty());
}

#[test]
fn scratchpad_plan_lifecycle() {
    let mut pad = Scratchpad::new();
    assert!(pad.plan().is_none());
    assert!(pad.plan_progress().is_none());

    let plan = ExecutionPlan {
        steps: vec![
            PlanStep {
                index: 0,
                description: "Search web".to_string(),
                expected_tool: Some("web_search".to_string()),
                completed: false,
            },
            PlanStep {
                index: 1,
                description: "Summarize results".to_string(),
                expected_tool: None,
                completed: false,
            },
            PlanStep {
                index: 2,
                description: "Create task".to_string(),
                expected_tool: Some("task".to_string()),
                completed: false,
            },
        ],
        raw_text: "1. Search web\n2. Summarize\n3. Create task".to_string(),
    };
    pad.set_plan(plan);

    assert!(pad.plan().is_some());
    assert_eq!(pad.plan_progress(), Some((0, 3)));

    pad.mark_step_completed("web_search");
    assert_eq!(pad.plan_progress(), Some((1, 3)));

    // Same tool again doesn't double-count
    pad.mark_step_completed("web_search");
    assert_eq!(pad.plan_progress(), Some((1, 3)));

    pad.mark_step_completed("task");
    assert_eq!(pad.plan_progress(), Some((2, 3)));
}

#[test]
fn reasoning_trace_has_plan_step_index() {
    let trace = ReasoningTrace {
        cycle: 1,
        thought: "test".to_string(),
        planned_actions: vec![],
        actual_action: "test".to_string(),
        reflection: None,
        timestamp: Utc::now(),
        plan_step_index: Some(0),
    };
    assert_eq!(trace.plan_step_index, Some(0));
}

#[test]
fn plan_remaining_steps() {
    let mut pad = Scratchpad::new();
    let plan = ExecutionPlan {
        steps: vec![
            PlanStep {
                index: 0,
                description: "Step A".to_string(),
                expected_tool: Some("tool_a".to_string()),
                completed: false,
            },
            PlanStep {
                index: 1,
                description: "Step B".to_string(),
                expected_tool: Some("tool_b".to_string()),
                completed: false,
            },
        ],
        raw_text: String::new(),
    };
    pad.set_plan(plan);

    pad.mark_step_completed("tool_a");
    let remaining = pad.plan_remaining();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].description, "Step B");
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(plan_step)' -E 'test(execution_plan)' -E 'test(scratchpad_plan)' -E 'test(plan_remaining)'`
Expected: FAIL — `PlanStep`, `ExecutionPlan`, `set_plan`, `mark_step_completed`, `plan_progress`, `plan_remaining`, `plan_step_index` do not exist.

**Step 3: Implement the structs and methods**

In `scratchpad.rs`, add before the `ReasoningTrace` struct (after line 4):

```rust
/// A single step in an execution plan generated before the ReAct loop.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub index: usize,
    pub description: String,
    /// Expected tool name (if parseable from LLM output). None = free-form step.
    pub expected_tool: Option<String>,
    pub completed: bool,
}

/// Structured execution plan generated in iteration 0 for complex tasks.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ExecutionPlan {
    pub steps: Vec<PlanStep>,
    /// The raw plan text from the LLM (kept for context injection).
    pub raw_text: String,
}
```

Add `plan_step_index` field to `ReasoningTrace`:

```rust
pub struct ReasoningTrace {
    pub cycle: u32,
    pub thought: String,
    pub planned_actions: Vec<String>,
    pub actual_action: String,
    pub reflection: Option<String>,
    pub timestamp: DateTime<Utc>,
    /// Which plan step this trace corresponds to (if a plan exists).
    pub plan_step_index: Option<usize>,
}
```

Add `plan` field to `Scratchpad` and implement plan methods:

```rust
pub struct Scratchpad {
    traces: Vec<ReasoningTrace>,
    plan: Option<ExecutionPlan>,
}

impl Scratchpad {
    // ... existing methods unchanged ...

    pub fn set_plan(&mut self, plan: ExecutionPlan) {
        self.plan = Some(plan);
    }

    pub fn plan(&self) -> Option<&ExecutionPlan> {
        self.plan.as_ref()
    }

    /// Mark the first uncompleted plan step that matches the given tool name.
    pub fn mark_step_completed(&mut self, tool_name: &str) {
        if let Some(ref mut plan) = self.plan {
            if let Some(step) = plan.steps.iter_mut().find(|s| {
                !s.completed && s.expected_tool.as_deref() == Some(tool_name)
            }) {
                step.completed = true;
            }
        }
    }

    /// (completed, total) plan progress. None if no plan.
    pub fn plan_progress(&self) -> Option<(usize, usize)> {
        self.plan.as_ref().map(|p| {
            let done = p.steps.iter().filter(|s| s.completed).count();
            (done, p.steps.len())
        })
    }

    /// Return uncompleted plan steps.
    pub fn plan_remaining(&self) -> Vec<&PlanStep> {
        self.plan
            .as_ref()
            .map(|p| p.steps.iter().filter(|s| !s.completed).collect())
            .unwrap_or_default()
    }
}
```

Update `Scratchpad::default()` — since we're deriving Default, change to manual impl or add `#[derive(Default)]` to the struct:

```rust
#[derive(Debug, Default)]
pub struct Scratchpad {
    traces: Vec<ReasoningTrace>,
    plan: Option<ExecutionPlan>,
}
```

Update all existing `ReasoningTrace` construction sites to include `plan_step_index: None`:
- `scratchpad.rs:87-94` (test helper `make_trace`)
- `scratchpad.rs:126-133` (test `test_summarize_with_traces`)
- `reactive.rs:101-108` (FinalResponse trace)
- `reactive.rs:144-154` (FabricatedResponse trace)
- `reactive.rs:203-210` (ToolsExecuted trace)
- `reactive.rs:214-221` (EmptyResponse trace)

Update `execution/mod.rs` re-exports (line 8):

```rust
pub use scratchpad::{ExecutionPlan, PlanStep, ReasoningTrace, Scratchpad};
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(scratchpad)' -E 'test(plan)'`
Expected: All pass, including existing tests.

**Step 5: Commit**

```bash
git add crates/agent/src/execution/scratchpad.rs crates/agent/src/execution/mod.rs crates/agent/src/intent_pipeline/engines/reactive.rs
git commit -m "feat(agent): add ExecutionPlan and PlanStep structs to Scratchpad (R8)"
```

---

### Task 2: Add planning_prompt to ExecutionParams

**Files:**
- Modify: `crates/agent/src/execution/types.rs:9-20` (add field)
- Modify: `crates/agent/src/execution/types.rs:22-57` (add builder method)

**Step 1: Write the failing test**

Add to the existing `mod tests` in `types.rs`:

```rust
#[test]
fn execution_params_with_planning_prompt() {
    let params = ExecutionParams::new("mock")
        .with_planning_prompt("Create a step-by-step plan.".to_string());
    assert_eq!(
        params.planning_prompt.as_deref(),
        Some("Create a step-by-step plan.")
    );
}

#[test]
fn execution_params_default_no_planning() {
    let params = ExecutionParams::new("mock");
    assert!(params.planning_prompt.is_none());
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(planning_prompt)'`
Expected: FAIL — `planning_prompt` field and `with_planning_prompt` method don't exist.

**Step 3: Implement**

Add field to `ExecutionParams` (after `original_message` at line 19):

```rust
pub struct ExecutionParams {
    pub tool_timeout: Duration,
    pub chat_params: ChatParams,
    pub max_iterations: u32,
    pub max_fabrication_retries: u32,
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
    pub original_message: String,
    /// Chain-of-thought planning prompt for complex tasks.
    /// When set, the reactive engine injects this before iteration 1.
    pub planning_prompt: Option<String>,
}
```

Update `new()` to include `planning_prompt: None`.

Add builder method (after `with_original_message`):

```rust
pub fn with_planning_prompt(mut self, prompt: String) -> Self {
    self.planning_prompt = Some(prompt);
    self
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(execution_params)'`
Expected: All pass.

**Step 5: Commit**

```bash
git add crates/agent/src/execution/types.rs
git commit -m "feat(agent): add planning_prompt field to ExecutionParams (R8)"
```

---

### Task 3: Add new AgentEvent variants for planning

**Files:**
- Modify: `crates/agent/src/events.rs` (add 3 new variants)

**Step 1: Write the failing test**

No test needed — these are just data types. Verify compilation in Step 3.

**Step 2: Add the event variants**

Add before the closing `}` of the `AgentEvent` enum in `events.rs`:

```rust
    /// Chain-of-thought planning has started for a complex task.
    PlanningStarted {
        #[serde(rename = "complexityScore")]
        complexity_score: u8,
    },

    /// A structured execution plan was generated.
    PlanGenerated {
        steps: Vec<String>,
        #[serde(rename = "complexityScore")]
        complexity_score: u8,
        #[serde(rename = "rawPlan")]
        raw_plan: String,
    },

    /// A plan step was completed during execution.
    PlanStepCompleted {
        #[serde(rename = "stepIndex")]
        step_index: usize,
        description: String,
        #[serde(rename = "toolName")]
        tool_name: String,
    },
```

**Step 3: Verify compilation**

Run: `cargo build -p agent`
Expected: Compiles without errors or warnings.

**Step 4: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "feat(agent): add PlanningStarted/PlanGenerated/PlanStepCompleted events (R8)"
```

---

### Task 4: Implement plan parsing and planning-aware ReactiveEngine

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs` (core logic)

This is the largest task. The ReactiveEngine needs to:
1. Detect `params.planning_prompt` and inject it before iteration 1
2. Parse the plan from the LLM's first response
3. Track plan step completion on each subsequent iteration
4. Include plan progress in the synthesis prompt

**Step 1: Write the failing tests**

Add to the existing `mod tests` block in `reactive.rs`:

```rust
#[tokio::test]
async fn planning_prompt_injected_and_plan_parsed() {
    // First response: plan text + tool call. Second response: final text.
    let provider = MockSequenceProvider::new(vec![
        LlmResponse {
            content: Some(
                "Here's my plan:\n1. Search the web [tool: ok_tool]\n2. Summarize results\n\nExecuting step 1..."
                    .to_string(),
            ),
            tool_calls: vec![providers::ToolCall {
                id: "call_1".to_string(),
                name: "ok_tool".to_string(),
                arguments: serde_json::json!({}),
            }],
            finish_reason: "tool_calls".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        },
        text_response("Here are the results."),
    ]);
    let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
    let engine = ReactiveEngine::new(core, 10);

    let params = default_params()
        .with_planning_prompt("Create a plan then execute step 1.".to_string());

    let result = engine
        .execute(
            vec![Message::user("complex task")],
            &[],
            &params,
            &routing_ctx(),
            None,
        )
        .await
        .unwrap();

    let EngineResult::Complete {
        content, traces, ..
    } = result
    else {
        panic!("expected Complete");
    };

    assert!(content.contains("results"));
    // Should have a planning trace + tool trace + final trace
    assert!(traces.len() >= 2);
    // First trace should be the planning iteration
    assert_eq!(traces[0].actual_action, "plan_and_execute");
}

#[tokio::test]
async fn planning_text_only_response_continues_loop() {
    // LLM returns plan text without tool calls on iteration 1.
    // Engine should continue to iteration 2 instead of returning early.
    let provider = MockSequenceProvider::new(vec![
        text_response("My plan:\n1. Do thing A [tool: ok_tool]\n2. Do thing B"),
        tool_call_response("ok_tool"),
        text_response("All done."),
    ]);
    let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
    let engine = ReactiveEngine::new(core, 10);

    let params = default_params()
        .with_planning_prompt("Create a plan.".to_string());

    let result = engine
        .execute(
            vec![Message::user("complex task")],
            &[],
            &params,
            &routing_ctx(),
            None,
        )
        .await
        .unwrap();

    let EngineResult::Complete {
        content,
        iterations,
        ..
    } = result
    else {
        panic!("expected Complete");
    };

    assert!(content.contains("done"));
    // Should have gone past iteration 1 (plan-only) to iterations 2-3
    assert!(iterations >= 2);
}

#[tokio::test]
async fn no_planning_prompt_behaves_normally() {
    // Without planning_prompt, engine behaves exactly as before.
    let provider = MockSequenceProvider::new(vec![
        tool_call_response("ok_tool"),
        text_response("Done!"),
    ]);
    let core = Arc::new(ExecutionCore::new(provider, registry_with_ok_tool()));
    let engine = ReactiveEngine::new(core, 10);

    let result = engine
        .execute(
            vec![Message::user("simple task")],
            &[],
            &default_params(),
            &routing_ctx(),
            None,
        )
        .await
        .unwrap();

    let EngineResult::Complete { content, .. } = result else {
        panic!("expected Complete");
    };
    assert!(content.contains("Done"));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(planning_prompt_injected)' -E 'test(planning_text_only)' -E 'test(no_planning_prompt_behaves)'`
Expected: FAIL — planning logic doesn't exist yet.

**Step 3: Implement the plan parsing function**

Add at the top of `reactive.rs` (after imports):

```rust
use once_cell::sync::Lazy;
use regex::Regex;
use crate::execution::scratchpad::{ExecutionPlan, PlanStep};

static PLAN_STEP_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^\d+\.\s+(?P<desc>.+?)(?:\s*\[tool:\s*(?P<tool>\w+)\])?\s*$").unwrap()
});

/// Parse numbered steps from LLM plan text.
fn parse_plan(text: &str) -> ExecutionPlan {
    let mut steps = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(caps) = PLAN_STEP_RE.captures(trimmed) {
            let description = caps["desc"].trim().to_string();
            let expected_tool = caps.name("tool").map(|m| m.as_str().to_string());
            steps.push(PlanStep {
                index: steps.len(),
                description,
                expected_tool,
                completed: false,
            });
        }
    }
    ExecutionPlan {
        steps,
        raw_text: text.to_string(),
    }
}
```

Check if `regex` and `once_cell` are already dependencies of the `agent` crate:

Run: `grep -E 'regex|once_cell' crates/agent/Cargo.toml`

If not present, add them. (Likely `regex` is available since it's commonly used; `once_cell` may need to be replaced with `std::sync::LazyLock` if on Rust 1.80+.)

**Step 4: Implement the planning-aware execute method**

Modify `ReactiveEngine::execute` in `reactive.rs`. The key changes are:

1. Before the loop: if `params.planning_prompt` is Some, inject it as `Message::user`.
2. Inside iteration 1: after `run_cycle`, check if we're in a planning iteration. If the response is `FinalResponse` (text-only plan), parse the plan, store it, and CONTINUE the loop instead of returning.
3. Inside `ToolsExecuted`: if a plan exists, call `scratchpad.mark_step_completed(tool_name)` and emit events.
4. At synthesis: include plan progress in the prompt.

Replace the `execute` method body with:

```rust
async fn execute(
    &self,
    messages: Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    ctx: &RoutingContext,
    event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
) -> Result<EngineResult> {
    let mut messages = messages;
    let mut scratchpad = Scratchpad::new();
    let max_iterations = if params.max_iterations > 0 {
        params.max_iterations
    } else {
        self.max_iterations
    };
    let mut accumulated_usage = providers::Usage::default();
    let mut fabrication_retries = 0u32;
    let max_fabrication_retries = params.max_fabrication_retries;
    let mut seen_tool_calls: HashSet<String> = HashSet::new();
    let mut last_tool_name: Option<String> = None;

    // Planning: inject planning prompt before iteration 1
    let is_planning = params.planning_prompt.is_some();
    if let Some(ref prompt) = params.planning_prompt {
        messages.push(Message::user(prompt));
    }

    for iteration in 1..=max_iterations {
        // Check cancellation
        if let Some(ref token) = params.cancel_token {
            if token.is_cancelled() {
                return Ok(EngineResult::Complete {
                    content: String::new(),
                    usage: accumulated_usage,
                    iterations: iteration - 1,
                    traces: scratchpad.traces().to_vec(),
                    tool_name: last_tool_name,
                });
            }
        }

        if let Some(ref tx) = event_tx {
            let _ = tx
                .send(crate::events::AgentEvent::IterationStart {
                    iteration: iteration as usize,
                    max: max_iterations as usize,
                })
                .await;
        }

        let (outcome, cycle_usage) = self
            .core
            .run_cycle(
                &mut messages,
                tools,
                params,
                ctx,
                event_tx.as_ref(),
                Some(&mut seen_tool_calls),
            )
            .await?;
        accumulate_usage(&mut accumulated_usage, &cycle_usage);

        match outcome {
            CycleOutcome::FinalResponse { content } => {
                // Planning iteration: if this is iteration 1 with a planning prompt,
                // the LLM may have returned plan text without tool calls.
                // Parse the plan and continue the loop instead of returning.
                if is_planning && iteration == 1 && scratchpad.plan().is_none() {
                    let plan = parse_plan(&content);
                    if !plan.steps.is_empty() {
                        if let Some(ref tx) = event_tx {
                            let step_descs: Vec<String> =
                                plan.steps.iter().map(|s| s.description.clone()).collect();
                            let _ = tx
                                .send(crate::events::AgentEvent::PlanGenerated {
                                    steps: step_descs,
                                    complexity_score: 0, // filled by runtime
                                    raw_plan: plan.raw_text.clone(),
                                })
                                .await;
                        }
                        scratchpad.set_plan(plan);
                    }

                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Generated execution plan".to_string(),
                        planned_actions: scratchpad
                            .plan()
                            .map(|p| p.steps.iter().map(|s| s.description.clone()).collect())
                            .unwrap_or_default(),
                        actual_action: "plan_generated".to_string(),
                        reflection: None,
                        timestamp: Utc::now(),
                        plan_step_index: None,
                    });
                    continue;
                }

                scratchpad.add(ReasoningTrace {
                    cycle: iteration,
                    thought: "Received final response".to_string(),
                    planned_actions: vec![],
                    actual_action: "final_response".to_string(),
                    reflection: None,
                    timestamp: Utc::now(),
                    plan_step_index: None,
                });

                return Ok(EngineResult::Complete {
                    content,
                    usage: accumulated_usage,
                    iterations: iteration,
                    traces: scratchpad.traces().to_vec(),
                    tool_name: last_tool_name,
                });
            }

            CycleOutcome::FabricatedResponse { content } => {
                fabrication_retries += 1;
                if fabrication_retries > max_fabrication_retries {
                    debug!(
                        "ReactiveEngine: fabrication retries exhausted ({}), returning as-is",
                        max_fabrication_retries
                    );
                    return Ok(EngineResult::Complete {
                        content,
                        usage: accumulated_usage,
                        iterations: iteration,
                        traces: scratchpad.traces().to_vec(),
                        tool_name: last_tool_name,
                    });
                }
                let tool_list: Vec<&str> = tools.iter().filter_map(tool_def_name).collect();

                messages.push(Message::user(format!(
                    "You returned a text response instead of calling a tool. \
                     You have these tools available: [{}]. \
                     You MUST call the appropriate tool to complete the user's request. \
                     Do NOT respond with text describing what you would do — actually call the tool.",
                    tool_list.join(", ")
                )));

                scratchpad.add(ReasoningTrace {
                    cycle: iteration,
                    thought: "Detected fabricated tool response — forcing retry".to_string(),
                    planned_actions: tool_list.iter().map(|s| s.to_string()).collect(),
                    actual_action: "fabrication_retry".to_string(),
                    reflection: Some(
                        "LLM returned text instead of tool call, injecting force prompt"
                            .to_string(),
                    ),
                    timestamp: Utc::now(),
                    plan_step_index: None,
                });
            }

            CycleOutcome::ToolsExecuted { results } => {
                let tool_names: Vec<String> =
                    results.iter().map(|r| r.tool_name.clone()).collect();
                if let Some(name) = tool_names.last() {
                    last_tool_name = Some(name.clone());
                }

                // Planning iteration 1: parse plan from content if not yet parsed
                if is_planning && iteration == 1 && scratchpad.plan().is_none() {
                    // The LLM's content (plan text) is in the assistant message
                    // that run_cycle appended. Extract it.
                    let plan_text = messages
                        .iter()
                        .rev()
                        .find_map(|m| match m {
                            Message::Assistant { content, .. } => content.as_deref(),
                            _ => None,
                        })
                        .unwrap_or("");
                    let plan = parse_plan(plan_text);
                    if !plan.steps.is_empty() {
                        if let Some(ref tx) = event_tx {
                            let step_descs: Vec<String> =
                                plan.steps.iter().map(|s| s.description.clone()).collect();
                            let _ = tx
                                .send(crate::events::AgentEvent::PlanGenerated {
                                    steps: step_descs,
                                    complexity_score: 0,
                                    raw_plan: plan.raw_text.clone(),
                                })
                                .await;
                        }
                        scratchpad.set_plan(plan);
                    }
                }

                // Track plan step completion
                let mut completed_step_index = None;
                for tool_name in &tool_names {
                    let before = scratchpad.plan_progress();
                    scratchpad.mark_step_completed(tool_name);
                    let after = scratchpad.plan_progress();
                    if before != after {
                        // Find which step was just completed
                        if let Some(plan) = scratchpad.plan() {
                            if let Some(step) = plan.steps.iter().find(|s| {
                                s.completed && s.expected_tool.as_deref() == Some(tool_name)
                            }) {
                                completed_step_index = Some(step.index);
                                if let Some(ref tx) = event_tx {
                                    let _ = tx
                                        .send(crate::events::AgentEvent::PlanStepCompleted {
                                            step_index: step.index,
                                            description: step.description.clone(),
                                            tool_name: tool_name.clone(),
                                        })
                                        .await;
                                }
                            }
                        }
                    }
                }

                let had_failure = results.iter().any(|r| !r.success);
                let failure_details: Vec<String> = results
                    .iter()
                    .filter(|r| !r.success)
                    .map(|r| format!("{}: {}", r.tool_name, r.result))
                    .collect();

                let all_skipped_duplicates = !results.is_empty()
                    && results
                        .iter()
                        .all(|r| !r.success && r.result.starts_with("Skipped:"));

                let mut reflection = None;

                if all_skipped_duplicates {
                    debug!(
                        "ReactiveEngine: blocked duplicate tool calls on iteration {}: {:?}",
                        iteration, tool_names
                    );
                    let dup_prompt = format!(
                        "You just attempted to call the same tool(s) with the same arguments as a previous iteration: [{}]. \
                         The calls were blocked. Do NOT repeat these calls. Either proceed with a final response or take a DIFFERENT action.",
                        tool_names.join(", ")
                    );
                    messages.push(Message::user(&dup_prompt));
                    reflection = Some(dup_prompt);
                }

                if reflection.is_none() && had_failure {
                    let reflection_prompt = format!(
                        "Reflection: Tool failures occurred: {}. What went wrong and how should I adjust my approach?",
                        failure_details.join("; ")
                    );
                    messages.push(Message::user(&reflection_prompt));
                    reflection = Some(reflection_prompt);
                }

                let actual_action = if is_planning && iteration == 1 {
                    "plan_and_execute".to_string()
                } else {
                    "tools_executed".to_string()
                };

                scratchpad.add(ReasoningTrace {
                    cycle: iteration,
                    thought: format!("Executed {} tool(s)", tool_names.len()),
                    planned_actions: tool_names,
                    actual_action,
                    reflection,
                    timestamp: Utc::now(),
                    plan_step_index: completed_step_index,
                });
            }

            CycleOutcome::EmptyResponse => {
                scratchpad.add(ReasoningTrace {
                    cycle: iteration,
                    thought: "Received empty response from LLM".to_string(),
                    planned_actions: vec![],
                    actual_action: "empty_response".to_string(),
                    reflection: None,
                    timestamp: Utc::now(),
                    plan_step_index: None,
                });
            }
        }
    }

    // Max iterations — synthesize with plan progress
    debug!(
        "ReactiveEngine: max iterations ({}) reached, synthesizing final response",
        max_iterations
    );

    let synthesis_prompt = if let Some((done, total)) = scratchpad.plan_progress() {
        let remaining: Vec<String> = scratchpad
            .plan_remaining()
            .iter()
            .map(|s| s.description.clone())
            .collect();
        format!(
            "You've used all available iterations. Plan progress: {}/{} steps completed.{} \
             Based on the work completed so far, provide a complete response to the user's \
             original request. Summarize what you accomplished and any remaining steps.",
            done,
            total,
            if remaining.is_empty() {
                String::new()
            } else {
                format!(" Remaining: {}", remaining.join(", "))
            }
        )
    } else {
        "You've used all available iterations. Based on the work completed so far, \
         provide a complete response to the user's original request. \
         Summarize what you accomplished and any remaining steps."
            .to_string()
    };

    messages.push(Message::user(&synthesis_prompt));

    let (synthesis_outcome, synthesis_usage) = self
        .core
        .run_cycle(&mut messages, &[], params, ctx, event_tx.as_ref(), None)
        .await?;
    accumulate_usage(&mut accumulated_usage, &synthesis_usage);

    let synthesis_content = match synthesis_outcome {
        CycleOutcome::FinalResponse { content } => content,
        CycleOutcome::FabricatedResponse { content } => content,
        other => {
            tracing::warn!(
                "ReactiveEngine: synthesis call produced {:?} instead of text",
                other
            );
            String::new()
        }
    };

    Ok(EngineResult::Complete {
        content: synthesis_content,
        usage: accumulated_usage,
        iterations: max_iterations,
        traces: scratchpad.traces().to_vec(),
        tool_name: last_tool_name,
    })
}
```

**Step 5: Run all reactive engine tests**

Run: `cargo nextest run -p agent -E 'test(reactive)'`
Expected: All pass — both new planning tests and existing tests.

**Step 6: Add plan parsing unit tests**

Add to the `mod tests` block:

```rust
#[test]
fn parse_plan_extracts_steps_with_tools() {
    let text = "Here's my plan:\n1. Search the web [tool: web_search]\n2. Summarize results\n3. Create a task [tool: task]";
    let plan = parse_plan(text);
    assert_eq!(plan.steps.len(), 3);
    assert_eq!(plan.steps[0].description, "Search the web");
    assert_eq!(plan.steps[0].expected_tool.as_deref(), Some("web_search"));
    assert_eq!(plan.steps[1].description, "Summarize results");
    assert!(plan.steps[1].expected_tool.is_none());
    assert_eq!(plan.steps[2].expected_tool.as_deref(), Some("task"));
    assert!(!plan.raw_text.is_empty());
}

#[test]
fn parse_plan_handles_empty_text() {
    let plan = parse_plan("No plan here, just text.");
    assert!(plan.steps.is_empty());
}

#[test]
fn parse_plan_handles_mixed_content() {
    let text = "I'll help you with that.\n\n1. First step [tool: alpha]\nSome explanation.\n2. Second step\n\nMore text.";
    let plan = parse_plan(text);
    assert_eq!(plan.steps.len(), 2);
}
```

**Step 7: Run all tests**

Run: `cargo nextest run -p agent -E 'test(parse_plan)' -E 'test(reactive)'`
Expected: All pass.

**Step 8: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/reactive.rs
git commit -m "feat(agent): implement plan-aware ReactiveEngine with CoT parsing (R8)"
```

---

### Task 5: Wire planning trigger in AgentRuntime

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs:399-428` (add Step 7c)

**Step 1: Implement the complexity gate and planning prompt builder**

After Step 7b (line 399, after `inject_delegation_tool`) and before Step 8 (line 411, `router.execute`), add:

```rust
// Step 7c: Chain-of-thought planning for complex tasks
const COT_COMPLEXITY_THRESHOLD: u8 = 5;

let planning_prompt = match analysis.mode {
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

Wire it into `ExecutionParams` construction (modify the existing block at ~line 421):

```rust
let mut params = ExecutionParams::new(&self.config.execution_model)
    .with_max_iterations(analysis.mode.max_iterations())
    .with_original_message(message.to_string());

if let Some(token) = cancel_token {
    params = params.with_cancel_token(token);
}
if let Some(prompt) = planning_prompt {
    params = params.with_planning_prompt(prompt);
}
```

Add the `build_planning_prompt` function at the bottom of `runtime.rs` (before `#[cfg(test)]`):

```rust
/// Build a chain-of-thought planning prompt for complex tasks.
fn build_planning_prompt(user_message: &str, tools: &[serde_json::Value]) -> String {
    let tool_names: Vec<&str> = tools
        .iter()
        .filter_map(common::utils::tool_def_name)
        .collect();
    format!(
        "This is a complex request. Before executing, create a step-by-step plan.\n\
         \n\
         User request: {user_message}\n\
         Available tools: [{}]\n\
         \n\
         Format each step as:\n\
         1. <description> [tool: <tool_name>]\n\
         2. <description> [tool: <tool_name>]\n\
         ...\n\
         \n\
         Keep the plan concise (3-7 steps). Then execute step 1.",
        tool_names.join(", ")
    )
}
```

**Step 2: Verify compilation and existing tests pass**

Run: `cargo build -p agent && cargo nextest run -p agent`
Expected: Compiles and all tests pass. The runtime tests use mock providers that won't trigger the complexity threshold (score defaults to 0 in heuristic analysis for simple mocks).

**Step 3: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): wire CoT planning trigger at complexity >= 5 in AgentRuntime (R8)"
```

---

### Task 6: Run full workspace build and test suite

**Step 1: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (fix any new ones).

**Step 2: Formatting**

Run: `cargo fmt --all --check`
Expected: No formatting issues (run `cargo fmt --all` to fix if needed).

**Step 3: Full test suite**

Run: `cargo nextest run --workspace`
Expected: All tests pass.

**Step 4: Commit any fixes**

```bash
git add -u
git commit -m "chore: fix clippy/fmt issues from R8 chain-of-thought"
```

---

### Task 7: Update SYSTEM_ANALYSIS.md

**Files:**
- Modify: `SYSTEM_ANALYSIS.md` (mark R8 as solved)

**Step 1: Update the R8 entry**

In section 9.3 (Medium Priority), change:

```
#### R8: Add structured chain-of-thought
```

to:

```
#### ~~R8: Add structured chain-of-thought~~ — SOLVED
**Fix:** ReactiveEngine now injects a planning prompt for tasks with complexity score >= 5. The LLM generates a structured plan (parsed into `ExecutionPlan` steps) on iteration 1, then executes against it. Plan progress is tracked in the `Scratchpad`, emitted via `AgentEvent::PlanGenerated`/`PlanStepCompleted`, and included in synthesis prompts when max iterations are reached. Advisory only — the LLM is free to deviate.
```

Also update the weakness in section 6.2 if applicable.

**Step 2: Commit**

```bash
git add SYSTEM_ANALYSIS.md
git commit -m "docs: mark R8 as solved in system analysis"
```
