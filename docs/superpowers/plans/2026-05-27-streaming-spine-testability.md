# Streaming Spine Testability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the chat/voice streaming spine testable by (1) giving the agent's `MockProvider` a scripted-stream mode that exercises `PartialToolCall` fragment reassembly, and (2) decomposing the 951-line `relay_chat_stream` into a pure `ChatEventTranslator`, a `TurnFinalizer`, and a thin `ChatRelay` shell.

**Architecture:** Behaviour-preserving. The translation is a pure state machine — `handle(event) -> Vec<UiEmission>` accumulating a `RelayState`; terminal events return a `TurnOutcome` instead of doing I/O. The shell (`ChatRelay`) keeps the lifecycle (`StreamGuard`, two-channel fan-in, heartbeat, `select!`), emits what the translator returns, and hands terminal outcomes to `TurnFinalizer` (the only DB/bus/journey toucher). The pure-arm move is mechanical: redefine the in-scope `emit!` macro to push onto a local `out` vec, and the 22 pure arms move verbatim.

**Tech Stack:** Rust, `tokio`, `async-trait`, `futures-util` (stream construction), `serde_json`, `cargo-nextest`. Tests use the agent crate's existing `MockProvider` patterns and `StoragePool::connect_in_memory()`.

**Source of truth for the existing code:** `crates/app-core/src/handlers/chat/streaming.rs:424-1372` (`relay_chat_stream`), `crates/agent/src/execution/core.rs:286-518` (`call_provider_streaming` + `PartialToolCall`), `crates/agent/src/test_utils.rs` (`MockProvider`).

---

## File Structure

- **Modify** `crates/agent/src/test_utils.rs` — add `StreamScript` builder, a `streams` field + `MockProvider::with_streams`, and a `chat_stream` override that replays scripted chunks (falling back to wrapping `chat()`).
- **Modify** `crates/agent/src/execution/core.rs` — add ONE test in the existing `#[cfg(test)] mod tests` (line ~1071) driving the streaming branch through `MockProvider::with_streams`.
- **Modify** `crates/agent/Cargo.toml` — ensure `futures-util` is a normal (non-dev) dependency, because `test_utils` is a `pub mod` (not `#[cfg(test)]`).
- **Create** `crates/app-core/src/handlers/chat/event_translator.rs` — `ChatEventTranslator`, `RelayState`, `UiEmission`, `TurnOutcome`, and all 25 event arms (22 moved verbatim + 3 terminal rewritten). Owns the in-crate `#[cfg(test)]` tests.
- **Create** `crates/app-core/src/handlers/chat/turn_finalizer.rs` — `TurnFinalizer` performing the `Done` persist + `ChatTurnCompleted` publish + journey milestone. Owns its `#[cfg(test)]` tests.
- **Create** `crates/app-core/src/handlers/chat/relay.rs` — `ChatRelay` (the lifecycle shell extracted from `relay_chat_stream`).
- **Modify** `crates/app-core/src/handlers/chat/mod.rs` — declare the three new modules.
- **Modify** `crates/app-core/src/handlers/chat/streaming.rs` — `spawn_chat_relay` constructs and runs `ChatRelay`; delete the old `relay_chat_stream` body once `ChatRelay` is wired.

### Key existing types (reference; do not redefine)

- `providers::LlmStreamChunk { content: Option<String>, tool_call_delta: Option<ToolCallDelta>, is_final: bool, finish_reason: Option<String>, reasoning_content: Option<String>, usage: Option<Usage> }`
- `providers::ToolCallDelta { index: usize, id: Option<String>, name: Option<String>, arguments: Option<String> }`
- `providers::LlmStream = Pin<Box<dyn Stream<Item = common::Result<LlmStreamChunk>> + Send>>`
- `providers::{LlmResponse, Usage, ProviderCapabilities, ProviderHealth, DEFAULT_CONTEXT_WINDOW}`
- `LlmProvider::chat_stream(&self, messages: &[Message], tools: Option<&[Value]>, params: &ChatParams, cache_breakpoints: &[CacheBreakpoint]) -> Result<LlmStream>` (default impl wraps `chat()`)
- `ExecutionCore::new(provider, registry)`, `ExecutionParams::new("mock", 128_000)`, `CycleOutcome::{FinalResponse { content }, ToolsExecuted { results }}` where `results[i].{tool_name, success, result}`.
- `agent::events::AgentEvent` (`#[non_exhaustive]`, `serde(tag="type")`) — variants used below.
- `desktop_shared::events::{MessageSegment, TransparencyData, ...}` (the `events::` alias inside `streaming.rs`).
- `common::EntityCard`.
- `super::thread_event_v2_translator::agent_event_to_thread_event(event: AgentEvent, session_key: String, generation: u64) -> Option<ThreadEvent>`.
- Repo + side-effect APIs used by the finalizer: `repos.sessions.update_assistant_metadata_by_id(message_id, None, Some(&meta))`, `repos.sessions.update_last_assistant_metadata(sk, None, Some(&meta))`, `bus::DomainEvent::ChatTurnCompleted { session_key, user_message }`, `crate::journey::{JourneyTracker, Milestone::FirstChatResponse}`.

