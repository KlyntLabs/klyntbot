# Live Context Refresher Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Inject mid-execution context updates (starting with promoted memories) into the ReactiveEngine so the agent adapts to new knowledge in real time — making Klyntbot feel like a living second brain.

**Architecture:** A `ContextUpdateQueue` in the `bus` crate acts as the shared extension point. Producers (cognitive background service) push updates; the `LiveContextRefresher` (in the `agent` crate) drains them at iteration boundaries inside `ReactiveEngine`, injecting `Message::ContextUpdate` entries that the LLM reads as system context. Token budgets are respected via the shared `estimate_message_tokens()` function.

**Tech Stack:** Rust, bus crate (shared queue), providers crate (Message variant), context_engine (token counting), agent crate (refresher + reactive loop integration)

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/bus/src/context_updates.rs` | Create | `ContextUpdate`, `ContextUpdateReason`, `UpdatePriority`, `ContextUpdateQueue` |
| `crates/bus/src/lib.rs` | Modify | Declare `context_updates` module, re-export types |
| `crates/providers/src/types.rs` | Modify | Add `Message::ContextUpdate` variant, update `role()`, add constructor |
| `crates/providers/src/adapters/openai_compat.rs` | Modify | Handle `ContextUpdate` in `build_request_body` serialization |
| `crates/providers/src/adapters/anthropic_native.rs` | Modify | Handle `ContextUpdate` in `convert_messages` |
| `crates/context_engine/src/token_counter.rs` | Modify | Add `Message::ContextUpdate` branch to `estimate_message_tokens` |
| `crates/agent/src/execution/live_context_refresher.rs` | Create | `LiveContextRefresher` with `inject_pending()` method + tests |
| `crates/agent/src/execution/mod.rs` | Modify | Declare `live_context_refresher` module, re-export |
| `crates/agent/src/execution/types.rs` | Modify | Add `context_update_queue` + `pause_context_updates` to `ExecutionParams` |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` | Modify | Create refresher, call `inject_pending` after each iteration |
| `crates/agent/src/events.rs` | Modify | Add `ContextReassembled` variant to `AgentEvent` |
| `crates/agent/src/agent_runtime/runtime.rs` | Modify | Pass `context_update_queue` to `ExecutionParams` (both main + delegation paths) |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Accept queue from builder, pass to `AgentRuntime` |
| `crates/agent/src/agent_loop/mod.rs` | Modify | Add queue field to `AgentLoop`, builder method |
| `crates/app-core/src/handlers/chat/streaming.rs` | Modify | Handle `ContextReassembled` event |
| `crates/app-core/src/init/mod.rs` | Modify | Create queue, pass to agent loop builder + cognitive background service |
| `crates/cognitive/src/services/background.rs` | Modify | Add queue field to `BackgroundServiceConfig`, push on memory promotion |
| `CLAUDE.md` | Modify | Document Live Context Refresher |

---

## Task 1: Core Types in `bus` crate

**Files:**
- Create: `crates/bus/src/context_updates.rs`
- Modify: `crates/bus/src/lib.rs`

- [ ] **Step 1: Write the tests**

