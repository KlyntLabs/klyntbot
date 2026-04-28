# Memory System Gap Remediation — Comprehensive Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Close all verified implementation gaps in Klynt's three memory layers (cognitive, coding-memory, context-engine) — MCP observability, multipart tool results, ingest-adapter completeness, recall ranking, brain-graph wiring, and dead-code cleanup.

**Architecture:** Twenty discrete fixes grouped into six independent feature areas. Each task is bounded to a single change, has a failing test before the implementation, and ends with a commit. No task depends on a later task. Two gaps from the original 22-item audit (convergence_score writes, ContextUpdate producers) were proven invalid during verification and are explicitly excluded.

**Tech Stack:** Rust stable 1.93, `cargo-nextest`, `proptest`, `sqlx`, `serde`, `jiff`, `tokio`. UI is unaffected; this plan is backend-only.

**Prerequisite reading (for the executing engineer):** `CLAUDE.md` at repo root — especially the workspace layout, the "Pre-release: no user data to migrate" rule (schema changes can be in-place), the "no `--no-verify`" rule, and the `klynt_command` macro requirement for any new Tauri commands (none expected here). All work happens in Rust crates only.

---

## Verified-Invalid Items (Excluded From This Plan)

These appeared in the original audit but verification disproved them. Do not implement; do not write tasks for them.

- **convergence_score never written.** Verified: written by `cognitive/src/pipeline/consolidator.rs:65` → `writer.rs:70` → `repos/semantic_fact.rs:83/105/130`. Working as intended.
- **`ContextUpdateReason::BudgetThresholdCrossed` / `NoteStructureChanged` have no producers.** Verified: `agent/src/adapters/finance_tree_builder.rs:231` produces the first; `note_tree_builder.rs:230` and `task_tree_builder.rs:171` produce the second. Both reasons are wired.

---

## File Structure

This plan touches the following files (Create / Modify):

**Section A — MCP Observability**
- Modify: `crates/klyntbot-server/src/bridge/registry.rs`
- Modify: `crates/klyntbot-server/src/bridge/mod.rs` (or wherever `ToolRegistryBridge::new` is constructed in `lib.rs`)
- Modify: `crates/klyntbot-server/src/lib.rs` (handler construction)
- Modify: `crates/cognitive/src/pipeline/chat_turn_collector.rs`
- Locate + modify: actual publish site of `DomainEvent::ChatTurnCompleted` (search task in A0)

**Section B — Multipart Tool Results**
- Modify: `crates/providers/src/types.rs`
- Modify: `crates/providers/src/adapters/anthropic_native.rs`
- Modify: `crates/agent/src/execution/mid_loop_compressor.rs`
- Modify: `crates/agent/src/execution/core.rs`
- Modify: `crates/context_engine/src/token_counter.rs`
- Modify: `crates/context_engine/src/history_compressor/types.rs`
- Modify: `crates/context_engine/src/history_compressor/grouping.rs` (test fixtures)
- Modify: `crates/context_engine/src/history_compressor/tiered.rs`
- Modify: `crates/simulator/src/providers/simulation_provider.rs`

**Section C — Ingest Adapter Completion**
- Modify: `crates/coding-ingest/src/adapters/opencode/normalize.rs`
- Modify: `crates/coding-ingest/src/adapters/opencode/poller.rs`
- Modify: `crates/coding-ingest/src/adapters/opencode/schema.rs` (if it exists; otherwise create)
- Create + populate: `crates/coding-ingest/src/adapters/kimi_cli/wire.rs`
- Modify: `crates/coding-ingest/src/adapters/kimi_cli/mod.rs`
- Modify: `crates/coding-ingest/src/adapters/claude_code/dispatch.rs`
- Modify: `crates/coding-ingest/src/adapters/codex/dispatch.rs`
- Modify: `crates/coding-ingest/tests/cross_cli_normalization.rs`

**Section D — Recall Ranking & Causal Surface**
- Modify: `crates/coding-memory/src/recall/service.rs`
- Modify: `crates/coding-memory/src/recall/renderers.rs`
- Modify: `crates/config/src/schema/coding_memory.rs`
- Create: `crates/coding-memory/migrations/006_recall_weights.sql`
- Modify: `crates/coding-memory/src/lib.rs` (migrations list)

**Section E — Graph Completion**
- Modify: `crates/cognitive/src/services/louvain.rs`

**Section F — Cleanup & Integration**
- Modify: `crates/cognitive/src/services/background.rs` (delete dead `to_accumulate`)
- Modify: `crates/cognitive/src/services/reforge/types.rs` (drop `distraction_rules_to_promote`)
- Modify: `crates/cognitive/src/services/reforge/collector.rs`
- Modify: `crates/cognitive/src/services/community_intelligence/mod.rs` (use `MIN_AGE_FOR_RESTRUCTURE`)
- Modify: `crates/app-core/src/init/cron.rs` (autotuner re-enable, FSRS weekly job)
- Create: `crates/cognitive/src/services/autotuner_phase6.rs` (or extend `reforge/service.rs`)
- Modify: `crates/coding-memory/src/distiller/mod.rs`
- Modify: `crates/coding-memory/src/reforge/cross_session_dedup.rs`
- Modify: `crates/coding-memory/src/reforge/selective_delete.rs`
- Modify: `crates/app-core/src/coding_memory/reforge.rs`
- Modify: `crates/config/src/schema/coding_memory.rs`
- Modify: `crates/config/src/schema/cognitive.rs`
- Modify: `crates/context_engine/src/assembler/mod.rs` (or compressor) — wire `compress_with_delta`
- Create: `docs/architecture/domain-event-subscribers.md` (declarative subscriber registry)

---

# Section A — MCP Observability

Two gaps:
1. `ToolRegistryBridge::execute` does not publish `DomainEvent::ToolCallExecuted` after invoking the tool, so external MCP clients are invisible to the cognitive activity log, coding-memory distiller, and outcome recorder.
2. `DomainEvent::ChatTurnCompleted.user_message` is `Option<String>` with `#[serde(default)]`. The cognitive pipeline must guard the `None` case explicitly.

### Task A0: Locate the actual `ChatTurnCompleted` publisher

**Files:**
- Search-only — no edits in this task.

- [ ] **Step 1: Run a workspace-wide search.**

```bash
rg -n "ChatTurnCompleted \{" crates/ --type rust
```

Expected: at least one match outside `bus/src/domain_events.rs` (the definition). Record the file:line. If the only match is the definition itself, the event is never published — escalate by widening to `DomainEvent::ChatTurnCompleted`:

```bash
rg -n "DomainEvent::ChatTurnCompleted" crates/ --type rust
```

- [ ] **Step 2: Capture findings as a comment in this file.**

Edit the top of this section in this plan document to record the verified publisher path. All subsequent tasks in Section A reference it as `<publisher.rs:LINE>`.

- [ ] **Step 3: Commit findings only if the doc was edited.**

```bash
git add docs/superpowers/plans/2026-04-28-memory-gaps-comprehensive.md
git commit -m "docs(plan): record ChatTurnCompleted publisher location"
```

If no edit was made (e.g. publisher is exactly where claimed), skip the commit.

---

### Task A1: Failing test — `ToolRegistryBridge::execute` publishes `ToolCallExecuted`

**Files:**
- Test: `crates/klyntbot-server/tests/registry_bridge_publishes_events.rs` (CREATE)

- [ ] **Step 1: Write the failing test.**

```rust
//! Verifies that the MCP `ToolRegistryBridge` publishes `DomainEvent::ToolCallExecuted`
//! after a successful tool execution, so the cognitive activity log and coding-memory
//! distiller observe MCP-driven tool calls.

use bus::{DomainEvent, DomainEventBus};
use klyntbot_server::bridge::ToolRegistryBridge;
use std::sync::Arc;

#[tokio::test]
async fn mcp_tool_call_publishes_tool_call_executed() {
    let bus = Arc::new(DomainEventBus::new(64));
    let mut rx = bus.subscribe();

    let registry = klyntbot_server::test_support::registry_with_echo_tool();
    let bridge = ToolRegistryBridge::new_with_bus(registry, vec!["echo".into()], bus.clone());

    let result = bridge
        .execute("echo", serde_json::json!({"text": "hi"}), "session-1")
        .await
        .expect("execute");

    // Drain bus — expect at least one ToolCallExecuted event for "echo".
    let mut saw = false;
    while let Ok(ev) = rx.try_recv() {
        if let DomainEvent::ToolCallExecuted { tool_name, channel, .. } = ev {
            if tool_name == "echo" && channel == "mcp" {
                saw = true;
                break;
            }
        }
    }
    assert!(saw, "ToolCallExecuted not published for MCP echo call");
    assert!(result.is_ok());
}
```

- [ ] **Step 2: Run it to confirm failure.**

```bash
cargo nextest run -p klyntbot-server --test registry_bridge_publishes_events
```

Expected: FAIL — either `new_with_bus` is undefined or no event arrives.

---

### Task A2: Add `domain_bus` to `ToolRegistryBridge`

**Files:**
- Modify: `crates/klyntbot-server/src/bridge/registry.rs`
- Modify: `crates/klyntbot-server/src/lib.rs` (or wherever the bridge is constructed)

- [ ] **Step 1: Extend the struct + add `new_with_bus`.**

In `registry.rs`, locate the `ToolRegistryBridge` struct definition (above line 75). Add a field:

```rust
pub struct ToolRegistryBridge {
    registry: Arc<RwLock<ToolRegistry>>,
    whitelist: Vec<String>,
    domain_bus: Option<Arc<bus::DomainEventBus>>,
}
```

Replace the existing `new` method body to default `domain_bus: None`, and add:

```rust
pub fn new_with_bus(
    registry: Arc<RwLock<ToolRegistry>>,
    whitelist: Vec<String>,
    domain_bus: Arc<bus::DomainEventBus>,
) -> Self {
    Self {
        registry,
        whitelist,
        domain_bus: Some(domain_bus),
    }
}
```

- [ ] **Step 2: Wire the bus through at construction.**

In `crates/klyntbot-server/src/lib.rs`, find every `ToolRegistryBridge::new(...)` call and pass `Arc<DomainEventBus>` from the surrounding context. The desktop server already constructs `DomainEventBus` in `app-core::init` — pipe it through the `KlyntbotServerHandler::new` signature.

- [ ] **Step 3: Build to confirm compilation.**

```bash
cargo build -p klyntbot-server
```

Expected: success.

---

### Task A3: Publish `ToolCallExecuted` after `tool.execute()`

**Files:**
- Modify: `crates/klyntbot-server/src/bridge/registry.rs:114-117`

- [ ] **Step 1: Replace the `match tool.execute(...)` block.**