---

## Task 1: `MockProvider` scripted-stream mode (Candidate 4)

**Files:**
- Modify: `crates/agent/Cargo.toml`
- Modify: `crates/agent/src/test_utils.rs`
- Test: `crates/agent/src/execution/core.rs` (existing `mod tests`, ~line 1071)

- [ ] **Step 1: Ensure `futures-util` is a normal dependency**

`test_utils` is `pub mod` (declared at `crates/agent/src/lib.rs:35`), so any crate it uses must be a non-dev dependency. Open `crates/agent/Cargo.toml`. Under `[dependencies]`, confirm a `futures-util` line exists. If it is only under `[dev-dependencies]` (or missing), add it to `[dependencies]` mirroring how `providers` declares it:

```toml
futures-util = { workspace = true }
```

- [ ] **Step 2: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `crates/agent/src/execution/core.rs` (after `test_cycle_tool_execution`):

```rust
#[tokio::test]
async fn test_streaming_reconstructs_fragmented_tool_call() {
    use crate::test_utils::StreamScript;
    use providers::Usage;

    // A tool call whose JSON arguments arrive in TWO fragments at the same
    // index (id+name only on the first delta) — exactly how Anthropic/OpenAI
    // stream them. Only call_provider_streaming + PartialToolCall can reassemble
    // this; the response-queue path (chat()) never exercises it.
    let cycle1 = StreamScript::new()
        .text("Let me check that")
        .tool_call("call_1", "echo", &[r#"{"msg":"#, r#""hi"}"#])
        .usage(Usage {
            prompt_tokens: 10,
            completion_tokens: 4,
            total_tokens: 14,
            cache_read_tokens: 0,
            cache_write_tokens: 0,
        })
        .finish("tool_calls");

    let provider = Arc::new(MockProvider::with_streams(vec![cycle1]));
    let registry = make_registry_with(EchoTool);
    let core = ExecutionCore::new(provider, registry);

    let mut messages = vec![Message::user("hi")];
    let params = ExecutionParams::new("mock", 128_000);
    let tools = vec![];

    // event_tx = Some(..) FORCES the streaming branch.
    let (tx, mut rx) = tokio::sync::mpsc::channel::<crate::events::AgentEvent>(64);

    let (outcome, usage) = core
        .run_cycle(&mut messages, &tools, &params, &routing_ctx(), Some(&tx), None, &[])
        .await
        .unwrap();

    // Fragments concatenated into valid JSON -> tool executed with {"msg":"hi"}.
    match outcome {
        CycleOutcome::ToolsExecuted { results } => {
            assert_eq!(results.len(), 1);
            assert_eq!(results[0].tool_name, "echo");
            assert!(results[0].success);
            assert!(results[0].result.contains("\"msg\""));
            assert!(results[0].result.contains("hi"));
        }
        other => panic!("expected ToolsExecuted, got {:?}", other),
    }

    // Real usage flowed from the stream, not character estimation.
    assert_eq!(usage.prompt_tokens, 10);
    assert_eq!(usage.completion_tokens, 4);

    // The streaming branch fanned out a ContentChunk for the text delta.
    drop(tx);
    let mut saw_content = false;
    while let Ok(ev) = rx.try_recv() {
        if matches!(ev, crate::events::AgentEvent::ContentChunk { .. }) {
            saw_content = true;
        }
    }
    assert!(saw_content, "expected a ContentChunk fanned out during streaming");
}
```

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo nextest run -p agent -E 'test(test_streaming_reconstructs_fragmented_tool_call)'`
Expected: FAIL — compile error, `StreamScript` not found and `MockProvider` has no `with_streams`.

- [ ] **Step 4: Add the `StreamScript` builder**

At the top of `crates/agent/src/test_utils.rs`, extend the `use providers::{...}` line to also import `LlmStream, LlmStreamChunk, ToolCallDelta`. Then add, below the `MockProvider` impl:

```rust
/// Builds a scripted `Vec<LlmStreamChunk>` for `MockProvider::with_streams`.
///
/// `tool_call` splits a call's JSON arguments across multiple deltas at the
/// same index (id+name only on the first), which is what forces
/// `PartialToolCall` in the execution core to concatenate fragments.
pub struct StreamScript {
    chunks: Vec<LlmStreamChunk>,
    next_tool_index: usize,
}

impl Default for StreamScript {
    fn default() -> Self {
        Self::new()
    }
}

impl StreamScript {
    pub fn new() -> Self {
        Self { chunks: Vec::new(), next_tool_index: 0 }
    }