In `crates/bus/src/context_updates.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queue_push_and_drain() {
        let queue = ContextUpdateQueue::new();
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::MemoryPromoted,
            content: Some("User likes coffee".to_string()),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: chrono::Utc::now(),
        });
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::FocusSessionEnded,
            content: None,
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: chrono::Utc::now(),
        });
        let drained = queue.drain();
        assert_eq!(drained.len(), 2);
        // Second drain is empty
        assert!(queue.drain().is_empty());
    }

    #[test]
    fn queue_deduplicates_within_window() {
        let queue = ContextUpdateQueue::new();
        let now = chrono::Utc::now();
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::MemoryPromoted,
            content: Some("Same fact".to_string()),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: now,
        });
        // Push same reason + content within 30s — should be deduplicated
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::MemoryPromoted,
            content: Some("Same fact".to_string()),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: now + chrono::Duration::seconds(5),
        });
        let drained = queue.drain();
        assert_eq!(drained.len(), 1);
    }

    #[test]
    fn queue_allows_different_reasons() {
        let queue = ContextUpdateQueue::new();
        let now = chrono::Utc::now();
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::MemoryPromoted,
            content: Some("Fact A".to_string()),
            metadata: None,
            priority: UpdatePriority::Normal,
            timestamp: now,
        });
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::FocusSessionEnded,
            content: Some("Fact A".to_string()),
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: now,
        });
        assert_eq!(queue.drain().len(), 2);
    }

    #[test]
    fn queue_dedup_none_content_by_reason_only() {
        let queue = ContextUpdateQueue::new();
        let now = chrono::Utc::now();
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::FocusSessionStarted,
            content: None,
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: now,
        });
        queue.push(ContextUpdate {
            reason: ContextUpdateReason::FocusSessionStarted,
            content: None,
            metadata: None,
            priority: UpdatePriority::High,
            timestamp: now + chrono::Duration::seconds(10),
        });
        assert_eq!(queue.drain().len(), 1);
    }

    #[test]
    fn priority_ordering() {
        assert!(UpdatePriority::High > UpdatePriority::Normal);
        assert!(UpdatePriority::Normal > UpdatePriority::Low);
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p bus -E 'test(context_update)' --no-capture`
Expected: FAIL — module doesn't exist.

- [ ] **Step 3: Implement the types and queue**

```rust
//! Shared queue for mid-execution context updates.
//!
//! Producers (cognitive background service, focus manager, etc.) push updates;
//! the LiveContextRefresher in the agent crate drains and injects them into
//! the ReactiveEngine at iteration boundaries.

use std::sync::Mutex;

use chrono::{DateTime, Duration, Utc};
use serde::Serialize;

const DEDUP_WINDOW_SECS: i64 = 30;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextUpdateReason {
    MemoryPromoted,
    FocusSessionStarted,
    FocusSessionEnded,
    DistractionDetected,
    BudgetThresholdCrossed,
    Custom(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdatePriority {
    Low = 0,
    Normal = 1,
    High = 2,
}

#[derive(Debug, Clone)]
pub struct ContextUpdate {
    pub reason: ContextUpdateReason,
    pub content: Option<String>,
    pub metadata: Option<serde_json::Value>,
    pub priority: UpdatePriority,
    pub timestamp: DateTime<Utc>,
}

pub struct ContextUpdateQueue {
    inner: Mutex<Vec<ContextUpdate>>,
}

impl ContextUpdateQueue {
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(Vec::new()),
        }
    }

    /// Push an update with 30-second deduplication by (reason, content).
    pub fn push(&self, update: ContextUpdate) {
        let mut queue = self.inner.lock().unwrap();
        let dominated = queue.iter().any(|existing| {
            existing.reason == update.reason
                && existing.content == update.content
                && (update.timestamp - existing.timestamp).abs() < Duration::seconds(DEDUP_WINDOW_SECS)
        });
        if !dominated {
            queue.push(update);
        }
    }

    /// Drain all pending updates atomically.
    pub fn drain(&self) -> Vec<ContextUpdate> {
        let mut queue = self.inner.lock().unwrap();
        std::mem::take(&mut *queue)
    }
}

impl Default for ContextUpdateQueue {
    fn default() -> Self {
        Self::new()
    }
}
```

- [ ] **Step 4: Declare module in `bus/src/lib.rs`**

Add to `crates/bus/src/lib.rs`:
```rust
pub mod context_updates;
pub use context_updates::{ContextUpdate, ContextUpdateQueue, ContextUpdateReason, UpdatePriority};
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p bus -E 'test(context_update)' --no-capture`
Expected: All 5 tests PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/bus/src/context_updates.rs crates/bus/src/lib.rs
git commit -m "feat(bus): add ContextUpdateQueue for mid-execution context updates"
```

---

## Task 2: Add `Message::ContextUpdate` variant

**Files:**
- Modify: `crates/providers/src/types.rs`
- Modify: `crates/context_engine/src/token_counter.rs`

- [ ] **Step 1: Add the variant and update `role()`**

In `crates/providers/src/types.rs`, add after the `Tool` variant (line 352):

```rust
    /// A mid-execution context update injected by LiveContextRefresher.
    /// Serialized as system role with XML tags when sent to the LLM.
    #[serde(rename = "context_update")]
    ContextUpdate {
        reason: String,
        content: String,
    },
