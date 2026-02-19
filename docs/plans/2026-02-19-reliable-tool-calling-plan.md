# Reliable Tool Calling Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Make tool calling work reliably with all LLM models by detecting fabricated text responses and forcing a retry.

**Architecture:** Two-layer defense — (1) fabrication detector in `ExecutionCore` that recognizes when LLM returns text instead of tool calls, (2) force-retry logic in `ReactPlusEngine` that re-prompts the LLM once. Cleanup removes the now-unnecessary code guard from `TodoTool`.

**Tech Stack:** Rust, tokio, serde_json, regex (for detection heuristics)

---

### Task 1: Add `FabricatedResponse` variant to `CycleOutcome`

**Files:**
- Modify: `crates/agent/src/execution/types.rs:39-47`
- Modify: `crates/agent/src/execution/direct.rs:44-51`

**Step 1: Add the new enum variant**

In `crates/agent/src/execution/types.rs`, add `FabricatedResponse` to the `CycleOutcome` enum:

```rust
/// Outcome of a single LLM-tool cycle.
#[derive(Debug)]
pub enum CycleOutcome {
    /// LLM requested tool calls; they were executed and results appended.
    ToolsExecuted { results: Vec<ToolExecutionResult> },
    /// LLM returned a final text response (no tool calls).
    FinalResponse { content: String },
    /// LLM returned an empty response.
    EmptyResponse,
    /// LLM returned text that looks like a fabricated tool response.
    FabricatedResponse { content: String },
}
```

**Step 2: Handle the new variant in DirectEngine**

In `crates/agent/src/execution/direct.rs`, update the match in `execute()` to handle `FabricatedResponse` (treat as regular text in direct mode since direct mode has no tools):

```rust
match outcome {
    CycleOutcome::FinalResponse { content } => Ok(DirectOutcome::Response(content)),
    CycleOutcome::FabricatedResponse { content } => Ok(DirectOutcome::Response(content)),
    CycleOutcome::ToolsExecuted { .. } => {
        Ok(DirectOutcome::EscalateToToolAssisted { messages })
    }
    CycleOutcome::EmptyResponse => Ok(DirectOutcome::Response(String::new())),
}
```

**Step 3: Build to verify compilation**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: Compiler errors in `react_plus.rs` and `core.rs` about non-exhaustive match — that's correct, we'll fix those in the next tasks.

**Step 4: Commit**

```bash
git add crates/agent/src/execution/types.rs crates/agent/src/execution/direct.rs
git commit -m "feat(execution): add FabricatedResponse variant to CycleOutcome"
```

---

### Task 2: Implement `is_fabricated_tool_response()` detector

**Files:**
- Modify: `crates/agent/src/execution/core.rs:1-16` (imports)
- Modify: `crates/agent/src/execution/core.rs:122-132` (text response path)

**Step 1: Write the failing tests**

Add these tests to the `#[cfg(test)] mod tests` block at the bottom of `crates/agent/src/execution/core.rs`:

```rust
#[test]
fn test_detects_fabricated_todo_response() {
    let tool_names = vec!["todo", "calendar", "web_search"];
    let fabricated = "I've created the task for you:\n\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Description:** Weekly shopping\n- **Priority:** P3 (Medium)\n- **Due Date:** Tomorrow";
    assert!(is_fabricated_tool_response(fabricated, &tool_names));
}

#[test]
fn test_detects_fabricated_search_response() {
    let tool_names = vec!["todo", "calendar", "web_search"];
    let fabricated = "I searched the web for you and found these results:\n1. Rust programming language\n2. Rust game";
    assert!(is_fabricated_tool_response(fabricated, &tool_names));
}

#[test]
fn test_does_not_flag_normal_response() {
    let tool_names = vec!["todo", "calendar", "web_search"];
    let normal = "Sure! I'd be happy to help you create a task. What would you like the task to be about?";
    assert!(!is_fabricated_tool_response(normal, &tool_names));
}

#[test]
fn test_does_not_flag_explanation_about_tools() {
    let tool_names = vec!["todo", "calendar", "web_search"];
    let explanation = "I have access to a todo tool that can help you manage tasks. Would you like me to create one?";
    assert!(!is_fabricated_tool_response(explanation, &tool_names));
}

#[test]
fn test_detects_fabricated_with_fake_id() {
    let tool_names = vec!["todo"];
    let fabricated = "Task added! ID: a1b2c3d4. Title: Buy milk. Priority: High.";
    assert!(is_fabricated_tool_response(fabricated, &tool_names));
}

#[test]
fn test_no_tools_means_no_fabrication() {
    let tool_names: Vec<&str> = vec![];
    let text = "Task Created: Buy groceries (ID: 9c4e5f3b)";
    assert!(!is_fabricated_tool_response(text, &tool_names));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(fabricated)' 2>&1 | tail -10`
