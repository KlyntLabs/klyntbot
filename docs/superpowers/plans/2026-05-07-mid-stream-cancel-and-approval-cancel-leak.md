# Mid-LLM-Stream Cancel + Approval Cancel-Leak Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make user cancellation observable mid-LLM-stream (10–50 ms granularity) and close the `ApprovalChannel` cancel-leak so a hung approval modal cannot pin the agent loop forever. Phase 0.3 of the long-running-task roadmap.

**Architecture:** No trait API changes. Race the existing `LlmStream::next()` and `ApprovalChannel::request()` futures against `params.cancel_token.cancelled()` at the call site using `tokio::select!`. Dropping the stream tears down the underlying `reqwest::Response` and HTTP connection automatically. On cancel, partial content/reasoning accumulated so far is preserved and surfaced to the UI via a new `LlmResponse` finish_reason and `AgentEvent::Cancelled`.

**Tech Stack:** Rust 1.93, `tokio::select!`, `tokio_util::sync::CancellationToken`, `tokio-stream`, existing `providers::LlmStream`, `agent::execution::ExecutionParams`, `approval::ApprovalGate`.

---

## File Structure

| File | Role | Change |
|---|---|---|
| `crates/agent/src/execution/core.rs` | Provider streaming consumer + tool-loop coordinator | Add cancel-aware loop in `call_provider_streaming`; thread token through `run_cycle` and the `gate.check` call site |
| `crates/agent/src/execution/execute_loop.rs` | Top-level iteration loop | Treat `finish_reason == "cancelled"` from a cycle as a clean cancel exit and emit `AgentEvent::Cancelled` with partial content |
| `crates/agent/src/events.rs` | `AgentEvent` enum | Add a `Cancelled { partial_content, partial_reasoning }` variant |
| `crates/approval/src/gate.rs` | Approval gating | Change `ApprovalGate::check` signature to take `&CancellationToken`; race `channel.request()` against it |
| `crates/approval/src/lib.rs` | Re-exports | (no change expected — verify) |
| `crates/agent/tests/cancel_during_stream.rs` *(new)* | Integration test | Hung-stream cancel test |
| `crates/approval/src/gate.rs` (test mod) | Unit tests | Cancel-during-channel-request test |

The cancel-token plumbing is "explicit argument all the way down." No new types. `params.cancel_token: Option<CancellationToken>` already exists; a helper unwraps it to a never-cancelled token when `None` so call sites stay simple.

---

## Task 1: Add `Cancelled` variant to `AgentEvent`

**Files:**
- Modify: `crates/agent/src/events.rs`

- [ ] **Step 1: Read current enum to find correct insertion point**

Run: `grep -n "pub enum AgentEvent" crates/agent/src/events.rs`
Expected: line ~12. Read 30 lines around it to see existing variants and serde tagging.

- [ ] **Step 2: Add the `Cancelled` variant**

Open `crates/agent/src/events.rs` and add this variant inside the `AgentEvent` enum, alphabetically near other lifecycle variants (next to `SubagentCancelled` is fine):

```rust
    /// Emitted when the user cancels mid-LLM-stream. Carries whatever
    /// content/reasoning had been streamed before the cancel was observed.
    /// Both fields may be empty strings if cancel raced the very first chunk.
    Cancelled {
        partial_content: String,
        partial_reasoning: String,
    },
```

Match the existing serde attribute style (e.g. `#[serde(rename_all = "snake_case")]` on the enum) — the variant inherits the convention; no per-variant attribute should be needed.

- [ ] **Step 3: Build to confirm the variant compiles**

Run: `cargo build -p agent`
Expected: success. If exhaustive `match` arms elsewhere break, add `AgentEvent::Cancelled { .. } => {}` placeholders — fix them properly in later tasks (turn_handler, bridge).

- [ ] **Step 4: Commit**

```bash
git add crates/agent/src/events.rs
git commit -m "feat(agent): add AgentEvent::Cancelled variant for mid-stream cancel"
```

