# Enhanced Loop Detection — Hash-Based Iteration Signatures with Collaborative Escalation

**Date:** 2026-03-25
**Status:** Design approved, ready for implementation planning
**Origin:** DeerFlow 2.0 comparative analysis (Feature #8) — replacing the fundamentally broken `detect_oscillation()` with a reliable, explainable, second-brain-native loop detection system

---

## Problem

The current oscillation detector (`scratchpad.rs:detect_oscillation()`) compares generic `actual_action` string labels like `"tools_executed"` across a window of 6 traces. This is fundamentally broken:

- **False positives:** Three consecutive iterations that call *different* tools all record `"tools_executed"`, triggering oscillation detection and breaking legitimate progress.
- **False negatives:** Agent calls the same tool with slightly different args (semantic loop) — the generic label can't distinguish this from productive work.
- **No escalation:** Detection is binary — either break the loop or don't. No warning, no steering, no chance for self-correction.
- **No explainability:** The user has no visibility into why the agent stopped.

DeerFlow solves this with `LoopDetectionMiddleware`: MD5 hash of sorted tool call sets per iteration, warn at 3, strip tools at 5. Their approach is proven but treats detection as an invisible safety net. A second brain should do better — it should *notice* it's circling, *explain* what it sees, and *invite collaboration*.

---

## Design

### Core: LoopDetector

A new `LoopDetector` struct replaces `detect_oscillation()`. It lives in `scratchpad.rs` (or a new `loop_detector.rs` next to it) and is owned by the `Scratchpad`.

```rust
pub struct LoopDetector {
    history: VecDeque<IterationSignature>,  // sliding window, max window_size
    warned_hashes: HashSet<String>,         // once-per-hash warning guard
    window_size: usize,                     // default 20
}

pub struct IterationSignature {
    pub hash: String,                       // hex hash of sorted tool calls
    pub tools: Vec<String>,                 // tool names for transparency display
    pub iteration: usize,
}

pub enum LoopStatus {
    NoLoop,
    Warning {
        count: u32,
        hash: String,
        tools_summary: String,              // e.g. "web_search, web_fetch, grep"
    },
    HardStop {
        count: u32,
        tools_summary: String,
    },
}
```

### Hash Computation

Each iteration, after tool calls execute, the reactive engine feeds the tool call set to the detector:

```rust
impl LoopDetector {
    pub fn record_iteration(
        &mut self,
        iteration: usize,
        tool_calls: &[(String, serde_json::Value)],  // (name, args)
    ) -> LoopStatus {
        // 1. Build hash: sort by (name, stable_json(args)), hash with DefaultHasher
        // 2. Push IterationSignature to history, trim to window_size
        // 3. Count consecutive identical hashes at tail of history
        // 4. Return LoopStatus based on count vs thresholds
    }
}
```

**Hash details:**
- Sort tool calls lexicographically by name, then by a **key-sorted** JSON serialization of args
- Args key sorting must be recursive (nested objects also sorted) — do NOT reuse `hash_json_value()` from `core.rs` which iterates in `IndexMap` insertion order. Write a dedicated `stable_json_hash()` that walks the `serde_json::Value` tree and hashes keys in sorted order.
- Use `std::collections::hash_map::DefaultHasher` (no crypto needed, session-scoped only)
- Format as hex `u64` for storage and comparison
- Order-independent: permutations of the same tool call set produce the same hash

**Thresholds:**
- Warning: 3 consecutive identical hashes (configurable via `warn_threshold`)
- HardStop: 5 consecutive identical hashes (configurable via `hard_stop_threshold`)

### Escalation Behavior

**At Warning (3 identical hashes):**

1. The `record_iteration()` method checks `warned_hashes` first — if this hash was already warned, return `NoLoop` (once-per-hash guard, matching DeerFlow). This prevents duplicate events.
2. Insert hash into `warned_hashes`
3. Return `LoopStatus::Warning` — the **reactive engine** (not the detector) emits `AgentEvent::LoopDetected` only when it receives a `Warning` status
4. Inject a collaborative steering message into the conversation as a `Message::User`:
   > "I'm noticing I've been repeating the same set of tools ({tools_summary}) for the last 3 steps without finding new information. Would you like me to summarize what I've found so far, try a different approach, or keep going?"
5. Log at `WARN` level
6. The LLM still has full tool access — it gets a chance to self-correct

**At HardStop (5 identical hashes):**

1. Emit `AgentEvent::LoopHardStop { iteration, tools_summary }` for the transparency panel
2. Strip tool schemas from the next LLM call (pass empty tools array) — forces text-only response
3. Inject a synthesis message:
   > "I've been circling on this pattern. Here's what I've found so far — let me know how you'd like me to proceed."
4. Write an episodic memory (fire-and-forget):
   - domain: `"meta"`, importance: `0.6`
   - content: `"Loop detected: repeated {tools_summary} 5 times during {skill_name} task"`
5. Break the reactive loop after the synthesis response completes
6. Log at `WARN` level

### Integration with ReactiveEngine

In `reactive.rs`, the main loop currently does:

```rust
// Line 334 — current (REMOVE)
if scratchpad.detect_oscillation(3) {
    tracing::warn!("ReactiveEngine: oscillation detected...");
    break;
}
```

Replace with:

```rust
// After tool execution, extract tool call signatures from CycleOutcome
let tool_calls = extract_tool_signatures(&outcome);  // Vec<(String, Value)>
match scratchpad.loop_detector.record_iteration(iteration, &tool_calls) {
    LoopStatus::NoLoop => {}
    LoopStatus::Warning { tools_summary, .. } => {
        // Emit transparency event
        emitter.emit(AgentEvent::LoopDetected { iteration, tools_summary: tools_summary.clone(), suggestion: "..." });
        // Inject steering message
        messages.push(Message::user(format!(
            "I'm noticing I've been repeating the same set of tools ({}) ...", tools_summary
        )));
        tracing::warn!("ReactiveEngine: loop warning at iteration {iteration} — {tools_summary}");
    }
    LoopStatus::HardStop { tools_summary, .. } => {
        // Emit transparency event
        emitter.emit(AgentEvent::LoopHardStop { iteration, tools_summary: tools_summary.clone() });
        // Strip tools for next call, inject synthesis message
        // Write episodic memory
        tracing::warn!("ReactiveEngine: loop hard-stop at iteration {iteration} — {tools_summary}");
        // Perform one final tool-less LLM call, then break
    }
}
```

### Extracting Tool Signatures from CycleOutcome

`CycleOutcome::ToolsExecuted { results }` already carries `Vec<ToolExecutionResult>`, where each result has `tool_name: String` and `arguments: serde_json::Value`. Extract `(name, args)` pairs directly from the results — this is the same data the duplicate-dedup logic already uses.

```rust
// In reactive.rs, inside the ToolsExecuted match arm:
let tool_calls: Vec<(String, serde_json::Value)> = results.iter()
    .map(|r| (r.tool_name.clone(), r.arguments.clone()))
    .collect();
let loop_status = scratchpad.loop_detector.record_iteration(iteration, &tool_calls);
```

Do NOT extract from the assistant message's `tool_calls` field — using `CycleOutcome.results` is cleaner and already available at the right point in the loop.

### New AgentEvent Variants

In `crates/agent/src/events.rs`:

```rust
/// The agent detected a repeating tool call pattern (warning level).
LoopDetected {
    iteration: usize,
    tools_summary: String,
    suggestion: String,
},
/// The agent hit the hard-stop threshold for loop detection.
LoopHardStop {
    iteration: usize,
    tools_summary: String,
},
```

These are handled by the exhaustive `AgentEvent` match in `crates/app-core/src/handlers/chat/streaming.rs` (`relay_chat_stream` function). The `events_tests.rs` `all_variants()` helper must also be updated to cover the new variants.

### Episodic Memory on HardStop

Uses a fire-and-forget `tokio::spawn` pattern (same as Mirror's `write_episodic`).

**Wiring required:** `ReactiveEngine::new()` currently takes only `(Arc<ExecutionCore>, u32)`. Add an `Option<EpisodicMemoryRepo>` field to `ReactiveEngine` (or to `ExecutionCore`). The repo is available during agent init in `builder.rs` where cognitive repos are constructed. Pass it through `ExecutionRouter` → `ReactiveEngine`.

If the repo is `None` (e.g., cognitive not configured), skip the memory write silently.

**Important:** `DefaultHasher` hashes are process-scoped (random seed per run). Never persist hash values to SQLite or compare across restarts. The episodic memory stores human-readable content (`"Loop detected: repeated web_search, grep 5 times"`), not the hash itself.

---

## What Stays Unchanged

- **Duplicate tool call dedup** (`core.rs:434-504`): Hash-based `(name, args)` dedup that prevents re-execution of identical calls. This is a fast first-line defense that complements the loop detector. The dedup catches individual repeated calls; the loop detector catches repeated *patterns* of calls.
- **Fabrication detection** (`core.rs:98-167`): Multi-heuristic detection of fake tool responses. Orthogonal concern.
- **Max iterations + synthesis** (`reactive.rs:93, 356-389`): The iteration budget and synthesis prompt at exhaustion. The loop detector may trigger *before* max iterations if the agent is stuck.
- **Mid-loop compression** (`reactive.rs:343`): Context compression at 70% token usage. Orthogonal concern.

---

## What Gets Removed

- `Scratchpad::detect_oscillation()` method and its tests
- The oscillation check block in `reactive.rs` (line 334)
- The `actual_action` field on `ReasoningTrace` is no longer used for loop detection (it can remain for logging/debugging but is no longer load-bearing)

---

## File Changes

### New files

| File | Responsibility |
|------|---------------|
| `crates/agent/src/execution/loop_detector.rs` | `LoopDetector` struct, `LoopStatus` enum, `IterationSignature`, hash computation, tests |

### Modified files

| File | Change |
|------|--------|
| `crates/agent/src/execution/scratchpad.rs` | Remove `detect_oscillation()`, add `pub loop_detector: LoopDetector` field, initialize in `new()` |
| `crates/agent/src/execution/mod.rs` | Add `pub mod loop_detector;` and re-exports |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` | Replace oscillation check with `loop_detector.record_iteration()`, handle Warning/HardStop with steering messages and tool stripping |
| `crates/agent/src/events.rs` | Add `LoopDetected` and `LoopHardStop` variants to `AgentEvent` |
| `crates/agent/src/events_tests.rs` | Update `all_variants()` helper to cover new variants |
| `crates/app-core/src/handlers/chat/streaming.rs` | Add match arms for `LoopDetected` and `LoopHardStop` in `relay_chat_stream` |
| `crates/agent/src/intent_pipeline/engines/reactive.rs` or `crates/agent/src/execution/core.rs` | Thread `Option<EpisodicMemoryRepo>` for HardStop memory write |

---

## Testing Strategy

### Unit Tests (loop_detector.rs)

| Test | What it proves |
|------|----------------|
| `test_hash_consistency` | Same tool calls → same hash every time |
| `test_hash_order_independence` | `[(a, x), (b, y)]` and `[(b, y), (a, x)]` produce same hash |
| `test_different_args_different_hash` | Same tool name, different args → different hash |
| `test_no_loop_below_threshold` | 2 identical hashes → NoLoop |
| `test_warning_at_threshold` | 3 identical hashes → Warning |
| `test_hard_stop_at_threshold` | 5 identical hashes → HardStop |
| `test_warning_once_per_hash` | Second warning for same hash → NoLoop (warned guard) |
| `test_sliding_window_eviction` | Window overflows → old hashes trimmed, loop counter resets |
| `test_different_hash_resets_count` | Mixed hashes don't trigger (only consecutive identical) |
| `test_empty_tool_calls` | Empty set hashes consistently, doesn't crash |

### Integration Tests (reactive engine)

| Test | What it proves |
|------|----------------|
| `test_loop_warning_injects_message` | Warning triggers steering message injection |
| `test_loop_hard_stop_strips_tools` | HardStop strips tool schemas and forces synthesis |

---

## Deferred (not in scope)

- **Mid-execution interactive buttons** (needs engine-level interrupt support beyond `AskUserTool`)
- **Per-skill threshold tuning** (can add later via `ExecutionParams`)
- **Cognitive procedural rule creation** from user's response to loop steering (needs session-level user reply tracking)
- **Confidence-modulated thresholds** (low-confidence loops escalate faster — future refinement)