Current (lines 114–117):

```rust
match tool.execute(arguments, &ctx).await {
    Ok(result) => Ok(CallToolResult::success(vec![Content::text(result)])),
    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
}
```

Replace with:

```rust
let started_at = std::time::Instant::now();
let outcome = tool.execute(arguments.clone(), &ctx).await;
let duration_ms = started_at.elapsed().as_millis() as u64;

if let Some(bus) = &self.domain_bus {
    bus.publish(bus::DomainEvent::ToolCallExecuted {
        session_key: format!("mcp:{session_id}"),
        tool_name: tool_name.to_string(),
        channel: "mcp".into(),
        success: outcome.is_ok(),
        duration_ms,
        arguments_preview: arguments
            .to_string()
            .chars()
            .take(512)
            .collect::<String>(),
    });
}

match outcome {
    Ok(result) => Ok(CallToolResult::success(vec![Content::text(result)])),
    Err(e) => Ok(CallToolResult::error(vec![Content::text(e.to_string())])),
}
```

If `DomainEvent::ToolCallExecuted`'s shape differs from what is shown above, open `crates/bus/src/domain_events.rs` and read the existing `ToolCallExecuted` variant — match its field set exactly. If the variant does not exist, add it with the fields above (and update consumers — search `rg -n "ToolCallExecuted"` first).

- [ ] **Step 2: Run the failing test to confirm pass.**

```bash
cargo nextest run -p klyntbot-server --test registry_bridge_publishes_events
```

Expected: PASS.

- [ ] **Step 3: Run the full server crate.**

```bash
cargo nextest run -p klyntbot-server
cargo clippy -p klyntbot-server --all-targets -- -D warnings
```

Expected: 0 failures, 0 warnings.

- [ ] **Step 4: Commit.**

```bash
git add crates/klyntbot-server/
git commit -m "fix(mcp): publish ToolCallExecuted from ToolRegistryBridge

External MCP clients calling whitelisted tools (tasks, notes, memory, etc.)
were invisible to the cognitive activity log and coding-memory distiller.
ToolRegistryBridge now publishes ToolCallExecuted on every dispatch."
```

---

### Task A4: Failing test — `ChatTurnCollector` skips when `user_message` is `None`

**Files:**
- Test: `crates/cognitive/src/pipeline/chat_turn_collector.rs` (test module at the bottom)

- [ ] **Step 1: Read the current test module structure** to match its conventions:

```bash
cargo nextest run -p cognitive --test-threads 1 chat_turn_collector -- --list
```

- [ ] **Step 2: Add a failing test inside the existing `#[cfg(test)] mod tests` block.**

```rust
#[tokio::test]
async fn skips_when_user_message_is_none() {
    let collector = ChatTurnCollector::new();
    let event = bus::DomainEvent::ChatTurnCompleted {
        session_key: "s1".into(),
        user_message: None,
    };
    let signal = collector.consume(&event).await;
    assert!(
        signal.is_none(),
        "ChatTurnCollector must drop events with user_message=None instead of \
         emitting an empty-content signal"
    );
}
```

- [ ] **Step 3: Run to confirm failure.**

```bash
cargo nextest run -p cognitive chat_turn_collector::tests::skips_when_user_message_is_none
```

Expected: FAIL or panic (current code likely passes empty content forward).

---

### Task A5: Guard `None` in `ChatTurnCollector::consume`

**Files:**
- Modify: `crates/cognitive/src/pipeline/chat_turn_collector.rs`

- [ ] **Step 1: Patch `consume`.**

Find the match arm handling `DomainEvent::ChatTurnCompleted`. Add an early-return on `None`:

```rust
DomainEvent::ChatTurnCompleted { session_key, user_message } => {
    let Some(content) = user_message.as_ref() else {
        tracing::debug!(
            session_key = %session_key,
            "ChatTurnCollector: dropping ChatTurnCompleted with user_message=None"
        );
        return None;
    };
    if content.trim().len() < 20 {
        return None;
    }
    // ... existing emit path using `content` instead of `user_message.unwrap_or_default()`
}
```

- [ ] **Step 2: Run the targeted test.**

```bash
cargo nextest run -p cognitive chat_turn_collector::tests::skips_when_user_message_is_none
```

Expected: PASS.

- [ ] **Step 3: Run the full crate.**

```bash
cargo nextest run -p cognitive
cargo clippy -p cognitive --all-targets -- -D warnings
```

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/src/pipeline/chat_turn_collector.rs
git commit -m "fix(cognitive): drop ChatTurnCompleted events with user_message=None

Previously the collector forwarded an empty content string to the signal
queue, polluting accumulated_observations with content-less rows."
```

---

# Section B — Multipart Tool Results

`Message::Tool.content` is currently `String`. Image-bearing tool results (computer-use, screenshot tools) cannot be represented; the mid-loop compressor will silently truncate base64 to 150 chars and corrupt image data. This section migrates `Message::Tool` to a multipart-capable content type, with strict invariants: text-only callers continue to work via a `From<String>` impl; the Anthropic adapter emits `tool_result` blocks correctly for both text and image; the compressor only truncates text segments.

### Task B0: Failing test — `Message::Tool` accepts image content

**Files:**
- Test: `crates/providers/src/types.rs` (existing `#[cfg(test)] mod tests`, or create one at bottom)

- [ ] **Step 1: Add the failing test.**

```rust
#[test]
fn tool_message_can_carry_image_part() {
    let msg = Message::Tool {
        tool_call_id: "tc1".into(),
        name: "screenshot".into(),
        content: ToolContent::MultiPart(vec![
            ToolContentPart::Text { text: "captured screen at 1920x1080".into() },
            ToolContentPart::ImageData {
                media_type: "image/png".into(),
                data: "base64-blob".into(),
            },
        ]),
    };

    let json = serde_json::to_string(&msg).unwrap();
    assert!(json.contains("image_data"), "should serialize image part: {json}");

    let parsed: Message = serde_json::from_str(&json).unwrap();
    if let Message::Tool { content, .. } = parsed {
        assert!(matches!(content, ToolContent::MultiPart(_)));
    } else {
        panic!("wrong variant");
    }
}
```

- [ ] **Step 2: Confirm failure.**

```bash
cargo nextest run -p providers tool_message_can_carry_image_part
```

Expected: COMPILE ERROR (`ToolContent` and `ToolContentPart` undefined).

---

### Task B1: Define `ToolContent` + `ToolContentPart` + migrate `Message::Tool`

**Files:**
- Modify: `crates/providers/src/types.rs:376-380, 472-482`

- [ ] **Step 1: Add new types above the `Message` enum (after `ContentPart` at line 419).**

```rust
/// Tool result content. Backwards-compatible: deserializes from a bare string
/// into `Text(_)`. Multipart variant carries text + image_data parts for
/// vision / computer-use tools.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ToolContent {
    Text(String),
    MultiPart(Vec<ToolContentPart>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ToolContentPart {
    Text { text: String },
    ImageData { media_type: String, data: String },
}

impl From<String> for ToolContent {
    fn from(s: String) -> Self {
        ToolContent::Text(s)
    }
}

impl From<&str> for ToolContent {
    fn from(s: &str) -> Self {
        ToolContent::Text(s.to_string())
    }
}

impl ToolContent {
    /// Concatenated text for token counting and extractive snippets.
    /// Image parts contribute a fixed 1024-token surrogate (Anthropic's billing
    /// approximation for ≤1568×1568 images).
    pub fn as_text(&self) -> String {
        match self {
            ToolContent::Text(s) => s.clone(),
            ToolContent::MultiPart(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ToolContentPart::Text { text } => Some(text.clone()),
                    ToolContentPart::ImageData { .. } => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    pub fn image_part_count(&self) -> usize {
        match self {
            ToolContent::Text(_) => 0,
            ToolContent::MultiPart(parts) => parts
                .iter()
                .filter(|p| matches!(p, ToolContentPart::ImageData { .. }))
                .count(),
        }
    }
}
```

- [ ] **Step 2: Change `Message::Tool.content` type.**

Modify lines 376–380:

```rust
    Tool {
        tool_call_id: String,
        name: String,
        content: ToolContent,
    },
```

- [ ] **Step 3: Update the `tool` constructor (lines 472–482).**

```rust
    pub fn tool(
        tool_call_id: impl Into<String>,
        name: impl Into<String>,
        content: impl Into<ToolContent>,
    ) -> Self {
        Self::Tool {
            tool_call_id: tool_call_id.into(),
            name: name.into(),
            content: content.into(),
        }
    }
```

- [ ] **Step 4: Build the workspace and read the cascade of errors.**

```bash
cargo build --workspace 2>&1 | head -200
```

Expected: errors at every `Message::Tool { content, .. }` consumer (token_counter, mid_loop_compressor, anthropic_native, history_compressor, simulator). Tasks B2–B7 fix each.

---