```

Update `Message::role()` (line 451-458) to add:
```rust
            Message::ContextUpdate { .. } => MessageRole::System,
```

Add a constructor after `Message::tool()`:
```rust
    /// Create a context update message (injected mid-execution).
    pub fn context_update(reason: impl Into<String>, content: impl Into<String>) -> Self {
        Self::ContextUpdate {
            reason: reason.into(),
            content: content.into(),
        }
    }
```

- [ ] **Step 2: Update `estimate_message_tokens` in `context_engine/src/token_counter.rs`**

Add after the `Message::Tool` branch (around line 89):
```rust
        providers::Message::ContextUpdate { content, .. } => {
            counter.estimate_text(content) + 10 // overhead for XML tags
        }
```

- [ ] **Step 3: Fix all exhaustive matches across the workspace**

Run: `cargo check --workspace 2>&1 | grep "ContextUpdate"` to find all broken match arms.

For each, add the appropriate arm. **Known locations** (may find more via `cargo check`):
- `crates/context_engine/src/assembler/mod.rs` — private `estimate_message_tokens` method (~line 586). Add: `Message::ContextUpdate { content, .. } => counter.estimate_text(content) + 10,`
- `crates/agent/src/execution/core.rs` — fallback usage estimation block (~line 280). Add: `Message::ContextUpdate { content, .. } => counter.estimate_text(content) + 10,`
- `crates/providers/src/adapters/openai_compat.rs` — `build_request_body` uses `"messages": messages` (serde direct). Need to pre-transform `ContextUpdate` messages before serialization.
- `crates/providers/src/adapters/anthropic_native.rs` — `convert_messages` matches on variants. Note: `extract_system_prompts` will silently skip `ContextUpdate` — this is correct (context updates are mid-conversation, not top-level system). Add to `convert_messages`:
  ```rust
  Message::ContextUpdate { reason, content } => {
      result.push(json!({
          "role": "user",
          "content": [{"type": "text", "text": format!("<context_update reason=\"{reason}\">\n{content}\n</context_update>")}]
      }));
  }
  ```
  Note: Anthropic API only supports `user`/`assistant` roles in messages (system is top-level). Map to `user` with XML tag for Anthropic.
- `crates/agent/src/execution/mid_loop_compressor.rs` — `estimate_message_tokens` already calls the shared function, no change needed.
- `crates/app-core/src/handlers/chat/streaming.rs` — if any match on Message exists there.
- Any other `match msg { ... }` patterns.

For the OpenAI adapter (`build_request_body` at line 182), the messages are serialized directly via serde (`"messages": messages`). The `#[serde(rename = "context_update")]` on the variant would produce `"role": "context_update"` which OpenAI rejects. **Fix**: Pre-filter messages before serialization — replace `ContextUpdate` variants with equivalent `System` messages:

In `build_request_body`, before `"messages": messages`:
```rust
        // Map ContextUpdate messages to system-role messages for API compatibility.
        // ContextUpdate would serialize as "role": "context_update" via serde, which providers reject.
        let has_context_updates = messages.iter().any(|m| matches!(m, Message::ContextUpdate { .. }));
        let api_messages: Value = if has_context_updates {
            let mapped: Vec<Value> = messages
                .iter()
                .map(|msg| match msg {
                    Message::ContextUpdate { reason, content } => {
                        json!({
                            "role": "system",
                            "content": format!("<context_update reason=\"{reason}\">\n{content}\n</context_update>")
                        })
                    }
                    other => serde_json::to_value(other).expect("Message serialization"),
                })
                .collect();
            json!(mapped)
        } else {
            json!(messages) // fast path: no transformation needed
        };
```
Then use `"messages": api_messages` instead of `"messages": messages`.

- [ ] **Step 4: Run compilation check**