---

## Task 2: Failing test — provider stream cancels mid-stream

**Files:**
- Create: `crates/agent/tests/cancel_during_stream.rs`

- [ ] **Step 1: Write the failing test**

Create the file with this exact content:

```rust
//! Verifies that `call_provider_streaming` (consumed by `ExecutionCore`)
//! observes a `CancellationToken` between SSE chunks and returns promptly
//! with `finish_reason == "cancelled"`, preserving any partial content.

use std::sync::Arc;
use std::time::Duration;

use futures_util::stream;
use providers::{
    ChatParams, DynProvider, LlmProvider, LlmResponse, LlmStream, LlmStreamChunk, Message,
    ToolCallDelta, Usage,
};
use tokio_util::sync::CancellationToken;

/// A provider that streams one content chunk, then hangs forever on the next
/// chunk until cancelled. Mirrors a slow / stuck Anthropic SSE connection.
struct HungStreamProvider;

#[async_trait::async_trait]
impl LlmProvider for HungStreamProvider {
    fn name(&self) -> &str { "hung-stream-provider" }

    async fn chat(
        &self,
        _msgs: &[Message],
        _tools: Option<&[serde_json::Value]>,
        _params: &ChatParams,
        _bp: &[providers::CacheBreakpoint],
    ) -> common::Result<LlmResponse> {
        unreachable!("test only exercises chat_stream")
    }

    async fn chat_stream(
        &self,
        _msgs: &[Message],
        _tools: Option<&[serde_json::Value]>,
        _params: &ChatParams,
        _bp: &[providers::CacheBreakpoint],
    ) -> common::Result<LlmStream> {
        // First yield one content chunk, then a future that never resolves.
        let chunk = LlmStreamChunk {
            content: Some("partial-".to_string()),
            reasoning_content: None,
            tool_call_delta: None,
            finish_reason: None,
            usage: None,
        };
        let first = stream::iter(vec![Ok::<_, common::KlyntbotError>(chunk)]);
        let pending = stream::pending::<common::Result<LlmStreamChunk>>();
        Ok(Box::pin(first.chain(pending)))
    }
}

#[tokio::test(flavor = "current_thread")]
async fn cancel_during_stream_returns_partial_within_50ms() {
    use agent::execution::ExecutionCore;

    let provider: DynProvider = Arc::new(HungStreamProvider);
    let core = ExecutionCore::new(provider.clone(), 200_000);

    let token = CancellationToken::new();
    let token_for_task = token.clone();

    // Drive the cycle in a task; cancel after 20ms.
    let handle = tokio::spawn(async move {
        let params = agent::execution::ExecutionParams::new("hung-stream-provider", 200_000)
            .with_cancel_token(token_for_task);

        let (tx, _rx) = tokio::sync::mpsc::channel(64);
        core.run_cycle(
            &[Message::User { content: providers::UserContent::Text("hi".into()) }],
            &[],
            &params,
            Some(&tx),
            None,
            &[],
        )
        .await
    });

    tokio::time::sleep(Duration::from_millis(20)).await;
    token.cancel();

    let result = tokio::time::timeout(Duration::from_millis(200), handle)
        .await
        .expect("run_cycle should return within 200ms of cancel")
        .expect("task should not panic")
        .expect("run_cycle should return Ok");

    // Whatever the cycle returns, finish_reason MUST be "cancelled" and any
    // accumulated content from the first chunk MUST be preserved.
    assert_eq!(result.response.finish_reason, "cancelled");
    assert_eq!(result.response.content, "partial-");
}
```