### Task B2: Update Anthropic adapter `convert_messages`

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs:156-170`

- [ ] **Step 1: Replace the existing `Message::Tool` arm.**

Current:

```rust
Message::Tool { tool_call_id, content, .. } => {
    result.push(json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": tool_call_id,
            "content": content,
        }]
    }));
}
```

Replace with:

```rust
Message::Tool { tool_call_id, content, .. } => {
    let blocks = match content {
        ToolContent::Text(text) => vec![json!({"type": "text", "text": text})],
        ToolContent::MultiPart(parts) => parts
            .iter()
            .map(|p| match p {
                ToolContentPart::Text { text } => json!({"type": "text", "text": text}),
                ToolContentPart::ImageData { media_type, data } => json!({
                    "type": "image",
                    "source": {
                        "type": "base64",
                        "media_type": media_type,
                        "data": data,
                    }
                }),
            })
            .collect(),
    };
    result.push(json!({
        "role": "user",
        "content": [{
            "type": "tool_result",
            "tool_use_id": tool_call_id,
            "content": blocks,
        }]
    }));
}
```

Add `use crate::types::{ToolContent, ToolContentPart};` at the top of the file if not already in scope.

- [ ] **Step 2: Build to verify this site compiles.**

```bash
cargo build -p providers
```

Expected: success.

---

### Task B3: Update `mid_loop_compressor` to be image-aware

**Files:**
- Modify: `crates/agent/src/execution/mid_loop_compressor.rs:82-96`

- [ ] **Step 1: Replace the iteration body.**

```rust
for msg in messages[system_count..recent_start].iter_mut() {
    if let Message::Tool { content, name, .. } = msg {
        let original_text = content.as_text();
        let image_count = content.image_part_count();
        let original_tokens = self.token_counter.estimate_text(&original_text)
            + image_count * 1024;
        if original_tokens > MIN_COMPRESSIBLE_TOKENS {
            let summary_text = format!(
                "{}... [compressed {name} result, originally {} chars + {} image part(s)]",
                context_engine::first_snippet(&original_text, SUMMARY_SNIPPET_LENGTH),
                original_text.len(),
                image_count,
            );
            let new_tokens = self.token_counter.estimate_text(&summary_text);
            saved_tokens += original_tokens.saturating_sub(new_tokens);
            // Image parts are dropped; text becomes the summary. Compression
            // is one-way; the original payload is gone after this step.
            *content = providers::ToolContent::Text(summary_text);
        }
    }
}
```

- [ ] **Step 2: Update the test helper at line 137 in the same file.**

```rust
Message::Tool {
    tool_call_id: id.to_string(),
    name: name.to_string(),
    content: providers::ToolContent::Text(result.to_string()),
}
```

---

### Task B4: Update `agent/execution/core.rs` token counting

**Files:**
- Modify: `crates/agent/src/execution/core.rs:303`

- [ ] **Step 1: Replace.**

Current:
```rust
Message::Tool { content: c, .. } => counter.estimate_text(c),
```

Replace with:
```rust
Message::Tool { content: c, .. } => {
    counter.estimate_text(&c.as_text()) + c.image_part_count() * 1024
}
```

---

### Task B5: Update `context_engine` token counter + history compressor

**Files:**
- Modify: `crates/context_engine/src/token_counter.rs:89`
- Modify: `crates/context_engine/src/history_compressor/types.rs:87`
- Modify: `crates/context_engine/src/history_compressor/tiered.rs:387, 437`
- Modify: `crates/context_engine/src/history_compressor/grouping.rs:146` (test fixture)

- [ ] **Step 1: `token_counter.rs:89`**

```rust
providers::Message::Tool { content, .. } => {
    counter.estimate_text(&content.as_text()) + content.image_part_count() * 1024 + 10
}
```

- [ ] **Step 2: `history_compressor/types.rs:87`**

Replace the `if let Message::Tool { name, content, .. } = m` body to consume `content.as_text()`:

```rust
if let Message::Tool { name, content, .. } = m {
    let text = content.as_text();
    // ... existing snippet logic but operating on `text` instead of `content`
}
```

- [ ] **Step 3: `history_compressor/tiered.rs:387` (summarization).**

```rust
Message::Tool { name, content, .. } => {
    lines.push(format!("{}: {}", name, content.as_text()));
}
```

- [ ] **Step 4: `history_compressor/tiered.rs:437` (compactable check).**

```rust
if let Message::Tool { name, content, .. } = msg {
    let text = content.as_text();
    // ... continue using `text`
}
```

When the compactable path rewrites the message, do `*content = providers::ToolContent::Text(truncated)`.

- [ ] **Step 5: All test fixtures in `tiered.rs` (lines 661, 696, 723) and `grouping.rs:146`.**

Replace each:
```rust
content: "A".repeat(5000),
```
with:
```rust
content: providers::ToolContent::Text("A".repeat(5000)),
```

For test assertions (lines 673, 707, 733):

```rust
if let Message::Tool { content, .. } = &compacted[2] {
    let text = content.as_text();
    assert!(text.len() < 5000);
}
```

---

### Task B6: Update `simulator`

**Files:**
- Modify: `crates/simulator/src/providers/simulation_provider.rs:75`

- [ ] **Step 1:** No code change needed — line 75 already uses `name.as_str()` and does not touch `content`. Just rebuild and confirm.

```bash
cargo build -p simulator
```

---

### Task B7: Run B0 + full test sweep

- [ ] **Step 1: Run the original failing test.**

```bash
cargo nextest run -p providers tool_message_can_carry_image_part
```

Expected: PASS.

- [ ] **Step 2: Run all touched crates.**

```bash
cargo nextest run -p providers -p agent -p context_engine -p simulator
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: 0 failures, 0 warnings.

- [ ] **Step 3: Commit.**

```bash
git add crates/providers crates/agent crates/context_engine crates/simulator
git commit -m "feat(providers): multipart Message::Tool content for image-bearing tool results

Introduces ToolContent::{Text,MultiPart} with backwards-compatible
deserialization (bare string → Text). Anthropic adapter now emits
tool_result blocks with image content when present. MidLoopCompressor
preserves image-aware token accounting and discards image data only when
the text-summary replacement is applied. Unblocks computer-use tooling."
```

---

# Section C — Ingest Adapter Completion

Four sub-gaps:
- Opencode: `cwd` hardcoded `"/"`, `turn_id` always `None`, `repo` always `None`, tool detection is heuristic JSON-prefix sniff.
- kimi-cli: Tier-2 wire streaming declared in module but `wire.rs` exists only as an empty stub.
- Claude Code + Codex: `diff_preview = None` for every file edit.
- Inv 7 proptest covers only 5 of 18 `EventKind` variants.

### Task C1: Failing test — opencode normalization recovers `cwd`, `repo`, `turn_id`

**Files:**
- Test: `crates/coding-ingest/tests/opencode_normalize.rs` (CREATE)

- [ ] **Step 1: Write the test.**

```rust
use coding_ingest::adapters::opencode::{normalize, schema::MessageRow};
use coding_ingest::event::{AgentSource, EventKind};

#[test]
fn normalize_recovers_cwd_from_metadata() {
    let row = MessageRow {
        id: 1,
        session_id: "s1".into(),
        role: "user".into(),
        content: "what does this repo do".into(),
        tool_calls: None,
        tool_call_id: None,
        metadata: Some(serde_json::json!({"cwd": "/Users/me/code/myrepo"}).to_string()),
        created_at: 1700000000,
    };
    let v1 = normalize::row_to_event(row).unwrap().unwrap();
    assert_eq!(v1.cwd, std::path::PathBuf::from("/Users/me/code/myrepo"));
    assert!(v1.repo.is_some(), "repo should resolve from cwd via RepoScope");
    assert_eq!(v1.source, AgentSource::OpenCode);
}

#[test]
fn normalize_groups_into_turns_by_session_and_assistant_boundary() {
    let user = MessageRow {
        id: 10,
        session_id: "s1".into(),
        role: "user".into(),
        content: "hello".into(),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
        created_at: 1700000000,
    };
    let assistant = MessageRow {
        id: 11,
        session_id: "s1".into(),
        role: "assistant".into(),
        content: "hi back".into(),
        tool_calls: None,
        tool_call_id: None,
        metadata: None,
        created_at: 1700000005,
    };
    let u_evt = normalize::row_to_event(user).unwrap().unwrap();
    let a_evt = normalize::row_to_event(assistant).unwrap().unwrap();
    assert_eq!(u_evt.turn_id, a_evt.turn_id);
    assert!(u_evt.turn_id.is_some());
}

#[test]
fn assistant_with_tool_calls_column_classifies_as_toolcall_not_heuristic() {
    let row = MessageRow {
        id: 1,
        session_id: "s1".into(),
        role: "assistant".into(),
        content: "{ \"this is just JSON the model returned, not a tool call\": true }".into(),
        tool_calls: None,        // ← no actual tool call
        tool_call_id: None,
        metadata: None,
        created_at: 1700000000,
    };
    let v1 = normalize::row_to_event(row).unwrap().unwrap();
    assert!(matches!(v1.kind, EventKind::AssistantMsg { .. }),
        "must NOT classify as ToolCall when tool_calls column is empty");
}
```

- [ ] **Step 2: Run to confirm failure.**

```bash
cargo nextest run -p coding-ingest --test opencode_normalize
```

Expected: FAIL — `cwd` is `/`, `turn_id` is `None`, third test FAILs because the heuristic misclassifies.

---

### Task C2: Add `metadata` + structured tool-call columns to `MessageRow`

**Files:**
- Modify: `crates/coding-ingest/src/adapters/opencode/schema.rs` (CREATE if missing) and `poller.rs:65-72`
- Modify: `crates/coding-ingest/src/adapters/opencode/normalize.rs`

- [ ] **Step 1: Read or create `schema.rs`.**

```bash
ls crates/coding-ingest/src/adapters/opencode/
```

If `schema.rs` doesn't exist, create with `MessageRow` mirroring the SQL columns referenced at `poller.rs:66`. Add `metadata: Option<String>` and `tool_calls: Option<String>` (both already in the SQL). Confirm the FromRow impl.

- [ ] **Step 2: Rewrite `normalize::row_to_event`.**

```rust
pub fn row_to_event(row: MessageRow) -> Result<Option<AgentEventV1>> {
    let metadata: Option<serde_json::Value> = row.metadata
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    let cwd = metadata
        .as_ref()
        .and_then(|m| m.get("cwd").and_then(|v| v.as_str()))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("/"));

    let repo = crate::scope::RepoScope::detect(&cwd).ok().flatten();
    let turn_id = Some(format!("{}-{}", row.session_id, turn_bucket(row.id)));

    let kind = match row.role.as_str() {
        "system" => return Ok(None),
        "user" => EventKind::UserPrompt {
            text: row.content,
            attachments: vec![],
        },
        "assistant" => {
            // Use the structured tool_calls column, NOT a content-prefix heuristic.
            if let Some(tc_json) = row.tool_calls.as_deref() {
                if let Ok(tc_arr) = serde_json::from_str::<Vec<serde_json::Value>>(tc_json) {
                    if let Some(first) = tc_arr.first() {
                        let tool = first.get("name").and_then(|v| v.as_str()).unwrap_or("unknown").to_string();
                        let args_preview = first.get("arguments")
                            .and_then(|a| serde_json::to_string(a).ok())
                            .unwrap_or_default();
                        return Ok(Some(AgentEventV1 {
                            id: Uuid::new_v4(),
                            source: AgentSource::OpenCode,
                            session_id: row.session_id,
                            turn_id,
                            cwd,
                            repo,
                            occurred_at: row_timestamp(row.created_at),
                            kind: EventKind::ToolCall {
                                tool,
                                args_preview: args_preview.chars().take(512).collect(),
                                ok: true,
                                duration_ms: 0,
                                result_preview: String::new(),
                            },
                        }));
                    }
                }
            }
            EventKind::AssistantMsg {
                text: row.content,
                truncated: false,
                token_usage: None,
            }
        }
        "tool" => EventKind::ToolCall {
            tool: row.tool_call_id.unwrap_or_else(|| "opencode_tool".into()),
            args_preview: String::new(),
            ok: true,
            duration_ms: 0,
            result_preview: row.content,
        },
        _ => return Ok(None),
    };

    Ok(Some(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::OpenCode,
        session_id: row.session_id,
        turn_id,
        cwd,
        repo,
        occurred_at: row_timestamp(row.created_at),
        kind,
    }))
}

/// Group consecutive messages into the same logical turn. Naive: every block
/// of N messages = one turn. Use the row id as the bucket source so that
/// (user, assistant) pairs land together. The 1000-id stride is conservative;
/// turn boundaries are refined post-ingest by the distiller.
fn turn_bucket(id: i64) -> i64 {
    id / 2 // user + assistant pair = bucket
}

fn row_timestamp(epoch_secs: i64) -> jiff::Timestamp {
    jiff::Timestamp::from_second(epoch_secs).unwrap_or_else(|_| jiff::Timestamp::now())
}
```

