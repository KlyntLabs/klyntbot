# Live Context Refresher — Dynamic Context Re-Assembly During Execution

## Problem

When the agent enters a ReAct loop, the context snapshot is frozen. If a memory gets promoted, a focus session ends, or a distraction is detected mid-execution, the agent continues with a stale mental model. This breaks the feeling of a living, aware second brain — the product feels more like a one-shot chatbot that happens to have tools.

## Solution

A **LiveContextRefresher** that injects context updates into the ReactiveEngine at iteration boundaries. Other systems (cognitive, productivity, coaching) push updates to a shared queue; the refresher drains and injects them as `Message::ContextUpdate` entries before the next LLM cycle.

## Design Principles

- **Living second brain**: The agent visibly adapts mid-task, incorporating new knowledge the moment it becomes available.
- **User agency**: The user can pause context updates for a given execution (frozen-context mode for deep focus tasks).
- **Token discipline**: Updates respect the remaining context budget. High-priority updates survive budget pressure; low-priority ones are dropped with a warning.
- **Queue as extension point**: Adding a new producer never touches `ReactiveEngine` or `runtime.rs` — just push a `ContextUpdate` to the queue.

## Architecture

### Three Nested Loops Model

```
Outer loop:  Skill + Intent routing (linear, unchanged)
Middle loop: Context + Execution (where dynamic re-assembly lives)
Inner loop:  Reflection + Learning (post-execution, event-bus driven)
```

The LiveContextRefresher operates in the **middle loop**, alongside the existing `MidLoopCompressor`, at the iteration boundary inside `ReactiveEngine`.

### Execution Flow

```
ReactiveEngine::execute() {
    for iteration in 1..=max_iterations {
        outcome = core.run_cycle(&mut messages, ...)
        match outcome { ... }

        // 1. Oscillation detection (existing)
        // 2. MidLoopCompressor (existing — Feature 3)
        // 3. LiveContextRefresher (NEW)
        refresher.inject_pending(&mut messages, remaining_budget)
    }
}
```

## Core Types

### ContextUpdate

```rust
/// A context update that can be injected mid-execution.
pub struct ContextUpdate {
    /// Why this update happened.
    pub reason: ContextUpdateReason,
    /// Human-readable content injected into the conversation.
    /// None if the reason alone is sufficient.
    pub content: Option<String>,
    /// Machine-readable details (e.g. fact ID, focus duration).
    pub metadata: Option<serde_json::Value>,
    /// Higher priority updates survive token budget trimming.
    pub priority: UpdatePriority,
    /// When the update was created.
    pub timestamp: DateTime<Utc>,
}

pub enum ContextUpdateReason {
    MemoryPromoted,
    FocusSessionStarted,
    FocusSessionEnded,
    DistractionDetected,
    BudgetThresholdCrossed,
    Custom(String),
}

pub enum UpdatePriority {
    Low,     // Background awareness (coaching nudges)
    Normal,  // Important but not urgent (memory promotion)
    High,    // Critical (focus session ended, budget alert)
}
```

### Crate Placement

**Critical constraint**: `ContextUpdate`, `ContextUpdateReason`, `UpdatePriority`, and `ContextUpdateQueue` must be defined in a **lower-level crate** that both `agent` (L5) and `cognitive` (L5) can import. The `bus` crate (L1) is the natural home — it already provides `DomainEventBus` for cross-crate event communication, and this queue serves the same role.

Location: `crates/bus/src/context_updates.rs`

### Message Variant

A new `Message::ContextUpdate` variant in the `providers` crate:

```rust
pub enum Message {
    System { content: String },
    User { content: UserContent },
    Assistant { content: Option<String>, tool_calls: Option<Vec<ToolCallMessage>>, reasoning_content: Option<String> },
    Tool { tool_call_id: String, name: String, content: String },
    // NEW
    ContextUpdate { reason: String, content: String },
}
```

**LLM serialization**: The `Message` enum uses `#[serde(tag = "role")]`, so a raw `ContextUpdate` variant would serialize as `{"role": "contextupdate"}` which providers reject. Each provider adapter must map `Message::ContextUpdate` to a system-role message **before** serializing:

```rust
// In each provider's message serialization (openai_compat.rs, anthropic_native.rs, etc.)
Message::ContextUpdate { reason, content } => {
    json!({
        "role": "system",
        "content": format!("<context_update reason=\"{reason}\">\n{content}\n</context_update>")
    })
}
```