> Note: `ExecutionCore::run_cycle` returns a `CycleOutcome` whose `response` field carries the `LlmResponse`. If the actual struct names differ in `crates/agent/src/execution/types.rs::CycleOutcome`, adjust the assertion path — but do NOT change behaviour, only field access.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p agent --test cancel_during_stream`
Expected: FAIL — either compile error if `run_cycle` API differs, or timeout because cancel is currently observed only between iterations, not mid-stream.

- [ ] **Step 3: Commit the failing test**

```bash
git add crates/agent/tests/cancel_during_stream.rs
git commit -m "test(agent): failing test for mid-stream cancel"
```

---

## Task 3: Make `call_provider_streaming` cancel-aware

**Files:**
- Modify: `crates/agent/src/execution/core.rs:278-378`

- [ ] **Step 1: Add `cancel_token: &CancellationToken` parameter**

Find the function signature at `crates/agent/src/execution/core.rs:278`:

```rust
async fn call_provider_streaming(
    provider: &DynProvider,
    messages: &[Message],
    tools: &[serde_json::Value],
    params: &providers::ChatParams,
    event_tx: &tokio::sync::mpsc::Sender<crate::events::AgentEvent>,
    domain_bus: Option<&Arc<bus::DomainEventBus>>,
    cache_breakpoints: &[providers::CacheBreakpoint],
) -> Result<providers::LlmResponse> {
```

Add a `cancel_token: &tokio_util::sync::CancellationToken` argument as the **last** parameter:

```rust
async fn call_provider_streaming(
    provider: &DynProvider,
    messages: &[Message],
    tools: &[serde_json::Value],
    params: &providers::ChatParams,
    event_tx: &tokio::sync::mpsc::Sender<crate::events::AgentEvent>,
    domain_bus: Option<&Arc<bus::DomainEventBus>>,
    cache_breakpoints: &[providers::CacheBreakpoint],
    cancel_token: &tokio_util::sync::CancellationToken,
) -> Result<providers::LlmResponse> {
```

- [ ] **Step 2: Replace the consume loop with a cancel-aware select**

The current loop at line 304 is:

```rust
    while let Some(result) = stream.next().await {
        let chunk = result?;
        // ...
    }
```

Replace it with:

```rust
    loop {
        tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                tracing::info!(
                    content_len = content.len(),
                    reasoning_len = reasoning.len(),
                    "call_provider_streaming: cancellation observed mid-stream"
                );
                // Drop `stream` here on scope exit → tears down the
                // underlying reqwest::Response → closes the upstream
                // HTTP connection automatically.
                fan_out_event(
                    Some(event_tx),
                    domain_bus,
                    crate::events::AgentEvent::Cancelled {
                        partial_content: content.clone(),
                        partial_reasoning: reasoning.clone(),
                    },
                )
                .await;
                finish_reason = "cancelled".to_string();
                break;
            }
            maybe_chunk = stream.next() => {
                match maybe_chunk {
                    None => break,
                    Some(result) => {
                        let chunk = result?;
                        // ── existing chunk-handling body, unchanged ──
                        if let Some(text) = chunk.content {
                            if !text.is_empty() {
                                content.push_str(&text);
                                fan_out_event(
                                    Some(event_tx),
                                    domain_bus,
                                    crate::events::AgentEvent::ContentChunk { data: text },
                                )
                                .await;
                            }
                        }
                        if let Some(r) = chunk.reasoning_content {
                            if !r.is_empty() {
                                reasoning.push_str(&r);
                                fan_out_event(
                                    Some(event_tx),
                                    domain_bus,
                                    crate::events::AgentEvent::ReasoningChunk { data: r },
                                )
                                .await;
                            }
                        }
                        if let Some(delta) = chunk.tool_call_delta {
                            while partials.len() <= delta.index {
                                partials.push(PartialToolCall {
                                    id: String::new(),
                                    name: String::new(),
                                    args: String::new(),
                                });
                            }
                            let partial = &mut partials[delta.index];
                            if let Some(id) = delta.id { partial.id = id; }
                            if let Some(name) = delta.name { partial.name.push_str(&name); }
                            if let Some(args) = delta.arguments { partial.args.push_str(&args); }
                        }
                        if let Some(reason) = chunk.finish_reason {
                            finish_reason = reason;
                        }
                        if let Some(chunk_usage) = chunk.usage {
                            has_real_usage = true;
                            if chunk_usage.prompt_tokens > 0 {
                                accumulated_usage.prompt_tokens = chunk_usage.prompt_tokens;
                            }
                            if chunk_usage.completion_tokens > 0 {
                                accumulated_usage.completion_tokens = chunk_usage.completion_tokens;
                            }
                            if chunk_usage.cache_read_tokens > 0 {
                                accumulated_usage.cache_read_tokens = chunk_usage.cache_read_tokens;
                            }
                            if chunk_usage.cache_write_tokens > 0 {
                                accumulated_usage.cache_write_tokens = chunk_usage.cache_write_tokens;
                            }
                        }
                    }
                }
            }
        }
    }