    /// Append a visible-content (text) delta.
    pub fn text(mut self, s: &str) -> Self {
        self.chunks.push(LlmStreamChunk {
            content: Some(s.to_string()),
            tool_call_delta: None,
            is_final: false,
            finish_reason: None,
            reasoning_content: None,
            usage: None,
        });
        self
    }

    /// Append a reasoning (extended-thinking) delta.
    pub fn reasoning(mut self, s: &str) -> Self {
        self.chunks.push(LlmStreamChunk {
            content: None,
            tool_call_delta: None,
            is_final: false,
            finish_reason: None,
            reasoning_content: Some(s.to_string()),
            usage: None,
        });
        self
    }

    /// Append a tool call whose `arguments` arrive fragmented across same-index
    /// deltas. `id`/`name` are sent only on the first fragment.
    pub fn tool_call(mut self, id: &str, name: &str, arg_fragments: &[&str]) -> Self {
        let index = self.next_tool_index;
        self.next_tool_index += 1;

        if arg_fragments.is_empty() {
            self.chunks.push(LlmStreamChunk {
                content: None,
                tool_call_delta: Some(ToolCallDelta {
                    index,
                    id: Some(id.to_string()),
                    name: Some(name.to_string()),
                    arguments: Some(String::new()),
                }),
                is_final: false,
                finish_reason: None,
                reasoning_content: None,
                usage: None,
            });
            return self;
        }

        for (i, frag) in arg_fragments.iter().enumerate() {
            let first = i == 0;
            self.chunks.push(LlmStreamChunk {
                content: None,
                tool_call_delta: Some(ToolCallDelta {
                    index,
                    id: if first { Some(id.to_string()) } else { None },
                    name: if first { Some(name.to_string()) } else { None },
                    arguments: Some(frag.to_string()),
                }),
                is_final: false,
                finish_reason: None,
                reasoning_content: None,
                usage: None,
            });
        }
        self
    }

    /// Append a usage-only chunk (mirrors message_start / message_delta).
    pub fn usage(mut self, usage: Usage) -> Self {
        self.chunks.push(LlmStreamChunk {
            content: None,
            tool_call_delta: None,
            is_final: false,
            finish_reason: None,
            reasoning_content: None,
            usage: Some(usage),
        });
        self
    }

    /// Terminal chunk carrying the finish reason. Consumes the builder.
    pub fn finish(mut self, reason: &str) -> Vec<LlmStreamChunk> {
        self.chunks.push(LlmStreamChunk {
            content: None,
            tool_call_delta: None,
            is_final: true,
            finish_reason: Some(reason.to_string()),
            reasoning_content: None,
            usage: None,
        });
        self.chunks
    }
}
```

- [ ] **Step 5: Add the `streams` field, `with_streams`, and the `chat_stream` override**

In `MockProvider`'s struct definition add a field:

```rust
    streams: Mutex<Vec<Vec<LlmStreamChunk>>>,
```

Add `streams: Mutex::new(Vec::new()),` to EACH existing constructor (`with_response`, `with_error`, `with_responses` — `with_text` and `with_tool_call` delegate to `with_response`, so they need no change). Then add the new constructor:

```rust
    /// Create a mock that replays a scripted stream per `chat_stream()` call.
    /// Each call pops the next scripted `Vec<LlmStreamChunk>` from the queue.
    pub fn with_streams(streams: Vec<Vec<LlmStreamChunk>>) -> Self {
        Self {
            responses: Mutex::new(Vec::new()),
            streams: Mutex::new(streams),
            context_window: providers::DEFAULT_CONTEXT_WINDOW,
            capabilities: ProviderCapabilities::default(),
            health: ProviderHealth::Healthy,
        }
    }
```

In the `impl LlmProvider for MockProvider` block, add the override (alongside `chat`):

```rust
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> common::Result<LlmStream> {
        let scripted = {
            let mut s = self.streams.lock().unwrap();
            if s.is_empty() { None } else { Some(s.remove(0)) }
        };

        let chunks: Vec<common::Result<LlmStreamChunk>> = match scripted {
            // Replay the scripted stream verbatim.
            Some(chunks) => chunks.into_iter().map(Ok).collect(),
            // No script queued: wrap chat() exactly like the trait default,
            // so non-streaming mocks keep working when driven via chat_stream.
            None => {
                let response = self.chat(messages, tools, params, cache_breakpoints).await?;
                let mut out = Vec::with_capacity(response.tool_calls.len() + 1);
                for (i, tc) in response.tool_calls.iter().enumerate() {
                    out.push(Ok(LlmStreamChunk {
                        content: None,
                        tool_call_delta: Some(ToolCallDelta {
                            index: i,
                            id: Some(tc.id.clone()),
                            name: Some(tc.name.clone()),
                            arguments: Some(serde_json::to_string(&tc.arguments).unwrap_or_default()),
                        }),
                        is_final: false,
                        finish_reason: None,
                        reasoning_content: None,
                        usage: None,
                    }));
                }
                out.push(Ok(LlmStreamChunk {
                    content: response.content,
                    tool_call_delta: None,
                    is_final: true,
                    finish_reason: Some(response.finish_reason),
                    reasoning_content: response.reasoning_content,
                    usage: Some(response.usage),
                }));
                out
            }
        };

        Ok(Box::pin(futures_util::stream::iter(chunks)))
    }
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `cargo nextest run -p agent -E 'test(test_streaming_reconstructs_fragmented_tool_call)'`
Expected: PASS.