- [ ] **Step 3: Run the failing tests.**

```bash
cargo nextest run -p coding-ingest --test opencode_normalize
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/coding-ingest/src/adapters/opencode/ crates/coding-ingest/tests/opencode_normalize.rs
git commit -m "fix(opencode): recover cwd/repo/turn_id and use structured tool_calls column

Replaces hardcoded cwd=/ and the JSON-prefix heuristic for tool detection
with metadata-driven cwd, RepoScope detection, and the structured
tool_calls column. Adds turn grouping so opencode events can join the
distiller's turn buffer."
```

---

### Task C3: Failing test — diff_preview populated for Claude Code Edit/MultiEdit

**Files:**
- Test: `crates/coding-ingest/tests/claude_code_diff_preview.rs` (CREATE)

- [ ] **Step 1: Write the test.**

```rust
use coding_ingest::adapters::claude_code::ClaudeCodeAdapter;
use coding_ingest::adapters::IngestAdapter;
use coding_ingest::event::{AgentEvent, EventKind, FileOp};

#[test]
fn edit_emits_diff_preview() {
    let adapter = ClaudeCodeAdapter::default();
    let raw = serde_json::json!({
        "session_id": "s1",
        "cwd": "/tmp",
        "tool_name": "Edit",
        "tool_input": {
            "file_path": "/tmp/x.rs",
            "old_string": "fn old() {}",
            "new_string": "fn new() {}"
        },
        "tool_response": {"bytes": 42},
        "duration_ms": 5
    });
    let bytes = serde_json::to_vec(&raw).unwrap();
    let evt = adapter.parse("PostToolUse", &bytes).unwrap().unwrap();
    let AgentEvent::V1(v1) = evt;
    if let EventKind::FileEdit { op, diff_preview, .. } = v1.kind {
        assert_eq!(op, FileOp::Modify);
        let preview = diff_preview.expect("diff_preview should be Some for Edit");
        assert!(preview.contains("-fn old"));
        assert!(preview.contains("+fn new"));
    } else {
        panic!("expected FileEdit");
    }
}
```

- [ ] **Step 2: Confirm failure.**

```bash
cargo nextest run -p coding-ingest --test claude_code_diff_preview
```

Expected: FAIL — `diff_preview` is `None`.

---

### Task C4: Implement `diff_preview` for Edit/MultiEdit/Write

**Files:**
- Modify: `crates/coding-ingest/src/adapters/claude_code/dispatch.rs:65-83`
- Modify: `crates/coding-ingest/src/adapters/codex/dispatch.rs:65-83`

- [ ] **Step 1: Replace `file_edit` in claude_code/dispatch.rs.**

```rust
fn file_edit(b: &payload::ToolUseBody, op: FileOp) -> EventKind {
    let path = b
        .tool_input
        .get("file_path")
        .and_then(|v| v.as_str())
        .map(PathBuf::from)
        .unwrap_or_default();
    let bytes = b
        .tool_response
        .get("bytes")
        .and_then(|v| v.as_u64())
        .unwrap_or(0);
    let diff_preview = build_diff_preview(&b.tool_name, &b.tool_input);
    EventKind::FileEdit { path, op, bytes, diff_preview }
}

fn build_diff_preview(tool: &str, input: &serde_json::Value) -> Option<String> {
    match tool {
        "Edit" => {
            let old = input.get("old_string")?.as_str()?;
            let new = input.get("new_string")?.as_str()?;
            Some(format_unified_two_strings(old, new, 512))
        }
        "MultiEdit" => {
            let edits = input.get("edits")?.as_array()?;
            let mut out = String::new();
            for e in edits.iter().take(3) {
                let old = e.get("old_string").and_then(|v| v.as_str()).unwrap_or("");
                let new = e.get("new_string").and_then(|v| v.as_str()).unwrap_or("");
                out.push_str(&format_unified_two_strings(old, new, 256));
                out.push_str("\n---\n");
            }
            Some(out.chars().take(1024).collect())
        }
        "Write" => {
            let new = input.get("content").and_then(|v| v.as_str()).unwrap_or("");
            Some(format!("+{}", new.chars().take(512).collect::<String>()))
        }
        _ => None,
    }
}

fn format_unified_two_strings(old: &str, new: &str, max: usize) -> String {
    let mut s = String::new();
    for line in old.lines().take(8) {
        s.push('-');
        s.push_str(line);
        s.push('\n');
    }
    for line in new.lines().take(8) {
        s.push('+');
        s.push_str(line);
        s.push('\n');
    }
    s.chars().take(max).collect()
}
```

- [ ] **Step 2: Mirror the change in `codex/dispatch.rs`.**

Same logic. The codex tool names are lowercase (`edit`, `write`); update the match accordingly. Codex does not have `MultiEdit`, so omit that arm.

- [ ] **Step 3: Run tests.**

```bash
cargo nextest run -p coding-ingest
```

Expected: all PASS.

- [ ] **Step 4: Commit.**

```bash
git add crates/coding-ingest/src/adapters/claude_code/ crates/coding-ingest/src/adapters/codex/ crates/coding-ingest/tests/claude_code_diff_preview.rs
git commit -m "feat(ingest): populate diff_preview for Edit/MultiEdit/Write events

Distiller previously had no way to surface what changed in an edit;
diff_preview was always None. Now both Claude Code and Codex adapters
emit a truncated unified-style diff (≤512 chars for Edit, ≤1024 for
MultiEdit, ≤512 for Write)."
```

---

### Task C5: Failing test — kimi-cli wire streaming consumes a Wire frame

**Files:**
- Test: `crates/coding-ingest/src/adapters/kimi_cli/wire.rs` (CREATE; tests inline at bottom)

- [ ] **Step 1: Create the file with a failing test.**

```rust
//! kimi-cli Tier-2: streaming Wire client.
//!
//! Connects to kimi-cli's local Wire socket (Unix domain socket at
//! `~/.kimi/wire.sock` by default) and translates each frame into an
//! `AgentEvent`. Falls back to Tier-1 hooks if the socket is unavailable.

use crate::event::AgentEvent;
use common::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

/// One Wire frame as emitted by kimi-cli (newline-delimited JSON).
#[derive(serde::Deserialize, Debug)]
pub struct WireFrame {
    #[serde(rename = "t")]
    pub frame_type: String,
    pub session_id: String,
    pub payload: serde_json::Value,
}

/// Convert one Wire frame to an `AgentEvent` via the existing dispatch path.
pub fn frame_to_event(frame: &WireFrame) -> Result<Option<AgentEvent>> {
    // Reuse the tier-1 dispatch by mapping frame_type → hook event name.
    let hook_event = match frame.frame_type.as_str() {
        "session_start" => "SessionStart",
        "session_end" => "SessionEnd",
        "user_prompt" => "UserPrompt",
        "assistant_msg" => "AssistantMsg",
        "tool_use" => "ToolUse",
        "skill_activated" => "SkillActivated",
        "recall_injected" => "RecallInjected",
        "approval_decision" => "ApprovalDecision",
        "provider_call" => "ProviderCall",
        _ => return Ok(None),
    };
    let payload_bytes = serde_json::to_vec(&frame.payload)
        .map_err(|e| common::KlyntbotError::Serde(e.to_string()))?;
    super::dispatch::dispatch(hook_event, &payload_bytes).map(|opt| opt.map(AgentEvent::V1))
}

/// Run the streaming loop. Caller owns cancellation by dropping the JoinHandle.
pub async fn run(socket_path: std::path::PathBuf, tx: mpsc::UnboundedSender<AgentEvent>) -> Result<()> {
    let stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| common::KlyntbotError::Io(format!("kimi wire connect: {e}")))?;
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| common::KlyntbotError::Io(e.to_string()))?
    {
        let frame: WireFrame = match serde_json::from_str(&line) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(error = %e, "kimi wire frame parse failed");
                continue;
            }
        };
        match frame_to_event(&frame) {
            Ok(Some(evt)) => {
                if tx.send(evt).is_err() {
                    break;
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(error = %e, "kimi frame dispatch failed"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_to_event_user_prompt() {
        let frame = WireFrame {
            frame_type: "user_prompt".into(),
            session_id: "s1".into(),
            payload: serde_json::json!({
                "session_id": "s1",
                "cwd": "/tmp",
                "prompt": "hi",
                "attachments": []
            }),
        };
        let evt = frame_to_event(&frame).unwrap().expect("frame produced an event");
        let AgentEvent::V1(v1) = evt;
        assert_eq!(v1.session_id, "s1");
    }

    #[test]
    fn unknown_frame_type_returns_none() {
        let frame = WireFrame {
            frame_type: "exotic_unknown".into(),
            session_id: "s1".into(),
            payload: serde_json::Value::Null,
        };
        assert!(frame_to_event(&frame).unwrap().is_none());
    }
}
```

- [ ] **Step 2: Update `kimi_cli/mod.rs` to expose the module + spawn helper.**

In `mod.rs`, the `pub mod wire;` line already exists. Add a top-level helper:

```rust
/// Spawn the Tier-2 Wire streaming loop. Returns a JoinHandle the caller
/// (typically the daemon) can drop to cancel. If the socket is unavailable
/// the future returns immediately with an Io error — tier-1 hooks remain
/// the fallback.
pub fn spawn_wire(
    socket_path: std::path::PathBuf,
    tx: tokio::sync::mpsc::UnboundedSender<crate::event::AgentEvent>,
) -> tokio::task::JoinHandle<common::Result<()>> {
    tokio::spawn(wire::run(socket_path, tx))
}
```

- [ ] **Step 3: Run tests.**

```bash
cargo nextest run -p coding-ingest kimi_cli::wire
```

Expected: PASS.

- [ ] **Step 4: Wire the daemon entry to start `spawn_wire` when configured.**

In `crates/coding-ingest/src/daemon.rs`, find where `OpencodePoller` is conditionally spawned (the comment in CLAUDE.md mentions this lives there). Add a parallel branch:

```rust
if let Some(path) = config.kimi_wire_socket.clone() {
    coding_ingest::adapters::kimi_cli::spawn_wire(path, event_tx.clone());
}
```

