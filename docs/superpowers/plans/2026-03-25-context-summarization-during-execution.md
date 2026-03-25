# Context Summarization During Execution Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prevent context window exhaustion during long ReAct loops by compressing older tool results mid-execution when accumulated tokens approach the budget limit.

**Architecture:** Add a `MidLoopCompressor` to the `ReactiveEngine` that checks token usage after each iteration. When the accumulated message tokens exceed a configurable threshold (default 70% of context window), it replaces older `Message::Tool` results with extractive summaries while keeping the system prompt, recent iterations, and all user/assistant messages intact. This is a lightweight, non-LLM compression pass — no extra API calls.

**Tech Stack:** Rust, context_engine::TokenCounter, providers::Message

---

## File Structure

| File | Action | Responsibility |
|---|---|---|
| `crates/agent/src/execution/mid_loop_compressor.rs` | Create | `MidLoopCompressor` — token counting + tool result compression for the reactive loop |
| `crates/agent/src/execution/mod.rs` | Modify | Declare `mid_loop_compressor` module |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` | Modify | Inject compressor, call after each iteration |
| `crates/agent/src/agent_loop/builder.rs` | Modify | Wire `TokenCounter` into `ReactiveEngine` via `ExecutionCore` or directly |
| `crates/agent/src/execution/core.rs` | Modify | Add `token_counter` field to `ExecutionCore` |
| `crates/agent/src/execution/types.rs` | Modify | Add `context_window` to `ExecutionParams` |
| `CLAUDE.md` | Modify | Document mid-loop compression behavior |

---

## Task 1: Add `context_window` to `ExecutionParams`

**Files:**
- Modify: `crates/agent/src/execution/types.rs`

- [ ] **Step 1: Write the test**

In `crates/agent/src/execution/types.rs`, add to the existing `mod tests`:

```rust
    #[test]
    fn execution_params_with_context_window() {
        let params = ExecutionParams::new("mock").with_context_window(128_000);
        assert_eq!(params.context_window, 128_000);
    }

    #[test]
    fn execution_params_default_context_window() {
        let params = ExecutionParams::new("mock");
        assert_eq!(params.context_window, 128_000);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(context_window)' --no-capture`
Expected: FAIL — field doesn't exist.

- [ ] **Step 3: Implement**

Add to `ExecutionParams`:

```rust
    /// Context window size in tokens. Used for mid-loop compression threshold.
    pub context_window: usize,
```

Add default in `ExecutionParams::new`:
```rust
            context_window: 128_000,
```

Add builder method:
```rust
    pub fn with_context_window(mut self, tokens: usize) -> Self {
        self.context_window = tokens;
        self
    }
```

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p agent -E 'test(execution_params)' --no-capture`
Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/execution/types.rs
git commit -m "feat(execution): add context_window to ExecutionParams"
```

---

## Task 2: Add `TokenCounter` to `ExecutionCore`

**Files:**
- Modify: `crates/agent/src/execution/core.rs`
- Modify: `crates/agent/src/agent_loop/builder.rs`

- [ ] **Step 1: Add `token_counter` field and builder method to `ExecutionCore`**

In `crates/agent/src/execution/core.rs`, add to the `ExecutionCore` struct:

```rust
    /// Token counter for mid-loop compression estimates.
    pub token_counter: Arc<dyn context_engine::TokenCounter>,
```

**IMPORTANT**: `ExecutionCore` uses `::new()` + builder methods (like `with_domain_bus`, `with_outcome_recorder`). The `tool_semaphore` field is private, so struct literals are impossible from outside.

Update `ExecutionCore::new()` to initialize token_counter with a default:

```rust
    pub fn new(provider: DynProvider, tool_registry: Arc<RwLock<ToolRegistry>>) -> Self {
        Self {
            provider,
            tool_registry,
            outcome_recorder: None,
            domain_event_bus: None,
            interceptor_chain: None,
            tool_semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT_TOOLS)),
            token_counter: Arc::new(context_engine::CharTokenCounter),
        }
    }
```

Add a builder method:

```rust
    /// Set the token counter for mid-loop compression.
    pub fn with_token_counter(mut self, counter: Arc<dyn context_engine::TokenCounter>) -> Self {
        self.token_counter = counter;
        self
    }
```

- [ ] **Step 2: Wire token counter in builder.rs**

In `crates/agent/src/agent_loop/builder.rs`, find where `ExecutionCore::new(...)` is called (around line 1412). The builder already creates a token counter via `context_engine::token_counter_for_model(...)` around line 546 for the `ContextEngine`. Reuse that same `Arc`. Chain the builder method:

```rust
    ExecutionCore::new(provider.clone(), Arc::clone(&tool_registry))
        .with_token_counter(Arc::clone(&token_counter))
        // ... existing .with_domain_bus(...) etc.
```

If the existing `token_counter` variable is not in scope at the `ExecutionCore` construction point, clone it earlier and pass it down.

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p agent`
Expected: Compiles. Test helpers use `ExecutionCore::new()` which now has a default `CharTokenCounter` — no changes needed.

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p agent`
Expected: All PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/agent/src/execution/core.rs crates/agent/src/agent_loop/builder.rs
git commit -m "feat(execution): add TokenCounter to ExecutionCore"
```

---

## Task 3: Create `MidLoopCompressor`

**Files:**
- Create: `crates/agent/src/execution/mid_loop_compressor.rs`
- Modify: `crates/agent/src/execution/mod.rs`

- [ ] **Step 1: Write the tests**

Create `crates/agent/src/execution/mid_loop_compressor.rs` with tests first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_compressor(context_window: usize) -> MidLoopCompressor {
        MidLoopCompressor::new(
            Arc::new(context_engine::CharTokenCounter),
            context_window,
        )
    }

    fn system_msg(text: &str) -> Message {
        Message::System { content: text.to_string() }
    }

    fn user_msg(text: &str) -> Message {
        Message::user(text)
    }

    fn assistant_msg(text: &str) -> Message {
        Message::Assistant {
            content: Some(text.to_string()),
            tool_calls: None,
            reasoning_content: None,
        }
    }

    fn tool_msg(id: &str, name: &str, result: &str) -> Message {
        Message::Tool {
            tool_call_id: id.to_string(),
            name: name.to_string(),
            content: result.to_string(),
        }
    }

    #[test]
    fn test_no_compression_under_threshold() {
        let compressor = make_compressor(10_000);
        let mut messages = vec![
            system_msg("System prompt"),
            user_msg("Hello"),
            assistant_msg("I'll help"),
            tool_msg("1", "tasks", "result 1"),
        ];
        let original_len = messages.len();
        let result = compressor.compress_if_needed(&mut messages);
        assert!(result.is_none(), "should not compress under threshold");
        assert_eq!(messages.len(), original_len);
    }

    #[test]
    fn test_compression_over_threshold() {
        // Context window of 100 tokens (~400 chars). Fill with large tool results.
        let compressor = make_compressor(100);
        let large_content = "x".repeat(200);
        let mut messages = vec![
            system_msg("System prompt"),
            user_msg("Do something"),
            assistant_msg("Calling tool"),
            tool_msg("1", "web_fetch", &large_content), // ~50 tokens, will be in older body
            user_msg("Continue"),
            assistant_msg("Calling another tool"),
            tool_msg("2", "web_fetch", &"y".repeat(200)), // ~50 tokens — now over 70% of 100
        ];
        let result = compressor.compress_if_needed(&mut messages);
        // Should have compressed — returns Some((before, after))
        assert!(result.is_some(), "should have triggered compression");
        let (before, after) = result.unwrap();
        assert!(after < before, "after ({after}) should be less than before ({before})");
        // System prompt should survive
        assert!(matches!(&messages[0], Message::System { .. }));
        // The older tool result (index 3) should be compressed — content shortened
        let older_tool = &messages[3];
        if let Message::Tool { content, .. } = older_tool {
            assert!(content.contains("[compressed"), "older tool should contain compression marker");
            assert!(content.len() < large_content.len(), "compressed content should be shorter");
        } else {
            panic!("expected Tool message at index 3");
        }
    }

    #[test]
    fn test_system_messages_never_compressed() {
        let compressor = make_compressor(50);
        let mut messages = vec![
            system_msg("Important system prompt that must survive"),
            user_msg("query"),
            assistant_msg("calling tool"),
            tool_msg("1", "big_tool", &"z".repeat(500)),
        ];
        let _ = compressor.compress_if_needed(&mut messages);
        assert!(matches!(&messages[0], Message::System { content } if content.contains("Important")));
    }

    #[test]
    fn test_preserves_recent_window() {
        let compressor = make_compressor(80);
        let mut messages = vec![
            system_msg("sys"),
            // iteration 1 (old)
            user_msg("q1"),
            assistant_msg("a1"),
            tool_msg("1", "t1", &"old".repeat(100)),
            // iteration 2 (old)
            user_msg("q2"),
            assistant_msg("a2"),
            tool_msg("2", "t2", &"old".repeat(100)),
            // iteration 3 (recent — should survive)
            user_msg("q3"),
            assistant_msg("a3"),
            tool_msg("3", "t3", "recent result"),
        ];
        let _ = compressor.compress_if_needed(&mut messages);
        // Most recent tool result should be preserved verbatim
        assert!(messages.iter().any(|m| matches!(m, Message::Tool { content, .. } if content == "recent result")));
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo nextest run -p agent -E 'test(mid_loop)' --no-capture`
Expected: FAIL — `MidLoopCompressor` doesn't exist.

- [ ] **Step 3: Implement `MidLoopCompressor`**

```rust
//! Mid-loop context compressor for the ReactiveEngine.
//!
//! During long ReAct loops, tool results accumulate and can exhaust the
//! context window. This compressor checks token usage after each iteration
//! and replaces older tool results with extractive summaries when the
//! accumulated tokens exceed a threshold.

use std::sync::Arc;

use context_engine::TokenCounter;
use providers::Message;
use tracing::info;

/// Threshold: compress when accumulated tokens exceed this fraction of context_window.
const COMPRESSION_THRESHOLD: f64 = 0.70;

/// Number of recent messages to always keep verbatim (from the end of the vec).
const MIN_RECENT_MESSAGES: usize = 8;

/// Maximum length of a compressed tool result summary (chars).
const SUMMARY_SNIPPET_LENGTH: usize = 150;

pub struct MidLoopCompressor {
    token_counter: Arc<dyn TokenCounter>,
    context_window: usize,
}

impl MidLoopCompressor {
    pub fn new(token_counter: Arc<dyn TokenCounter>, context_window: usize) -> Self {
        Self {
            token_counter,
            context_window,
        }
    }

    /// Estimate the total token count of the message vec.
    fn estimate_tokens(&self, messages: &[Message]) -> usize {
        messages
            .iter()
            .map(|m| self.estimate_message_tokens(m))
            .sum()
    }

    fn estimate_message_tokens(&self, msg: &Message) -> usize {
        match msg {
            Message::System { content } => self.token_counter.estimate_text(content) + 4,
            Message::User { content } => {
                let text = match content {
                    providers::UserContent::Text(t) => t.as_str(),
                    providers::UserContent::MultiPart(parts) => {
                        return parts.len() * 10; // flat heuristic for multipart
                    }
                };
                self.token_counter.estimate_text(text) + 4
            }
            Message::Assistant { content, .. } => {
                content
                    .as_deref()
                    .map(|c| self.token_counter.estimate_text(c))
                    .unwrap_or(0)
                    + 20 // overhead for tool_calls JSON
            }
            Message::Tool { content, name, .. } => {
                self.token_counter.estimate_text(content)
                    + self.token_counter.estimate_text(name)
                    + 10
            }
        }
    }

    /// Compress older tool results if total tokens exceed the threshold.
    ///
    /// Strategy:
    /// 1. Count total tokens across all messages
    /// 2. If under threshold, return without changes
    /// 3. Split messages into: system prefix + older body + recent tail
    /// 4. Replace Tool messages in the older body with truncated summaries
    /// 5. Rebuild the messages vec
    /// Returns `Some((before_tokens, after_tokens))` if compression was applied, `None` otherwise.
    pub fn compress_if_needed(&self, messages: &mut Vec<Message>) -> Option<(usize, usize)> {
        let total_tokens = self.estimate_tokens(messages);
        let threshold = (self.context_window as f64 * COMPRESSION_THRESHOLD) as usize;

        if total_tokens <= threshold {
            return None;
        }

        info!(
            total_tokens,
            threshold,
            message_count = messages.len(),
            "mid-loop compression triggered"
        );

        // Preserve system messages at the front
        let system_count = messages
            .iter()
            .take_while(|m| matches!(m, Message::System { .. }))
            .count();

        // Preserve recent tail verbatim
        let recent_start = messages.len().saturating_sub(MIN_RECENT_MESSAGES).max(system_count);

        // Compress tool results in the older body (between system prefix and recent tail)
        for msg in messages[system_count..recent_start].iter_mut() {
            if let Message::Tool { content, name, .. } = msg {
                let original_tokens = self.token_counter.estimate_text(content);
                if original_tokens > 50 {
                    let summary = Self::summarize_tool_result(name, content);
                    *content = summary;
                }
            }
        }

        let new_tokens = self.estimate_tokens(messages);
        info!(
            before = total_tokens,
            after = new_tokens,
            saved = total_tokens.saturating_sub(new_tokens),
            "mid-loop compression complete"
        );

        Some((total_tokens, new_tokens))
    }

    /// Create a short summary of a tool result.
    fn summarize_tool_result(tool_name: &str, content: &str) -> String {
        let trimmed = content.trim();
        if trimmed.len() <= SUMMARY_SNIPPET_LENGTH {
            return trimmed.to_string();
        }
        // Take first SUMMARY_SNIPPET_LENGTH chars, find a clean break point
        let snippet: String = trimmed.chars().take(SUMMARY_SNIPPET_LENGTH).collect();
        let break_point = snippet
            .rfind('\n')
            .or_else(|| snippet.rfind(". "))
            .or_else(|| snippet.rfind(' '))
            .unwrap_or(snippet.len());
        format!(
            "{}... [compressed {tool_name} result, originally {} chars]",
            &snippet[..break_point],
            trimmed.len()
        )
    }
}
```

- [ ] **Step 4: Declare the module**

In `crates/agent/src/execution/mod.rs`, add:
```rust
pub mod mid_loop_compressor;
```

- [ ] **Step 5: Run tests**

Run: `cargo nextest run -p agent -E 'test(mid_loop)' --no-capture`
Expected: All 4 tests PASS.

- [ ] **Step 6: Run clippy**

Run: `cargo clippy -p agent`
Expected: Zero new warnings.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/execution/mid_loop_compressor.rs crates/agent/src/execution/mod.rs
git commit -m "feat(execution): add MidLoopCompressor for mid-loop context compression"
```

---

## Task 4: Integrate `MidLoopCompressor` into `ReactiveEngine`

**Files:**
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs`

- [ ] **Step 1: Create the compressor in the execute method**

At the top of `ReactiveEngine::execute`, after `let mut messages = messages;`, create the compressor:

```rust
        let compressor = crate::execution::mid_loop_compressor::MidLoopCompressor::new(
            Arc::clone(&self.core.token_counter),
            params.context_window,
        );
```

- [ ] **Step 2: Add compression check after each iteration**

Inside the `for` loop, after the `match outcome { ... }` block and after the oscillation detection check (around line 336), add:

```rust
            // Mid-loop compression: compress older tool results if approaching context limit
            compressor.compress_if_needed(&mut messages);
```

This placement is ideal because:
- It runs after tool results have been appended (from `run_cycle`)
- It runs after reflection/duplicate prompts have been added
- It runs before the next iteration's `run_cycle` call
- It's a no-op when under threshold (returns immediately)

- [ ] **Step 3: Verify compilation**

Run: `cargo check -p agent`
Expected: Compiles.

- [ ] **Step 4: Run all agent tests**

Run: `cargo nextest run -p agent`
Expected: All PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/intent_pipeline/engines/reactive.rs
git commit -m "feat(reactive): integrate MidLoopCompressor into ReAct loop"
```

---

## Task 5: Wire `context_window` from runtime config

**Files:**
- Modify: `crates/agent/src/agent_runtime/runtime.rs`

- [ ] **Step 1: Pass `context_window` to `ExecutionParams`**

In `runtime.rs`, find **both** places where `ExecutionParams` is constructed (search for `ExecutionParams::new`):

1. **Main pipeline** (around line 692) — the primary execution path
2. **Delegation path** (around line 1382) — when an agent delegates to a sub-agent

Add `.with_context_window(self.config.context_window)` to **both** builder chains:

```rust
    .with_context_window(self.config.context_window)
```

`self.config.context_window` is confirmed to exist on `PipelineConfig` (line 140 of `runtime.rs` types) and defaults to `128_000`. This is the same value that `BudgetAllocator` uses during initial context assembly.

- [ ] **Step 2: Verify compilation**

Run: `cargo check -p agent`
Expected: Compiles.

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p agent`
Expected: All PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/agent_runtime/runtime.rs
git commit -m "feat(runtime): pass context_window to ExecutionParams for mid-loop compression"
```

---

## Task 6: Add event emission for compression transparency

**Files:**
- Modify: `crates/agent/src/events.rs` (or wherever `AgentEvent` is defined)
- Modify: `crates/agent/src/execution/mid_loop_compressor.rs`
- Modify: `crates/agent/src/intent_pipeline/engines/reactive.rs`

- [ ] **Step 1: Add `ContextCompressed` event variant**

Find where `AgentEvent` enum is defined (likely `crates/agent/src/events.rs`). Add:

```rust
    /// Mid-loop context compression was triggered.
    ContextCompressed {
        before_tokens: usize,
        after_tokens: usize,
        iteration: usize,
    },
```

- [ ] **Step 2: Emit the event in the reactive loop**

`compress_if_needed` already returns `Option<(usize, usize)>` from Task 3.

In `reactive.rs`, update the compression call:

```rust
            if let Some((before, after)) = compressor.compress_if_needed(&mut messages) {
                if let Some(ref tx) = event_tx {
                    let _ = tx
                        .send(crate::events::AgentEvent::ContextCompressed {
                            before_tokens: before,
                            after_tokens: after,
                            iteration: iteration as usize,
                        })
                        .await;
                }
            }
```

- [ ] **Step 3: Run tests**

Run: `cargo nextest run -p agent`
Expected: All PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/events.rs crates/agent/src/intent_pipeline/engines/reactive.rs
git commit -m "feat(execution): emit ContextCompressed event for transparency"
```

---

## Task 7: Full verification + CLAUDE.md update

**Files:**
- Modify: `CLAUDE.md`

- [ ] **Step 1: Run full workspace tests**

Run: `cargo nextest run --workspace`
Expected: All tests PASS (except the pre-existing `test_estimation_stats` flake).

- [ ] **Step 2: Run clippy**

Run: `cargo clippy --workspace --all-targets`
Expected: Zero new warnings.

- [ ] **Step 3: Run format check**

Run: `cargo fmt --all --check`
Expected: No formatting issues.

- [ ] **Step 4: Update CLAUDE.md**

In the "Agent runtime" section, add after the existing execution modes description:

```
**Mid-loop context compression:** During Reactive execution, the `MidLoopCompressor` checks token usage after each iteration. When accumulated message tokens exceed 70% of the context window, older `Message::Tool` results are replaced with extractive summaries (first 150 chars + metadata). System messages and recent iterations (last 8 messages) are always preserved verbatim. Emits `AgentEvent::ContextCompressed` for UI transparency.
```

- [ ] **Step 5: Commit**

```bash
git add CLAUDE.md
git commit -m "docs: document mid-loop context compression in CLAUDE.md"
```