- [ ] **Step 7: Confirm no regression in existing agent tests**

Run: `cargo nextest run -p agent`
Expected: PASS (existing `MockProvider` users unaffected — empty `streams` falls back to wrapping `chat()`).

- [ ] **Step 8: Commit**

```bash
cargo fmt --all
git add crates/agent/src/test_utils.rs crates/agent/src/execution/core.rs crates/agent/Cargo.toml
git commit -m "feat(agent): add MockProvider scripted-stream mode for PartialToolCall tests

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: `ChatEventTranslator` (pure) (Candidate 1)

**Files:**
- Create: `crates/app-core/src/handlers/chat/event_translator.rs`
- Modify: `crates/app-core/src/handlers/chat/mod.rs`
- Reference (source of arms to move): `crates/app-core/src/handlers/chat/streaming.rs:484-1347`

- [ ] **Step 1: Declare the module**

In `crates/app-core/src/handlers/chat/mod.rs`, add:

```rust
pub mod event_translator;
```

- [ ] **Step 2: Write the failing test**

Create `crates/app-core/src/handlers/chat/event_translator.rs` with ONLY the test module first (the types/impl come next), so the test drives the API:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use agent::events::AgentEvent;

    #[test]
    fn content_chunk_accumulates_text_and_emits() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        let emits = tr.handle(AgentEvent::ContentChunk { data: "hello ".to_string() });
        // v2 thread:event (if mapped) + v1 AGENT_CONTENT_CHUNK both pushed.
        assert!(emits.iter().any(|e| e.event == "agent:content_chunk"
            || e.event.contains("content")), "expected a content emission, got {:?}",
            emits.iter().map(|e| e.event).collect::<Vec<_>>());
        // Text accumulates in state, not yet flushed into a segment.
        assert_eq!(tr.state().current_text, "hello ");
        assert!(tr.state().segments.is_empty());
        assert!(tr.take_terminal().is_none());
    }

    #[test]
    fn tool_end_pushes_a_tool_segment() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        tr.handle(AgentEvent::ToolStart {
            name: "tasks".to_string(),
            args: serde_json::json!({"action": "list"}),
            agent: None,
            call_id: None,
        });
        tr.handle(AgentEvent::ToolEnd {
            name: "tasks".to_string(),
            success: true,
            duration_ms: 12,
            result: Some("ok".to_string()),
            agent: None,
            call_id: None,
        });
        assert_eq!(tr.state().segments.len(), 1);
        assert_eq!(tr.state().tool_names, vec!["tasks".to_string()]);
    }

    #[test]
    fn done_yields_terminal_outcome_without_io() {
        let mut tr = ChatEventTranslator::new("sess-1".to_string(), 0);
        tr.handle(AgentEvent::ContentChunk { data: "answer".to_string() });
        let _ = tr.handle(AgentEvent::Done {
            content: "answer".to_string(),
            message_id: Some("m1".to_string()),
        });
        match tr.take_terminal() {
            Some(TurnOutcome::Done { content, message_id, segments, .. }) => {
                assert_eq!(content, "answer");
                assert_eq!(message_id.as_deref(), Some("m1"));
                // The flushed text became a segment carried out in the outcome.
                assert!(!segments.is_empty());
            }
            other => panic!("expected Done outcome, got {:?}", other),
        }
    }
}
```

> NOTE on the content-event name: confirm the exact constant by checking `streaming.rs` (the `AGENT_CONTENT_CHUNK` constant and its string value). Replace the `e.event == "agent:content_chunk"` literal in the first test with the actual constant value if it differs. This is a one-line lookup, not a guess to leave in.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo nextest run -p app-core -E 'test(event_translator)'`
Expected: FAIL — compile error, `ChatEventTranslator`/`TurnOutcome` not found.

- [ ] **Step 4: Add the types and the translator skeleton**

At the TOP of `crates/app-core/src/handlers/chat/event_translator.rs` (above the test module), copy the `use` block from `streaming.rs` (the imports for `events::`, the `AGENT_*` payload structs + constants, `AgentEvent`, `EntityCard`, `TransparencyData`, etc.), then add:

```rust
use std::collections::{HashMap, VecDeque};

/// One UI emission produced by the translator. The relay shell forwards each
/// to `AppEventEmitter::emit_event`.
pub struct UiEmission {
    pub event: &'static str,
    pub payload: serde_json::Value,
}