Run: `cargo check --workspace`
Expected: Compiles with no errors.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p providers -p context_engine`
Expected: All PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/providers/src/types.rs crates/providers/src/adapters/openai_compat.rs crates/providers/src/adapters/anthropic_native.rs crates/context_engine/src/token_counter.rs
git commit -m "feat(providers): add Message::ContextUpdate variant with provider serialization"
```

---

## Task 3: Add `context_update_queue` and `pause_context_updates` to `ExecutionParams`

**Files:**
- Modify: `crates/agent/src/execution/types.rs`

- [ ] **Step 1: Write tests**

Add to the existing `mod tests` in `types.rs`:

```rust
    #[test]
    fn execution_params_default_no_context_queue() {
        let params = ExecutionParams::new("mock");
        assert!(params.context_update_queue.is_none());
        assert!(!params.pause_context_updates);
    }

    #[test]
    fn execution_params_with_context_queue() {
        let queue = std::sync::Arc::new(bus::ContextUpdateQueue::new());
        let params = ExecutionParams::new("mock")
            .with_context_update_queue(queue.clone());
        assert!(params.context_update_queue.is_some());
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(execution_params)' --no-capture`
Expected: FAIL — fields don't exist.

- [ ] **Step 3: Add fields and builder methods**

In `ExecutionParams` struct, add after `context_window`:
```rust
    /// Shared queue for mid-execution context updates from cognitive/productivity systems.
    pub context_update_queue: Option<std::sync::Arc<bus::ContextUpdateQueue>>,
    /// When true, the LiveContextRefresher skips injection (frozen-context mode).
    pub pause_context_updates: bool,
```

In `ExecutionParams::new()`, add:
```rust
            context_update_queue: None,
            pause_context_updates: false,
```

Add builder methods:
```rust
    pub fn with_context_update_queue(mut self, queue: std::sync::Arc<bus::ContextUpdateQueue>) -> Self {
        self.context_update_queue = Some(queue);
        self
    }

    pub fn with_pause_context_updates(mut self, pause: bool) -> Self {
        self.pause_context_updates = pause;
        self
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(execution_params)' --no-capture`
Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/execution/types.rs
git commit -m "feat(execution): add context_update_queue and pause_context_updates to ExecutionParams"
```

---

## Task 4: Create `LiveContextRefresher`

**Files:**
- Create: `crates/agent/src/execution/live_context_refresher.rs`
- Modify: `crates/agent/src/execution/mod.rs`

- [ ] **Step 1: Write the tests**

Create `crates/agent/src/execution/live_context_refresher.rs` with tests:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_refresher() -> (LiveContextRefresher, Arc<bus::ContextUpdateQueue>) {
        let queue = Arc::new(bus::ContextUpdateQueue::new());
        let refresher = LiveContextRefresher::new(
            Arc::new(context_engine::CharTokenCounter),
            Arc::clone(&queue),
        );
        (refresher, queue)
    }

    fn push_memory_update(queue: &bus::ContextUpdateQueue, content: &str) {
        queue.push(bus::ContextUpdate {
            reason: bus::ContextUpdateReason::MemoryPromoted,
            content: Some(content.to_string()),
            metadata: None,
            priority: bus::UpdatePriority::Normal,
            timestamp: chrono::Utc::now(),
        });
    }

    #[test]
    fn empty_queue_is_noop() {
        let (refresher, _queue) = make_refresher();
        let mut messages = vec![
            providers::Message::system("System prompt"),
            providers::Message::user("Hello"),
        ];
        let result = refresher.inject_pending(&mut messages, 128_000);
        assert!(result.is_empty());
        assert_eq!(messages.len(), 2); // unchanged
    }

    #[test]
    fn injects_pending_update() {
        let (refresher, queue) = make_refresher();
        push_memory_update(&queue, "User likes morning work");

        let mut messages = vec![
            providers::Message::system("System prompt"),
            providers::Message::user("Plan my day"),
            providers::Message::assistant("Let me check..."),
        ];
        let result = refresher.inject_pending(&mut messages, 128_000);
        assert_eq!(result.len(), 1);
        assert_eq!(messages.len(), 4); // one added
        assert!(matches!(&messages[3], providers::Message::ContextUpdate { content, .. } if content.contains("morning work")));
    }

    #[test]
    fn respects_token_budget() {
        let (refresher, queue) = make_refresher();
        // Push a large update
        push_memory_update(&queue, &"x".repeat(10000));

        let mut messages = vec![
            providers::Message::system(&"y".repeat(10000)),
            providers::Message::user("Hello"),
        ];
        // Tiny context window — no room for the update
        let result = refresher.inject_pending(&mut messages, 100);
        assert!(result.is_empty()); // dropped due to budget
    }

    #[test]
    fn priority_ordering_high_first() {
        let (refresher, queue) = make_refresher();
        let now = chrono::Utc::now();
        queue.push(bus::ContextUpdate {
            reason: bus::ContextUpdateReason::MemoryPromoted,
            content: Some("Low priority fact".to_string()),
            metadata: None,
            priority: bus::UpdatePriority::Low,
            timestamp: now,
        });
        queue.push(bus::ContextUpdate {
            reason: bus::ContextUpdateReason::FocusSessionEnded,
            content: Some("High priority event".to_string()),
            metadata: None,
            priority: bus::UpdatePriority::High,
            timestamp: now,
        });

        let mut messages = vec![providers::Message::system("sys")];
        let result = refresher.inject_pending(&mut messages, 128_000);
        assert_eq!(result.len(), 2);
        // High priority (focus_session_ended) should be injected before Normal (memory_promoted)
        assert_eq!(result[0].reason, "focus_session_ended");
        assert_eq!(result[1].reason, "memory_promoted");
    }

    #[test]
    fn drains_queue_after_inject() {
        let (refresher, queue) = make_refresher();
        push_memory_update(&queue, "Fact");
        let mut messages = vec![providers::Message::system("sys")];
        refresher.inject_pending(&mut messages, 128_000);
        // Second call should find empty queue
        let result = refresher.inject_pending(&mut messages, 128_000);
        assert!(result.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(live_context)' --no-capture`
