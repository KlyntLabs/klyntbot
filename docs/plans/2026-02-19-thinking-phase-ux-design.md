# Thinking Phase UX — Design

## Problem

When the AI processes a message, the CLI shows only `Thinking...` with a braille spinner. Users have no visibility into what's happening: intent classification, context assembly, tool calls, iteration loops. The spinner hides all intermediate work.

## Decision Summary

- **Target**: CLI terminal only (channels unchanged)
- **Detail level**: Stages + tool calls by default, full args/results with `--verbose`
- **After completion**: Collapse trace to one-line summary
- **Approach**: Event propagation — pipe `AgentEvent` through pipeline stages in real-time

## Architecture

### Event Flow

```
pipeline.process_message(msg, ctx, event_tx)
  ├─ classify()      → emit ClassificationComplete
  ├─ assemble()      → emit ContextAssembled
  └─ execute()       → emit ExecutionStarted
      └─ engine
          ├─ iteration loop → emit IterationStart
          └─ tool calls     → emit ToolStart / ToolEnd (real-time)
                                    ↓
                              event_tx.send()
                                    ↓
                          cli chat.rs event loop
                                    ↓
                          ThinkingRenderer.on_event()
```

### New Event Variants

Add to `AgentEvent` in `agent/src/events.rs`:

```rust
ClassificationComplete {
    result: String,        // e.g. "ToolAssisted"
    confidence: f64,
    method: String,        // "heuristic" or "llm"
    duration_ms: u64,
}

ContextAssembled {
    total_tokens: usize,
    budget: usize,
    duration_ms: u64,
}

ExecutionStarted {
    engine: String,        // "Direct", "ReactPlus"
    max_iterations: usize,
}
```

### Event Channel Threading

`pipeline.process_message()` gains `event_tx: Option<Sender<AgentEvent>>`. Optional so channel integrations don't break. Execution engines receive it via `ExecutionContext`.

### ThinkingRenderer

New component in `common::utils::terminal::thinking_renderer`.

**Normal mode:**
```
✓ Classified → ToolAssisted           0.3s
✓ Context assembled                   0.1s
▸ Executing (iteration 1/5)
  ✓ todo_add                           0.8s
  ⡇ todo_search
```

**Verbose mode (`--verbose`):**
```
✓ Classified: ToolAssisted (0.85)      312ms
  method: heuristic, escalated: false
✓ Context: 2,400/8,192 tokens          48ms
  system: 1,200 | history: 800 | tools: 400
▸ ReactPlus iteration 1/5
  ✓ todo_add                            832ms
    args: {title: "Finance...", priority: 3}
    result: Created task #42
  ⡇ ask_user
    args: {question: "What priority?"}
```

**Collapse on completion:**
After `AgentEvent::Done`, ThinkingRenderer rewrites the block to:
```
── o4-mini · 5.1s (3 tools, 2 iters) ─────────
```

Uses crossterm cursor manipulation. Non-TTY fallback: print events as plain lines, no collapse.

### CLI Flag

Add `--verbose` / `-V` to the `chat` subcommand.

## Files Changed

| Crate | File | Change |
|-------|------|--------|
| `agent` | `events.rs` | Add 3 new event variants |
| `agent` | `pipeline.rs` | Accept `event_tx`, emit stage events |
| `agent` | `execution/core.rs` | Emit ToolStart/ToolEnd in real-time |
| `agent` | `execution/dispatch.rs` | Pass event_tx to engines |
| `agent` | `execution/react_plus.rs` | Emit IterationStart, pass event_tx |
| `agent` | `agent_loop.rs` | Pass event_tx into pipeline |
| `common` | `utils/terminal/thinking_renderer.rs` | NEW: ThinkingRenderer component |
| `common` | `utils/stream_renderer.rs` | Un-suppress IterationStart, delegate to ThinkingRenderer |
| `cli` | `chat.rs` | Replace Spinner with ThinkingRenderer |
| `cli` | `lib.rs` | Add `--verbose` flag |

## Constraints

- `event_tx` is `Option<Sender<AgentEvent>>` — channels that don't support streaming pass `None`
- ThinkingRenderer must handle non-TTY gracefully (plain text, no cursor control)
- Collapse uses crossterm to rewrite lines — must track line count accurately
- Existing `StreamRenderer` tool display still works for the response phase (after thinking)
- No changes to channel integrations (Telegram, Discord, etc.) in this iteration

## Non-Goals

- Runtime verbose toggle (too complex for terminal)
- Channel integration thinking display (future work)
- Config-based verbosity (use CLI flag)