Expected: FAIL — `is_fabricated_tool_response` not found.

**Step 3: Implement `is_fabricated_tool_response()`**

Add this function above the `impl ExecutionCore` block in `crates/agent/src/execution/core.rs`:

```rust
/// Detect if a text response is actually a fabricated tool response.
///
/// Some LLMs (DeepSeek, Kimi, etc.) skip tool calls and generate text that
/// looks like a tool result. This function uses heuristics to detect that pattern.
///
/// Returns `true` if the text appears to be a fabricated tool execution.
fn is_fabricated_tool_response(text: &str, tool_names: &[&str]) -> bool {
    if tool_names.is_empty() {
        return false;
    }

    let lower = text.to_lowercase();

    // Pattern 1: Contains a fake ID pattern (hex ID like "9c4e5f3b" or "a1b2c3d4")
    let has_fake_id = {
        let mut found = false;
        // Look for "ID:" or "(ID:" followed by hex-like string
        for pattern in &["id:", "(id:"] {
            if let Some(pos) = lower.find(pattern) {
                let after = &lower[pos + pattern.len()..];
                let trimmed = after.trim_start();
                // Check if next chars are hex-like (at least 6 hex chars)
                let hex_chars = trimmed.chars().take(10).take_while(|c| c.is_ascii_hexdigit()).count();
                if hex_chars >= 6 {
                    found = true;
                    break;
                }
            }
        }
        found
    };

    // Pattern 2: Structured result patterns that indicate fabricated output
    let structured_result_indicators = [
        "task created",
        "task added",
        "i've created the task",
        "i searched the web",
        "search results:",
        "here are the results",
        "i found these results",
        "event created",
        "reminder set",
        "calendar event added",
    ];
    let has_structured_result = structured_result_indicators.iter().any(|p| lower.contains(p));

    // Pattern 3: Has multiple field-like patterns (Priority:, Due Date:, Description:, Tags:)
    let field_patterns = ["priority:", "due date:", "description:", "tags:", "estimated time:"];
    let field_count = field_patterns.iter().filter(|p| lower.contains(**p)).count();
    let has_multiple_fields = field_count >= 2;

    // Decision: fabricated if (has fake ID AND structured result) OR (structured result AND multiple fields)
    (has_fake_id && has_structured_result) || (has_structured_result && has_multiple_fields)
}
```

**Step 4: Run tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(fabricated)' 2>&1 | tail -10`
Expected: All 6 tests PASS.

**Step 5: Commit**

```bash
git add crates/agent/src/execution/core.rs
git commit -m "feat(execution): add fabrication detector for LLM text responses"
```

---

### Task 3: Wire fabrication detection into `ExecutionCore::run_cycle()`

**Files:**
- Modify: `crates/agent/src/execution/core.rs:122-132` (the text response path)

**Step 1: Write the failing test**

Add to the test module in `crates/agent/src/execution/core.rs`:

```rust
#[tokio::test]
async fn test_cycle_detects_fabricated_response() {
    // Provider returns text that looks like a fabricated todo result
    let provider = Arc::new(MockProvider {
        responses: Mutex::new(vec![LlmResponse {
            content: Some(
                "I've created the task for you:\n\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Priority:** P3\n- **Due Date:** Tomorrow".to_string()
            ),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        }]),
    });
    let registry = make_registry_with(EchoTool);
    let core = ExecutionCore::new(provider, registry);

    let mut messages = vec![Message::user("create task: buy")];
    let params = ExecutionParams::new("mock");
    let tools = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "todo",
            "description": "Manage tasks",
            "parameters": {"type": "object", "properties": {}}
        }
    })];

    let (outcome, _usage) = core
        .run_cycle(&mut messages, &tools, &params, &routing_ctx())
        .await
        .unwrap();

    assert!(matches!(outcome, CycleOutcome::FabricatedResponse { .. }));
}

#[tokio::test]
async fn test_cycle_normal_text_not_flagged() {
    let provider = MockProvider::with_text("Sure, I can help you create a task. What would you like?");
    let registry = make_registry_with(EchoTool);
    let core = ExecutionCore::new(provider, registry);

    let mut messages = vec![Message::user("create task: buy")];
    let params = ExecutionParams::new("mock");
    let tools = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "todo",
            "description": "Manage tasks",
            "parameters": {"type": "object", "properties": {}}
        }
    })];

    let (outcome, _usage) = core
        .run_cycle(&mut messages, &tools, &params, &routing_ctx())
        .await
        .unwrap();

    assert!(matches!(outcome, CycleOutcome::FinalResponse { .. }));
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(cycle_detects_fabricated)' 2>&1 | tail -10`
Expected: FAIL — returns `FinalResponse` instead of `FabricatedResponse`.

**Step 3: Wire detection into run_cycle()**

In `crates/agent/src/execution/core.rs`, replace the text response section (after `// No tool calls — check for text content`) with:

```rust
// No tool calls — check for text content
debug!("ExecutionCore: LLM returned text response (no tool calls)");
if let Some(content) = response.content {
    if !content.trim().is_empty() {
        // Extract tool names from the tool definitions for fabrication check
        let tool_names: Vec<&str> = tools
            .iter()
            .filter_map(|t| {
                t.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
            })
            .collect();

        if !tool_names.is_empty() && is_fabricated_tool_response(&content, &tool_names) {
            debug!(
                "ExecutionCore: detected fabricated tool response (tools available: {:?})",
                tool_names
            );
            return Ok((CycleOutcome::FabricatedResponse { content }, usage));
        }

        return Ok((CycleOutcome::FinalResponse { content }, usage));
    }
}

Ok((CycleOutcome::EmptyResponse, usage))
```