Add `kimi_wire_socket: Option<PathBuf>` to the daemon's config struct (search the daemon module for the existing config type; it's likely `IngestDaemonConfig`).

- [ ] **Step 5: Commit.**

```bash
git add crates/coding-ingest/
git commit -m "feat(kimi-cli): implement Tier-2 Wire streaming adapter

wire.rs was a documented stub; this adds a real Unix-socket NDJSON reader
that translates Wire frames into the existing tier-1 dispatch path.
Daemon spawns the loop when kimi_wire_socket is configured."
```

---

### Task C6: Extend Inv 7 proptest to cover all base `EventKind` variants

**Files:**
- Modify: `crates/coding-ingest/tests/cross_cli_normalization.rs:20-46`

- [ ] **Step 1: Replace `arb_event_kind`.**

```rust
fn arb_event_kind() -> impl Strategy<Value = EventKind> {
    use coding_ingest::event::FileOp;
    prop_oneof![
        Just(EventKind::SessionStart {
            model: Some("test-model".into()),
            source_reason: "cli".into(),
        }),
        Just(EventKind::SessionEnd { reason: "success".into() }),
        Just(EventKind::UserPrompt {
            text: "hello world".into(),
            attachments: vec![],
        }),
        Just(EventKind::AssistantMsg {
            text: "hi there".into(),
            truncated: false,
            token_usage: None,
        }),
        Just(EventKind::ToolCall {
            tool: "read".into(),
            args_preview: "{}".into(),
            ok: true,
            duration_ms: 100,
            result_preview: "ok".into(),
        }),
        Just(EventKind::FileEdit {
            path: PathBuf::from("/tmp/x.rs"),
            op: FileOp::Modify,
            bytes: 42,
            diff_preview: Some("-old\n+new".into()),
        }),
        Just(EventKind::TestRun {
            command: "cargo test".into(),
            framework: Some("cargo".into()),
            passed: 10,
            failed: 0,
            duration_ms: 5000,
        }),
        Just(EventKind::CompactEvent {
            trigger: "manual".into(),
            token_count: 4096,
        }),
        Just(EventKind::Error {
            tool: Some("bash".into()),
            message: "exit 1".into(),
        }),
    ]
}
```

- [ ] **Step 2: Run.**

```bash
cargo nextest run -p coding-ingest --test cross_cli_normalization
```

Expected: PASS (proptest with 64 cases × 9 variants × 5 sources = 2880 implicit cases).

- [ ] **Step 3: Commit.**

```bash
git add crates/coding-ingest/tests/cross_cli_normalization.rs
git commit -m "test(ingest): widen Inv 7 proptest to all 9 base EventKind variants"
```

---

# Section D — Recall Ranking & Causal Surface

Two gaps:
- The 12-axis recall weight vector at `coding-memory/src/recall/service.rs:381-386` is hardcoded with `// train in Phase 6`. Move to config + persisted learned weights so reforge can update them.
- `recall/renderers.rs:145-146` is a hardcoded stub string for the causal-context section. Wire actual causal-edge retrieval.

### Task D1: Failing test — recall weights load from config and DB

**Files:**
- Test: `crates/coding-memory/tests/recall_weights_persistence.rs` (CREATE)

- [ ] **Step 1: Write.**

```rust
use coding_memory::recall::{load_recall_weights, store_recall_weights};
use storage::StoragePool;

#[tokio::test]
async fn recall_weights_round_trip() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    cognitive::run_migrations(&pool).await.unwrap();
    coding_memory::run_migrations(&pool).await.unwrap();

    let custom = [0.4, 0.05, 0.1, 0.05, 0.05, 0.15, 0.05, 0.05, 0.04, 0.04, 0.02, 0.0];
    store_recall_weights(&pool, &custom).await.unwrap();
    let loaded = load_recall_weights(&pool).await.unwrap();
    assert_eq!(loaded, custom);
}

#[tokio::test]
async fn recall_weights_default_when_unset() {
    let pool = StoragePool::connect_in_memory().await.unwrap();
    cognitive::run_migrations(&pool).await.unwrap();
    coding_memory::run_migrations(&pool).await.unwrap();

    let loaded = load_recall_weights(&pool).await.unwrap();
    let default = coding_memory::recall::default_weights();
    assert_eq!(loaded, default);
}
```

- [ ] **Step 2: Confirm failure.**

```bash
cargo nextest run -p coding-memory --test recall_weights_persistence
```

Expected: FAIL — `load_recall_weights` / `store_recall_weights` don't exist.

---

### Task D2: Add migration + weight persistence

**Files:**
- Create: `crates/coding-memory/migrations/006_recall_weights.sql`
- Modify: `crates/coding-memory/src/lib.rs` (migrations array)
- Modify: `crates/coding-memory/src/recall/service.rs` (or create `crates/coding-memory/src/recall/weights.rs`)

- [ ] **Step 1: Migration.**

```sql
-- 006_recall_weights.sql
-- Persisted 12-axis recall ranking weights. Single-row table keyed on 'local'.
CREATE TABLE IF NOT EXISTS recall_weights (
    id TEXT PRIMARY KEY DEFAULT 'local',
    weights TEXT NOT NULL,           -- JSON array of 12 f64
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    source TEXT NOT NULL DEFAULT 'default'  -- 'default' | 'reforge_trained' | 'manual'
);
INSERT OR IGNORE INTO recall_weights (id, weights, source) VALUES (
    'local',
    '[0.35,0.05,0.10,0.05,0.05,0.20,0.05,0.05,0.02,0.02,0.05,0.01]',
    'default'
);
```

- [ ] **Step 2: Add to migrations list.** Search `crates/coding-memory/src/lib.rs` for the existing migrations array (likely `&[(name, sql), ...]`) and append `("006_recall_weights", include_str!("../migrations/006_recall_weights.sql"))`.

- [ ] **Step 3: Create the loader/storer.**

In `recall/service.rs` (or a new `recall/weights.rs`):

```rust
pub async fn load_recall_weights(pool: &storage::StoragePool) -> common::Result<[f64; 12]> {
    let row: Option<(String,)> = sqlx::query_as("SELECT weights FROM recall_weights WHERE id = 'local'")
        .fetch_optional(pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("load_recall_weights: {e}")))?;
    let json = match row {
        Some((j,)) => j,
        None => return Ok(default_weights()),
    };
    let parsed: Vec<f64> = serde_json::from_str(&json)
        .map_err(|e| common::KlyntbotError::Serde(format!("recall weights JSON: {e}")))?;
    if parsed.len() != 12 {
        return Ok(default_weights());
    }
    let mut out = [0.0; 12];
    out.copy_from_slice(&parsed);
    Ok(out)
}

pub async fn store_recall_weights(pool: &storage::StoragePool, w: &[f64; 12]) -> common::Result<()> {
    let json = serde_json::to_string(w).map_err(|e| common::KlyntbotError::Serde(e.to_string()))?;
    sqlx::query("UPDATE recall_weights SET weights = ?1, updated_at = datetime('now'), source = 'reforge_trained' WHERE id = 'local'")
        .bind(json)
        .execute(pool.inner())
        .await
        .map_err(|e| common::KlyntbotError::Storage(format!("store_recall_weights: {e}")))?;
    Ok(())
}
```

- [ ] **Step 4: Wire `CodingRecallService` to read at startup.**

In `service.rs`, find where `default_weights()` is currently invoked (around line 381 referenced from `app-core`). Replace with:

```rust
let weights = load_recall_weights(&pool).await.unwrap_or_else(|e| {
    tracing::warn!(error = %e, "load_recall_weights failed, using defaults");
    default_weights()
});
```

- [ ] **Step 5: Run.**

```bash
cargo nextest run -p coding-memory --test recall_weights_persistence
```

Expected: PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/coding-memory/
git commit -m "feat(coding-memory): persist 12-axis recall weights for trainable ranking

Adds migration 006_recall_weights, load/store helpers, and wires the
CodingRecallService to read weights from the DB at startup. Reforge can
now write learned weights via store_recall_weights with source =
'reforge_trained'."
```

---

### Task D3: Failing test — causal context renders edges

**Files:**
- Test: `crates/coding-memory/tests/render_causal_section.rs` (CREATE)

- [ ] **Step 1: Write.**

```rust
use coding_memory::recall::renderers::render_user_prompt_block;

#[tokio::test]
async fn causal_section_lists_seeded_edges() {
    let svc = coding_memory::test_support::build_recall_service_with_causal_seed().await;
    let block = render_user_prompt_block(&svc, "hello", Some("repoA")).await.unwrap();
    assert!(
        !block.contains("populated when causal edges are seeded"),
        "stub string still present: {block}"
    );
    assert!(
        block.contains("### Causal context"),
        "section header missing"
    );
    assert!(
        block.contains("→") || block.contains("caused by"),
        "expected causal edge marker"
    );
}
```

- [ ] **Step 2: Confirm failure.**

```bash
cargo nextest run -p coding-memory --test render_causal_section
```

Expected: FAIL — stub string still emitted, or `build_recall_service_with_causal_seed` undefined. (You will need to add a small `test_support` helper that seeds 2-3 rows in `memory_causal_edges` and returns a wired-up service.)

---

### Task D4: Replace causal stub in `render_user_prompt_block`

**Files:**
- Modify: `crates/coding-memory/src/recall/renderers.rs:145-146`

- [ ] **Step 1: Replace lines 145-146 with real retrieval.**

```rust
// Causal context — list edges originating from the top likely-relevant
// memories (up to 3) at depth 1. Empty section is suppressed.
let mut causal = String::new();
if let Some(top) = idx.results.first() {
    if let Ok(parsed_id) = uuid::Uuid::parse_str(&top.id.to_string()) {
        match svc.trace_causes(parsed_id, repo, 1).await {
            Ok(trace) if !trace.edges.is_empty() => {
                causal.push_str("### Causal context\n");
                for edge in trace.edges.iter().take(5) {
                    causal.push_str(&format!(
                        "- `{}` → `{}` ({})\n",
                        short_id(&edge.from.to_string()),
                        short_id(&edge.to.to_string()),
                        edge.relation
                    ));
                }
                causal.push('\n');
            }
            Ok(_) => {} // No edges — omit section entirely.
            Err(e) => {
                tracing::debug!(error = %e, "trace_causes failed in render_user_prompt_block");
            }
        }
    }
}
```

If `trace_causes` returns a different shape, adapt — read `crates/coding-memory/src/recall/causal_walker.rs` for the actual `CausalTraceResponse` fields.

- [ ] **Step 2: Run the test + full crate.**

```bash
cargo nextest run -p coding-memory
```

Expected: PASS.

- [ ] **Step 3: Commit.**

```bash
git add crates/coding-memory/src/recall/renderers.rs crates/coding-memory/tests/render_causal_section.rs
git commit -m "feat(recall): populate causal-context section from memory_causal_edges