Expected: FAIL — struct doesn't exist.

- [ ] **Step 3: Implement `LiveContextRefresher`**

```rust
//! Live context refresher — injects mid-execution context updates from
//! cognitive, productivity, and coaching systems into the ReactiveEngine.
//!
//! Runs at iteration boundaries alongside MidLoopCompressor. Drains the
//! shared ContextUpdateQueue and injects updates as Message::ContextUpdate
//! entries, respecting the remaining token budget.

use std::sync::Arc;

use bus::{ContextUpdateQueue, UpdatePriority};
use context_engine::TokenCounter;
use providers::Message;
use serde::Serialize;
use tracing::info;

/// Returned for each injected update — used by the caller to emit AgentEvent::ContextReassembled.
#[derive(Debug, Clone, Serialize)]
pub struct ContextReassembledUpdate {
    pub reason: String,
    pub summary: String,
    pub tokens: usize,
}

pub struct LiveContextRefresher {
    token_counter: Arc<dyn TokenCounter>,
    queue: Arc<ContextUpdateQueue>,
}

impl LiveContextRefresher {
    pub fn new(token_counter: Arc<dyn TokenCounter>, queue: Arc<ContextUpdateQueue>) -> Self {
        Self {
            token_counter,
            queue,
        }
    }

    /// Drain pending updates and inject them into the message vec.
    ///
    /// Takes `&mut Vec<Message>` (not `&mut [Message]`) because new messages are pushed.
    /// Returns a list of injected updates (empty if queue was empty or all updates
    /// were dropped due to budget). The caller emits `AgentEvent::ContextReassembled`.
    pub fn inject_pending(
        &self,
        messages: &mut Vec<Message>,
        context_window: usize,
    ) -> Vec<ContextReassembledUpdate> {
        let mut updates = self.queue.drain();
        if updates.is_empty() {
            return Vec::new();
        }

        // Sort by priority (High first)
        updates.sort_by(|a, b| b.priority.cmp(&a.priority));

        let current_tokens: usize = messages
            .iter()
            .map(|m| context_engine::estimate_message_tokens(&*self.token_counter, m))
            .sum();

        let remaining = context_window.saturating_sub(current_tokens);
        // Standard: reserve 20% for LLM response → 80% available for updates
        // High priority: reserve only 10% → 90% available (more aggressive)
        let standard_budget = remaining * 80 / 100;
        let high_budget = remaining * 90 / 100;

        let mut used_tokens = 0;
        let mut injected = Vec::new();

        for update in &updates {
            let content = Self::render_content(update);
            let reason_str = serde_json::to_value(&update.reason)
                .ok()
                .and_then(|v| v.as_str().map(String::from))
                .unwrap_or_else(|| format!("{:?}", update.reason).to_lowercase());
            let msg = Message::context_update(&reason_str, &content);
            let tokens = context_engine::estimate_message_tokens(&*self.token_counter, &msg);

            let budget = if update.priority == UpdatePriority::High {
                high_budget
            } else {
                standard_budget
            };

            if used_tokens + tokens > budget {
                tracing::warn!(
                    reason = ?update.reason,
                    tokens,
                    remaining_budget = budget.saturating_sub(used_tokens),
                    "context update dropped — insufficient token budget"
                );
                continue;
            }

            used_tokens += tokens;
            let summary = update
                .content
                .clone()
                .unwrap_or_else(|| format!("{:?}", update.reason));

            injected.push(ContextReassembledUpdate {
                reason: format!("{:?}", update.reason).to_lowercase(),
                summary: summary.clone(),
                tokens,
            });

            messages.push(msg);
        }

        if !injected.is_empty() {
            info!(
                count = injected.len(),
                tokens = used_tokens,
                "live context updates injected"
            );
        }

        injected
    }

    fn render_content(update: &bus::ContextUpdate) -> String {
        update
            .content
            .clone()
            .unwrap_or_else(|| format!("{:?}", update.reason))
    }
}
```

