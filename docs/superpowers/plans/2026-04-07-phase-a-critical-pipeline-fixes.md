# Phase A: Critical Pipeline Fixes — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the 4 broken/disconnected pipelines that prevent Klynt's cognitive system from extracting facts from conversations, compressing long tool chains, and refreshing context mid-execution — then validate all fixes produce measurable metric improvements via the simulator.

**Architecture:** `ChatTurnCompleted` gets its `user_message` field restored so the cognitive pipeline can extract facts. `MidLoopCompressor` and `LiveContextRefresher` get wired into `execute_loop` at the iteration boundary. `MemoryTool` gains a `record_fact` action that publishes `UserStatedFact` events. Simulator scenarios get updated checkpoints to validate all fixes produce metric improvements.

**Tech Stack:** Rust, SQLite, LanceDB, tokio, serde, cargo-nextest

---

### Task 1: Restore `user_message` on `ChatTurnCompleted`

**Files:**
- Modify: `crates/bus/src/domain_events.rs:274-276`
- Modify: `crates/cognitive/src/services/background.rs:738-741`
- Modify: `crates/agent/src/agent_loop/mod.rs:682-685`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs:1639,1668,1810`
- Modify: `crates/activity-log/src/normalizers.rs:115`
- Modify: `crates/app-core/src/handlers/cognitive/operations.rs:164`
- Modify: `crates/simulator/src/harness.rs:1057`
- Modify: `crates/cognitive/src/services/salience.rs:246` (test)
- Modify: `crates/cognitive/src/services/background.rs:1322` (test)
- Modify: `crates/activity-log/src/normalizers.rs:652` (test)
- Test: `crates/cognitive/src/services/background.rs` (existing tests)

- [ ] **Step 1: Add `user_message` field to `ChatTurnCompleted` variant**

In `crates/bus/src/domain_events.rs`, change:

```rust
// -- Chat --
ChatTurnCompleted {
    session_key: String,
},
```

to:

```rust
// -- Chat --
ChatTurnCompleted {
    session_key: String,
    /// The user's message content for cognitive extraction.
    /// `None` for legacy events or when content is unavailable.
    #[serde(default)]
    user_message: Option<String>,
},
```

- [ ] **Step 2: Fix all compile errors — publish sites**

In `crates/agent/src/agent_loop/mod.rs` around line 682, the agent loop has access to the original message content. Change:

```rust
bus.publish(bus::DomainEvent::ChatTurnCompleted {
    session_key: session_key.to_string(),
});
```

to:

```rust
bus.publish(bus::DomainEvent::ChatTurnCompleted {
    session_key: session_key.to_string(),
    user_message: Some(msg.content.clone()),
});
```

`msg` is the `InboundMessage` available in `process_message`. Verify the variable name by checking the function signature — it's the `msg: InboundMessage` parameter at the top of `process_message`.

In `crates/app-core/src/handlers/chat/streaming.rs`, all 3 publish sites receive `content: String` as a function parameter. At line 1639:

```rust
bus.publish(bus::DomainEvent::ChatTurnCompleted {
    session_key,
    user_message: Some(content.clone()),
});
```

Note: `content` is consumed by `chat_send` earlier in the function. If it's been moved, use `content.clone()` before the move, or capture a clone before the `chat_send` call. Check whether `content` is still available at line 1639 — if not, add `let user_content = content.clone();` before the `chat_send` call and use `user_message: Some(user_content)`.

Repeat for lines 1668 (voice) and 1810 (squad). Each function receives `content: String`.

- [ ] **Step 3: Fix all compile errors — match sites**

In `crates/activity-log/src/normalizers.rs` line 115, the destructure uses `session_key` — add `..` to ignore the new field (it likely already has `..` or you can add `user_message: _`):

```rust
bus::DomainEvent::ChatTurnCompleted { session_key, .. } => (
```

In `crates/app-core/src/handlers/cognitive/operations.rs` line 164, add the new field to the debug replay constructor:

```rust
"ChatTurnCompleted" => bus::DomainEvent::ChatTurnCompleted {
    session_key: payload["session_key"].as_str().unwrap_or("debug").to_string(),
    user_message: payload.get("user_message").and_then(|v| v.as_str()).map(|s| s.to_string()),
},
```

- [ ] **Step 4: Fix all compile errors — test sites**

Add `user_message: None` to all test constructions:

`crates/cognitive/src/services/salience.rs` line 246:
```rust
let verdict = evaluate_salience(&DomainEvent::ChatTurnCompleted {
    session_key: "test-session".into(),
    user_message: None,
});
```

`crates/cognitive/src/services/background.rs` line 1322:
```rust
let event = DomainEvent::ChatTurnCompleted {
    session_key: "session-1".into(),
    user_message: None,
};
```

`crates/activity-log/src/normalizers.rs` line 652:
```rust
let event = bus::DomainEvent::ChatTurnCompleted {
    session_key: "sk-1".into(),
    user_message: None,
};
```

`crates/simulator/src/harness.rs` line 1057:
```rust
let chat_event = DomainEvent::ChatTurnCompleted {
    session_key: "sim-session".to_string(),
    user_message: Some(msg.content.clone()),
};
```

Note: the simulator now passes message content, which enables `event_to_observation` to extract from it.

- [ ] **Step 5: Restore `event_to_observation` for `ChatTurnCompleted`**

In `crates/cognitive/src/services/background.rs`, replace lines 738-741:

```rust
// ChatTurnCompleted no longer carries the user message (payload reduction),
// so there is no content to extract facts from. Skip it.
DomainEvent::ChatTurnCompleted { .. } => None,
```

with:

```rust
DomainEvent::ChatTurnCompleted { user_message, .. } => {
    user_message.as_ref().map(|content| Observation {
        domain: "general".into(),
        content: content.clone(),
        importance: 0.5,
        source_event: "ChatTurnCompleted".into(),
        timestamp: now,
    })
}
```

This returns `None` when `user_message` is absent (backward-compatible) and creates an `Observation` when present. The `importance: 0.5` is lower than `UserStatedFact` (1.0) because chat messages are less targeted.

- [ ] **Step 6: Build and run existing tests**

```bash
cargo build --workspace 2>&1 | head -50
```

Fix any remaining compile errors from the field addition (grep for `ChatTurnCompleted {` to find any missed sites).

```bash
cargo nextest run --workspace -E 'test(cognitive) | test(salience) | test(normaliz) | test(background)' --no-capture
```

Expected: all existing tests pass. The background.rs test at line 1322 uses `user_message: None`, so it still verifies the skip-when-absent behavior.

- [ ] **Step 7: Add test for extraction from `ChatTurnCompleted` with content**

In `crates/cognitive/src/services/background.rs`, add a new test in the `#[cfg(test)] mod tests` block:

```rust
#[test]
fn event_to_observation_chat_turn_with_message() {
    let event = DomainEvent::ChatTurnCompleted {
        session_key: "session-1".into(),
        user_message: Some("I'm a software engineer working on Rust projects".into()),
    };
    let obs = event_to_observation(&event);
    assert!(obs.is_some(), "should create observation when user_message is present");
    let obs = obs.unwrap();
    assert_eq!(obs.source_event, "ChatTurnCompleted");
    assert!(obs.content.contains("software engineer"));
    assert_eq!(obs.domain, "general");
    assert!((obs.importance - 0.5).abs() < f64::EPSILON);
}

#[test]
fn event_to_observation_chat_turn_without_message() {
    let event = DomainEvent::ChatTurnCompleted {
        session_key: "session-1".into(),
        user_message: None,
    };
    let obs = event_to_observation(&event);
    assert!(obs.is_none(), "should skip when user_message is None");
}
```

- [ ] **Step 8: Run the new tests**

```bash
cargo nextest run -p cognitive -E 'test(event_to_observation_chat_turn)' --no-capture
```

Expected: both tests PASS.

- [ ] **Step 9: Commit**

```bash
git add crates/bus/src/domain_events.rs crates/agent/src/agent_loop/mod.rs crates/app-core/src/handlers/chat/streaming.rs crates/cognitive/src/services/background.rs crates/activity-log/src/normalizers.rs crates/app-core/src/handlers/cognitive/operations.rs crates/simulator/src/harness.rs crates/cognitive/src/services/salience.rs
git commit -m "fix(cognitive): restore user_message on ChatTurnCompleted for fact extraction

ChatTurnCompleted was stripped of message content for payload reduction,
which broke the entire chat-based fact extraction pipeline. Re-add
user_message as Option<String> with #[serde(default)] for backward
compatibility. event_to_observation now creates Observations when
content is present."
```

---

### Task 2: Wire MidLoopCompressor into execute_loop

**Files:**
- Modify: `crates/agent/src/execution/execute_loop.rs:9,17-19,43-50,152-191`
- Test: `crates/agent/src/execution/execute_loop.rs` (new integration-style test) or validated via existing `mid_loop_compressor.rs` tests + clippy

- [ ] **Step 1: Add import and construct compressor before the loop**

In `crates/agent/src/execution/execute_loop.rs`, add import at line 9:

```rust
use super::mid_loop_compressor::MidLoopCompressor;
```

Inside `execute_loop`, after line 56 (`let mut wrap_up_injected = false;`), add:

```rust
    let compressor = MidLoopCompressor::new(
        core.token_counter().clone(),
        params.context_window,
    );
```

- [ ] **Step 2: Wire compression at the iteration boundary**

Replace the placeholder comment block at lines 189-191:

```rust
        // ── Mid-loop compression ─────────────────────────────
        // The caller (process_message) handles compression via MidLoopCompressor
        // by wrapping this loop. We emit the event for transparency.
```

with:

```rust
        // ── Mid-loop compression ─────────────────────────────
        if let Some((before_tokens, after_tokens)) = compressor.compress_if_needed(&mut messages) {
            if let Some(ref tx) = event_tx {
                let _ = tx
                    .send(AgentEvent::ContextCompressed {
                        before_tokens,
                        after_tokens,
                    })
                    .await;
            }
        }
```

- [ ] **Step 3: Build and verify**

```bash
cargo build -p agent 2>&1 | head -20
```

Expected: clean build. The `MidLoopCompressor::compress_if_needed` takes `&mut [Message]` — `&mut messages` coerces from `&mut Vec<Message>` to `&mut [Message]` automatically.

- [ ] **Step 4: Run all agent tests**

```bash
cargo nextest run -p agent --no-capture 2>&1 | tail -20
```

Expected: all tests pass. The existing `mid_loop_compressor.rs` unit tests validate the compression logic independently. The wiring is validated by the build succeeding and matching the type signatures.

- [ ] **Step 5: Run clippy**

```bash
cargo clippy -p agent --all-targets --all-features 2>&1 | head -20
```

Expected: 0 warnings.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/execution/execute_loop.rs
git commit -m "fix(agent): wire MidLoopCompressor into execute_loop

MidLoopCompressor existed and was tested but was never called.
Now constructed before the loop and invoked at the iteration
boundary after ToolsExecuted, emitting ContextCompressed events."
```

---

### Task 3: Wire LiveContextRefresher into execute_loop

**Files:**
- Modify: `crates/agent/src/execution/execute_loop.rs` (after the compressor wiring from Task 2)

- [ ] **Step 1: Add import**

In `crates/agent/src/execution/execute_loop.rs`, add import:

```rust
use super::live_context_refresher::LiveContextRefresher;
```

- [ ] **Step 2: Construct refresher before the loop (conditional)**

After the `compressor` construction (from Task 2), add:

```rust
    let refresher = params
        .context_update_queue
        .as_ref()
        .map(|queue| LiveContextRefresher::new(core.token_counter().clone(), queue.clone()));
```

- [ ] **Step 3: Wire injection after compression**

After the mid-loop compression block (from Task 2), add:

```rust
        // ── Live context refresh ─────────────────────────────
        if !params.pause_context_updates {
            if let Some(ref refresher) = refresher {
                let updates = refresher.inject_pending(&mut messages, params.context_window);
                if !updates.is_empty() {
                    let tokens_added: usize = updates.iter().map(|u| u.tokens).sum();
                    if let Some(ref tx) = event_tx {
                        let _ = tx
                            .send(AgentEvent::ContextReassembled {
                                updates,
                                tokens_added,
                            })
                            .await;
                    }
                }
            }
        }
```

- [ ] **Step 4: Build and verify**

```bash
cargo build -p agent 2>&1 | head -20
```

Expected: clean build. `LiveContextRefresher::inject_pending` takes `&mut Vec<Message>` which matches.

- [ ] **Step 5: Run all agent tests**

```bash
cargo nextest run -p agent --no-capture 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 6: Run clippy**

```bash
cargo clippy -p agent --all-targets --all-features 2>&1 | head -20
```

Expected: 0 warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/execution/execute_loop.rs
git commit -m "fix(agent): wire LiveContextRefresher into execute_loop

LiveContextRefresher existed and was tested but the injection point
was missing. Now constructed conditionally (when context_update_queue
is present) and called after mid-loop compression, respecting the
pause_context_updates flag. Emits ContextReassembled events."
```

---

### Task 4: Add `record_fact` action to MemoryTool

**Files:**
- Modify: `crates/tools/src/domain/memory_tool.rs:18-29,97,114-155,157-172`
- Modify: `crates/agent/src/agent_loop/builder.rs:1222-1238`
- Test: `crates/tools/src/domain/memory_tool.rs` (new test)

- [ ] **Step 1: Add `DomainEventBus` field to `MemoryTool`**

In `crates/tools/src/domain/memory_tool.rs`, add the import at the top:

```rust
use bus::DomainEventBus;
```

Add the field to the struct (after line 28, before the closing `}`):

```rust
    /// Domain event bus for publishing UserStatedFact events.
    domain_bus: Option<Arc<DomainEventBus>>,
```

Add to the `new()` constructor (after `embedding_store: None,`):

```rust
            domain_bus: None,
```

Add builder method (after `with_embedding_store`):

```rust
    /// Inject domain event bus for fact recording.
    pub fn with_domain_bus(mut self, bus: Arc<DomainEventBus>) -> Self {
        self.domain_bus = Some(bus);
        self
    }
```

- [ ] **Step 2: Add `record_fact` to tool description and parameters**

Update the `description()` return (line 97):

```rust
    fn description(&self) -> &str {
        "Search past conversations and record facts about the user. Actions: search_conversations (search conversation history), search_all (unified search across todos and conversations), record_fact (remember a fact the user stated), purge (clear embeddings), status (show memory stats)."
    }
```

Update the `parameters()` JSON (line 121, the `"enum"` array):

```rust
                "action": {
                    "type": "string",
                    "enum": ["search_conversations", "search_all", "record_fact", "purge", "status"],
                    "description": "Action to perform"
                },
```

Add new parameters for `record_fact` (after the `before_date` property, before the closing `}}`):

```rust
                "fact": {
                    "type": "string",
                    "description": "A fact about the user to remember (required for record_fact). Use a clear, concise statement."
                },
                "domain": {
                    "type": "string",
                    "enum": ["identity", "energy", "work", "finance", "learning", "preferences", "general"],
                    "description": "Domain category for the fact (required for record_fact)"
                }