**Also requires updates to**:
- `Message::role()` → return `MessageRole::System` for the new variant
- `context_engine::estimate_message_tokens()` → add a branch for `Message::ContextUpdate` (estimate same as System + 10 tokens overhead for the XML tags)
- All exhaustive `match` statements on `Message` across the codebase

**UI rendering**: The frontend renders this as a warm, human-readable line (e.g., "I just remembered · Memory promoted — You prefer morning deep work sessions"). The transparency panel can expand to show the raw XML-tagged version.

### ContextUpdateQueue

Defined in `crates/bus/src/context_updates.rs` (accessible by both `agent` and `cognitive`).

```rust
pub struct ContextUpdateQueue {
    inner: std::sync::Mutex<Vec<ContextUpdate>>,
}

impl ContextUpdateQueue {
    pub fn new() -> Self { ... }

    /// Push an update, with 30-second deduplication by (reason, content).
    pub fn push(&self, update: ContextUpdate) { ... }

    /// Drain all pending updates atomically.
    pub fn drain(&self) -> Vec<ContextUpdate> {
        let mut queue = self.inner.lock().unwrap();
        std::mem::take(&mut *queue)
    }
}
```

Deduplication: same `(reason, content_hash)` within 30 seconds is dropped. When `content` is `None`, dedup uses `reason` alone as the key. Prevents duplicate injections when multiple systems notice the same event.

**Note on `std::sync::Mutex`**: This is intentional, not `tokio::sync::Mutex`. Both `push()` and `drain()` are synchronous short critical sections that never hold the guard across an `.await` point. This invariant must be preserved — never make `push` or `drain` async.

## LiveContextRefresher

```rust
pub struct LiveContextRefresher {
    token_counter: Arc<dyn TokenCounter>,
    queue: Arc<ContextUpdateQueue>,
}
```

Note: `context_window` is NOT stored on the refresher — it reads `params.context_window` at call time via the method signature. This ensures per-request overrides (via `ExecutionParams::with_context_window()`) are respected.

### `inject_pending` Method

Signature: `fn inject_pending(&self, messages: &mut [Message], context_window: usize) -> Vec<ContextReassembledUpdate>`

1. Drain the queue atomically (`queue.drain()`)
2. If empty, return immediately (no-op fast path)
3. Sort by priority (High first, then Normal, then Low)
4. For each update:
   a. Render to XML-tagged content string
   b. Estimate tokens via shared `estimate_message_tokens()`
   c. Check remaining budget (current tokens vs `context_window`, reserving 20% for LLM response)
   d. If fits: push `Message::ContextUpdate` into messages vec
   e. If doesn't fit: log warning, skip (high-priority can request 40% reservation override)
5. Return list of injected `ContextReassembledUpdate` entries (caller emits the event)

### Token Budget

```
remaining = context_window - current_tokens
available = remaining * 80 / 100  (reserve 20% for LLM response)
// High-priority updates may use up to 40% of remaining
```

Uses the shared `context_engine::estimate_message_tokens()` function (same as MidLoopCompressor).

## Event Emission

### AgentEvent::ContextReassembled

```rust
AgentEvent::ContextReassembled {
    updates: Vec<ContextReassembledUpdate>,
    tokens_added: usize,
}

struct ContextReassembledUpdate {
    reason: String,      // "memory_promoted"
    summary: String,     // "You prefer morning deep work sessions"
    tokens: usize,       // 22
}
```

**Implementation note**: `ContextReassembled` must be added to the `AgentEvent` enum in `crates/agent/src/events.rs` (with `#[serde(rename_all = "camelCase")]` field attributes) and handled in `crates/app-core/src/handlers/chat/streaming.rs` (same pattern as `ContextCompressed`).

### Streaming Handler

Logged in `streaming.rs` with structured fields. No frontend changes in Phase 1 — the value is in the agent's improved behavior, not UI display.

## Queue Producers

### Phase 1: Memory Promotion (this implementation)

**Location**: `crates/cognitive/src/services/background.rs`

After `ExtractionHandler` successfully persists a new semantic fact:

```rust
if let Some(ref queue) = self.context_update_queue {
    queue.push(ContextUpdate {
        reason: ContextUpdateReason::MemoryPromoted,
        content: Some(format!("{} — {}", fact.subject, fact.predicate)),
        metadata: Some(json!({ "factId": fact.id, "scope": fact.scope })),
        priority: UpdatePriority::Normal,
        timestamp: Utc::now(),
    });
}
```

