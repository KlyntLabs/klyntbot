# Infrastructure Gaps Fix — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix the `ChatTurnCompleted` timing issue (fires before LLM response is ready) and remove dead per-skill budget code that can never execute.

**Architecture:** Gap 1 moves `ChatTurnCompleted` publishing from `AppCore::chat_send` (which fires before streaming completes) into `relay_chat_stream` (which fires after `AgentEvent::Done`, when the assistant response is saved). Gap 2 removes the unreachable `skill_budget_for` function and simplifies `ExecutionBudget::new` to take only `DepthMode`.

**Tech Stack:** Rust, tokio, cargo-nextest

---

## File Structure

| File | Action | Responsibility |
|------|--------|---------------|
| `crates/app-core/src/handlers/chat/streaming.rs` | Modify | Move `ChatTurnCompleted` from `chat_send`/`chat_send_voice`/`chat_send_squad` into `relay_chat_stream` |
| `crates/agent/src/execution/budget.rs` | Modify | Remove `skill_budget_for`, simplify `ExecutionBudget::new` |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Update `ExecutionBudget::new` call |
| `crates/config/src/schema/execution.rs` | Modify | Remove `SkillBudgetOverride` and `skill_budgets` field |

---

### Task 1: Move `ChatTurnCompleted` to `relay_chat_stream`

The problem: `ChatTurnCompleted` is published in `AppCore::chat_send` (line 1639), `chat_send_voice` (line 1673), and `chat_send_squad` (line 1817) — all BEFORE the streaming background task starts. The cognitive pipeline receives the event and tries to load session history, but the assistant response hasn't been saved yet.

The fix: remove the publish from all three `AppCore` methods and instead publish inside `relay_chat_stream` when `AgentEvent::Done` fires — at that point the assistant response has been saved to the session.

**Files:**
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

- [ ] **Step 1: Add `domain_event_bus` and `user_message` parameters to `relay_chat_stream`**

In `crates/app-core/src/handlers/chat/streaming.rs`, find the `relay_chat_stream` function signature (line 931). Add two new parameters after `journey_tracker`:

```rust
pub async fn relay_chat_stream(
    repos: Repos,
    session_key: String,
    active_streams: Arc<ActiveStreams>,
    pending_interactions: Arc<PendingInteractions>,
    mut event_rx: mpsc::Receiver<AgentEvent>,
    mut interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    emitter: Arc<dyn crate::events::AppEventEmitter>,
    has_context: bool,
    journey_tracker: Option<crate::journey::JourneyTracker>,
    domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    user_message: Option<String>,
) {
```

- [ ] **Step 2: Publish `ChatTurnCompleted` inside the `AgentEvent::Done` handler**

Inside `relay_chat_stream`, find the `AgentEvent::Done { content, message_id }` arm (around line 1113). After the metadata persist block and before the journey milestone and `AGENT_DONE` emit, add:

```rust
                        // Publish ChatTurnCompleted AFTER response is saved to session.
                        // This ensures the cognitive pipeline can load the full conversation
                        // (including the assistant response) when it processes the event.
                        if let Some(ref bus) = domain_event_bus {
                            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                                session_key: sk.to_string(),
                                user_message: user_message.clone(),
                            });
                        }
```

Place this right before the existing `// Wire: FirstChatResponse journey milestone` comment (around line 1158).

- [ ] **Step 3: Remove `ChatTurnCompleted` from `AppCore::chat_send`**

Find the publish block at lines 1638-1644:

```rust
        // Publish chat turn to cognitive consolidation pipeline
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                session_key,
                user_message: Some(user_content),
            });
        }
```

Delete this entire block. The `user_content` variable (line 1623) is still needed if it's used elsewhere — check. If `user_content` is only used for the bus publish, remove the `let user_content = content.clone();` line too.

- [ ] **Step 4: Remove `ChatTurnCompleted` from `chat_send_voice`**

Find the publish block at lines 1672-1677 (approximately). Delete the same pattern:

```rust
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                session_key,
                user_message: Some(user_content),
            });
        }
```

Also remove the `let user_content = content.clone();` if it was only used for this publish.

- [ ] **Step 5: Remove `ChatTurnCompleted` from `chat_send_squad`**

Find the publish block at lines 1816-1822 (approximately). Delete the same pattern.

- [ ] **Step 6: Update all call sites of `relay_chat_stream`**

`relay_chat_stream` is called from `AppCore::spawn_chat_relay` or directly. Search:

```bash
grep -rn "relay_chat_stream" crates/app-core/src/
```

At each call site, pass the two new parameters:
- `domain_event_bus`: `self.domain_event_bus.clone()`
- `user_message`: `Some(user_content)` where `user_content` is the cloned user message

There are likely 1-2 call sites (one in `spawn_chat_relay`, possibly one in the dev server). Check each and update.

The `user_content` clone needs to happen in the caller before `content` is moved. The pattern is `let user_content = content.clone();` before the `chat_send(...)` call, then pass `Some(user_content)` to `relay_chat_stream`.

- [ ] **Step 7: Build and test**

```bash
cargo build -p app-core -p agent 2>&1 | tail -10
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

Expected: clean build, all tests pass.

- [ ] **Step 8: Commit**

```bash
git add crates/app-core/src/handlers/chat/streaming.rs
git commit -m "fix(chat): move ChatTurnCompleted to relay_chat_stream on Done