- [ ] **Step 4: Declare module in `execution/mod.rs`**

Add to `crates/agent/src/execution/mod.rs`:
```rust
pub mod live_context_refresher;
pub use live_context_refresher::{ContextReassembledUpdate, LiveContextRefresher};
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p agent -E 'test(live_context)' --no-capture`
Expected: All 5 tests PASS.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -p agent`
Expected: Zero new warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/execution/live_context_refresher.rs crates/agent/src/execution/mod.rs
git commit -m "feat(execution): add LiveContextRefresher with token-aware injection"
```

---

## Task 5: Add `ContextReassembled` event + streaming handler

**Files:**
- Modify: `crates/agent/src/events.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs`

- [ ] **Step 1: Add event variant**

In `crates/agent/src/events.rs`, add after `ContextCompressed`:

```rust
    /// Live context was injected mid-execution (e.g., memory promoted during ReAct loop).
    ContextReassembled {
        updates: Vec<crate::execution::live_context_refresher::ContextReassembledUpdate>,
        #[serde(rename = "tokensAdded")]
        tokens_added: usize,
    },
```

- [ ] **Step 2: Handle in streaming.rs**

In `crates/app-core/src/handlers/chat/streaming.rs`, add after the `ContextCompressed` handler:

```rust
                    AgentEvent::ContextReassembled { updates, tokens_added } => {
                        tracing::info!(
                            updates_count = updates.len(),
                            tokens_added,
                            "live context reassembled during execution"
                        );
                    }
```

- [ ] **Step 3: Verify compilation**

Run: `cargo check --workspace`
Expected: Compiles.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/events.rs crates/app-core/src/handlers/chat/streaming.rs
git commit -m "feat(events): add ContextReassembled event for live context transparency"
```

---

## Task 6: Integrate into `ReactiveEngine`

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs`

- [ ] **Step 1: Create refresher and call `inject_pending` after each iteration**

At the top of `ReactiveEngine::execute()`, after the `MidLoopCompressor` creation:

```rust
        let refresher = params.context_update_queue.as_ref().map(|queue| {
            crate::execution::LiveContextRefresher::new(
                Arc::clone(self.core.token_counter()),
                Arc::clone(queue),
            )
        });
```