Removes the Phase-6 stub string; the section now lists outbound edges
from the top likely-relevant memory at depth 1, or is suppressed if no
edges are present."
```

---

# Section E — Graph Completion

One gap: Louvain Phase 2 (super-node contraction) is missing. `services/louvain.rs::detect_communities` only does Phase 1 local moves; on large graphs it cannot find the optimal partition. Five callers depend on it (verified): `scoring.rs:126`, `community_intelligence/mod.rs:209`, `agent/community_builder.rs:123`, `app-core/handlers/cognitive/graph.rs:144`, `cognitive/services/retrieval.rs:212`.

### Task E1: Failing test — Phase 2 contraction improves modularity on barbell graph

**Files:**
- Test: append to `crates/cognitive/src/services/louvain.rs` `#[cfg(test)] mod tests`

- [ ] **Step 1: Add the failing test.**

```rust
#[test]
fn phase2_contraction_finds_two_communities_in_barbell() {
    // Two K4 cliques connected by a single bridge edge.
    let edges = vec![
        // Left clique
        ("a".into(), "b".into(), 1.0),
        ("a".into(), "c".into(), 1.0),
        ("a".into(), "d".into(), 1.0),
        ("b".into(), "c".into(), 1.0),
        ("b".into(), "d".into(), 1.0),
        ("c".into(), "d".into(), 1.0),
        // Bridge
        ("d".into(), "e".into(), 1.0),
        // Right clique
        ("e".into(), "f".into(), 1.0),
        ("e".into(), "g".into(), 1.0),
        ("e".into(), "h".into(), 1.0),
        ("f".into(), "g".into(), 1.0),
        ("f".into(), "h".into(), 1.0),
        ("g".into(), "h".into(), 1.0),
    ];
    let result = detect_communities(&edges);
    let unique: std::collections::HashSet<_> = result.assignments.values().collect();
    assert_eq!(unique.len(), 2, "expected exactly 2 communities, got {}: {:?}", unique.len(), result.assignments);
    // Same-clique nodes share community.
    assert_eq!(result.assignments["a"], result.assignments["b"]);
    assert_eq!(result.assignments["e"], result.assignments["f"]);
    assert_ne!(result.assignments["a"], result.assignments["e"]);
    assert!(result.modularity > 0.35, "expected modularity > 0.35, got {}", result.modularity);
}
```

- [ ] **Step 2: Run.**

```bash
cargo nextest run -p cognitive louvain::tests::phase2_contraction_finds_two_communities_in_barbell
```

Expected: FAIL — Phase 1 alone often leaves the bridge node `d` or `e` in the wrong community.

---

### Task E2: Implement Phase 2 contraction loop

**Files:**
- Modify: `crates/cognitive/src/services/louvain.rs`

- [ ] **Step 1: Read the current `detect_communities` body fully** to understand existing types, then refactor into a multi-pass loop:

```rust
pub fn detect_communities(edges: &[(String, String, f64)]) -> CommunityAssignment {
    if edges.is_empty() {
        return CommunityAssignment {
            assignments: Default::default(),
            modularity: 0.0,
        };
    }
    // Outer loop: alternate Phase 1 (local moves) and Phase 2 (contraction)
    // until no improvement.
    let mut active_edges: Vec<(String, String, f64)> = edges.to_vec();
    let mut node_to_origin: std::collections::HashMap<String, Vec<String>> = active_edges
        .iter()
        .flat_map(|(a, b, _)| [a.clone(), b.clone()])
        .map(|n| (n.clone(), vec![n]))
        .collect();
    let mut last_modularity = f64::NEG_INFINITY;

    loop {
        let phase1 = phase1_local_moves(&active_edges);
        if phase1.modularity <= last_modularity + 1e-6 {
            // Map back to original node ids.
            return finalize(phase1, &node_to_origin);
        }
        last_modularity = phase1.modularity;

        // Phase 2: contract — each community becomes a super-node.
        let (contracted_edges, new_node_to_origin) = contract(&active_edges, &phase1.assignments, &node_to_origin);
        if contracted_edges.len() == active_edges.len() {
            // No reduction — fixed point reached.
            return finalize(phase1, &node_to_origin);
        }
        active_edges = contracted_edges;
        node_to_origin = new_node_to_origin;
    }
}

fn phase1_local_moves(edges: &[(String, String, f64)]) -> CommunityAssignment {
    // Existing Phase 1 implementation goes here — extract from current `detect_communities`.
    // ... unchanged from current code ...
    todo!("paste current Phase 1 body here")
}

fn contract(
    edges: &[(String, String, f64)],
    assignments: &std::collections::HashMap<String, usize>,
    node_to_origin: &std::collections::HashMap<String, Vec<String>>,
) -> (Vec<(String, String, f64)>, std::collections::HashMap<String, Vec<String>>) {
    let mut acc: std::collections::HashMap<(String, String), f64> = Default::default();
    for (a, b, w) in edges {
        let ca = assignments[a].to_string();
        let cb = assignments[b].to_string();
        let (lo, hi) = if ca <= cb { (ca, cb) } else { (cb, ca) };
        *acc.entry((lo, hi)).or_insert(0.0) += w;
    }
    let new_edges: Vec<(String, String, f64)> = acc
        .into_iter()
        .filter(|((a, b), _)| a != b)
        .map(|((a, b), w)| (a, b, w))
        .collect();

    // Each super-node tracks which original nodes it represents.
    let mut new_origin: std::collections::HashMap<String, Vec<String>> = Default::default();
    for (orig, comm) in assignments {
        let comm_str = comm.to_string();
        new_origin
            .entry(comm_str)
            .or_default()
            .extend(node_to_origin.get(orig).cloned().unwrap_or_default());
    }
    (new_edges, new_origin)
}

fn finalize(
    contracted: CommunityAssignment,
    node_to_origin: &std::collections::HashMap<String, Vec<String>>,
) -> CommunityAssignment {
    let mut out: std::collections::HashMap<String, usize> = Default::default();
    for (super_node, comm) in contracted.assignments {
        if let Some(originals) = node_to_origin.get(&super_node) {
            for orig in originals {
                out.insert(orig.clone(), comm);
            }
        }
    }
    CommunityAssignment {
        assignments: out,
        modularity: contracted.modularity,
    }
}
```

Important: when extracting the existing Phase 1 logic into `phase1_local_moves`, preserve its behavior exactly. The Phase 2 wrapper composes it; the math is unchanged at the inner layer.

- [ ] **Step 2: Run the new test + the existing tests.**

```bash
cargo nextest run -p cognitive louvain
```

Expected: all PASS, including the existing tests (regressions would indicate Phase 1 was disturbed during refactoring).

- [ ] **Step 3: Run callers' tests to verify no behavior changes.**

```bash
cargo nextest run -p cognitive -p agent -p app-core
```

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/src/services/louvain.rs
git commit -m "feat(cognitive): add Louvain Phase 2 super-node contraction

Phase 1 (local moves) alone cannot find the optimal partition on graphs
with weak inter-community bridges. The outer loop now alternates Phase 1
and Phase 2 (contraction) until modularity stops improving."
```

---

# Section F — Cleanup & Integration

Ten sub-gaps consolidated:
- F1: Remove dead `to_accumulate` (`background.rs:351`)
- F2: Wire autotuner into Reforge Phase 6 (`cron.rs:1148`)
- F3: Wire `compress_with_delta` into the assembler, or delete it
- F4: Drop `distraction_rules_to_promote` field
- F5: Use `MIN_AGE_FOR_RESTRUCTURE` in restructure logic
- F6: Schedule FSRS optimizer weekly cron
- F7: Move distiller model to config
- F8: Move cross-session-dedup threshold to config
- F9: Move selective-delete threshold to config
- F10: Fix `NEWER` SQL alias in `cross_session_dedup.rs:96`
- F11: Subscriber registry doc

### Task F1: Delete dead `to_accumulate` code path

**Files:**
- Modify: `crates/cognitive/src/services/background.rs:351, 743-791`

- [ ] **Step 1: Read lines 320–800 to confirm scope.**

```bash
sed -n '320,810p' crates/cognitive/src/services/background.rs | head -200
```

- [ ] **Step 2: Delete the empty-Vec declaration at line 351.**

Remove:
```rust
let to_accumulate: Vec<(String, Observation)> = Vec::new();
```

- [ ] **Step 3: Delete the unreachable loop at lines 743–772 (`for (key, obs) in to_accumulate { ... }`).**

The block following at 774–791 (`MAX_ACCUMULATOR_ENTRIES` pruning) is still reachable and reasonable to keep — it bounds the in-memory accumulator. But trace whether it can ever grow now that `to_accumulate` is gone. Check `accumulator.entry(...)` write sites elsewhere in the function.

If `accumulator` is no longer written anywhere → delete the pruning block too. If it is still written by DLQ retries / promotion path → keep the prune.

- [ ] **Step 4: Build and test.**

```bash
cargo build -p cognitive
cargo nextest run -p cognitive
cargo clippy -p cognitive --all-targets -- -D warnings
```

- [ ] **Step 5: Commit.**

```bash
git add crates/cognitive/src/services/background.rs
git commit -m "chore(cognitive): delete dead to_accumulate code in BackgroundConsolidationService

The legacy classify_batch path was removed earlier but the consumer
loop kept iterating an always-empty Vec. SignalRouter is the canonical
producer of accumulated observations now."
```

---

### Task F2: Wire autotuner into Reforge Phase 6 (Task 12)

**Files:**
- Modify: `crates/app-core/src/init/cron.rs:1148-1154`
- Modify: `crates/cognitive/src/services/reforge/service.rs` (Phase 6)
- Possibly: `crates/agent/src/autotuner/orchestrator.rs` for an `evaluate_for_reforge` entry point

- [ ] **Step 1: Failing test — Phase 6 calls AutoTuner evaluation.**

Test: `crates/cognitive/tests/reforge_phase6_autotuner.rs` (CREATE)

```rust
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

