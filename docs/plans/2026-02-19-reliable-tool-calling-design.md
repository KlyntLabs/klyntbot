# Reliable Tool Calling Across All LLM Models

**Problem**: Models like DeepSeek, Kimi, Gemini skip tool calls entirely and fabricate text responses that look like tool results. The `todo` tool is never called — the LLM generates fake "Task Created: Buy groceries (ID: 9c4e5f3b)" output as plain text. No task is saved to the database.

**Root cause confirmed**: `ExecutionCore: LLM returned text response (no tool calls)` — DeepSeek returns a `FinalResponse` with zero tool calls. The ReAct+ loop exits immediately on the first iteration.

**Goal**: Make tool calling work reliably with ALL models by detecting fabricated responses and forcing a retry.

## Architecture: Two-Layer Defense

### Layer 1: Fabrication Detector in ExecutionCore

**File**: `crates/agent/src/execution/core.rs`

After `provider.chat()` returns a response with no tool calls, check if the text looks like a fabricated tool response. Detection is a pure function:

```rust
fn is_fabricated_tool_response(text: &str, tool_names: &[&str]) -> bool
```

**Heuristics**:
- Text mentions known tool names from the registry
- Contains structured patterns: "Task Created:", "ID: " + hex, fake bullet-point results
- Contains markdown-formatted "results" with fields like Priority/Due Date/Tags
- Pattern: text describes executing an action the user asked for, but no tool was called

**New CycleOutcome variant**:
```rust
pub enum CycleOutcome {
    FinalResponse { content: String },
    ToolsExecuted { results: Vec<ToolExecutionResult> },
    EmptyResponse,
    FabricatedResponse { content: String },  // NEW
}
```

Returns `FabricatedResponse` when fabrication is detected. Otherwise returns `FinalResponse` as before.

### Layer 2: Zero-Tool-Call Guard in ReactPlusEngine

**File**: `crates/agent/src/execution/react_plus.rs`

When `run_cycle()` returns `FabricatedResponse` (or `FinalResponse` on iteration 1 with zero tools ever called in a `ToolAssisted` strategy), the engine:

1. Injects a force message into the conversation:
   ```
   "You returned a text response instead of calling a tool.
    You have these tools available: [tool_names].
    You MUST call the appropriate tool. Do NOT respond with text."
   ```
2. Re-calls `run_cycle()` once (retry)
3. If the retry also returns text → graceful degradation, return text as `FinalResponse`

**Key constraint**: Only retry ONCE. A `force_retried` boolean prevents infinite loops.

### Layer 3: Remove TodoTool Code Guard (Cleanup)

The `should_guard_creation()` function, `confirmed` parameter, and `creation_mode` check in `TodoTool.execute()` become unnecessary. The guard now lives at the execution engine level (universal, not tool-specific).

Remove:
- `should_guard_creation()` function and its unit tests
- `confirmed` parameter from tool schema
- `creation_mode` field and check in `execute()`
- `CreationMode` enum from config (or keep for future use)

The todo skill's ask-first instructions handle the workflow once tool calling works.

## Data Flow (After)

```
User: "create task: buy"
  ↓
ReactPlusEngine iteration 1:
  ExecutionCore::run_cycle()
    → LLM returns text: "Task Created: Buy groceries..."
    → is_fabricated_tool_response() → true
    → returns CycleOutcome::FabricatedResponse
  ↓
  ReactPlusEngine detects FabricatedResponse
    → injects force-tool-use message
    → sets force_retried = true
  ↓
ReactPlusEngine iteration 2 (retry):
  ExecutionCore::run_cycle()
    → LLM returns tool call: ask_user("What do you want to buy?")
    → tool executes, returns result
    → returns CycleOutcome::ToolsExecuted
  ↓
ReactPlusEngine iteration 3:
  → LLM returns tool call: todo(add, "Buy groceries for the weekend")
  → tool executes, task saved to DB
  ↓
ReactPlusEngine iteration 4:
  → LLM returns FinalResponse with confirmation
  → returned to user
```

## Files Changed

| File | Change |
|------|--------|
| `crates/agent/src/execution/core.rs` | Add `is_fabricated_tool_response()`, return `FabricatedResponse` variant |
| `crates/agent/src/execution/types.rs` | Add `FabricatedResponse` to `CycleOutcome` enum |
| `crates/agent/src/execution/react_plus.rs` | Handle `FabricatedResponse` with force-retry logic |
| `crates/agent/src/execution/direct.rs` | Handle new `FabricatedResponse` variant (treat as text) |
| `crates/tools/src/todo.rs` | Remove `should_guard_creation()`, `confirmed` param, `creation_mode` check |
| `crates/config/src/schema/core.rs` | Keep `CreationMode` but remove from `TodoTool` constructor |

## Testing

- Unit test `is_fabricated_tool_response()` with known fabricated outputs
- Unit test ReactPlusEngine retry logic with mock provider sequences
- Integration test: mock provider returns fabricated text first, tool call second
- Manual test: `klyntbot chat "create task: buy"` with DeepSeek