/// Terminal result of a chat turn. The relay shell hands this to `TurnFinalizer`
/// (Done persists; Error/Cancelled only emit + clean up).
#[derive(Debug)]
pub enum TurnOutcome {
    Done {
        content: String,
        message_id: Option<String>,
        segments: Vec<events::MessageSegment>,
        transparency: TransparencyData,
    },
    Error { message: String },
    Cancelled { partial_content: Option<String>, partial_reasoning: Option<String> },
}

/// Accumulated turn state. Public for assertions; mutated only by the translator.
#[derive(Default)]
pub struct RelayState {
    pub segments: Vec<events::MessageSegment>,
    pub transparency: TransparencyData,
    pub current_text: String,
    pub tool_names: Vec<String>,
    pub entity_cards: Vec<common::EntityCard>,
    pub tool_token_sum: u32,
    pub(crate) pending_actions: HashMap<String, VecDeque<String>>,
    pub(crate) pending_approvals: HashMap<String, (String, Option<String>)>,
}

/// Pure translation of the agent's `AgentEvent` stream into UI emissions.
/// No I/O, no emitter, no clock. The relay shell drives it.
pub struct ChatEventTranslator {
    session_key: String,
    generation: u64,
    state: RelayState,
    terminal: Option<TurnOutcome>,
}

impl ChatEventTranslator {
    pub fn new(session_key: String, generation: u64) -> Self {
        Self { session_key, generation, state: RelayState::default(), terminal: None }
    }

    pub fn state(&self) -> &RelayState {
        &self.state
    }

    pub fn take_terminal(&mut self) -> Option<TurnOutcome> {
        self.terminal.take()
    }

    fn flush_text(&mut self) {
        if !self.state.current_text.is_empty() {
            self.state.segments.push(events::MessageSegment::Text {
                content: std::mem::take(&mut self.state.current_text),
            });
        }
    }

    /// Translate one event: mutate state, return UI emissions. Terminal events
    /// also stash a `TurnOutcome` (retrieve via `take_terminal`).
    pub fn handle(&mut self, event: AgentEvent) -> Vec<UiEmission> {
        let mut out: Vec<UiEmission> = Vec::new();
        let sk = self.session_key.clone();

        // v2 thread:event (pure) — preserved from the old loop head.
        if let Some(te) = super::thread_event_v2_translator::agent_event_to_thread_event(
            event.clone(),
            sk.clone(),
            self.generation,
        ) {
            if let Ok(val) = serde_json::to_value(&te) {
                out.push(UiEmission { event: "thread:event", payload: val });
            }
        }

        // Redefined `emit!`: push a UiEmission instead of calling an emitter.
        // This lets the v1 arms below move VERBATIM from `relay_chat_stream`.
        macro_rules! emit {
            ($event:expr, $payload:expr) => {
                if let Ok(val) = serde_json::to_value(&$payload) {
                    out.push(UiEmission { event: $event, payload: val });
                }
            };
        }

        match event {
            AgentEvent::ContentChunk { data } => {
                self.state.current_text.push_str(&data);
                emit!(AGENT_CONTENT_CHUNK, ContentChunkPayload { session_key: sk.clone(), data });
            }
            // ── remaining arms moved here in Step 5 ──
            _ => {}
        }

        out
    }
}
```

> The single `ContentChunk` arm above + the `_ => {}` placeholder make Step 2's first test pass. Steps 5–6 replace `_ => {}` with the real arms.

- [ ] **Step 5: Move the pure arms verbatim**

Replace the `// ── remaining arms ──` / `_ => {}` region with the arms from `streaming.rs:611-1347`, applying this MECHANICAL transform (no behaviour change):

1. Copy every non-terminal arm verbatim (`ToolStart`, `ToolEnd`, `EntityCreated`, `ExecutionStarted`, `PipelineStarted`, `ContextAssembled`, `RetrievalEnhanced`, `IterationStart`, `ConfidenceAssessed`, `UsageReport`, `MemoryAccess`, `SkillLoaded`, `LearningEvent`, `AgentSelected`, `SubagentSpawned`, `DelegationStarted`, `DelegationCompleted`, `McpServerStatus`, `McpStartupComplete`, `PlanningStarted`, `PlanGenerated`, `PlanStepCompleted`, `BudgetWarning`, `MemoryPromoted`, `AutoTunerReport`, `AutoTunerPromotion`, `AutoTunerRollback`, `ContextCompressed`, the `agent:recall_injected` / `agent:dead_end_warning_surfaced` / `agent:plan_mode_changed` arms, the dropped-telemetry group, and the `_ =>` warn).
2. Substitutions inside the moved arms:
   - `flush_text(&mut current_text, &mut segments)` → `self.flush_text()`
   - bare locals `current_text` / `segments` / `transparency` / `tool_names` / `entity_cards` / `pending_actions` / `tool_token_sum` → `self.state.<field>`
   - bare `emitter.emit_event(name, payload)` (the `ENTITY_UPDATED` pushes in `ToolEnd`/`EntityCreated`, and the `json!`-built `agent:recall_injected` etc.) → `out.push(UiEmission { event: name, payload })`
   - `emit!(...)` calls stay **unchanged** — the redefined macro handles them.