#[tokio::test]
async fn reforge_phase6_invokes_autotuner_bridge() {
    let calls = Arc::new(AtomicUsize::new(0));
    let bridge = cognitive::test_support::stub_autotuner_bridge(calls.clone());
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    cognitive::run_migrations(&pool).await.unwrap();

    cognitive::services::reforge::run_phase6_autotuner(&pool, &bridge).await.unwrap();
    assert!(calls.load(Ordering::SeqCst) > 0, "autotuner bridge was not invoked");
}
```

```bash
cargo nextest run -p cognitive --test reforge_phase6_autotuner
```

Expected: FAIL.

- [ ] **Step 2: Add `run_phase6_autotuner` in `reforge/service.rs`.**

Define a new public function:

```rust
pub async fn run_phase6_autotuner(
    pool: &storage::StoragePool,
    bridge: &Arc<dyn AutotunerBridge>,
) -> common::Result<()> {
    let suggestions = load_pending_trial_suggestions(pool).await?;
    for s in suggestions {
        bridge.evaluate(&s).await?;
    }
    Ok(())
}
```

If `AutotunerBridge` doesn't expose `evaluate(&TrialSuggestion)`, add it to the trait. The agent-side implementation lives in `agent/src/autotuner/`; route to its existing `evaluate_trial` or equivalent.

- [ ] **Step 3: Re-enable the cron registration in `cron.rs`.**

Replace lines 1148–1154 with:

```rust
agent::autotuner::AutoTunerOrchestrator::ensure_nightly_job(
    cron_service,
    &config.autotuner.schedule,
)
.await?;
```

The cron job's callback should call `run_phase6_autotuner(&pool, &autotuner_bridge)`. Wire `autotuner_bridge` from the `AppCore` constructor that's already in scope at this call site.

- [ ] **Step 4: Run.**

```bash
cargo nextest run -p cognitive --test reforge_phase6_autotuner
cargo nextest run -p app-core
```

- [ ] **Step 5: Commit.**

```bash
git add crates/app-core/src/init/cron.rs crates/cognitive/src/services/reforge/ crates/cognitive/tests/reforge_phase6_autotuner.rs
git commit -m "feat(reforge): wire autotuner evaluation into Phase 6 (Task 12)

Closes the autotuner-vs-Reforge merge. Phase 6 now calls
run_phase6_autotuner which iterates pending trial_suggestions through
the AutotunerBridge.evaluate path. Replaces the disabled-comment block."
```

---

### Task F3: Wire `compress_with_delta` into the assembler

**Files:**
- Modify: `crates/context_engine/src/assembler/mod.rs:344` (the existing `self.compressor.compress(...)` call)
- Modify: `crates/context_engine/src/history_compressor/tiered.rs` (export delta state)

- [ ] **Step 1: Decision check.**

Run a static call-site survey:
```bash
rg -n "compress_with_delta" crates/
```

If the only callers are `tiered.rs:131` (def), `tiered.rs:775` (internal), `tiered.rs:816` (test) — then the production assembler never uses delta compression. Two valid resolutions:

**Option A (recommended):** wire it. The assembler caches the compressed result; passing the previous compressed prefix avoids re-summarizing turns that didn't change.

**Option B (if delta caching is unbounded complexity):** delete `compress_with_delta` and the test, document in CLAUDE.md that delta compression is intentionally not used.

Choose A unless the engineer finds wiring blocked.

- [ ] **Step 2 (Option A): Add a `delta_state` field to the assembler cache entry** (`assembler/cache.rs`):

```rust
pub struct CacheEntry {
    pub assembled: AssembledContext,
    pub last_compressed_through_idx: usize,
    pub last_compressed_messages: Vec<Message>,
}
```

- [ ] **Step 3 (Option A): On cache hit + new turns appended, call `compress_with_delta` with the prior state.** Replace `self.compressor.compress(...)` at `mod.rs:344` with a branch:

```rust
let (compressed_summaries, last_idx) = match self.cache.get(&key) {
    Some(entry) if entry.last_compressed_through_idx <= history.len() => {
        self.compressor
            .compress_with_delta(history, &entry.last_compressed_messages, entry.last_compressed_through_idx)
            .await
    }
    _ => self.compressor.compress(history).await,
};
```

Update cache writes accordingly.

- [ ] **Step 4: Run context_engine tests.**

```bash
cargo nextest run -p context_engine
```

- [ ] **Step 5: Commit.**

```bash
git add crates/context_engine/
git commit -m "feat(context-engine): wire compress_with_delta into assembler cache hits

The delta path existed but was orphaned. Cache entries now retain the
compressed prefix so repeat assemblies skip resummarizing unchanged turns."
```

---

### Task F4: Remove vestigial `distraction_rules_to_promote` field

**Files:**
- Modify: `crates/cognitive/src/services/reforge/types.rs:64`
- Modify: `crates/cognitive/src/services/reforge/collector.rs:376`
- Modify: any consumer that reads the field (search first)

- [ ] **Step 1: Find all readers.**

```bash
rg -n "distraction_rules_to_promote" crates/
```

- [ ] **Step 2: Delete the field + all consumers.**

In `types.rs:64`, remove the line:
```rust
pub distraction_rules_to_promote: u32,
```

In `collector.rs:376`, remove the line:
```rust
distraction_rules_to_promote: 0,
```

In any `synthesize` / `narrate` / `apply` callers that read it: remove the read. If the field is ever rendered into a prompt template, remove the placeholder there too.

- [ ] **Step 3: Build + test.**

```bash
cargo build -p cognitive
cargo nextest run -p cognitive
```

- [ ] **Step 4: Commit.**

```bash
git add crates/cognitive/
git commit -m "chore(reforge): remove vestigial distraction_rules_to_promote field

Hardcoded to 0 in the collector and never populated. Drop from
ReforgeCollected and update consumers."
```

---

### Task F5: Use `MIN_AGE_FOR_RESTRUCTURE` in community intelligence

**Files:**
- Modify: `crates/cognitive/src/services/community_intelligence/mod.rs:58-64`

- [ ] **Step 1: Read `apply_intelligence`** to find where merges/splits are gated.

```bash
rg -n "apply_intelligence\b\|merge\b\|split\b" crates/cognitive/src/services/community_intelligence/
```

- [ ] **Step 2: Add an age guard in the merge / split apply functions.**

Wherever a community is the target of a merge or split, fetch its age (`(now - created_at).as_days()`) and skip the operation if `< MIN_AGE_FOR_RESTRUCTURE`:

```rust
let age_days = (jiff::Timestamp::now() - community.created_at).as_seconds() / 86400;
if (age_days as u32) < MIN_AGE_FOR_RESTRUCTURE {
    tracing::debug!(community_id = %community.id, age_days, "skipping restructure: too young");
    continue;
}
```

Remove the `#[allow(dead_code)]` annotation now that the constant is read.

- [ ] **Step 3: Add a unit test.**

```rust
#[tokio::test]
async fn merge_skipped_for_young_community() {
    let pool = storage::StoragePool::connect_in_memory().await.unwrap();
    cognitive::run_migrations(&pool).await.unwrap();
    // Insert a 1-day-old community + an old one targeting a merge.
    // Run apply_intelligence with a synthetic merge proposal.
    // Assert the young community was not merged.
    todo!("seed pool, run, assert");
}
```

(Engineer: pattern this after existing tests in the same file.)

- [ ] **Step 4: Run + commit.**

```bash
cargo nextest run -p cognitive community_intelligence
git add crates/cognitive/src/services/community_intelligence/
git commit -m "fix(community-intelligence): enforce MIN_AGE_FOR_RESTRUCTURE in merge/split

Prevents thrashing of just-formed communities by skipping merges and
splits on communities younger than 3 days."
```

---

### Task F6: Schedule FSRS optimizer weekly cron

**Files:**
- Modify: `crates/app-core/src/init/cron.rs`
- Reference: `crates/cognitive/src/services/fsrs_optimizer.rs::optimize_weights`
- Reference: `crates/agent/src/adapters/fsrs_writeback.rs:44` (existing caller)

- [ ] **Step 1: Add a constant + ensure_job block.**

In `cron.rs` near the other `JOB_*` constants:

```rust
const JOB_FSRS_OPTIMIZE: &str = "__klyntbot_fsrs_optimize_weekly";
```

Inside `ensure_cron_jobs` (right after `JOB_ATOM_DECAY` block):

```rust
ensure_job!(
    JOB_FSRS_OPTIMIZE,
    scheduling::CronSchedule::Cron {
        expr: "0 0 4 * * 0".to_string(), // Sunday 04:00 local
        tz: Some(config.timezone.clone()),
    },
    "Weekly FSRS-5 weight optimisation",
    "system"
);
```

- [ ] **Step 2: Register a cron callback.**

In `register_cron_callbacks`, add:

```rust
cron_executor.register(JOB_FSRS_OPTIMIZE, {
    let pool = pool.clone();
    move |_ctx| {
        let pool = pool.clone();
        Box::pin(async move {
            let baseline = cognitive::services::fsrs_optimizer::BASELINE_WEIGHTS;
            let cfg = cognitive::services::fsrs_optimizer::OptimiserConfig::default();
            match cognitive::services::fsrs_optimizer::optimize_weights(&pool, &baseline, &cfg).await {
                Ok(Some(outcome)) if outcome.improved => {
                    agent::adapters::fsrs_writeback::persist_weights(&pool, &outcome.trained_weights).await
                }
                Ok(_) => Ok(()),
                Err(e) => {
                    tracing::warn!(error = %e, "fsrs optimize failed");
                    Ok(())
                }
            }
        })
    }
});
```

If `persist_weights` doesn't exist, factor it out of `fsrs_writeback.rs`'s existing entry. Read `fsrs_writeback.rs:44` for the existing call shape.

- [ ] **Step 3: Run.**

```bash
cargo build -p app-core
cargo nextest run -p app-core
```

- [ ] **Step 4: Commit.**

```bash
git add crates/app-core/src/init/cron.rs
git commit -m "feat(cron): schedule weekly FSRS-5 weight optimization

Sunday 04:00 local time. Runs the existing fsrs_optimizer pipeline and
persists trained weights only if holdout loss improves."
```

---

### Task F7: Move distiller model + thresholds to `coding_memory` config

**Files:**
- Modify: `crates/config/src/schema/coding_memory.rs`
- Modify: `crates/coding-memory/src/distiller/mod.rs:131`
- Modify: `crates/app-core/src/coding_memory/reforge.rs:190, 198`
- Modify: `crates/coding-memory/src/reforge/selective_delete.rs:64`

- [ ] **Step 1: Failing test — config defaults match current hardcoded values.**

Test in `crates/config/src/schema/coding_memory.rs` (`#[cfg(test)] mod tests`):

```rust
#[test]
fn coding_memory_defaults_match_legacy_hardcoded_values() {
    let cfg = CodingMemoryConfig::default();
    assert_eq!(cfg.distiller.model.as_deref(), Some("claude-haiku-4-5-20251001"));
    assert!((cfg.reforge.cross_session_dedup_threshold - 0.92).abs() < 1e-6);
    assert_eq!(cfg.reforge.selective_delete_threshold, 5);
}
```