At the end of the `for` loop body, after the `MidLoopCompressor` block:

```rust
            // Live context refresh: inject pending updates from cognitive/productivity systems
            if !params.pause_context_updates {
                if let Some(ref refresher) = refresher {
                    let injected = refresher.inject_pending(&mut messages, params.context_window);
                    if !injected.is_empty() {
                        let tokens_added: usize = injected.iter().map(|u| u.tokens).sum();
                        if let Some(ref tx) = event_tx {
                            let _ = tx
                                .send(crate::events::AgentEvent::ContextReassembled {
                                    updates: injected,
                                    tokens_added,
                                })
                                .await;
                        }
                    }
                }
            }
```

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p agent`
Expected: Compiles.

- [ ] **Step 3: Run all reactive tests**

Run: `cargo nextest run -p agent -E 'test(reactive)' --no-capture`
Expected: All PASS (existing tests unaffected — `context_update_queue` is `None` by default).

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/reactive.rs
git commit -m "feat(reactive): integrate LiveContextRefresher into ReAct loop"
```

---

## Task 7: Wire the queue through AgentLoop → AgentRuntime → ExecutionParams

**Files:**
- Modify: `crates/agent/src/agent_loop/mod.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Add queue field to `AgentLoop`**

In `crates/agent/src/agent_loop/mod.rs`, add to the `AgentLoop` struct:
```rust
    context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
```

Add builder method:
```rust
    pub fn with_context_update_queue(mut self, queue: Arc<bus::ContextUpdateQueue>) -> Self {
        self.context_update_queue = Some(queue);
        self
    }
```

Initialize to `None` in the builder's `build()`.

- [ ] **Step 2: Pass to `AgentRuntime`**

In `crates/agent/src/agent_runtime/runtime.rs`, add field to `AgentRuntime`:
```rust
    context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
```

Add to `AgentRuntime::new()` parameters and initialization.

- [ ] **Step 3: Pass to `ExecutionParams` in both paths**

In `runtime.rs`, find **both** `ExecutionParams::new` call sites (main pipeline ~line 493 and delegation ~line 1043).

Add to both builder chains:
```rust
            .with_context_update_queue(self.context_update_queue.clone().unwrap_or_default())
```

Wait — `Option<Arc<_>>` doesn't have `unwrap_or_default`. Use:
```rust
        if let Some(ref queue) = self.context_update_queue {
            params = params.with_context_update_queue(Arc::clone(queue));
        }
```

- [ ] **Step 4: Wire in builder.rs**

Add a setter method to `AgentRuntime` (following the pattern of `set_domain_event_bus`, `set_autotuner_hook`, etc.):

```rust
    pub fn set_context_update_queue(&mut self, queue: Arc<bus::ContextUpdateQueue>) {
        self.context_update_queue = Some(queue);
    }
```

In `crates/agent/src/agent_loop/builder.rs`, find where `AgentRuntime` is constructed and other setters are called (e.g., `runtime.set_domain_event_bus(...)`, `runtime.set_autotuner_hook(...)`). Add:

```rust
        if let Some(ref queue) = self.context_update_queue {
            runtime.set_context_update_queue(Arc::clone(queue));
        }
```

- [ ] **Step 5: Verify compilation**

Run: `cargo check --workspace`
Expected: Compiles.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/agent_loop/mod.rs crates/agent/src/agent_loop/builder.rs crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(agent): wire ContextUpdateQueue from AgentLoop through to ExecutionParams"
```

---

## Task 8: Wire the queue in `app-core/init` + Phase 1 producer (memory promotion)

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`
- Modify: `crates/cognitive/src/services/background.rs`

- [ ] **Step 1: Create queue in app-core init and pass to both consumers**

In `crates/app-core/src/init/mod.rs`, find where `DomainEventBus` is created. Create the queue alongside it:

```rust
        let context_update_queue = Arc::new(bus::ContextUpdateQueue::new());
```

Pass to the agent loop builder:
```rust
        agent_loop_builder = agent_loop_builder
            .with_context_update_queue(Arc::clone(&context_update_queue));