3. Do NOT move `Done` / `Error` / `Cancelled` here — they are Step 6.

- [ ] **Step 6: Rewrite the three terminal arms**

Add these arms (replace the old persist/publish/cleanup bodies — those move to `TurnFinalizer`/`ChatRelay` in Tasks 3–4):

```rust
            AgentEvent::Done { content, message_id } => {
                self.flush_text();
                if self.state.tool_token_sum > 0 {
                    self.state.transparency.tool_tokens_total = Some(self.state.tool_token_sum);
                }
                emit!(AGENT_DONE, DonePayload { session_key: sk.clone(), content: content.clone() });
                emit!(CHAT_MESSAGE_ADDED, ChatMessagePayload {
                    session_key: sk.clone(),
                    source: "chat".to_string(),
                });
                self.terminal = Some(TurnOutcome::Done {
                    content,
                    message_id,
                    segments: std::mem::take(&mut self.state.segments),
                    transparency: std::mem::take(&mut self.state.transparency),
                });
            }
            AgentEvent::Error { message } => {
                emit!(AGENT_ERROR, AgentErrorPayload { session_key: sk.clone(), message: message.clone() });
                emit!(CHAT_MESSAGE_ADDED, ChatMessagePayload {
                    session_key: sk.clone(),
                    source: "agent_error".to_string(),
                });
                self.terminal = Some(TurnOutcome::Error { message });
            }
            AgentEvent::Cancelled { partial_content, partial_reasoning } => {
                emit!(AGENT_CANCELLED, CancelledPayload {
                    session_key: sk.clone(),
                    partial_content: partial_content.clone(),
                    partial_reasoning: partial_reasoning.clone(),
                });
                self.terminal = Some(TurnOutcome::Cancelled { partial_content, partial_reasoning });
            }
```

> Constant/payload names (`AGENT_DONE`, `DonePayload`, `CHAT_MESSAGE_ADDED`, `ChatMessagePayload`, `AGENT_ERROR`, `AgentErrorPayload`, `AGENT_CANCELLED`, `CancelledPayload`) are exactly those used in `streaming.rs:786-846`. Keep `tool_tokens_total` field name as in `streaming.rs:704`.

- [ ] **Step 7: Run the tests to verify they pass**

Run: `cargo nextest run -p app-core -E 'test(event_translator)'`
Expected: PASS (all three tests).

- [ ] **Step 8: Commit**

```bash
cargo fmt --all
git add crates/app-core/src/handlers/chat/event_translator.rs crates/app-core/src/handlers/chat/mod.rs
git commit -m "refactor(app-core): extract pure ChatEventTranslator from relay_chat_stream

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: `TurnFinalizer`

**Files:**
- Create: `crates/app-core/src/handlers/chat/turn_finalizer.rs`
- Modify: `crates/app-core/src/handlers/chat/mod.rs`
- Reference (source of behaviour): `crates/app-core/src/handlers/chat/streaming.rs:706-774`

- [ ] **Step 1: Declare the module**

In `crates/app-core/src/handlers/chat/mod.rs`, add `pub mod turn_finalizer;`.

- [ ] **Step 2: Write the failing test**

Create `crates/app-core/src/handlers/chat/turn_finalizer.rs` with the test module first:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn finalize_done_publishes_chat_turn_completed() {
        let bus = Arc::new(bus::DomainEventBus::new());
        let mut rx = bus.subscribe();

        let finalizer = TurnFinalizer {
            repos: None,
            domain_event_bus: Some(&bus),
            journey_tracker: None,
        };

        finalizer
            .finalize_done("sess-1", Some("hi there"), "answer", None, &[], &Default::default())
            .await;

        // ChatTurnCompleted reached the bus with the right session + user message.
        let evt = rx.try_recv().expect("expected a published domain event");
        match evt {
            bus::DomainEvent::ChatTurnCompleted { session_key, user_message } => {
                assert_eq!(session_key, "sess-1");
                assert_eq!(user_message.as_deref(), Some("hi there"));
            }
            other => panic!("expected ChatTurnCompleted, got {:?}", other),
        }
    }
}
```

> Confirm `bus::DomainEventBus::new()` and `subscribe()` exist (they are used in `streaming.rs:530`). If the constructor differs, mirror how `domain_event_bus` is built elsewhere in app-core tests.

- [ ] **Step 3: Run the test to verify it fails**

Run: `cargo nextest run -p app-core -E 'test(finalize_done_publishes)'`
Expected: FAIL — `TurnFinalizer` not found.