- [ ] **Step 2: Add fields to `CodingMemoryConfig`.**

In `coding_memory.rs`, locate the `CodingMemoryConfig` struct. Add (matching the camelCase + default-fn pattern documented in the verification report):

```rust
#[serde(default)]
pub distiller: DistillerConfigSection,

#[serde(default)]
pub reforge: ReforgeConfigSection,

// ...

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DistillerConfigSection {
    #[serde(default = "default_distiller_model", skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

fn default_distiller_model() -> Option<String> {
    Some("claude-haiku-4-5-20251001".to_string())
}

impl Default for DistillerConfigSection {
    fn default() -> Self {
        Self { model: default_distiller_model() }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReforgeConfigSection {
    #[serde(default = "default_cross_session_dedup_threshold")]
    pub cross_session_dedup_threshold: f32,
    #[serde(default = "default_selective_delete_threshold")]
    pub selective_delete_threshold: u32,
}

fn default_cross_session_dedup_threshold() -> f32 { 0.92 }
fn default_selective_delete_threshold() -> u32 { 5 }

impl Default for ReforgeConfigSection {
    fn default() -> Self {
        Self {
            cross_session_dedup_threshold: default_cross_session_dedup_threshold(),
            selective_delete_threshold: default_selective_delete_threshold(),
        }
    }
}
```

- [ ] **Step 3: Update consumers.**

In `crates/coding-memory/src/distiller/mod.rs:131`, change:
```rust
model: "claude-haiku-4-5-20251001".into(),
```
to take the value from `DistillerConfig` constructed by the wiring layer. The default constructor can keep the hardcoded string (it's documented as the floor); the production path in `app-core` should read from config.

In `crates/app-core/src/coding_memory/reforge.rs:190`:
```rust
let applied = CrossSessionDedup::run(&self.fact_repo, self.config.reforge.cross_session_dedup_threshold, None).await?;
```

In `selective_delete.rs:63-64` make the threshold a parameter on `apply` (already exists on `apply_with_threshold`):
```rust
pub async fn apply(pool: &storage::StoragePool, log: &SelectiveDeleteLogRepo, threshold: u32) -> Result<u32> {
    Self::apply_with_threshold(pool, log, threshold).await
}
```

In `app-core/coding_memory/reforge.rs::run_selective_delete`:
```rust
let applied = SelectiveDeleteSignal::apply(
    &self.pool,
    &self.selective_delete_log,
    self.config.reforge.selective_delete_threshold,
).await?;
```

- [ ] **Step 4: Run.**

```bash
cargo nextest run -p config -p coding-memory -p app-core
```

- [ ] **Step 5: Commit.**

```bash
git add crates/config crates/coding-memory crates/app-core
git commit -m "feat(config): expose distiller model + reforge thresholds via codingMemory config

Three values (distiller model, cross-session dedup threshold, selective
delete threshold) move from hardcoded literals to camelCase config keys
under codingMemory.{distiller,reforge}. Defaults preserve current behavior."
```

---

### Task F8: Fix `NEWER` SQL alias

**Files:**
- Modify: `crates/coding-memory/src/reforge/cross_session_dedup.rs:96`

- [ ] **Step 1: Lowercase the alias.**

Change line 96 from:
```sql
(older.scope_repo_id IS NEWER.scope_repo_id OR
```
to:
```sql
(older.scope_repo_id IS newer.scope_repo_id OR
```

- [ ] **Step 2: Run dedup tests.**

```bash
cargo nextest run -p coding-memory cross_session_dedup
```

- [ ] **Step 3: Commit.**

```bash
git add crates/coding-memory/src/reforge/cross_session_dedup.rs
git commit -m "style(coding-memory): lowercase newer alias in cross_session_dedup SQL"
```

---

### Task F9: Subscriber registry doc

**Files:**
- Create: `docs/architecture/domain-event-subscribers.md`

- [ ] **Step 1: Generate the registry by scanning the workspace.**

```bash
rg -n "DomainEvent::" crates/ --type rust | rg "match\b\|=>" | sort -u
```

- [ ] **Step 2: Write the doc.**

Create with a table organized by event variant:

```markdown
# Domain Event Subscriber Registry

Living index of which subsystems consume which `DomainEvent` variants. Update
when adding a new subscriber or event variant. Generated by manual audit;
keep in sync.

## Events → Subscribers

| Event | Subscriber | File:Line | Effect |
|---|---|---|---|
| `ChatTurnCompleted` | ChatTurnCollector | `cognitive/src/pipeline/chat_turn_collector.rs:38` | Emits CognitiveSignal for extraction |
| `ToolCallExecuted` | ActivityLogConsumer | `activity-log/src/consumer.rs:?` | Persists tool invocation |
| `ToolCallExecuted` | CodingMemoryDistiller | `coding-memory/src/distiller/mod.rs:253` | Feeds turn buffer |
| `ToolCallExecuted` | ToolRegistryBridge (publisher) | `klyntbot-server/src/bridge/registry.rs:114` | (publisher, see Task A3) |
| `TaskCreated` | BackgroundConsolidationService | `cognitive/src/services/background.rs:330` | Upsert entity |
| `TaskCompleted` | BackgroundConsolidationService | `cognitive/src/services/background.rs:867` | Counter + extraction trigger |
| `UserStatedFact` | (search rg) | … | … |
| `UserCorrectedAI` | Mirror routing source | `cognitive/src/mirror/sources/routing.rs:?` | Records correction |
| `NoteContentChanged` | NoteTreeBuilder | `agent/src/adapters/note_tree_builder.rs:?` | Tree rebuild |
| `FocusSessionStarted` | LiveContextRefresher | `agent/src/execution/live_context_refresher.rs:189` | ContextUpdate push |
| `FocusSessionEnded` | … | … | … |
| `DistractionDetected` | feature-productivity | … | ContextUpdate push |
| `CodingMemoryUpdated` | desktop UI invalidation | … | UI cache flush |
| `CommunityDiscovered/Updated/Weakened` | community_builder | `agent/src/adapters/community_builder.rs:275-315` | ContextUpdate push |
| `BudgetThresholdCrossed` | finance_tree_builder | `agent/src/adapters/finance_tree_builder.rs:231` | ContextUpdate push |
| `NoteStructureChanged` | note_tree_builder, task_tree_builder | `agent/src/adapters/note_tree_builder.rs:230`, `task_tree_builder.rs:171` | ContextUpdate push |

## Adding a new subscriber

1. Find the event variant in `bus/src/domain_events.rs`.
2. Add a `bus.subscribe()` call in your subsystem's init path.
3. Append a row to this table.
4. If the event doesn't exist, add it to `DomainEvent` and document why this
   subsystem is the canonical publisher.
```

(Engineer: complete the `?` rows by running the rg above.)

- [ ] **Step 3: Commit.**

```bash
git add docs/architecture/domain-event-subscribers.md
git commit -m "docs(architecture): add domain event subscriber registry"
```

---

# Final Verification

After all sections complete, run a full sweep:

- [ ] **Step 1: Workspace build.**

```bash
cargo build --workspace
```

- [ ] **Step 2: Workspace nextest.**

```bash
cargo nextest run --workspace
```

- [ ] **Step 3: Doctests (nextest skips them).**

```bash
cargo test --workspace --doc
```

- [ ] **Step 4: Clippy clean.**

```bash
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

- [ ] **Step 5: Format.**

```bash
cargo fmt --all --check
```

- [ ] **Step 6: Dependency hygiene.**

```bash
cargo machete
```

Any new unused deps introduced by added/removed code should be cleaned.

- [ ] **Step 7: Manual smoke — start the desktop app in dev mode.**

```bash
cd desktop-ui && bun run dev &
cargo tauri dev
```

Verify:
- Chat works (Section A doesn't break the per-turn path).
- An MCP client (`klyntbot mcp serve --stdio`) calling `tasks` produces a `ToolCallExecuted` line in the activity log.
- A coding session with a screenshot tool flows through without panicking (Section B).
- Open the DB at `~/.klyntbot-dev/data.db` and confirm `recall_weights` table populated and `mirror_routing_snapshots` has rows after a few minutes.

---

# Self-Review Notes

**Spec coverage check (each gap → task):**

| Gap | Task |
|---|---|
| #1 MCP `ToolCallExecuted` not published | A1, A2, A3 |
| #2 `Message::Tool.content` is plain String | B0–B7 |
| #3 opencode cwd/turn_id/repo | C1, C2 |
| #4 kimi-cli Tier-2 unimplemented | C5 |
| #5 causal context stub | D3, D4 |
| #6 hardcoded recall weights | D1, D2 |
| #7 Louvain Phase 2 missing | E1, E2 |
| #8 dead `to_accumulate` | F1 |
| #9 autotuner Task-12 TODO | F2 |
| #10 diff_preview hardcoded None | C3, C4 |
| #11 `ChatTurnCompleted.user_message` None guard | A0, A4, A5 |
| #12 opencode tool detection heuristic | C2 (subsumed) |
| #13 (INVALID — verified written) | excluded |
| #14 (INVALID — verified wired) | excluded |
| #15 `compress_with_delta` unused | F3 |
| #16 `distraction_rules_to_promote` hardcoded 0 | F4 |
| #17 `MIN_AGE_FOR_RESTRUCTURE` `dead_code` | F5 |
| #18 FSRS optimizer not scheduled | F6 |
| #19 distiller model hardcoded | F7 (subsumed) |
| #20 cross-session-dedup threshold hardcoded | F7 (subsumed) |
| #21 selective-delete threshold hardcoded | F7 (subsumed) |
| #22 no subscriber registry | F9 |
| (bonus) `NEWER` SQL alias | F8 |

**Type consistency check:** `ToolContent`, `ToolContentPart`, `ToolContent::Text`, `ToolContent::MultiPart`, `ToolContentPart::Text { text }`, `ToolContentPart::ImageData { media_type, data }` — referenced consistently across B1, B2, B3, B4, B5. `default_weights()`, `load_recall_weights`, `store_recall_weights` consistent across D1, D2. `run_phase6_autotuner` consistent across F2.

**Placeholder scan:** No "TBD", "TODO", or "implement later" patterns. One `todo!()` stub appears in Task E2 Step 1 — that is intentional shorthand for "paste the existing Phase 1 body unchanged at this point" and is annotated as such; the engineer must replace it with the literal contents of the current `detect_communities` body before running tests. Task F5 Step 3 contains a `todo!("seed pool, run, assert")` with explicit guidance to mirror existing tests in the same module — also intentional pattern-matching shorthand.

---

**End of plan.**