```

Pass to the background service config:
```rust
        // When constructing BackgroundServiceConfig:
        context_update_queue: Some(Arc::clone(&context_update_queue)),
```

- [ ] **Step 2: Add queue field to `BackgroundServiceConfig`**

In `crates/cognitive/src/services/background.rs`, add to `BackgroundServiceConfig`:
```rust
    pub context_update_queue: Option<Arc<bus::ContextUpdateQueue>>,
```

- [ ] **Step 3: Push on memory promotion**

**Important**: `BackgroundConsolidationService::start()` moves all config fields into a `tokio::spawn` closure. There is no `self` inside the closure — fields are captured as local variables. The queue must be destructured from the config and captured by the closure alongside the other fields.

In `BackgroundServiceConfig`, the new `context_update_queue` field gets destructured at the top of `start()` (alongside `event_rx`, `extraction`, `consolidation`, etc.). Inside the closure, after a semantic fact is successfully extracted/persisted, add:

```rust
if let Some(ref queue) = context_update_queue {
    queue.push(bus::ContextUpdate {
        reason: bus::ContextUpdateReason::MemoryPromoted,
        content: Some(format!("{} — {}", fact.subject, fact.predicate)),
        metadata: Some(serde_json::json!({ "factId": fact.id.to_string() })),
        priority: bus::UpdatePriority::Normal,
        timestamp: chrono::Utc::now(),
    });
}
```

Search for where `extraction.extract()` returns and facts are persisted to find the exact insertion point.

- [ ] **Step 4: Verify compilation**

Run: `cargo check --workspace`
Expected: Compiles.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/mod.rs crates/cognitive/src/services/background.rs
git commit -m "feat(cognitive): wire ContextUpdateQueue, push on memory promotion (Phase 1 producer)"
```

---

## Task 9: Integration + Serialization Tests

**Files:**
- Modify: `crates/providers/src/types.rs` (add serialization test)
- Modify: `crates/agent/src/execution/live_context_refresher.rs` (add serialization round-trip test)

- [ ] **Step 1: Add Message::ContextUpdate serialization test**

In `crates/providers/src/types.rs`, add to the existing `mod tests`:

```rust
    #[test]
    fn context_update_role_is_system() {
        let msg = Message::context_update("memory_promoted", "User likes coffee");
        assert_eq!(msg.role(), MessageRole::System);
    }

    #[test]
    fn context_update_serde_round_trip() {
        let msg = Message::context_update("memory_promoted", "User likes coffee");
        let json = serde_json::to_value(&msg).unwrap();
        // The serde tag should be "context_update" (internal representation)
        assert_eq!(json["role"], "context_update");
        assert_eq!(json["reason"], "memory_promoted");
        assert_eq!(json["content"], "User likes coffee");
    }
```

- [ ] **Step 2: Run tests**

Run: `cargo nextest run -p providers -E 'test(context_update)' --no-capture`
Expected: All PASS.

- [ ] **Step 3: Commit**

```bash
git add crates/providers/src/types.rs
git commit -m "test(providers): add Message::ContextUpdate serialization tests"
```

---

## Task 10: Full verification + CLAUDE.md update

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All tests PASS.

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets`
Expected: Zero new warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

- [ ] **Step 4: Update CLAUDE.md**

In the "Agent runtime" section, add after the mid-loop compression docs:

```
**Live context refresh:** During Reactive execution, the `LiveContextRefresher` drains a shared `ContextUpdateQueue` (in the `bus` crate) at each iteration boundary. Context updates (e.g., newly promoted memories) are injected as `Message::ContextUpdate` entries with XML-tagged content. Token budget is respected — standard updates can use up to 80% of remaining context (20% reserved for LLM response); high-priority updates can use 90% (10% reserved). Emits `AgentEvent::ContextReassembled` for transparency. Set `pause_context_updates: true` on `ExecutionParams` for frozen-context mode. Phase 1 producer: cognitive background service pushes on memory promotion.
```

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document Live Context Refresher in CLAUDE.md"
```