```

- [ ] **Step 3: Add `record_fact` match arm in `execute`**

In the `match action` block (line 163), add before the `_ =>` arm:

```rust
            "record_fact" => self.record_fact(&args).await,
```

- [ ] **Step 4: Implement `record_fact` method**

Add to the `impl MemoryTool` block (after `show_status`):

```rust
    /// Record a user-stated fact and publish to cognitive pipeline.
    async fn record_fact(&self, args: &Value) -> Result<String> {
        let fact = args
            .get("fact")
            .and_then(|v| v.as_str())
            .ok_or_else(|| common::ToolError::InvalidParams("fact required".to_string()))?;

        let domain = args
            .get("domain")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let bus = self.domain_bus.as_ref().ok_or_else(|| {
            common::ToolError::InvalidParams("Fact recording not available".to_string())
        })?;

        bus.publish(bus::DomainEvent::UserStatedFact {
            fact: fact.to_string(),
            domain: domain.to_string(),
        });

        Ok(format!("Recorded: \"{fact}\" (domain: {domain})"))
    }
```

- [ ] **Step 5: Wire `DomainEventBus` into `MemoryTool` in the builder**

In `crates/agent/src/agent_loop/builder.rs`, find the `MemoryTool` registration block (around line 1222-1238). After the existing builder chain calls (like `.with_todo_repo()`, `.with_embedding_store()`), add:

```rust
        if let Some(ref domain_bus) = self.domain_event_bus {
            memory_tool = memory_tool.with_domain_bus(Arc::clone(domain_bus));
        }
