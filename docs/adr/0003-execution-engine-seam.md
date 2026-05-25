# Replace the dead `ExecutionEngine` trait with a real, mockable execution seam

`crates/agent/src/engines/mod.rs` held an `ExecutionEngine` trait and an
`EngineResult` enum with **zero implementors and zero callers** anywhere in the
workspace. The trait was *stale*, not merely dormant: its return type
`EngineResult { Complete | Escalate }` encoded a Direct→Reactive *escalation*
model that the module's own doc comment admits was already unified away into
`execution::execute_loop`. Its signature didn't match the real path either (no
`cap: &mut SafetyCap`, wrong return type). The deletion test was unambiguous:
deleting the module concentrated no complexity — it was a pass-through to nowhere.

Meanwhile the *real* unifying contract already existed as a free function,
`execution::execute_loop::execute_loop(core, messages, tools, params, cap, ctx, event_tx)
-> ExecuteLoopResult`, with exactly two production adapters already calling it:
`AgentRuntime::process_message` (`runtime.rs:570`) and the subagent loop
(`subagent.rs:728`, `subagent.rs:863`). Two adapters → the seam is real; it was
just unnamed and **untestable** — there was no way to make `execute_loop` produce
a scripted result or scripted events, so the behaviour wrapped around it (the
subagent event-forwarder, the runtime recording tail) had no unit coverage.

We **deleted the stale trait and `EngineResult`**, and introduced a new
`ExecutionEngine` trait whose shape mirrors the real path:

```rust
#[async_trait]
pub trait ExecutionEngine: Send + Sync {
    async fn run(
        &self,
        messages: Vec<providers::Message>,
        tools: &[serde_json::Value],
        params: &ExecutionParams,
        cap: &mut SafetyCap,
        ctx: &RoutingContext,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<ExecuteLoopResult>;
}
```

with two adapters: `CoreEngine` (the production adapter, owns `Arc<ExecutionCore>`
and delegates to `execute_loop` — behaviour identical to today) and `MockEngine`
(a recording/scripting adapter for tests). Both production call sites route
through `CoreEngine`.

We deliberately **kept `execute_loop` as a free function** behind `CoreEngine`
rather than folding the loop body into the trait impl. The loop is already deep
and well-factored; the seam belongs *above* it. We also **left `ExecutionCore`
untouched** — it is genuinely deep (concurrency partitioning, dedup, truncation,
event fan-out behind `run_cycle`) and is not the shallow module here.

We **rejected reviving the old trait shape** (the chosen Option B over "revive as
written"): resurrecting an abstraction the codebase had already abandoned would
re-introduce the dead `Escalate` concept and a signature that doesn't match
reality. We **rejected trait-ifying `ExecutionCore` itself**: that is the deep
module, mocking it would mean mocking the entire LLM→tool cycle, and nothing asks
for that. We **rejected leaving the dead module in place** (Option C / "obsolete"):
dead, stale code that names a real concept misleads the next explorer into
thinking the seam is wired when it is not.

**The seam must earn its keep via the mock.** A trait with a single production
adapter is ceremony — its deletion test passes trivially (callers would just call
`execute_loop` again). The justification is that `MockEngine` unlocks tests that
could not exist before. The first such test (Phase B) covers the subagent
**event-forwarder** (`subagent.rs`): a `MockEngine` emits scripted
`IterationStart`/`ToolStart` events and we assert the progress map and
`SubagentLifecycleEvent::Progress` stream update correctly — behaviour that was
untestable while `execute_loop` was the only producer.

Migration phases (each independently revertible, behaviour-identical until B adds
tests):

- **Phase A** — replace `engines/mod.rs` contents (new trait + `CoreEngine`,
  delete `EngineResult`); route the runtime call site and both subagent call sites
  through `CoreEngine`. Mechanical; behaviour identical.
- **Phase B** — add `MockEngine`; thread `&dyn ExecutionEngine` through the
  subagent execute step; add the event-forwarder test. This is what makes the
  seam load-bearing.

This change touches the chat hot path (the runtime call site). The hot path is
under perf gates (`./scripts/run_chat_perf_gates.sh`); they are a hard gate before
commit. The added cost is one `Arc` clone + one dynamic dispatch per *message*
(not per token) — negligible.

See `docs/architecture/deep-dive-03-execution-engine-seam.md` for the
field-by-field design, the `MockEngine` shape, the phase plan, and open risks.