- [ ] **Step 4: Implement `TurnFinalizer`**

Above the test module, add (mirroring the side-effects at `streaming.rs:706-774`):

```rust
use std::sync::Arc;
use storage::Repos;
use crate::journey::{JourneyTracker, Milestone};
use desktop_shared::events::{MessageSegment, TransparencyData};

/// Performs the terminal side-effects of a Done turn: persist the
/// segments+transparency metadata, publish `ChatTurnCompleted`, and advance the
/// FirstChatResponse journey milestone. Error/Cancelled have no finalization.
pub struct TurnFinalizer<'a> {
    pub repos: Option<&'a Repos>,
    pub domain_event_bus: Option<&'a Arc<bus::DomainEventBus>>,
    pub journey_tracker: Option<&'a JourneyTracker>,
}

impl TurnFinalizer<'_> {
    pub async fn finalize_done(
        &self,
        session_key: &str,
        user_message: Option<&str>,
        _content: &str,
        message_id: Option<&str>,
        segments: &[MessageSegment],
        transparency: &TransparencyData,
    ) {
        // 1. Persist segments + transparency to the assistant message metadata.
        if let Some(repos) = self.repos {
            let mut meta = serde_json::Map::new();
            if !segments.is_empty() {
                meta.insert("segments".to_string(), serde_json::to_value(segments).unwrap_or_default());
            }
            meta.insert("transparency".to_string(), serde_json::to_value(transparency).unwrap_or_default());
            let meta_value = serde_json::Value::Object(meta);

            let persist_outcome = if let Some(mid) = message_id {
                repos.sessions.update_assistant_metadata_by_id(mid, None, Some(&meta_value)).await
            } else {
                repos.sessions.update_last_assistant_metadata(session_key, None, Some(&meta_value)).await
            };
            if let Err(e) = &persist_outcome {
                tracing::warn!("metadata persist sync failed for {session_key}: {e}");
            }
            if matches!(persist_outcome, Ok(false)) {
                let repos_clone = repos.clone();
                let sk_owned = session_key.to_string();
                let meta_clone = meta_value.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                    match repos_clone.sessions.update_last_assistant_metadata(&sk_owned, None, Some(&meta_clone)).await {
                        Ok(true) => {}
                        Ok(false) => tracing::warn!("metadata persist retry: no row {sk_owned}"),
                        Err(e) => tracing::warn!("metadata persist retry failed {sk_owned}: {e}"),
                    }
                });
            }
        }

        // 2. Publish ChatTurnCompleted AFTER the response is saved.
        if let Some(bus) = self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                session_key: session_key.to_string(),
                user_message: user_message.map(String::from),
            });
        }

        // 3. FirstChatResponse journey milestone.
        if let Some(tracker) = self.journey_tracker {
            if !tracker.is_complete(Milestone::FirstChatResponse).await {
                tracker.mark_complete(Milestone::FirstChatResponse).await;
            }
        }
    }
}
```

> `Repos` is `Clone` (it is `Repos::from_pool`-constructed and cloned in `streaming.rs:736`). Keep the detached retry exactly as the original.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo nextest run -p app-core -E 'test(finalize_done_publishes)'`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add crates/app-core/src/handlers/chat/turn_finalizer.rs crates/app-core/src/handlers/chat/mod.rs
git commit -m "refactor(app-core): extract TurnFinalizer for chat-turn side effects

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: `ChatRelay` shell + wire it in

**Files:**
- Create: `crates/app-core/src/handlers/chat/relay.rs`
- Modify: `crates/app-core/src/handlers/chat/mod.rs`
- Modify: `crates/app-core/src/handlers/chat/streaming.rs` (`spawn_chat_relay`; delete old `relay_chat_stream` body)

- [ ] **Step 1: Declare the module**

In `mod.rs`, add `pub mod relay;`.

- [ ] **Step 2: Move the shell into `ChatRelay::run`**

Create `crates/app-core/src/handlers/chat/relay.rs`. Move the body of `relay_chat_stream` (`streaming.rs:424-1372`) here as an `async fn run(self)` on a `ChatRelay` struct whose fields are the 13 former parameters. KEEP verbatim: the `StreamGuard` struct + guard construction, the two fan-in `tokio::spawn` tasks, the heartbeat interval, the `interaction_rx` arm (request_id + `AGENT_INTERACTION_REQUEST` emit + `pending_interactions` insert), and the trailing `auto_detect_context`.

REPLACE the inline `merged_rx` event-match (the old `601-1347` block) with a translator-driven loop. The terminal ordering MUST match the original (persist → eager cleanup → emit terminal):

```rust
let mut translator = super::event_translator::ChatEventTranslator::new(sk.clone(), generation);
// ... inside `event = merged_rx.recv() => { match event { Some(event) => { ... }`:
let emits = translator.handle(event);