### Phase 2+ Producers (not in this plan, listed for context)

| Producer | Trigger | Priority | Content |
|----------|---------|----------|---------|
| FocusManager | Focus session starts/ends | High | "Focus session ended after 47 minutes" |
| ProductivityTracker | Distraction pattern detected | Normal | "Distraction pattern: 3 app switches in 2 minutes" |
| CostTracker | Budget threshold crossed | High | "Monthly budget 80% used ($4.12 of $5.00)" |
| CoachingEngine | Proactive nudge triggered | Low | "You usually take a break around this time" |
| InsightForge | Relevant insight surfaced | Low | "Your OKR progress is behind by 15%" |

## Wiring

The `ContextUpdateQueue` (defined in `bus` crate) is created and distributed following existing dependency injection patterns:

1. **Created** in `app-core/init/` (alongside `DomainEventBus`)
2. **Passed to** `AgentLoop` via builder (same as `hot_config`, `domain_event_bus`)
3. **Stored on** `AgentRuntime` as a field
4. **Passed via** `ExecutionParams` (new field: `context_update_queue: Option<Arc<ContextUpdateQueue>>`) — this keeps it per-request and accessible in `ReactiveEngine::execute()` without modifying `ExecutionCore`. The `ReactiveEngine` creates `LiveContextRefresher` from `params.context_update_queue` the same way it creates `MidLoopCompressor` from `params.context_window`.
5. **Cloned to** `BackgroundConsolidationService` (Phase 1 producer)
6. **Future producers** receive their `Arc` clone during construction in `app-core/init/`

## What the LLM Sees

During a ReAct loop, after iteration 3, with a memory promotion:

```
[0] System: "You are Klyntbot, a personal AI agent..."
[1] User: "Help me plan my morning"
[2] Assistant: "Let me check your tasks..." + tool_call(tasks.list)
[3] Tool: { tasks: [...] }
[4] Assistant: "I see 5 tasks. Let me check your schedule..." + tool_call(productivity.today)
[5] Tool: { focus_hours: 3.2, ... }
[6] ContextUpdate: "<context_update reason=\"memory_promoted\">
      You prefer morning deep work sessions and usually start your hardest task before 10am.
      </context_update>"
[7] Assistant: "Based on your tasks and your preference for morning deep work..."
```

The agent at [7] naturally incorporates the promoted fact because it appeared in context before the next `run_cycle`. No special prompting needed.

## User Control

A `pause_context_updates` flag on the execution context. When set, the refresher skips injection entirely. This gives users agency over their focus — some tasks (deep writing, focused finance modeling) benefit from frozen context.

Phase 1 implementation: the flag exists on `ExecutionParams`. Future: surfaced as a UI toggle.

## Interaction with Existing Systems

- **MidLoopCompressor**: Runs *before* the refresher. If compression freed tokens, the refresher has more budget. If the refresher adds tokens, the compressor may fire on the next iteration. They complement each other naturally.
- **Cognitive memory**: Promoted facts now live in both long-term storage AND the agent's immediate working memory. Salience decay feels more natural because the agent actually uses fresh memory.
- **Coaching & productivity**: Once the queue pattern is proven, coaching nudges and productivity signals can be injected mid-task — making the agent feel like a proactive co-pilot.
- **AutoTuner**: Can measure dynamic behavior (how the agent reacts to mid-loop context changes) instead of just static routing.

## Non-Goals

- Full event-bus reactivity inside the ReactiveEngine (Approach 1 — deferred, may evolve into this later)
- Frontend transparency panel changes (Phase 2+ — the value is in agent behavior first)
- Multiple producers beyond memory promotion (Phase 2+ — prove the pattern first)
- Context re-assembly via `ContextEngine.assemble()` (too heavy — we inject lightweight system messages instead)

## Testing Strategy

1. **Unit tests for ContextUpdateQueue**: push, drain, deduplication, priority ordering
2. **Unit tests for LiveContextRefresher**: inject_pending with various budget scenarios, priority trimming, empty queue no-op
3. **Integration test**: Mock a reactive loop, push a ContextUpdate mid-execution, verify the message vec contains the injected update and the agent event was emitted
4. **Message serialization test**: Verify `Message::ContextUpdate` serializes to the correct `system` role JSON for provider APIs