ChatTurnCompleted was published before the streaming background task
started, meaning the cognitive pipeline received the event before the
assistant response was saved to the session. Now published inside
relay_chat_stream when AgentEvent::Done fires, ensuring full
conversation context is available for enriched fact extraction."
```

---

### Task 2: Remove dead per-skill budget code

The `skill_budget_for` function defines presets for "task-management", "finance-management", "communication", and "automation", but the only production call site always passes `"general"`. The `SkillRouter` was removed in the flat refactor — these presets can never be reached.

**Files:**
- Modify: `crates/agent/src/execution/budget.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`
- Modify: `crates/config/src/schema/execution.rs`

- [ ] **Step 1: Remove `skill_budget_for` and simplify `ExecutionBudget::new`**

In `crates/agent/src/execution/budget.rs`:

Delete the `skill_budget_for` function (lines 58-83):

```rust
/// Well-known skill budget presets.
pub fn skill_budget_for(skill_name: &str) -> SkillBudget {
    match skill_name {
        "task-management" => SkillBudget { ... },
        "finance-management" => SkillBudget { ... },
        "communication" => SkillBudget { ... },
        "automation" => SkillBudget { ... },
        _ => SkillBudget::default(),
    }
}
```

Change `ExecutionBudget::new` from:

```rust
    pub fn new(depth: DepthMode, skill_name: &str) -> Self {
        let base = skill_budget_for(skill_name);
```

to:

```rust
    pub fn new(depth: DepthMode) -> Self {
        let base = SkillBudget::default();
```

- [ ] **Step 2: Update the production call site**

In `crates/agent/src/agent_runtime/runtime.rs`, find line 212:

```rust
let mut budget = ExecutionBudget::new(depth, "general");
```

Change to:

```rust
let mut budget = ExecutionBudget::new(depth);
```

- [ ] **Step 3: Update test call sites**

Search for all `ExecutionBudget::new` calls in test code:

```bash
grep -rn "ExecutionBudget::new" crates/agent/src/
```

Update each to remove the `skill_name` parameter. Tests that were using `"task-management"` to test non-default budgets should use `ExecutionBudget::with_limits(...)` instead (which bypasses `skill_budget_for` anyway).

- [ ] **Step 4: Remove `SkillBudgetOverride` from config**

In `crates/config/src/schema/execution.rs`, remove:

```rust
    /// Per-skill budget overrides. Keys are skill names.
    #[serde(default)]
    pub skill_budgets: HashMap<String, SkillBudgetOverride>,
```

from the `ExecutionConfig` struct, and remove the `SkillBudgetOverride` struct and its `use std::collections::HashMap` import.

Update `Default for ExecutionConfig` to remove `skill_budgets: HashMap::new()`.

Check if `SkillBudgetOverride` is referenced elsewhere:

```bash
grep -rn "SkillBudgetOverride\|skill_budgets" crates/
```

Remove any references found.

- [ ] **Step 5: Build and test**

```bash
cargo build --workspace 2>&1 | tail -5
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/execution/budget.rs crates/agent/src/agent_runtime/runtime.rs crates/config/src/schema/execution.rs
git commit -m "refactor(agent): remove dead per-skill budget presets

skill_budget_for() defined presets for task-management, finance,
communication, and automation, but the only production call site
always passed 'general'. The SkillRouter was removed in the flat
refactor — these presets were unreachable. Simplified
ExecutionBudget::new to take only DepthMode. Also removed unused
SkillBudgetOverride from config schema."
```

---

### Task 3: Full validation

**Files:** None (validation only)

- [ ] **Step 1: Build**

```bash
cargo build --workspace
```

- [ ] **Step 2: Clippy**

```bash
cargo clippy --workspace --all-targets --all-features 2>&1 | grep "^error" | head -5
```

- [ ] **Step 3: Format**

```bash
cargo fmt --all --check
```

- [ ] **Step 4: Run all tests**

```bash
cargo nextest run --workspace --no-fail-fast -E 'not test(smoke) and not test(software_engineer) and not test(agent_validation) and not test(fact_contradiction) and not test(onboarding) and not test(finance_focused) and not test(coaching_persona) and not test(cognitive_llm) and not test(multi_channel)' 2>&1 | grep "Summary"
```

- [ ] **Step 5: Format if needed**

```bash
cargo fmt --all
git add -A && git diff --cached --stat
```

If changes: `git commit -m "style: format after infrastructure gap fixes"`

---

## Summary

| Task | What it fixes | Impact |
|------|--------------|--------|
| 1 | `ChatTurnCompleted` timing | Cognitive extraction sees full conversation (user + assistant) instead of just user message |
| 2 | Dead per-skill budget code | Cleaner codebase, no misleading unreachable code |
| 3 | Validation | No regressions |

## How to Test Task 1

After implementation, restart the app and:

1. Send a message in the desktop chat
2. Wait for response to complete
3. Check the `domain_event_log` table:
```sql
sqlite3 ~/.klyntbot-dev/data.db "SELECT event_type, substr(payload, 1, 100) FROM domain_event_log ORDER BY recorded_at DESC LIMIT 5"
```
4. The `ChatTurnCompleted` event should appear AFTER the session has the assistant response
5. Check that extracted facts include context from the assistant response:
```sql
sqlite3 ~/.klyntbot-dev/data.db "SELECT domain, predicate, object FROM semantic_facts ORDER BY recorded_at DESC LIMIT 5"
```
