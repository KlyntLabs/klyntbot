# Architecture Review & Production Readiness Report

**Date:** 2026-02-13
**Reviewer:** architecture-engineer
**Scope:** ask_user interactive clarification system
**Reference Design:** `docs/ASK_USER_ARCHITECTURE.md`

## Verification Results

| Check | Result |
|-------|--------|
| `cargo test --workspace` | 576+ tests, 0 failures, 1 ignored |
| `cargo clippy --workspace --all-targets --all-features` | 0 warnings |
| `cargo fmt --all --check` | NEEDS FIX — 5 files with cosmetic diffs |

### Formatting Fix Required

Run `cargo fmt --all` to fix cosmetic differences in:
- `crates/cli/src/wizard/ask_user_prompt.rs`
- `crates/tools/src/ask_user.rs`
- `crates/tools/src/todo.rs`
- `tests/ask_user_tests.rs`
- `tests/todo_chat_enrichment.rs`

---

## File-by-File Architecture Validation

| File | Status | Notes |
|------|--------|-------|
| `common/prompts.rs` | PASS | Types match architecture design exactly. 8 serde roundtrip tests. |
| `common/interaction.rs` | PASS | InteractionRenderer trait correct. |
| `common/lib.rs` | PASS | Re-exports updated, old types removed. |
| `tools/ask_user.rs` | PASS | Oneshot-per-request pattern correct. 19 unit tests. JSON schema with oneOf discriminator. Non-TTY fallback. Tool description includes parallel-call warning. |
| `tools/lib.rs` | PASS | InteractionBundle with oneshot::Sender. RoutingContext with `interaction_tx`. Two constructors. |
| `agent/agent_loop.rs` | PASS | StreamingChannels simplified to 1 field. StreamingHandle struct added. prompt_pending removed. Bridge task eliminated. AskUserTool registered. |
| `agent/events.rs` | PASS | PromptUser variant removed. 6 clean variants remain. |
| `agent/lib.rs` | PASS | StreamingHandle exported. |
| `agent/context.rs` | PASS | System prompt includes "Interactive Clarification" section covering all 6 identified gaps. |
| `cli/chat.rs` | PASS | StreamingHandle destructured correctly. `tokio::select!` dual-channel pattern. No old prompt code remains. |
| `tools/todo.rs` | PASS | Old enrichment prompt block removed. Low-confidence tasks now emit LLM-readable guidance text suggesting ask_user. |
| `cli/wizard/ask_user_prompt.rs` | PASS | 1,089-line tabbed UI. RAII RawModeGuard. All 4 answer types. Vim keys. Auto-advance. Non-TTY fallback. |
| `tests/ask_user_tests.rs` | PASS | 14 integration tests: happy paths, edge cases, validation, error handling. All use facade crate. |

---

## Architecture Design Adherence: 100%

Every decision from `docs/ASK_USER_ARCHITECTURE.md` is faithfully implemented:

1. **Oneshot-per-request pattern** — Each `ask_user` call creates its own `oneshot::channel()`. No shared state, no Mutex, no deadlock risk. Type-safe 1:1 correlation between request and response.

2. **InteractionBundle** — Carries `InteractionRequest` + `oneshot::Sender<FormResponse>`. Travels through `mpsc::Sender<InteractionBundle>` in RoutingContext.

3. **StreamingHandle** — Named struct replacing the previous 4-tuple return. Clean destructuring at call site.

4. **Agent loop zero prompt-specific code** — The agent loop has no awareness of ask_user. It only passes `interaction_tx` through RoutingContext. All blocking happens inside the tool's `execute()`.

5. **CLI dual-channel select** — `tokio::select!` on `event_rx` and `interaction_rx` with correct pause/resume of StreamRenderer.

6. **Bridge task eliminated** — No forwarding task needed. `interaction_tx` flows directly from `process_direct_streaming` into RoutingContext.

---

## Thread Safety: VERIFIED

- `InteractionBundle` contains `oneshot::Sender` — Send + !Clone (correct)
- `RoutingContext` with `Option<mpsc::Sender>` — Clone via Arc inside mpsc (correct)
- No shared mutable state across async boundaries
- TabbedFormState only used on the main thread (not Send/Sync, which is correct)
- No circular dependencies introduced

## Performance: NO ISSUES

- Single oneshot allocation per ask_user call — negligible overhead
- No unnecessary clones in hot path
- No locks in the interaction pathway
- `request.clone()` in `TabbedFormState::new()` is necessary (takes ownership)
- Buffer sizes appropriate (mpsc channel capacity = 4 for interaction_rx)

## Error Handling: COMPREHENSIVE

- `response_rx.await` handles RecvError (channel closed) — returns ToolError
- `interaction_tx.send()` handles SendError (receiver dropped) — returns ToolError
- All errors propagate through `common::Result<T>`
- 14 integration tests cover both happy paths and error paths

---

## Minor Findings (Non-blocking)

1. **Dead if/else branch** — `ask_user_prompt.rs:160-165`: Both branches return `Mode::Answering`. Functionally correct but the conditional is unnecessary. Comment explains intent for future differentiation.

2. **Empty BOLD colorize** — `ask_user_prompt.rs:338`: `colorize("", BOLD)` appended to reset state. Harmless but unnecessary.

---

## Gap Coverage

All 6 gaps from the requirements validation are addressed:

| Gap | Resolution |
|-----|-----------|
| GAP-1: Parallel tool call warning | Tool description includes "Never call ask_user alongside other tools" |
| GAP-2: Non-TTY fallback | `format_text_fallback()` returns structured text when `interaction_tx` is None |
| GAP-3: System prompt guidance | `context.rs` includes "Interactive Clarification" section with all key instructions |
| GAP-4: answer_type discriminator | JSON schema uses `oneOf` with `type` discriminator field |
| GAP-5: All answer types | 4 types implemented: single_select, multi_select, yes_no, free_text |
| GAP-6: Question limits | Validated: 1-4 questions, title max 12 chars, options min 2 |

---

## Test Coverage

| Category | Count |
|----------|-------|
| ask_user unit tests | 19 |
| ask_user integration tests | 14 |
| prompts.rs serde tests | 8 |
| **Total ask_user-specific** | **41** |
| Pre-existing workspace tests | 535+ |
| **Total workspace** | **576+** |

---

## Production Readiness Certification

| Criterion | Status |
|-----------|--------|
| All tests pass | PASS |
| Zero clippy warnings | PASS |
| Formatting | NEEDS `cargo fmt --all` |
| Architecture match | 100% |
| Thread safety | VERIFIED |
| Error handling | COMPREHENSIVE |
| No regressions | CONFIRMED |
| Test coverage | 41 ask_user-specific tests |

## Decision

**APPROVED — PRODUCTION READY**

Pending only `cargo fmt --all` to fix cosmetic formatting in 5 files.
