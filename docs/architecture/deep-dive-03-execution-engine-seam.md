# Deep dive: the `ExecutionEngine` seam

> Companion to ADR-0003. The ADR records *what* we decided and *why*; this doc
> records the field-by-field design, the adapters, the phase plan, and the risks.

## Vocabulary (LANGUAGE.md)

- **Module under review:** `crates/agent/src/engines/mod.rs` (was dead), plus the
  three call sites that invoke `execution::execute_loop`.
- **Seam:** the `ExecutionEngine` trait — the place a single agentic turn's
  "run the loop, give me the result" behaviour can be swapped without editing the
  callers in place.
- **Adapters:** `CoreEngine` (production) and `MockEngine` (test).
- **Depth being preserved:** `ExecutionCore` and `execute_loop` stay deep and
  unchanged; the seam sits *above* them.

## What was there

```
crates/agent/src/engines/mod.rs   (DEAD — 0 implementors, 0 callers)
  trait ExecutionEngine { async fn execute(messages, tools, params, ctx, event_tx)
                            -> EngineResult; fn mode(&self) -> &str; }
  enum EngineResult { Complete{..}, Escalate{usage} }   // Escalate = abandoned
                                                         // Direct→Reactive model
```

`grep -rn EngineResult|ExecutionEngine crates/` matched only this file (the other
`engines::` hits are the unrelated `voice-engine` module). Stale shape: returns
`EngineResult`, not `ExecuteLoopResult`; no `cap`. Reviving it = resurrecting a
model the codebase already deleted.

## The real contract (already exists)

```rust
// crates/agent/src/execution/execute_loop.rs
pub async fn execute_loop(
    core: &ExecutionCore,
    messages: Vec<providers::types::Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    cap: &mut SafetyCap,
    ctx: &RoutingContext,
    event_tx: Option<Sender<AgentEvent>>,
) -> Result<ExecuteLoopResult>
```

Three call sites, all building/holding `Arc<ExecutionCore>`:

| Caller | Location | event_tx | cap |
|--------|----------|----------|-----|
| `AgentRuntime::process_message` | `runtime.rs:570` | TTFT relay sender | `SafetyCap::new(depth)` (main agent: uncapped turns) |
| `run_subagent_loop` | `subagent.rs:728` | `None` | `with_limits(Normal, 0, max_turns)` |
| `run_subagent_task` | `subagent.rs:863` | `Some(agent_event_tx)` | `with_limits(Normal, 0, 500)` |

The third is the interesting one: `agent_event_tx` feeds a `forwarder` task
(`subagent.rs:831`) that turns `IterationStart`/`ToolStart` into
`SubagentLifecycleEvent::Progress` and updates the shared `progress` map. That
forwarder has **no test** because nothing can make `execute_loop` emit a chosen
event sequence on demand.

## The new seam

```rust
// crates/agent/src/engines/mod.rs (replacement)
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    async fn run(
        &self,
        messages: Vec<providers::Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        cap: &mut SafetyCap,
        ctx: &RoutingContext,
        event_tx: Option<tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    ) -> common::Result<ExecuteLoopResult>;
}

pub struct CoreEngine {
    core: Arc<ExecutionCore>,
}
impl CoreEngine {
    pub fn new(core: Arc<ExecutionCore>) -> Self { Self { core } }
}
#[async_trait]
impl ExecutionEngine for CoreEngine {
    async fn run(&self, messages, tools, params, cap, ctx, event_tx)
        -> Result<ExecuteLoopResult>
    {
        execute_loop(&self.core, messages, tools, params, cap, ctx, event_tx).await
    }
}
```

`#[async_trait]` (not native AFIT): the method takes `&mut SafetyCap` and is held
behind `&dyn ExecutionEngine`; boxing the future keeps the trait object simple and
matches every other boxed-future path in this crate. Cost is one box alloc per
*message* — noise next to an LLM round-trip.

### `MockEngine` (Phase B)

A recording + scripting adapter:

```rust
pub struct MockEngine {
    /// Events emitted (in order) before returning, to drive forwarder tests.
    scripted_events: Vec<AgentEvent>,
    /// The result `run` returns.
    result: ExecuteLoopResult,
    /// Captured (messages.len(), tools.len()) per call, for assertions.
    calls: Arc<Mutex<Vec<(usize, usize)>>>,
}
```

`run` records the call, fires each scripted event on `event_tx` (if `Some`), then
returns a clone of `result`. (`ExecuteLoopResult` gains `#[derive(Clone)]` — it is
plain data: `String`, `Usage`, `u32`, `bool`, `Vec<String>`, `LoopFinishReason`.)

## Phase plan

- **Phase A — route through `CoreEngine` (mechanical, behaviour-identical).**
  Replace `engines/mod.rs`; delete `EngineResult`. At each of the three call sites,
  build `CoreEngine::new(Arc::clone(&core))` and call `.run(...)` instead of
  `execute_loop(&core, ...)`. No struct changes. `execute_loop` stays `pub` (the
  adapter calls it). Verify: build + clippy + nextest + perf gates.

- **Phase B — make the seam load-bearing.** Add `MockEngine`; derive `Clone` on
  `ExecuteLoopResult`. Thread `&dyn ExecutionEngine` into the subagent execute step
  (the smallest injectable unit) and add a test: a `MockEngine` scripts
  `IterationStart{iteration:3}` + `ToolStart{name:"bash"}`, and we assert the
  `progress` map reaches `(3, Some("bash"))` and a `Progress` lifecycle event is
  sent. Verify: build + clippy + nextest (no hot-path change → perf gates
  unaffected, but re-run to be safe).

## Open risks

- **Hot path.** Phase A changes `runtime.rs:570`. The added cost is one `Arc`
  clone + one vtable dispatch per message. Perf gates are the gate; if TTFT or
  throughput regress, revert Phase A's runtime hunk (subagent hunks are
  independent).
- **`event_tx` ownership in the mock.** The forwarder relies on the sender being
  dropped when `run` returns so its `recv()` loop terminates. `MockEngine::run`
  takes `event_tx` by value like the real path, so the drop semantics match — the
  test must `await` the forwarder after `run` returns, exactly as
  `run_subagent_task` does.
- **Scope creep into the runtime tail.** Tempting to also extract the
  `process_message` "record" tail behind the seam for testing. Out of scope for
  #2 — recorded as a possible future candidate; do not expand here.