```

The `biased;` directive ensures the cancel branch is polled first on each iteration, so a token cancelled before the next chunk is always observed.

- [ ] **Step 3: Update the single internal call site (line 586)**

Find `call_provider_streaming(` at `crates/agent/src/execution/core.rs:586` (inside `ExecutionCore::run_cycle`). Threading the token requires it to be available in `run_cycle`. Check the surrounding code: `params: &ExecutionParams` is in scope. `params.cancel_token: Option<CancellationToken>`. Use a never-cancelled token when `None`:

Before the `call_provider_streaming(...)` call, add:

```rust
        let cancel_token_owned;
        let cancel_token_ref = match &params.cancel_token {
            Some(t) => t,
            None => {
                cancel_token_owned = tokio_util::sync::CancellationToken::new();
                &cancel_token_owned
            }
        };
```

Then update the call:

```rust
        call_provider_streaming(
            &self.provider,
            messages,
            tools,
            &chat_params,
            event_tx,
            domain_bus,
            cache_breakpoints,
            cancel_token_ref,
        )
        .await?
```

(Argument names match what's already there; only the new trailing arg is added.)

- [ ] **Step 4: Build to confirm it compiles**

Run: `cargo build -p agent`
Expected: success. If `tokio_util` isn't already a direct dep of `agent`, it is — confirmed by existing `params.cancel_token: Option<tokio_util::sync::CancellationToken>` field. No `Cargo.toml` change needed.

- [ ] **Step 5: Run the failing test from Task 2**

Run: `cargo nextest run -p agent --test cancel_during_stream`
Expected: PASS — cancel observed within 50ms, partial content preserved, finish_reason = "cancelled".

- [ ] **Step 6: Run the full agent test suite to catch regressions**

Run: `cargo nextest run -p agent`
Expected: all green.

- [ ] **Step 7: Commit**

```bash
git add crates/agent/src/execution/core.rs
git commit -m "feat(agent): cancel-aware SSE consume in call_provider_streaming

Race stream.next() against cancel_token.cancelled() with tokio::select!.
On cancel, drop the stream (closes upstream HTTP connection), preserve
accumulated content/reasoning, set finish_reason='cancelled', and emit
AgentEvent::Cancelled."
```

---

## Task 4: Surface mid-stream cancel in `execute_loop`

**Files:**
- Modify: `crates/agent/src/execution/execute_loop.rs`

**Why:** When `run_cycle` returns with `finish_reason == "cancelled"` (either because the token was cancelled mid-stream OR because cancel raced an iteration boundary), `execute_loop` should exit cleanly with `LoopFinishReason::Cancelled` rather than continuing to the next iteration.

- [ ] **Step 1: Read current cycle-completion handling**

Run: `grep -n "finish_reason\|run_cycle\|CycleOutcome" crates/agent/src/execution/execute_loop.rs | head -20`
Read the relevant region (look for where `run_cycle` is called and its outcome inspected; existing `Completed` and `LoopDetected` exits indicate the pattern).

- [ ] **Step 2: Add cancel detection right after `run_cycle` returns**

After the `run_cycle` call, before any other processing of its result, insert:

```rust
        if outcome.response.finish_reason == "cancelled" {
            return Ok(ExecuteLoopResult {
                content: outcome.response.content.clone(),
                usage: accumulated_usage,
                turns: cap.turns_used(),
                safety_cap_hit: false,
                tool_calls: all_tool_calls,
                finish_reason: LoopFinishReason::Cancelled,
            });
        }
```

(Adjust the binding name `outcome` to match what the existing code uses; if the existing pattern is `let outcome = core.run_cycle(...).await?;`, the snippet works as-is.)

- [ ] **Step 3: Build**

Run: `cargo build -p agent`
Expected: success.

- [ ] **Step 4: Run agent tests**

Run: `cargo nextest run -p agent`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/agent/src/execution/execute_loop.rs
git commit -m "feat(agent): exit execute_loop cleanly on mid-stream cancel"
```

---

## Task 5: Failing test — `ApprovalGate::check` cancels during channel.request

**Files:**
- Modify: `crates/approval/src/gate.rs` (add new test in `#[cfg(test)] mod tests`)

- [ ] **Step 1: Add a hung channel stub and the failing test**

In the `tests` module of `crates/approval/src/gate.rs`, add:

```rust
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    /// Channel whose `request` future never resolves until dropped.
    struct HungChannel;

    #[async_trait::async_trait]
    impl ApprovalChannel for HungChannel {
        async fn request(&self, _r: ApprovalRequest) -> ApprovalDecision {
            std::future::pending().await
        }
        fn capabilities(&self) -> ApprovalCapabilities {
            ApprovalCapabilities {
                supports_inline: true,
                supports_classes: HashSet::from([ApprovalClass::Destructive]),
            }
        }
    }

    #[tokio::test]
    async fn cancel_during_channel_request_returns_cancel_outcome() {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        let repo = ApprovalGrantsRepo::new(pool);
        let gate = ApprovalGate::new(repo, Arc::new(HungChannel));

        let token = CancellationToken::new();
        let token_for_task = token.clone();

        let req = ApprovalRequest {
            tool_name: "bash".into(),
            action: None,
            args: serde_json::json!({"cmd":"rm"}),
            class: ApprovalClass::Destructive,
            scope: ApprovalScope::ToolAction,
            ctx: ctx(),
            preview: None,
            suggested_grant: None,
        };

        let handle = tokio::spawn(async move { gate.check(req, &token_for_task).await });

        tokio::time::sleep(Duration::from_millis(20)).await;
        token.cancel();

        let outcome = tokio::time::timeout(Duration::from_millis(200), handle)
            .await
            .expect("gate.check should return within 200ms of cancel")
            .expect("task should not panic")
            .expect("gate.check should return Ok");

        assert!(matches!(outcome, GateOutcome::Cancel));
    }
```

Note: this test references `gate.check(req, &token_for_task)` — the new two-arg signature. It will fail to compile until Task 6 lands.

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo nextest run -p approval cancel_during_channel_request_returns_cancel_outcome`
Expected: FAIL with "expected 1 argument, got 2" (or equivalent compile error).

- [ ] **Step 3: Commit the failing test**

```bash
git add crates/approval/src/gate.rs
git commit -m "test(approval): failing test for ApprovalGate cancel-leak"
```

---

## Task 6: Make `ApprovalGate::check` cancel-aware

**Files:**
- Modify: `crates/approval/src/gate.rs:68-143`

- [ ] **Step 1: Change the signature**

Replace the line:

```rust
    pub async fn check(&self, mut req: ApprovalRequest) -> Result<GateOutcome> {
```

with:

```rust
    pub async fn check(
        &self,
        mut req: ApprovalRequest,
        cancel_token: &tokio_util::sync::CancellationToken,
    ) -> Result<GateOutcome> {
```

- [ ] **Step 2: Race the channel.request future against cancellation**

Find:

```rust
        // Prompt the channel.
        let decision = self.channel.request(req.clone()).await;
```

Replace with:

```rust
        // Prompt the channel — race against cancellation so a hung approval
        // modal cannot pin the agent loop forever.
        let decision = tokio::select! {
            biased;
            _ = cancel_token.cancelled() => {
                tracing::info!(
                    tool = %req.tool_name,
                    "approval: cancellation observed while awaiting channel decision"
                );
                return Ok(GateOutcome::Cancel);
            }
            d = self.channel.request(req.clone()) => d,
        };
```

(The `Channel::request` future is dropped on the cancel branch, which is the only safety property we need.)

- [ ] **Step 3: Update existing in-tree call sites of `gate.check`**

Find every production call site:

Run: `grep -rn "\.check(" crates/agent/src/ crates/app-core/src/ crates/approval/src/ | grep -v "test\|#\[cfg" | grep -i "gate\|approval"`

The known call site is `crates/agent/src/execution/core.rs:798`:

```rust
                    match gate.check(req).await? {
```

Change to:

```rust
                    let approval_cancel_owned;
                    let approval_cancel_ref = match &params.cancel_token {
                        Some(t) => t,
                        None => {
                            approval_cancel_owned = tokio_util::sync::CancellationToken::new();
                            &approval_cancel_owned
                        }
                    };
                    match gate.check(req, approval_cancel_ref).await? {
```

(If a later refactor in Task 3 already introduced a `cancel_token_ref` binding in this same function, reuse it instead of re-deriving — keep the code DRY within `run_cycle`.)

- [ ] **Step 4: Update existing test call sites in the same file**

In `crates/approval/src/gate.rs`, update each `gate.check(req).await` in the existing tests (`safe_class_auto_allows_without_prompt`, `destructive_session_grant_persists_for_session`, `decline_returns_deny`, `cancel_propagates`, `remote_channel_auto_allows_safe_when_capabilities_omit_it`) to pass a fresh never-cancelled token:

```rust
        let token = tokio_util::sync::CancellationToken::new();
        let out = gate.check(req, &token).await.unwrap();
```

For the `destructive_session_grant_persists_for_session` test that calls `gate.check` twice, declare one token and pass `&token` to both calls.

- [ ] **Step 5: Verify `tokio_util` is already a dep of `approval`**

Run: `grep -n "tokio-util\|tokio_util" crates/approval/Cargo.toml`
Expected: a `tokio-util = ...` line. If absent, add `tokio-util = { workspace = true, features = ["sync"] }` under `[dependencies]`.

- [ ] **Step 6: Build**

Run: `cargo build -p approval -p agent`
Expected: success.

- [ ] **Step 7: Run the test from Task 5**

Run: `cargo nextest run -p approval cancel_during_channel_request_returns_cancel_outcome`
Expected: PASS.

- [ ] **Step 8: Run all approval and agent tests**

Run: `cargo nextest run -p approval -p agent`
Expected: all green.

- [ ] **Step 9: Commit**

```bash
git add crates/approval/src/gate.rs crates/agent/src/execution/core.rs crates/approval/Cargo.toml
git commit -m "feat(approval): race channel.request against cancel_token

ApprovalGate::check now takes &CancellationToken and uses tokio::select!
to short-circuit a hung channel future. Closes the cancel-leak that
mirrored opencode's permission-gate bug."
```

---

## Task 7: Wire `AgentEvent::Cancelled` through the coding turn handler

**Files:**
- Modify: `crates/app-core/src/coding/turn_handler.rs`

**Why:** The new event variant is fanned out by the agent layer; coding mode's translator needs an arm so it propagates to the FE as a `ThreadEvent` and persists the partial assistant message to SQLite.

- [ ] **Step 1: Find the existing `AgentEvent` match in turn_handler**

Run: `grep -n "AgentEvent::" crates/app-core/src/coding/turn_handler.rs | head -20`
Expected: a match arm block translating each variant. Read 5 lines around the first hit to see the pattern.

- [ ] **Step 2: Add a `Cancelled` arm**

Add (preserving existing style):

```rust
            AgentEvent::Cancelled { partial_content, partial_reasoning } => {
                // Persist whatever was streamed before the cancel as a
                // partial assistant message so the user can see what the
                // model produced. SessionMode::Coding writes per-iteration.
                if !partial_content.is_empty() || !partial_reasoning.is_empty() {
                    self.persist_partial_assistant(
                        &partial_content,
                        &partial_reasoning,
                    )
                    .await?;
                }
                self.publish_thread_event(ThreadEvent::TurnCancelled {
                    partial_content,
                    partial_reasoning,
                })
                .await;
            }
```

If `ThreadEvent::TurnCancelled` does not exist yet, add it to the relevant enum (likely in `crates/app-core/src/coding/events.rs` or similar — search with `grep -rn "enum ThreadEvent" crates/app-core/src/`). The variant should mirror `AgentEvent::Cancelled`. If `persist_partial_assistant` does not exist, write a small helper that inserts a row into the existing coding messages table with `role = "assistant"`, `cancelled = true`, and the partial body.

> If the persistence path is non-trivial, scope it to a follow-up task and have this arm only emit the `ThreadEvent` for now — note the deferral in the commit message.

- [ ] **Step 3: Build the workspace**

Run: `cargo build --workspace`
Expected: success.

- [ ] **Step 4: Run app-core and agent tests**

Run: `cargo nextest run -p app-core -p agent`
Expected: all green.

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/coding/turn_handler.rs crates/app-core/src/coding/events.rs
git commit -m "feat(coding): handle AgentEvent::Cancelled in turn handler

Persist partial assistant content + emit ThreadEvent::TurnCancelled
so the FE renders cancellation immediately with whatever streamed."
```

---

## Task 8: Frontend — render cancelled state with partial content

**Files:**
- Modify: `desktop-ui/src/features/coding/hooks/useThreadEvents.ts`
- Modify: `desktop-ui/src/features/coding/components/ThreadItemList.tsx` (or the relevant assistant-message renderer)
- Modify: `desktop-ui/src/api/bindings.ts` (regenerated)

- [ ] **Step 1: Regenerate Tauri bindings**

Run: `cargo tauri dev` once and let it boot, then quit. Or, equivalent: trigger the specta build script. The new `ThreadEvent::TurnCancelled` variant should appear in `desktop-ui/src/api/bindings.ts`.

Verify: `grep -n "TurnCancelled" desktop-ui/src/api/bindings.ts`
Expected: at least one hit.

- [ ] **Step 2: Add a reducer arm for the cancelled event**

In `desktop-ui/src/features/coding/hooks/useThreadEvents.ts`, find the existing reducer switch on event kind (search for `case "ContentChunk"` or similar). Add:

```ts
    case "TurnCancelled": {
      const last = items[items.length - 1];
      if (last && last.role === "assistant") {
        last.cancelled = true;
        if (event.partial_content && !last.content) {
          last.content = event.partial_content;
        }
        if (event.partial_reasoning && !last.reasoning) {
          last.reasoning = event.partial_reasoning;
        }
      }
      return { ...state, items: [...items], turnState: "idle" };
    }
```

(Field names like `cancelled`, `content`, `reasoning` should match whatever the existing item type uses — check the type def at the top of the file. If `cancelled` doesn't exist, add it to the type.)

- [ ] **Step 3: Render a cancelled badge on assistant messages**

In the assistant message renderer (find with `grep -rn 'role === "assistant"' desktop-ui/src/features/coding/`), add a small badge when `item.cancelled === true`:

```tsx
{item.cancelled && (
  <span className="thread-item__cancelled-badge" title="Cancelled by user">
    Cancelled
  </span>
)}
```

Add a matching style block in the relevant CSS file (e.g. `desktop-ui/src/styles/coding.css`), using existing token colors; do not hardcode. Reference: `var(--text-secondary)`, `var(--fs-xs)`.

- [ ] **Step 4: Run frontend lint + typecheck**

Run: `cd desktop-ui && bun run lint && bun run typecheck`
Expected: zero errors.

- [ ] **Step 5: Manually verify in browser-dev mode**

In two terminals:

```
cd desktop-ui && bun run dev
cargo tauri dev
```

Then in the desktop window: open a coding thread, send a long prompt (e.g. "write a 2000-word essay on rust async"), and hit `Cmd+.` mid-stream. Expected: streaming halts within ~50ms, the assistant bubble shows whatever streamed plus a "Cancelled" badge.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/api/bindings.ts desktop-ui/src/features/coding/ desktop-ui/src/styles/
git commit -m "feat(ui): render TurnCancelled with partial assistant content"
```

---

## Task 9: Verify nothing else regressed + zero clippy warnings

- [ ] **Step 1: Full workspace build**

Run: `cargo build --workspace`
Expected: success.

- [ ] **Step 2: Full workspace tests**

Run: `cargo nextest run --workspace`
Expected: all green.

- [ ] **Step 3: Doctests**

Run: `cargo test --workspace --doc`
Expected: all green.

- [ ] **Step 4: Clippy**

Run: `cargo clippy --workspace --all-targets --all-features -- -D warnings`
Expected: zero warnings.

- [ ] **Step 5: Format**

Run: `cargo fmt --all --check`
Expected: zero diffs. If non-zero, run `cargo fmt --all` and stage the changes.

- [ ] **Step 6: Frontend build**

Run: `cd desktop-ui && bun run build`
Expected: success.

- [ ] **Step 7: KCA validation gate**

Run: `./scripts/run_kca_validation.sh`
Expected: all gates pass. If a gate fails for an unrelated reason (e.g. flaky perf test), document the failure in the PR description rather than papering over it.

- [ ] **Step 8: Final commit (if any formatting/binding changes remain) + push**

```bash
git status
# If there are formatting or binding diffs remaining:
git add -A
git commit -m "chore: format + regenerated bindings"
```

---

## Self-Review

**Spec coverage** (cross-checking the inline design from the brainstorming step):

| Design point | Implemented in |
|---|---|
| Mid-stream cancel observed at chunk boundary | Task 3 (select! in `call_provider_streaming`) |
| Drop stream → close HTTP connection | Task 3 (drop happens automatically on scope exit from select arm) |
| `finish_reason = "cancelled"` on cancel | Task 3 (set explicitly before break) |
| Partial content/reasoning preserved | Task 3 (accumulator vars are read into the returned `LlmResponse`) |
| `AgentEvent::Cancelled` emitted | Task 1 (variant added) + Task 3 (fan_out_event call) |
| `execute_loop` exits cleanly on cancel | Task 4 |
| `ApprovalChannel` cancel-leak fix | Task 5 (failing test) + Task 6 (implementation) |
| Explicit `&CancellationToken` arg | Task 6 (signature change) |
| Frontend renders cancelled state | Task 8 |
| Tests for both fixes | Task 2 (provider) + Task 5 (approval) |
| Zero trait-API breakage | Confirmed — `LlmProvider` and `ApprovalChannel` traits unchanged |

**Placeholder scan:** No "TBD"/"TODO" in step bodies. Task 7 has a fallback path explicitly described (defer persistence if non-trivial); not a placeholder.

**Type consistency:** `cancel_token: &CancellationToken` consistent across Tasks 3 and 6. `AgentEvent::Cancelled { partial_content, partial_reasoning }` consistent across Tasks 1, 3, 7, 8. `ThreadEvent::TurnCancelled` consistent across Tasks 7, 8. `LoopFinishReason::Cancelled` already exists — verified in Task 4.

**Risk note:** The `cancel_token_owned` / `cancel_token_ref` pattern in Task 3 / Task 6 introduces local bindings whose lifetime must outlive the `match` arm. Reviewers should confirm the bindings are at function scope, not block scope. The plan places them at the right scope.