if let Some(outcome) = translator.take_terminal() {
    // (a) persist/publish/journey FIRST (Done only) — matches old ordering.
    let finalizer = super::turn_finalizer::TurnFinalizer {
        repos: Some(&repos),
        domain_event_bus: domain_event_bus.as_ref(),
        journey_tracker: journey_tracker.as_ref(),
    };
    if let super::event_translator::TurnOutcome::Done {
        content: _, message_id, segments, transparency,
    } = &outcome
    {
        finalizer
            .finalize_done(sk, user_message.as_deref(), "", message_id.as_deref(), segments, transparency)
            .await;
    }
    // (b) eager active_streams cleanup BEFORE emitting terminal events (race fix).
    if let Some(entry) = active_streams.get(sk) {
        if entry.guard_id == guard_id {
            drop(entry);
            active_streams.remove(sk);
        }
    }
    // (c) emit terminal UI events.
    for e in emits {
        emitter.emit_event(e.event, e.payload);
    }
    break;
} else {
    for e in emits {
        emitter.emit_event(e.event, e.payload);
    }
}
```

> The `Done` arm's `content` is already inside the emitted `AGENT_DONE` payload (the translator pushed it), so `finalize_done`'s `_content` param is unused — pass `""`. After the loop, `auto_detect_context` reads `translator.state().tool_names` and `translator.state().entity_cards` instead of the old locals.

- [ ] **Step 3: Point `spawn_chat_relay` at `ChatRelay`**

In `streaming.rs`, change `spawn_chat_relay` to construct `ChatRelay { .. }` from `stream_info` + the same fields it passes today and `tokio::spawn(relay.run().in_current_span())`. Delete the old `relay_chat_stream` free function.

- [ ] **Step 4: Build the workspace**

Run: `cargo build -p app-core`
Expected: compiles. Fix any moved-import errors by copying the relevant `use` lines from the old `streaming.rs` into `relay.rs`.

- [ ] **Step 5: Run the full app-core + agent suites (behaviour preservation)**

Run: `cargo nextest run -p app-core -p agent`
Expected: PASS. (`relay_chat_stream` had no tests, so the safety net is the surrounding suite + the new unit tests.)

- [ ] **Step 6: Commit**

```bash
cargo fmt --all
git add crates/app-core/src/handlers/chat/relay.rs crates/app-core/src/handlers/chat/mod.rs crates/app-core/src/handlers/chat/streaming.rs
git commit -m "refactor(app-core): extract ChatRelay shell; relay_chat_stream now drives translator+finalizer

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Workspace verification

- [ ] **Step 1: Clippy (zero-warning policy)**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (the `desktop` crate's pre-existing exceptions aside). Fix any `clippy::too_many_arguments` on `ChatRelay::run` by adding `#[allow(clippy::too_many_arguments)]` ONLY if the original `relay_chat_stream` had it; otherwise prefer the `ChatRelay` struct fields (already the case).

- [ ] **Step 2: Format check**

Run: `cargo fmt --all --check`
Expected: clean.

- [ ] **Step 3: Full workspace test run**

Run: `cargo nextest run --workspace`
Expected: PASS.

- [ ] **Step 4: Final commit (if any fixups)**

```bash
git add -A
git commit -m "chore(app-core): clippy + fmt fixups for streaming-spine decomposition

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Self-Review

**Spec coverage:**
- Candidate 4 (MockProvider streaming, `PartialToolCall` reassembly) → Task 1. ✓
- Candidate 1 pure `ChatEventTranslator` (the chosen seam: returns emissions) → Task 2. ✓
- `TurnFinalizer` (terminal side-effects) → Task 3. ✓
- `ChatRelay` shell (full decomposition: fan-in + StreamGuard + select!) → Task 4. ✓
- Behaviour preservation (terminal ordering persist→cleanup→emit; v1+v2 both emitted; auto_detect_context) → Task 2 Step 6 + Task 4 Step 2. ✓

**Placeholder scan:** Two explicit one-line lookups remain (the exact `AGENT_CONTENT_CHUNK` string value in Task 2 Step 2; the `DomainEventBus::new()` constructor shape in Task 3 Step 2). Both are "confirm against existing code," not invented APIs — acceptable, but resolve them while implementing rather than guessing.

**Type consistency:** `ChatEventTranslator::{new, handle, state, take_terminal}`, `TurnOutcome::{Done, Error, Cancelled}`, `UiEmission { event, payload }`, `RelayState` fields, `TurnFinalizer::finalize_done`, and `StreamScript::{new, text, reasoning, tool_call, usage, finish}` + `MockProvider::with_streams` are used identically across all tasks.

**Honest scope note:** Tasks 2–3 produce real unit-test surfaces; Task 4 (`ChatRelay`) gains locality/navigability but its coverage stays integration-level (Task 4 Step 5 + Task 5 Step 3 are its safety net).