**Step 4: Run all core tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(test_cycle)' 2>&1 | tail -15`
Expected: All tests PASS (including existing ones — `test_cycle_final_response` still passes because "Hello world" doesn't trigger detection).

**Step 5: Commit**

```bash
git add crates/agent/src/execution/core.rs
git commit -m "feat(execution): wire fabrication detection into run_cycle"
```

---

### Task 4: Add force-retry logic in `ReactPlusEngine`

**Files:**
- Modify: `crates/agent/src/execution/react_plus.rs:84-180` (the main loop)

**Step 1: Write the failing test**

Add to the test module in `crates/agent/src/execution/react_plus.rs`:

```rust
#[tokio::test]
async fn test_fabricated_response_triggers_retry() {
    // First call: LLM returns fabricated text (no tool calls)
    // After force-retry prompt: LLM calls the tool correctly
    // Third call: LLM returns final response
    let responses = vec![
        // Iteration 1: fabricated response
        LlmResponse {
            content: Some("I've created the task:\n**Task Created:** Buy groceries (ID: 9c4e5f3b)\n- **Priority:** P3\n- **Due Date:** Tomorrow".to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        },
        // Iteration 2 (after force prompt): proper tool call
        make_tool_call_response("ok_tool"),
        // Iteration 3: final response
        make_text_response("Done! Task created successfully."),
    ];
    let provider = SequenceProvider::new(responses);
    let registry = make_registry_with_ok();
    let core = Arc::new(ExecutionCore::new(provider, registry));

    let engine = ReactPlusEngine::new(core).with_max_iterations(10);
    let messages = vec![Message::user("create task: buy")];

    // Need tool definitions so fabrication detection triggers
    let tools = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "todo",
            "description": "Manage tasks",
            "parameters": {"type": "object", "properties": {}}
        }
    })];

    let outcome = engine
        .execute(messages, &tools, &default_params(), &routing_ctx())
        .await
        .unwrap();

    match outcome {
        ReactOutcome::Response { content, iterations, .. } => {
            assert!(content.contains("Done"));
            // Should have taken 3 iterations: fabricated → retry with tool → final
            assert_eq!(iterations, 3);
        }
        other => panic!("Expected Response, got {:?}", other),
    }
}

#[tokio::test]
async fn test_fabricated_response_retry_only_once() {
    // Both attempts return fabricated text — should give up after one retry
    let responses = vec![
        // Iteration 1: fabricated
        LlmResponse {
            content: Some("Task Created: Buy groceries (ID: abcdef12)\n- Priority: P3\n- Due Date: Tomorrow".to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        },
        // Iteration 2 (after force prompt): still fabricated
        LlmResponse {
            content: Some("Task Created: Buy groceries (ID: abcdef12)\n- Priority: P3\n- Due Date: Tomorrow".to_string()),
            tool_calls: vec![],
            finish_reason: "stop".to_string(),
            usage: Usage::default(),
            reasoning_content: None,
        },
    ];
    let provider = SequenceProvider::new(responses);
    let registry = make_registry_with_ok();
    let core = Arc::new(ExecutionCore::new(provider, registry));

    let engine = ReactPlusEngine::new(core).with_max_iterations(10);
    let messages = vec![Message::user("create task: buy")];
    let tools = vec![serde_json::json!({
        "type": "function",
        "function": {
            "name": "todo",
            "description": "Manage tasks",
            "parameters": {"type": "object", "properties": {}}
        }
    })];

    let outcome = engine
        .execute(messages, &tools, &default_params(), &routing_ctx())
        .await
        .unwrap();

    // Should gracefully degrade — return the text as FinalResponse after one retry
    match outcome {
        ReactOutcome::Response { content, .. } => {
            assert!(content.contains("Task Created"));
        }
        other => panic!("Expected graceful degradation Response, got {:?}", other),
    }
}
```

**Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(fabricated_response)' 2>&1 | tail -10`
Expected: FAIL — `FabricatedResponse` variant not handled in match.

**Step 3: Implement force-retry in the ReAct+ loop**

In `crates/agent/src/execution/react_plus.rs`, update the `execute()` method. Add a `force_retried` boolean before the loop, and handle the new variant inside the loop:

Replace the loop body (from `for iteration in 1..=self.max_iterations {` through the end of the loop) with:

```rust
    pub async fn execute(
        &self,
        mut messages: Vec<Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        ctx: &RoutingContext,
    ) -> Result<ReactOutcome> {
        let mut scratchpad = Scratchpad::new();
        let escalation_threshold = (self.max_iterations as f32 * 0.8).ceil() as u32;
        let mut accumulated_usage = Usage::default();
        let mut force_retried = false;

        for iteration in 1..=self.max_iterations {
            let (outcome, cycle_usage) = self
                .core
                .run_cycle(&mut messages, tools, params, ctx)
                .await?;
            accumulate_usage(&mut accumulated_usage, &cycle_usage);

            match outcome {
                CycleOutcome::FinalResponse { content } => {
                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Received final response".to_string(),
                        planned_actions: vec![],
                        actual_action: "final_response".to_string(),
                        reflection: None,
                        timestamp: Utc::now(),
                    });

                    return Ok(ReactOutcome::Response {
                        content,
                        traces: scratchpad.traces().to_vec(),
                        iterations: iteration,
                        usage: accumulated_usage,
                    });
                }

                CycleOutcome::FabricatedResponse { content } => {
                    if force_retried {
                        // Already retried once — graceful degradation
                        debug!("ReactPlus: fabrication retry exhausted, returning text as-is");
                        scratchpad.add(ReasoningTrace {
                            cycle: iteration,
                            thought: "Fabricated response after retry — giving up".to_string(),
                            planned_actions: vec![],
                            actual_action: "fabrication_degraded".to_string(),
                            reflection: None,
                            timestamp: Utc::now(),
                        });
                        return Ok(ReactOutcome::Response {
                            content,
                            traces: scratchpad.traces().to_vec(),
                            iterations: iteration,
                            usage: accumulated_usage,
                        });
                    }

                    // First fabrication — inject force-tool-use prompt and retry
                    debug!("ReactPlus: detected fabricated response, injecting force-tool prompt");
                    force_retried = true;

                    let tool_list: Vec<&str> = tools
                        .iter()
                        .filter_map(|t| {
                            t.get("function")
                                .and_then(|f| f.get("name"))
                                .and_then(|n| n.as_str())
                        })
                        .collect();

                    messages.push(Message::user(&format!(
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
                        reflection: Some("LLM returned text instead of tool call, injecting force prompt".to_string()),
                        timestamp: Utc::now(),
                    });

                    // Continue to next iteration (the retry)
                }

                CycleOutcome::ToolsExecuted { results } => {
                    let tool_names: Vec<String> =
                        results.iter().map(|r| r.tool_name.clone()).collect();
                    let had_failure = results.iter().any(|r| !r.success);
                    let failure_details: Vec<String> = results
                        .iter()
                        .filter(|r| !r.success)
                        .map(|r| format!("{}: {}", r.tool_name, r.result))
                        .collect();

                    let mut reflection = None;

                    let should_reflect = match &self.reflection_mode {
                        ReflectionMode::OnFailure => had_failure,
                        ReflectionMode::EveryN(n) => iteration % n == 0,
                        ReflectionMode::Never => false,
                    };

                    if should_reflect {
                        let reflection_prompt = if had_failure {
                            format!(
                                "Reflection: Tool failures occurred: {}. What went wrong and how should I adjust my approach?",
                                failure_details.join("; ")
                            )
                        } else {
                            format!(
                                "Reflection (cycle {}): What progress have I made and what should I do next?",
                                iteration
                            )
                        };
                        messages.push(Message::user(&reflection_prompt));
                        reflection = Some(reflection_prompt);
                    }

                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: format!("Executed {} tool(s)", tool_names.len()),
                        planned_actions: tool_names,
                        actual_action: "tools_executed".to_string(),
                        reflection,
                        timestamp: Utc::now(),
                    });

                    if iteration >= escalation_threshold {
                        return Ok(ReactOutcome::EscalateToAutonomous {
                            reason: format!(
                                "Used {}% of max iterations ({}/{}), task may need planning",
                                (iteration * 100) / self.max_iterations,
                                iteration,
                                self.max_iterations
                            ),
                            usage: accumulated_usage,
                        });
                    }
                }

                CycleOutcome::EmptyResponse => {
                    scratchpad.add(ReasoningTrace {
                        cycle: iteration,
                        thought: "Received empty response from LLM".to_string(),
                        planned_actions: vec![],
                        actual_action: "empty_response".to_string(),
                        reflection: None,
                        timestamp: Utc::now(),
                    });
                }
            }
        }

        Ok(ReactOutcome::MaxIterationsReached {
            partial_content: None,
            usage: accumulated_usage,
        })
    }
```

Also add at the top of the file:
```rust
use tracing::debug;
```

**Step 4: Run all react_plus tests to verify they pass**

Run: `cargo nextest run -p agent -E 'test(/react_plus/)' 2>&1 | tail -15`
Expected: All tests PASS (existing + 2 new).

**Step 5: Commit**

```bash
git add crates/agent/src/execution/react_plus.rs
git commit -m "feat(execution): add force-retry on fabricated LLM responses"
```

---

### Task 5: Remove TodoTool code guard (cleanup)

**Files:**
- Modify: `crates/tools/src/todo.rs:175-196` (remove `should_guard_creation`)
- Modify: `crates/tools/src/todo.rs:415-418` (remove `confirmed` from schema)
- Modify: `crates/tools/src/todo.rs:442-469` (remove guard block in execute)
- Modify: `crates/tools/src/todo.rs:1884-1921` (remove guard tests)
- Modify: `crates/tools/src/todo.rs:26,42,52,66` (remove `creation_mode` field)
- Modify: `crates/agent/src/agent_loop.rs:240` (remove `creation_mode` argument)

**Step 1: Remove `should_guard_creation()` function**

Delete lines 170-196 in `crates/tools/src/todo.rs` (the entire `should_guard_creation` function and its doc comment).

**Step 2: Remove `confirmed` from the tool parameter schema**

Delete the `"confirmed"` property from the `parameters()` method (lines 415-418).

**Step 3: Remove the guard block from `execute()`**

In the `"add"` action handler, remove the creation guard block (lines 442-469 — from the `// Creation guard:` comment through the closing brace of the `if self.creation_mode` block). Keep the `let row = storage::TodoRow::from(&todo);` line that follows.

**Step 4: Remove `creation_mode` field from struct and constructor**

- Remove `creation_mode: config::CreationMode,` from the `TodoTool` struct (line 42)
- Remove `creation_mode: config::CreationMode,` from the `new()` parameter list (line 52)
- Remove `creation_mode,` from the `Self { ... }` initializer (line 66)
- Remove `use config::CreationMode;` import if present

**Step 5: Update `agent_loop.rs` to stop passing `creation_mode`**

In `crates/agent/src/agent_loop.rs:235-241`, change the `TodoTool::new()` call to:

```rust
let mut todo_tool = tools::todo::TodoTool::new(
    todo_repo,
    config.todo.focus.max_slots,
    config.todo.focus.deadline_hours,
    config.timezone.clone(),
);
```

**Step 6: Remove guard unit tests**

Delete the 5 test functions (lines 1884-1921):
- `test_guard_triggers_on_unconfirmed_with_2_fields`
- `test_guard_triggers_on_all_fields_unconfirmed`
- `test_guard_skips_when_confirmed`
- `test_guard_skips_when_few_optional_fields`
- `test_guard_skips_for_title_only`

**Step 7: Build and run all tests**

Run: `cargo build --workspace 2>&1 | tail -5`
Expected: Build succeeds.

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: All tests pass. The removed tests no longer run, existing tests unaffected.

**Step 8: Commit**

```bash
git add crates/tools/src/todo.rs crates/agent/src/agent_loop.rs
git commit -m "refactor(todo): remove code guard — fabrication detection handles this now"
```

---

### Task 6: Full build + manual smoke test

**Files:** None (verification only)

**Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -10`
Expected: 0 warnings.

**Step 2: Run all tests**

Run: `cargo nextest run --workspace 2>&1 | tail -10`
Expected: All pass.

**Step 3: Manual smoke test with DeepSeek**

Run: `RUST_LOG=agent=debug,tools=debug ./target/debug/klyntbot chat "create task: buy" 2>/tmp/test_debug.log`

Expected in `/tmp/test_debug.log`:
```
ExecutionCore: detected fabricated tool response
ReactPlus: detected fabricated response, injecting force-tool prompt
```

Expected user-visible behavior: either the LLM calls `ask_user`/`todo` tool on retry, OR gracefully returns text (no fake task created in DB).

**Step 4: Verify no task was hallucinated into DB**

Run inside klyntbot chat: `list tasks`
Expected: No "Buy groceries" phantom task exists.

**Step 5: Commit any final fixes**

```bash
git add -A && git commit -m "test: verify reliable tool calling with DeepSeek"
```