```

Make sure `memory_tool` is declared as `let mut memory_tool = MemoryTool::new()` (it may already be `mut`; if not, add `mut`).

- [ ] **Step 6: Build and verify**

```bash
cargo build --workspace 2>&1 | head -30
```

Expected: clean build.

- [ ] **Step 7: Add test for `record_fact`**

In `crates/tools/src/domain/memory_tool.rs`, add a `#[cfg(test)] mod tests` block at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn record_fact_publishes_event() {
        let bus = Arc::new(DomainEventBus::new(16));
        let mut rx = bus.subscribe();

        let tool = MemoryTool::new().with_domain_bus(bus);
        let args = serde_json::json!({
            "action": "record_fact",
            "fact": "User is a software engineer",
            "domain": "identity"
        });

        let ctx = RoutingContext::new("test".into(), "test".into());
        let result = tool.execute(args, &ctx).await.unwrap();
        assert!(result.contains("Recorded"));
        assert!(result.contains("software engineer"));

        // Verify event was published
        let event = rx.try_recv().unwrap();
        match event {
            bus::DomainEvent::UserStatedFact { fact, domain } => {
                assert_eq!(fact, "User is a software engineer");
                assert_eq!(domain, "identity");
            }
            other => panic!("Expected UserStatedFact, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn record_fact_requires_bus() {
        let tool = MemoryTool::new(); // no bus
        let args = serde_json::json!({
            "action": "record_fact",
            "fact": "some fact",
            "domain": "general"
        });
        let ctx = RoutingContext::new("test".into(), "test".into());
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn record_fact_requires_fact_param() {
        let bus = Arc::new(DomainEventBus::new(16));
        let tool = MemoryTool::new().with_domain_bus(bus);
        let args = serde_json::json!({
            "action": "record_fact",
            "domain": "general"
        });
        let ctx = RoutingContext::new("test".into(), "test".into());
        let result = tool.execute(args, &ctx).await;
        assert!(result.is_err());
    }
}
```

- [ ] **Step 8: Run the tests**

```bash
cargo nextest run -p tools -E 'test(record_fact)' --no-capture
```

Expected: all 3 tests PASS.

- [ ] **Step 9: Run clippy on tools and agent crates**

```bash
cargo clippy -p tools -p agent --all-targets --all-features 2>&1 | head -20
```

Expected: 0 warnings.

- [ ] **Step 10: Commit**

```bash
git add crates/tools/src/domain/memory_tool.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(memory): add record_fact action to MemoryTool

Adds a record_fact action that publishes UserStatedFact domain events
into the cognitive pipeline. The LLM can now explicitly record facts
about the user (name, role, preferences) which flow through
extraction → consolidation → SemanticFactRepo at importance 1.0.

DomainEventBus injected into MemoryTool via builder pattern,
matching the TaskTool/FinanceTool convention."
```

---

### Task 5: Update simulator to validate fact extraction metrics

**Files:**
- Modify: `crates/simulator/src/harness.rs:1777-1781` (already partially done in Task 1 Step 4)
- Modify: `tests/simulation/scenarios/software_engineer_1mo.toml` (add checkpoints)
- Test: `tests/simulation/smoke.rs` (run existing)

- [ ] **Step 1: Verify simulator uses `user_message` in `ChatTurnCompleted`**

This was done in Task 1 Step 4 — the simulator at line 1057 now passes `user_message: Some(msg.content.clone())`. Verify:

```bash
grep -n "user_message" crates/simulator/src/harness.rs
```

Expected: line 1057 shows `user_message: Some(msg.content.clone())`.

- [ ] **Step 2: Add fact extraction accuracy checkpoint to 1-month scenario**

Read the current `tests/simulation/scenarios/software_engineer_1mo.toml` to understand its structure:

```bash
head -80 tests/simulation/scenarios/software_engineer_1mo.toml
```

Add or update a checkpoint at day 30 that asserts fact extraction accuracy is above a meaningful threshold. Add to the TOML file:

```toml
[[checkpoints]]
at_day = 30
assertions = [
    { type = "MetricAbove", metric = "fact_extraction_accuracy", threshold = 0.3 },
    { type = "MetricAbove", metric = "knowledge_retention", threshold = 0.4 },
    { type = "MetricAbove", metric = "retrieval_precision", threshold = 0.1 },
]
```

These thresholds are intentionally conservative — they validate that the pipeline is producing *something* rather than the previous 0.0. Once baselined, they can be raised.

- [ ] **Step 3: Run the 1-month simulator**

```bash
cargo nextest run -p klyntbot -E 'test(software_engineer_1mo)' --no-capture 2>&1 | tail -40
```

Expected: the simulation completes and the fact extraction metrics are now non-zero. If the checkpoint fails, lower the threshold temporarily and investigate which step of the pipeline is still blocking.

- [ ] **Step 4: Run the full smoke test suite**

```bash
cargo nextest run -E 'test(smoke)' --no-capture 2>&1 | tail -40
```

Expected: all smoke tests pass.

- [ ] **Step 5: Commit**

```bash
git add tests/simulation/scenarios/software_engineer_1mo.toml
git commit -m "test(simulator): add fact extraction accuracy checkpoints

Validates that Phase A fixes produce measurable improvement in
fact_extraction_accuracy, knowledge_retention, and retrieval_precision.
Thresholds are conservative (0.3/0.4/0.1) as a baseline."
```

---

### Task 6: Full workspace validation

**Files:** None (validation only)

- [ ] **Step 1: Run full workspace build**

```bash
cargo build --workspace 2>&1 | tail -5
```

Expected: clean build.

- [ ] **Step 2: Run full clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | head -20
```

Expected: 0 warnings.

- [ ] **Step 3: Check formatting**

```bash
cargo fmt --all --check
```

Expected: no formatting issues.

- [ ] **Step 4: Run all workspace tests**

```bash
cargo nextest run --workspace 2>&1 | tail -20
```

Expected: all tests pass.

- [ ] **Step 5: Run doc tests**

```bash
cargo test --workspace --doc 2>&1 | tail -10
```

Expected: all doc tests pass.

- [ ] **Step 6: Commit any formatting fixes if needed, then final commit**

If any formatting was off:

```bash
cargo fmt --all
git add -A
git commit -m "style: format after Phase A changes"
```

---

## Summary of Changes

| Task | What it fixes | Key metric impact |
|------|--------------|-------------------|
| 1 | `ChatTurnCompleted` → `Observation` pipeline | `fact_extraction_accuracy` from 0 → >0.3 |
| 2 | MidLoopCompressor wired | Prevents context overflow in long tool chains |
| 3 | LiveContextRefresher wired | Newly promoted memories appear mid-execution |
| 4 | `record_fact` on MemoryTool | LLM can explicitly save facts → `knowledge_retention` ↑ |
| 5 | Simulator checkpoints | Validates all fixes produce measurable improvement |
| 6 | Full workspace validation | No regressions |

## Simulator Metrics to Watch

After Phase A, these metrics should improve from their broken baselines:

| Metric | Before (broken) | Expected after | Formula |
|--------|-----------------|---------------|---------|
| `fact_extraction_accuracy` | 0.0 | >0.3 | `facts_extracted / facts_introduced` |
| `knowledge_retention` | ~0.0 | >0.4 | `found_facts / known_facts` |
| `retrieval_precision` | ~0.0 | >0.1 | `relevant_retrieved / total_retrieved` |
| `retrieval_recall` | ~0.0 | >0.1 | `relevant_retrieved / total_relevant` |
| `personalization_score` | ~0.0 | >0.2 | `retention*0.4 + precision*0.3 + recall*0.3` |
