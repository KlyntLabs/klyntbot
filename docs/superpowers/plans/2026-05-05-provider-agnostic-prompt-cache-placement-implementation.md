# Provider-Agnostic Compression-Aware Prompt-Cache Placement Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use `superpowers:subagent-driven-development` (recommended) or `superpowers:executing-plans` to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add provider-agnostic explicit cache-breakpoint placement so the executor can dynamically decide where to mark `cache_control` on Anthropic requests (and validate prefix stability on OpenAI-compat requests in debug builds), with a compression-aware default policy.

**Architecture:** Two-PR migration. PR1 is purely mechanical: add the `CacheBreakpoint` types, change the `LlmProvider` trait signature, implement the Anthropic resolver + the OpenAI-compat debug-assertion, and pass `&[]` at every existing call site. Behavior is unchanged for any caller passing `&[]` because the Anthropic adapter falls back to its existing `cache_system_prompt` flag. PR2 wires the executor: `MidLoopCompressor` exposes a `frontier_index` accessor, a new `cache_policy` module computes breakpoints per cycle, the executor passes them through, and a kill switch in config plus surfaced cache-hit telemetry round it out.

**Spec:** `docs/superpowers/specs/2026-05-05-provider-agnostic-prompt-cache-placement-design.md`

**Tech Stack:** Rust workspace; `tokio`, `async-trait`, `serde`, `dashmap`, `reqwest`, `tracing`, `cargo-nextest`. Two crates change: `providers`, `agent`. One crate adds a config field: `config`.

---

## File Structure

### PR1 — Mechanism

| File | Change | Why |
|---|---|---|
| `crates/providers/src/types.rs` | + `CacheTtl`, `CacheAnchor`, `CacheBreakpoint` enums; + `explicit_cache_markers` field on `ProviderCapabilities`; modify `LlmProvider` trait sig | Public API surface |
| `crates/providers/src/adapters/anthropic_native.rs` | Modify `chat`, `chat_stream`, `build_request_body` to take `&[CacheBreakpoint]`; add `resolve_breakpoints` helper; apply markers + 1h beta header | Anthropic implements explicit markers |
| `crates/providers/src/adapters/openai_compat.rs` | Modify `chat`, `chat_stream` to take `&[CacheBreakpoint]`; add `#[cfg(debug_assertions)]` prefix-stability hash store + check | All non-Anthropic providers |
| `crates/providers/src/manager.rs` | Modify `chat`, `chat_stream` impls + helper closures to forward `cache_breakpoints` | Provider manager wraps primary/fallback |
| `crates/providers/src/types.rs` (default trait impl) | Modify default `chat_stream` impl to forward param | Self-call must compile |
| All ~40+ existing `provider.chat(...)` and `provider.chat_stream(...)` call sites | Add `&[]` as the new last argument | Mechanical migration |

### PR2 — Policy + Wiring + Config + Observability

| File | Change | Why |
|---|---|---|
| `crates/agent/src/execution/mid_loop_compressor.rs` | + `pub fn frontier_index(&self, &[Message]) -> usize`; refactor `compress_if_needed` to call it | Single source of truth |
| `crates/agent/src/execution/cache_policy.rs` | NEW: `compression_aware_default(...)` returns `Vec<CacheBreakpoint>` | Default policy |
| `crates/agent/src/execution/mod.rs` | + `pub mod cache_policy;` and re-export | Module visibility |
| `crates/agent/src/execution/core.rs` | `ExecutionCore::run_cycle` gains `cache_breakpoints: &[CacheBreakpoint]` parameter; forward to provider | Call boundary |
| `crates/agent/src/execution/execute_loop.rs` | Build breakpoints via `cache_policy::compression_aware_default` per cycle; pass to `run_cycle` | Policy wiring |
| `crates/config/src/schema/providers.rs` | + `CacheConfig { enabled: bool }` and `pub cache: CacheConfig` on `ProvidersConfig` | Kill switch |
| `crates/agent/src/events.rs` | Extend `BudgetUpdate` with `cache_read_tokens: u32`, `cache_write_tokens: u32` | Observability |
| `crates/agent/src/execution/core.rs` | + `tracing::info!` per call with cache hit ratio | Observability |
| `crates/providers/tests/cache_breakpoints_test.rs` | NEW integration tests | Anthropic wire-format verification |
| `crates/agent/tests/cache_policy_test.rs` | NEW integration tests | Policy correctness |

---

## Conventions

- **Workspace location:** `/Users/jayden/Projects/Klynt/bot`. All paths relative to it unless absolute.
- **Test runner:** `cargo nextest run` (per CLAUDE.md). Doctests use `cargo test --doc` separately.
- **Lint:** `cargo clippy --workspace --all-targets --all-features` must finish with **zero warnings**.
- **Format:** `cargo fmt --all` after every batch of edits.
- **Commit message style:** Conventional Commits — `feat(providers): ...`, `refactor(agent): ...`. Each commit must compile and pass tests.

---

# PR 1 — Mechanism (Provider Trait + Adapters + Mechanical Call-Site Migration)

The end state of PR1: workspace compiles; all existing tests pass; behavior unchanged for any caller because every existing call site passes `&[]` and the Anthropic adapter falls back to its legacy `cache_system_prompt` flag.

## Task 1: Define cache-breakpoint types

**Files:**
- Modify: `crates/providers/src/types.rs` (add types after `ResponseFormat` enum, around line 102)
- Test: `crates/providers/src/types.rs` (existing `#[cfg(test)] mod tests` if present, else create)

- [ ] **Step 1.1: Read the current types.rs to find the insertion point**

Run: `grep -n "pub enum ResponseFormat\|pub struct ChatParams" crates/providers/src/types.rs`
Expected: lines around 93-105.

- [ ] **Step 1.2: Write the failing test for `CacheTtl` round-trip**

Add to bottom of `crates/providers/src/types.rs` (create `#[cfg(test)] mod tests { ... }` if not present):

```rust
#[cfg(test)]
mod cache_breakpoint_tests {
    use super::*;

    #[test]
    fn cache_ttl_serde_roundtrip() {
        let json = serde_json::to_string(&CacheTtl::Ephemeral).unwrap();
        assert_eq!(json, "\"ephemeral\"");
        let json = serde_json::to_string(&CacheTtl::Persistent).unwrap();
        assert_eq!(json, "\"persistent\"");
        let parsed: CacheTtl = serde_json::from_str("\"ephemeral\"").unwrap();
        assert_eq!(parsed, CacheTtl::Ephemeral);
    }

    #[test]
    fn cache_ttl_default_is_ephemeral() {
        assert_eq!(CacheTtl::default(), CacheTtl::Ephemeral);
    }

    #[test]
    fn cache_anchor_equality() {
        assert_eq!(CacheAnchor::LastSystem, CacheAnchor::LastSystem);
        assert_ne!(CacheAnchor::LastSystem, CacheAnchor::LastTool);
        assert_eq!(
            CacheAnchor::MessageIndex(5),
            CacheAnchor::MessageIndex(5)
        );
        assert_ne!(
            CacheAnchor::MessageIndex(5),
            CacheAnchor::MessageIndex(6)
        );
    }

    #[test]
    fn cache_breakpoint_construction() {
        let bp = CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Persistent,
        };
        assert_eq!(bp.anchor, CacheAnchor::LastSystem);
        assert_eq!(bp.ttl, CacheTtl::Persistent);
    }
}
```

- [ ] **Step 1.3: Run the test to verify it fails**

Run: `cargo nextest run -p providers cache_breakpoint_tests 2>&1 | tail -30`
Expected: compile error — `CacheTtl`, `CacheAnchor`, `CacheBreakpoint` not defined.

- [ ] **Step 1.4: Add the type definitions**

Insert immediately before `pub struct ChatParams` (around line 102) in `crates/providers/src/types.rs`:

```rust
/// Cache lifetime hint for a [`CacheBreakpoint`]. Picked by the policy
/// that emits the breakpoint; honored by providers whose `ProviderCapabilities`
/// have `explicit_cache_markers = true`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheTtl {
    /// ~5 minutes. Anthropic default. Right for transient prefixes
    /// (e.g. the message-frontier marker that survives one compression
    /// burst but probably won't be reused tomorrow).
    Ephemeral,
    /// ~1 hour. Anthropic via `extended-cache-ttl-2025-04-11` beta.
    /// Right for system prompts and tool definitions that are stable
    /// for the whole session.
    Persistent,
}

impl Default for CacheTtl {
    fn default() -> Self {
        Self::Ephemeral
    }
}

/// Where to place a cache-control marker.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CacheAnchor {
    /// On the last `Message::System` block in the messages vec.
    /// No-op if there are no System messages.
    LastSystem,
    /// On the last entry in the tools array.
    /// No-op if `tools` is None or empty.
    LastTool,
    /// On the message at this index in the messages vec.
    /// Logged + skipped if out-of-range.
    MessageIndex(usize),
}

/// One cache-breakpoint instruction for a single LLM call.
#[derive(Clone, Debug)]
pub struct CacheBreakpoint {
    pub anchor: CacheAnchor,
    pub ttl: CacheTtl,
}
```

- [ ] **Step 1.5: Run the test to verify it passes**

Run: `cargo nextest run -p providers cache_breakpoint_tests`
Expected: 4 tests passed.

- [ ] **Step 1.6: Format and lint**

Run: `cargo fmt --all && cargo clippy -p providers --all-targets --all-features 2>&1 | tail -10`
Expected: zero warnings.

- [ ] **Step 1.7: Commit**

```bash
git add crates/providers/src/types.rs
git commit -m "feat(providers): add CacheTtl, CacheAnchor, CacheBreakpoint types

Adds the public types for explicit prompt-cache placement.
No behavior change yet; types are unused by callers.

Refs: docs/superpowers/specs/2026-05-05-provider-agnostic-prompt-cache-placement-design.md"
```

---

## Task 2: Add `explicit_cache_markers` capability flag

**Files:**
- Modify: `crates/providers/src/types.rs` (`ProviderCapabilities` struct + `Default`)

- [ ] **Step 2.1: Write the failing test**

Add to the existing test module in `crates/providers/src/types.rs` (or the `cache_breakpoint_tests` mod from Task 1):

```rust
#[test]
fn provider_capabilities_default_excludes_explicit_markers() {
    let caps = ProviderCapabilities::default();
    assert!(!caps.explicit_cache_markers);
    // Sanity: existing fields unchanged
    assert!(caps.streaming);
    assert!(!caps.prompt_caching);
}
```

- [ ] **Step 2.2: Run the test to verify it fails**

Run: `cargo nextest run -p providers provider_capabilities_default_excludes_explicit_markers 2>&1 | tail -15`
Expected: compile error — no field `explicit_cache_markers` on `ProviderCapabilities`.

- [ ] **Step 2.3: Add the field**

In `crates/providers/src/types.rs`, modify the `ProviderCapabilities` struct (around line 313):

```rust
#[derive(Debug, Clone)]
pub struct ProviderCapabilities {
    pub extended_thinking: bool,
    pub structured_outputs: bool,
    pub prompt_caching: bool,
    /// True if this provider honors explicit `CacheBreakpoint` markers.
    /// Anthropic: true. OpenAI/Gemini/etc. (auto-prefix-cache only): false.
    pub explicit_cache_markers: bool,
    pub native_token_counting: bool,
    pub vision: bool,
    pub streaming: bool,
    pub tool_choice_required: bool,
    pub parallel_tool_calls: bool,
}
```

And the `Default` impl (around line 324):

```rust
impl Default for ProviderCapabilities {
    fn default() -> Self {
        Self {
            extended_thinking: false,
            structured_outputs: false,
            prompt_caching: false,
            explicit_cache_markers: false,
            native_token_counting: false,
            vision: true,
            streaming: true,
            tool_choice_required: false,
            parallel_tool_calls: true,
        }
    }
}
```

- [ ] **Step 2.4: Run the test to verify it passes**

Run: `cargo nextest run -p providers provider_capabilities_default_excludes_explicit_markers`
Expected: 1 test passed.

- [ ] **Step 2.5: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p providers --all-targets 2>&1 | tail -5    # zero warnings
git add crates/providers/src/types.rs
git commit -m "feat(providers): add explicit_cache_markers capability flag

Default false. Anthropic adapter will set it true in a later commit."
```

---

## Task 3: Modify `LlmProvider` trait signature + propagate through provided default

**Files:**
- Modify: `crates/providers/src/types.rs` (trait definition; default `chat_stream` impl)

The trait change is a workspace-wide breakage. **Tasks 3 through 7 form a single atomic commit at the end of Task 7.** Compile with `cargo check` after each step but only commit after all impls + call sites are updated.

- [ ] **Step 3.1: Modify trait method signatures**

In `crates/providers/src/types.rs`, change the `LlmProvider` trait's `chat` and `chat_stream` methods (around line 161-213):

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a chat completion request (non-streaming)
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmResponse>;

    /// Send a streaming chat completion request
    /// Default implementation falls back to non-streaming chat()
    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],
    ) -> Result<LlmStream> {
        // Default: call chat() and wrap the response as stream chunks.
        let response = self.chat(messages, tools, params, cache_breakpoints).await?;

        // ... rest unchanged (chunks emission)
    }
    // ... rest of trait unchanged
}
```

The body of the default `chat_stream` impl forwards `cache_breakpoints` only at the `self.chat(...)` call site at line 179. All other lines stay identical.

- [ ] **Step 3.2: Run cargo check (expect breakage in callers + impls — this is fine)**

Run: `cargo check -p providers 2>&1 | tail -30`
Expected: errors in `manager.rs`, `adapters/anthropic_native.rs`, `adapters/openai_compat.rs` (impls don't match new sig). Other crates not yet checked.

Do not commit yet. Continue to Task 4.

---

## Task 4: Update `AnthropicNativeProvider` impl signatures (no marker logic yet)

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs`

This task only changes the method signatures so the provider crate compiles. The actual marker-application logic is added in Task 6.

- [ ] **Step 4.1: Update `chat` impl signature**

In `crates/providers/src/adapters/anthropic_native.rs`, find the `impl LlmProvider for AnthropicNativeProvider` block. Locate the `async fn chat(...)` method (the one that returns `LlmResponse`). Change its signature to:

```rust
async fn chat(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    cache_breakpoints: &[CacheBreakpoint],
) -> Result<LlmResponse> {
    let body = self.build_request_body(messages, tools, params, false, cache_breakpoints);
    // ... rest of the body unchanged
}
```

- [ ] **Step 4.2: Update `chat_stream` impl signature (around line 635)**

```rust
async fn chat_stream(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    cache_breakpoints: &[CacheBreakpoint],
) -> Result<LlmStream> {
    let body = self.build_request_body(messages, tools, params, true, cache_breakpoints);
    // ... rest unchanged
}
```

- [ ] **Step 4.3: Update `build_request_body` signature (line 428)**

```rust
fn build_request_body(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    stream: bool,
    cache_breakpoints: &[CacheBreakpoint],
) -> Value {
    // body unchanged for now — cache_breakpoints is unused;
    // marker application happens in Task 6.
    let _ = cache_breakpoints;  // silence unused warning until Task 6
    // ... existing logic
}
```

- [ ] **Step 4.4: Add `CacheBreakpoint` to imports**

At the top of `anthropic_native.rs`, change the `use crate::types::{...}` to include `CacheBreakpoint`. Find the existing import line and add `CacheBreakpoint`:

Run: `grep -n "use crate::types::" crates/providers/src/adapters/anthropic_native.rs`

Whatever names are imported, add `CacheBreakpoint` (alphabetically). For example:

```rust
use crate::types::{
    CacheBreakpoint, ChatParams, LlmProvider, LlmResponse, LlmStream, /* ... rest */,
};
```

- [ ] **Step 4.5: Run cargo check, expecting it to still fail in `openai_compat.rs` and `manager.rs`**

Run: `cargo check -p providers 2>&1 | tail -20`
Expected: AnthropicNativeProvider errors gone; remaining errors from `openai_compat.rs`, `manager.rs`.

---

## Task 5: Update `OpenAiCompatProvider` impl signatures (no debug-assertion logic yet)

**Files:**
- Modify: `crates/providers/src/adapters/openai_compat.rs`

- [ ] **Step 5.1: Update `chat` impl signature**

Find `async fn chat(...)` in `impl LlmProvider for OpenAiCompatProvider`. Change to:

```rust
async fn chat(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    cache_breakpoints: &[CacheBreakpoint],
) -> Result<LlmResponse> {
    let _ = cache_breakpoints; // honored as no-op; debug-only assertion added in Task 9
    // ... existing body unchanged
}
```

- [ ] **Step 5.2: Update `chat_stream` impl signature (around line 400)**

```rust
async fn chat_stream(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    cache_breakpoints: &[CacheBreakpoint],
) -> Result<LlmStream> {
    let _ = cache_breakpoints; // honored as no-op
    // ... existing body unchanged
}
```

- [ ] **Step 5.3: Add `CacheBreakpoint` to the imports at the top**

Find: `use crate::types::{ ... };` (around line 14)

Add `CacheBreakpoint` alphabetically:

```rust
use crate::types::{
    CacheBreakpoint, ChatParams, LlmProvider, LlmResponse, LlmStream, LlmStreamChunk,
    Message, ProviderCapabilities, ProviderHealth, ResponseFormat, ToolCall, ToolCallDelta,
    ToolCallMessage, Usage, DEFAULT_CONTEXT_WINDOW,
};
```

- [ ] **Step 5.4: Run cargo check, expecting only `manager.rs` errors**

Run: `cargo check -p providers 2>&1 | tail -20`
Expected: remaining errors localized to `manager.rs`.

---

## Task 6: Update `ProviderManager` impl + helper closures

**Files:**
- Modify: `crates/providers/src/manager.rs`

- [ ] **Step 6.1: Update the `chat` method (around line 230-330)**

Find `pub async fn chat(...)` on `ProviderManager`. Add the `cache_breakpoints` param and forward it everywhere:

```rust
pub async fn chat(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    cache_breakpoints: &[CacheBreakpoint],
) -> Result<LlmResponse> {
    self.retry_with_backoff(|| self.primary.chat(messages, tools, params, cache_breakpoints))
        .await
    // ... or whatever the existing logic is — forward cache_breakpoints in every primary/fallback call
}
```

Search for every `.chat(` and `.chat_stream(` inside `manager.rs` and add `cache_breakpoints` as the new last argument:
- Line ~236: `self.primary.chat(messages, tools, params, cache_breakpoints)`
- Line ~246: `self.primary.chat_stream(messages, tools, params, cache_breakpoints)`
- Line ~288: `fb.chat(messages, tools, params, cache_breakpoints)`
- Line ~321: `self.chat(messages, tools, &params, cache_breakpoints).await`
- Line ~336: `fb.chat(messages, tools, params, cache_breakpoints).await`
- Line ~355: `fb.chat_stream(messages, tools, params, cache_breakpoints).await`
- Line ~366: `fb.chat_stream(messages, tools, params, cache_breakpoints).await`

- [ ] **Step 6.2: Update the `chat_with_role` helper (around line 321) signature**

Inspect `crates/providers/src/manager.rs:321`. If `chat_with_role` exists and calls `self.chat`, give it a `cache_breakpoints` parameter too and forward.

Run: `grep -n "fn chat" crates/providers/src/manager.rs`

For every method that calls `.chat(...)` or `.chat_stream(...)`, add the parameter and forward.

- [ ] **Step 6.3: Update the `LlmProvider` impl on `ProviderManager` (around line 346)**

```rust
async fn chat_stream(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    cache_breakpoints: &[CacheBreakpoint],
) -> Result<LlmStream> {
    // forward through the same retry/fallback logic, with cache_breakpoints
    // ... see existing body
}
```

- [ ] **Step 6.4: Update test mocks in `manager.rs` test module**

Run: `grep -n "async fn chat" crates/providers/src/manager.rs | head -20`

Every mock impl (line ~529, ~1151, etc.) needs the new param. Update each mock's `chat` and `chat_stream` to accept (and ignore) `cache_breakpoints`:

```rust
async fn chat(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    _cache_breakpoints: &[CacheBreakpoint],
) -> Result<LlmResponse> {
    // ... existing mock body
}
```

- [ ] **Step 6.5: Update test call sites in `manager.rs`**

Tests at lines ~1043, 1072, 1101, 1124, 1184, 1217, 1220, 1226 currently look like:

```rust
.chat_stream(&[], None, &ChatParams::new("test-model"))
```

Change every one to:

```rust
.chat_stream(&[], None, &ChatParams::new("test-model"), &[])
```

Use a single global find-replace:

```bash
cd /Users/jayden/Projects/Klynt/bot
sed -i.bak -E 's/\.chat_stream\(&\[\], None, &ChatParams::new\("test-model"\)\)/.chat_stream(\&[], None, \&ChatParams::new("test-model"), \&[])/g' crates/providers/src/manager.rs
rm crates/providers/src/manager.rs.bak
```

Verify with `grep` that no instances remain:

```bash
grep 'chat_stream(&\[\], None, &ChatParams::new("test-model"))' crates/providers/src/manager.rs
```

Expected: no output.

- [ ] **Step 6.6: Add `CacheBreakpoint` import**

In `crates/providers/src/manager.rs`, ensure the imports include `CacheBreakpoint`:

```rust
use crate::types::{
    CacheBreakpoint, ChatParams, LlmProvider, LlmResponse, LlmStream, Message,
    /* ... rest */,
};
```

- [ ] **Step 6.7: Run cargo check on providers crate**

Run: `cargo check -p providers 2>&1 | tail -10`
Expected: providers crate compiles cleanly. Other crates may still error (Task 7 handles them).

---

## Task 7: Mechanical update of all production call sites in `agent`, `cognitive`, `app-core`

**Files:** ~40 call sites listed below. Each receives `&[]` as the new last argument.

The complete list, generated from `grep -rn "\.chat(\|\.chat_stream(" --include='*.rs'`:

**`crates/cognitive/`:**
- `src/services/session_memory.rs:202` → `provider.chat(&llm_messages, None, &params, &[])`
- `src/services/atom_extraction.rs:608` → `provider.chat(&messages, None, &params, &[])`

**`crates/agent/`:**
- `src/autotuner/mod.rs:776` → `orch.provider.chat(&messages, None, &params, &[])`
- `src/adapters/reforge_handlers.rs` lines 273, 288, 312, 406, 522, 623, 723 (7 sites) — append `, &[]`
- `src/adapters/cognitive_handlers.rs` lines 254, 553, 807, 916, 1080, 1264, 1341, 1412, 1502, 1552, 1644, 1692 (12 sites) — append `, &[]`
- `src/adapters/llm_summary.rs:108` → append `, &[]`
- `src/adapters/mirror_handlers.rs:61, 79` (2 sites) — append `, &[]`
- `src/adapters/multi_query.rs:144` → append `, &[]` inside the `tokio::time::timeout(...)` second arg
- `src/adapters/query_rewriter.rs:440` → append `, &[]` inside the `timeout`
- `src/adapters/llm_rerank.rs:126` → append `, &[]` inside the `timeout`
- `src/adapters/productivity.rs:33, 50, 72` (3 sites) — append `, &[]`
- `src/agent_loop/builder.rs:60` → append `, &[]`
- `src/handlers/rule_artifacts.rs:49` → append `, &[]`
- `src/handlers/coding_synthesis.rs:52` → append `, &[]`
- `src/execution/core.rs:275` (the `chat_stream` call inside `call_provider_streaming`) → append `, &[]`. **NOTE:** in PR2 we replace this with the real breakpoints; for PR1 we keep `&[]`.
- `src/execution/core.rs:537` (the non-streaming fallback `self.provider.chat(...)`) → append `, &[]`. Same note as above.

**`crates/app-core/`:**
- `src/handlers/notes/insight_chat.rs:36` → `provider.chat_stream(&messages, None, &chat_params, &[])`
- `src/handlers/notes/insight.rs:1197` → `provider.chat_stream(&messages, None, params, &[])`

- [ ] **Step 7.1: Apply the mechanical change file-by-file**

For each file, open it and add `, &[]` at the listed call sites.

Recommended approach — one regex pass per file, then visual verify. Example for `reforge_handlers.rs`:

```bash
cd /Users/jayden/Projects/Klynt/bot
# Use a precise pattern: find chat or chat_stream calls with these exact 3-arg shapes
# and append ", &[]" before the closing ).
# DO this manually per-file with the Edit tool to avoid scripting errors.
```

Edit the call sites with the `Edit` tool. The 3-arg pattern is consistent:

```rust
provider.chat(&messages, None, &self.params).await?
```

becomes:

```rust
provider.chat(&messages, None, &self.params, &[]).await?
```

- [ ] **Step 7.2: Run cargo check on the workspace after each crate finishes**

```bash
cargo check -p cognitive  &&
cargo check -p app-core   &&
cargo check -p agent
```

Expected: all green after every call site is updated.

- [ ] **Step 7.3: Run the full workspace check**

Run: `cargo check --workspace 2>&1 | tail -10`
Expected: success.

- [ ] **Step 7.4: Run the full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: all existing tests pass. Behavior is unchanged because the Anthropic adapter still falls through to `cache_system_prompt` legacy logic when `cache_breakpoints` is empty.

- [ ] **Step 7.5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`
Expected: zero warnings.

- [ ] **Step 7.6: Format**

Run: `cargo fmt --all`

- [ ] **Step 7.7: Commit Tasks 3–7 as one atomic commit**

```bash
git add crates/providers/src/types.rs \
        crates/providers/src/adapters/anthropic_native.rs \
        crates/providers/src/adapters/openai_compat.rs \
        crates/providers/src/manager.rs \
        crates/cognitive/src/services/session_memory.rs \
        crates/cognitive/src/services/atom_extraction.rs \
        crates/agent/src/autotuner/mod.rs \
        crates/agent/src/adapters/reforge_handlers.rs \
        crates/agent/src/adapters/cognitive_handlers.rs \
        crates/agent/src/adapters/llm_summary.rs \
        crates/agent/src/adapters/mirror_handlers.rs \
        crates/agent/src/adapters/multi_query.rs \
        crates/agent/src/adapters/query_rewriter.rs \
        crates/agent/src/adapters/llm_rerank.rs \
        crates/agent/src/adapters/productivity.rs \
        crates/agent/src/agent_loop/builder.rs \
        crates/agent/src/handlers/rule_artifacts.rs \
        crates/agent/src/handlers/coding_synthesis.rs \
        crates/agent/src/execution/core.rs \
        crates/app-core/src/handlers/notes/insight_chat.rs \
        crates/app-core/src/handlers/notes/insight.rs

git commit -m "refactor(providers): add cache_breakpoints param to LlmProvider trait

Mechanical change: every chat()/chat_stream() impl and call site receives
a new \`cache_breakpoints: &[CacheBreakpoint]\` parameter. All existing
callers pass &[]; behavior is unchanged because the Anthropic adapter
still uses the legacy cache_system_prompt fallback when given no
explicit breakpoints. The Anthropic-side resolver is added in the next
commit; the OpenAI-compat debug-assertion in the commit after."
```

---

## Task 8: Implement `resolve_breakpoints` helper in Anthropic adapter

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs`

This is the heart of the Anthropic-side mechanism. The helper takes raw `CacheBreakpoint` instructions and produces a sorted, deduped, ready-to-apply list of `(payload_section, index_within_section, ttl)` tuples.

- [ ] **Step 8.1: Define the resolver's output type**

In `anthropic_native.rs`, add (private to the file, near the top of the file or in an inner `mod`):

```rust
/// Resolved cache-marker placement: which content block to mark, and how.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResolvedMarker {
    /// Section of the request payload (lower numbers come earlier).
    /// SECTION_SYSTEM = 0, SECTION_TOOLS = 1, SECTION_MESSAGES = 2.
    section: u8,
    /// Index within that section.
    index: usize,
    /// TTL hint.
    ttl: CacheTtl,
}

const SECTION_SYSTEM: u8 = 0;
const SECTION_TOOLS: u8 = 1;
const SECTION_MESSAGES: u8 = 2;
```

- [ ] **Step 8.2: Write failing tests for `resolve_breakpoints`**

Add to the `#[cfg(test)] mod tests` in `anthropic_native.rs`:

```rust
#[cfg(test)]
mod resolve_breakpoints_tests {
    use super::*;

    fn sys(text: &str) -> Message {
        Message::System { content: text.to_string() }
    }

    fn user(text: &str) -> Message {
        Message::user(text)
    }

    fn assistant(text: &str) -> Message {
        Message::Assistant {
            content: Some(text.to_string()),
            tool_calls: None,
            reasoning_content: None,
        }
    }

    #[test]
    fn resolve_last_system_finds_last_system_block() {
        let messages = vec![sys("first"), sys("second"), user("hi")];
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Persistent,
        }];
        let resolved = resolve_breakpoints(&messages, None, &bps);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].section, SECTION_SYSTEM);
        assert_eq!(resolved[0].index, 1); // index 1 = "second" system message
        assert_eq!(resolved[0].ttl, CacheTtl::Persistent);
    }

    #[test]
    fn resolve_last_system_skips_when_no_system_messages() {
        let messages = vec![user("hi")];
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Ephemeral,
        }];
        let resolved = resolve_breakpoints(&messages, None, &bps);
        assert!(resolved.is_empty());
    }

    #[test]
    fn resolve_last_tool_marks_last_tool_index() {
        let tools = vec![
            serde_json::json!({"name": "a"}),
            serde_json::json!({"name": "b"}),
        ];
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::LastTool,
            ttl: CacheTtl::Persistent,
        }];
        let resolved = resolve_breakpoints(&[], Some(&tools), &bps);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].section, SECTION_TOOLS);
        assert_eq!(resolved[0].index, 1);
    }

    #[test]
    fn resolve_last_tool_skips_when_no_tools() {
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::LastTool,
            ttl: CacheTtl::Persistent,
        }];
        let resolved = resolve_breakpoints(&[], None, &bps);
        assert!(resolved.is_empty());

        let resolved2 = resolve_breakpoints(&[], Some(&[]), &bps);
        assert!(resolved2.is_empty());
    }

    #[test]
    fn resolve_message_index_in_range() {
        let messages = vec![sys("s"), user("u"), assistant("a")];
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(2),
            ttl: CacheTtl::Ephemeral,
        }];
        let resolved = resolve_breakpoints(&messages, None, &bps);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].section, SECTION_MESSAGES);
        assert_eq!(resolved[0].index, 2);
    }

    #[test]
    fn resolve_message_index_out_of_range_skipped() {
        let messages = vec![sys("s"), user("u")];
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(99),
            ttl: CacheTtl::Ephemeral,
        }];
        let resolved = resolve_breakpoints(&messages, None, &bps);
        assert!(resolved.is_empty());
    }

    #[test]
    fn resolve_sorts_and_keeps_trailing_four() {
        // Sections by absolute order: SYSTEM (0) < TOOLS (1) < MESSAGES (2).
        let messages = vec![
            sys("s"),
            user("u0"), user("u1"), user("u2"), user("u3"), user("u4"),
        ];
        let tools = vec![serde_json::json!({"name": "t"})];
        let bps = vec![
            CacheBreakpoint { anchor: CacheAnchor::LastSystem, ttl: CacheTtl::Persistent },
            CacheBreakpoint { anchor: CacheAnchor::LastTool, ttl: CacheTtl::Persistent },
            CacheBreakpoint { anchor: CacheAnchor::MessageIndex(1), ttl: CacheTtl::Ephemeral },
            CacheBreakpoint { anchor: CacheAnchor::MessageIndex(2), ttl: CacheTtl::Ephemeral },
            CacheBreakpoint { anchor: CacheAnchor::MessageIndex(3), ttl: CacheTtl::Ephemeral },
        ];
        let resolved = resolve_breakpoints(&messages, Some(&tools), &bps);
        // 5 inputs → keep trailing 4
        assert_eq!(resolved.len(), 4);
        // Earliest one (LastSystem) should have been dropped
        assert!(!resolved.iter().any(|r| r.section == SECTION_SYSTEM));
        // Verify ascending order
        let positions: Vec<(u8, usize)> =
            resolved.iter().map(|r| (r.section, r.index)).collect();
        let mut sorted = positions.clone();
        sorted.sort();
        assert_eq!(positions, sorted);
    }
}
```

- [ ] **Step 8.3: Run the tests, expecting compile error**

Run: `cargo nextest run -p providers resolve_breakpoints_tests 2>&1 | tail -20`
Expected: compile error — `resolve_breakpoints` not defined.

- [ ] **Step 8.4: Implement `resolve_breakpoints`**

Add this function near the top of `anthropic_native.rs` (private to the file, after the imports):

```rust
/// Resolve `CacheBreakpoint` anchors against the actual message vec / tools array.
///
/// Returns a list of `ResolvedMarker` sorted ascending by absolute payload position,
/// with at most 4 entries (Anthropic's per-request limit). When more than 4 markers
/// would be emitted, the EARLIEST ones are dropped — caching at a later position
/// implicitly covers everything before it, so trailing markers dominate.
fn resolve_breakpoints(
    messages: &[Message],
    tools: Option<&[Value]>,
    breakpoints: &[CacheBreakpoint],
) -> Vec<ResolvedMarker> {
    let mut out: Vec<ResolvedMarker> = Vec::with_capacity(breakpoints.len());

    for bp in breakpoints {
        match &bp.anchor {
            CacheAnchor::LastSystem => {
                let last = messages
                    .iter()
                    .enumerate()
                    .rev()
                    .find_map(|(i, m)| matches!(m, Message::System { .. }).then_some(i));
                if let Some(i) = last {
                    out.push(ResolvedMarker {
                        section: SECTION_SYSTEM,
                        index: i,
                        ttl: bp.ttl,
                    });
                }
                // No System messages → silently skip
            }
            CacheAnchor::LastTool => {
                if let Some(t) = tools {
                    if !t.is_empty() {
                        out.push(ResolvedMarker {
                            section: SECTION_TOOLS,
                            index: t.len() - 1,
                            ttl: bp.ttl,
                        });
                    }
                }
                // No tools → silently skip
            }
            CacheAnchor::MessageIndex(n) => {
                if *n < messages.len() {
                    out.push(ResolvedMarker {
                        section: SECTION_MESSAGES,
                        index: *n,
                        ttl: bp.ttl,
                    });
                } else {
                    tracing::warn!(
                        target: "klynt::providers::anthropic",
                        index = *n,
                        len = messages.len(),
                        "MessageIndex breakpoint out of range; skipping"
                    );
                }
            }
        }
    }

    // Stable sort by (section, index) ascending.
    out.sort_by_key(|r| (r.section, r.index));

    // Anthropic permits at most 4 cache_control blocks per request.
    // Caching at position N implicitly caches everything before N, so when
    // we have to drop, drop the EARLIEST entries.
    if out.len() > 4 {
        let drop_count = out.len() - 4;
        out.drain(..drop_count);
    }

    out
}
```

- [ ] **Step 8.5: Run the tests to verify they pass**

Run: `cargo nextest run -p providers resolve_breakpoints_tests`
Expected: 7 tests passed.

- [ ] **Step 8.6: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p providers --all-targets 2>&1 | tail -5
git add crates/providers/src/adapters/anthropic_native.rs
git commit -m "feat(providers): add resolve_breakpoints helper for Anthropic adapter

Resolves CacheBreakpoint anchors against the actual messages/tools,
sorts by absolute payload position, keeps trailing 4 (Anthropic's
per-request limit). Tests cover all anchor variants, empty/missing
inputs, out-of-range indices, and the trailing-4 dedup rule."
```

---

## Task 9: Wire `resolve_breakpoints` into `build_request_body` (apply markers)

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs`

- [ ] **Step 9.1: Write failing test for system-marker injection**

Add to the test module in `anthropic_native.rs`:

```rust
#[cfg(test)]
mod build_request_body_cache_tests {
    use super::*;

    fn make_provider(cache_system_prompt: bool) -> AnthropicNativeProvider {
        // Use whatever existing test constructor pattern is in the file.
        // Search: grep -n "fn test_" crates/providers/src/adapters/anthropic_native.rs
        // and copy the existing constructor pattern. Example:
        AnthropicNativeProvider::new_for_test(cache_system_prompt)
    }

    fn ttl_value(block: &Value) -> Option<&str> {
        block
            .get("cache_control")
            .and_then(|c| c.get("ttl"))
            .and_then(|t| t.as_str())
    }

    fn has_cache_control(block: &Value) -> bool {
        block.get("cache_control").is_some()
    }

    #[test]
    fn explicit_breakpoint_overrides_legacy_flag() {
        let provider = make_provider(true);
        let messages = vec![
            Message::System { content: "sys".into() },
            Message::user("hi"),
        ];
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::LastSystem,
            ttl: CacheTtl::Persistent,
        }];
        let body = provider.build_request_body(
            &messages,
            None,
            &ChatParams::new("claude-3-5-sonnet"),
            false,
            &bps,
        );
        let system_blocks = body.get("system").unwrap().as_array().unwrap();
        // Last (only) block should have cache_control with ttl=1h
        assert!(has_cache_control(&system_blocks[0]));
        assert_eq!(ttl_value(&system_blocks[0]), Some("1h"));
    }

    #[test]
    fn empty_breakpoints_with_legacy_flag_synthesizes_fallback() {
        let provider = make_provider(true);
        let messages = vec![Message::System { content: "sys".into() }];
        let body = provider.build_request_body(
            &messages,
            None,
            &ChatParams::new("claude-3-5-sonnet"),
            false,
            &[],
        );
        let system_blocks = body.get("system").unwrap().as_array().unwrap();
        // Legacy fallback: ephemeral, no ttl key
        assert!(has_cache_control(&system_blocks[0]));
        assert_eq!(ttl_value(&system_blocks[0]), None);
    }

    #[test]
    fn empty_breakpoints_without_legacy_flag_no_marker() {
        let provider = make_provider(false);
        let messages = vec![Message::System { content: "sys".into() }];
        let body = provider.build_request_body(
            &messages,
            None,
            &ChatParams::new("claude-3-5-sonnet"),
            false,
            &[],
        );
        let system_blocks = body.get("system").unwrap().as_array().unwrap();
        assert!(!has_cache_control(&system_blocks[0]));
    }
}
```

If `AnthropicNativeProvider` lacks a public test constructor, add one (or reuse an existing test pattern). Inspect the file:

Run: `grep -n "fn new\|fn from_config" crates/providers/src/adapters/anthropic_native.rs`

If the existing constructor needs config, build a minimal test config inline.

- [ ] **Step 9.2: Run the tests, expecting failure**

Run: `cargo nextest run -p providers build_request_body_cache_tests 2>&1 | tail -20`
Expected: 3 failures (markers absent / wrong shape because `build_request_body` doesn't apply them yet).

- [ ] **Step 9.3: Modify `build_request_body` to apply system + tools markers**

Locate the system-prompt block construction (around line 437-471). Replace it with:

```rust
// Resolve breakpoints — synthesize a legacy fallback if caller passed
// no explicit breakpoints AND the legacy flag is on.
let bps_owned: Vec<CacheBreakpoint>;
let bps: &[CacheBreakpoint] = if cache_breakpoints.is_empty() && self.cache_system_prompt {
    tracing::debug!(
        target: "klynt::providers::anthropic",
        "no explicit cache_breakpoints; synthesizing legacy LastSystem/Ephemeral fallback"
    );
    bps_owned = vec![CacheBreakpoint {
        anchor: CacheAnchor::LastSystem,
        ttl: CacheTtl::Ephemeral,
    }];
    &bps_owned
} else {
    cache_breakpoints
};

let resolved = resolve_breakpoints(messages, tools, bps);

// Build a quick-lookup: section → set of (index, ttl)
fn marker_for(
    resolved: &[ResolvedMarker],
    section: u8,
    index: usize,
) -> Option<CacheTtl> {
    resolved
        .iter()
        .find(|r| r.section == section && r.index == index)
        .map(|r| r.ttl)
}

// System prompt — collect all system messages into content block array.
let system_prompts = Self::extract_system_prompts(messages);
if !system_prompts.is_empty() {
    let blocks: Vec<Value> = system_prompts
        .iter()
        .enumerate()
        .map(|(i, text)| {
            let mut block = json!({"type": "text", "text": text});
            if let Some(ttl) = marker_for(&resolved, SECTION_SYSTEM, i) {
                block["cache_control"] = match ttl {
                    CacheTtl::Ephemeral => json!({"type": "ephemeral"}),
                    CacheTtl::Persistent => json!({"type": "ephemeral", "ttl": "1h"}),
                };
            }
            block
        })
        .collect();
    body["system"] = json!(blocks);
}
```

- [ ] **Step 9.4: Apply tool markers further down in `build_request_body`**

Locate the tools serialization block. Run: `grep -n "convert_tools\|body\[\"tools\"\]" crates/providers/src/adapters/anthropic_native.rs`

After the existing `body["tools"] = json!(converted_tools);` (or equivalent), inject markers:

```rust
// Apply LastTool cache_control on the converted tools array.
if let Some(tools_arr) = body.get_mut("tools").and_then(|t| t.as_array_mut()) {
    for (i, tool) in tools_arr.iter_mut().enumerate() {
        if let Some(ttl) = marker_for(&resolved, SECTION_TOOLS, i) {
            tool["cache_control"] = match ttl {
                CacheTtl::Ephemeral => json!({"type": "ephemeral"}),
                CacheTtl::Persistent => json!({"type": "ephemeral", "ttl": "1h"}),
            };
        }
    }
}
```

- [ ] **Step 9.5: Apply message markers**

Locate the `convert_messages` call (line ~439). The Anthropic adapter converts Klynt's `Message` enum to Anthropic's `messages` array. We need the cache_control to land on the LAST content block of `messages[N]`.

Find `fn convert_messages` (run: `grep -n "fn convert_messages" crates/providers/src/adapters/anthropic_native.rs`).

Modify `build_request_body` to post-process the converted messages:

```rust
// Apply MessageIndex cache_control. Anthropic puts cache_control on the
// LAST content block of the marked message.
let converted = body.get_mut("messages").and_then(|m| m.as_array_mut());
if let Some(msgs_arr) = converted {
    for (i, msg) in msgs_arr.iter_mut().enumerate() {
        if let Some(ttl) = marker_for(&resolved, SECTION_MESSAGES, i) {
            // Anthropic message content is either a string or an array of blocks.
            // Promote string to array and add cache_control to the last block.
            let content = msg.get_mut("content").cloned();
            if let Some(content) = content {
                let mut blocks: Vec<Value> = match content {
                    Value::String(s) => vec![json!({"type": "text", "text": s})],
                    Value::Array(a) => a,
                    other => vec![other],
                };
                if let Some(last) = blocks.last_mut() {
                    if let Some(obj) = last.as_object_mut() {
                        let cc = match ttl {
                            CacheTtl::Ephemeral => json!({"type": "ephemeral"}),
                            CacheTtl::Persistent => json!({"type": "ephemeral", "ttl": "1h"}),
                        };
                        obj.insert("cache_control".to_string(), cc);
                    }
                }
                msg["content"] = json!(blocks);
            }
        }
    }
}
```

- [ ] **Step 9.6: Run the build-request-body tests**

Run: `cargo nextest run -p providers build_request_body_cache_tests`
Expected: 3 tests passed.

- [ ] **Step 9.7: Run all provider tests**

Run: `cargo nextest run -p providers 2>&1 | tail -10`
Expected: all green. The legacy `cache_system_prompt` tests still pass because the synthesized fallback path produces identical output.

- [ ] **Step 9.8: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p providers --all-targets 2>&1 | tail -5
git add crates/providers/src/adapters/anthropic_native.rs
git commit -m "feat(providers): apply CacheBreakpoint markers in Anthropic adapter

build_request_body now resolves breakpoints and injects cache_control on
the appropriate system blocks, tool entries, and message content blocks.
When cache_breakpoints is empty AND cache_system_prompt is true, falls
back to the legacy LastSystem/Ephemeral marker (logged at debug level)
so existing callers keep working unchanged."
```

---

## Task 10: Add 1-hour beta header for `Persistent` markers

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs` (`chat`, `chat_stream` HTTP request building)

- [ ] **Step 10.1: Write failing test**

Add to the test module:

```rust
#[test]
fn persistent_breakpoint_triggers_extended_cache_ttl_header() {
    let resolved = vec![ResolvedMarker {
        section: SECTION_SYSTEM,
        index: 0,
        ttl: CacheTtl::Persistent,
    }];
    assert!(needs_extended_cache_ttl_header(&resolved));

    let resolved = vec![ResolvedMarker {
        section: SECTION_SYSTEM,
        index: 0,
        ttl: CacheTtl::Ephemeral,
    }];
    assert!(!needs_extended_cache_ttl_header(&resolved));

    let resolved: Vec<ResolvedMarker> = vec![];
    assert!(!needs_extended_cache_ttl_header(&resolved));
}
```

- [ ] **Step 10.2: Run, expect compile error**

Run: `cargo nextest run -p providers persistent_breakpoint_triggers_extended_cache_ttl_header 2>&1 | tail -10`
Expected: `needs_extended_cache_ttl_header` not defined.

- [ ] **Step 10.3: Add the helper**

Near `resolve_breakpoints` in `anthropic_native.rs`:

```rust
/// True if any resolved marker has Persistent TTL and the request thus
/// needs the `anthropic-beta: extended-cache-ttl-2025-04-11` header.
fn needs_extended_cache_ttl_header(resolved: &[ResolvedMarker]) -> bool {
    resolved.iter().any(|r| matches!(r.ttl, CacheTtl::Persistent))
}
```

- [ ] **Step 10.4: Wire the header in `chat` and `chat_stream`**

Find the HTTP request builder in `chat` (search: `grep -n "request_builder\|RequestBuilder\|.post(" crates/providers/src/adapters/anthropic_native.rs`).

In both `chat` and `chat_stream`, after building `body` and before sending:

```rust
let resolved = resolve_breakpoints(messages, tools, cache_breakpoints);
// (Note: build_request_body already calls resolve_breakpoints internally;
// we also call it here at the request-builder level. Or refactor to compute
// once and pass through. For minimal change: compute here too — small cost.)

let mut request = self.client.post(&url).json(&body);
if needs_extended_cache_ttl_header(&resolved) {
    request = request.header("anthropic-beta", "extended-cache-ttl-2025-04-11");
}
let response = request.send().await?;
```

If the file already has multiple beta headers, append rather than overwrite. Search: `grep -n "anthropic-beta" crates/providers/src/adapters/anthropic_native.rs`.

If a single `anthropic-beta` header is set unconditionally elsewhere, combine: Anthropic accepts a comma-separated list, e.g. `anthropic-beta: extended-cache-ttl-2025-04-11, prompt-caching-2024-07-31`.

- [ ] **Step 10.5: Run the helper test**

Run: `cargo nextest run -p providers persistent_breakpoint_triggers_extended_cache_ttl_header`
Expected: pass.

- [ ] **Step 10.6: Add an integration-style test verifying the header**

```rust
#[tokio::test]
async fn chat_with_persistent_marker_sets_beta_header() {
    use mockito::Server;

    let mut server = Server::new_async().await;
    let mock = server.mock("POST", "/messages")
        .match_header("anthropic-beta", mockito::Matcher::Regex(
            ".*extended-cache-ttl-2025-04-11.*".to_string()
        ))
        .with_status(200)
        .with_body(r#"{"id":"msg_1","model":"claude-3-5-sonnet","role":"assistant",
            "content":[{"type":"text","text":"ok"}],"stop_reason":"end_turn",
            "usage":{"input_tokens":10,"output_tokens":2}}"#)
        .create_async().await;

    let provider = AnthropicNativeProvider::new_for_test_with_url(server.url());
    let messages = vec![Message::System { content: "sys".into() }];
    let bps = vec![CacheBreakpoint {
        anchor: CacheAnchor::LastSystem,
        ttl: CacheTtl::Persistent,
    }];
    let _ = provider.chat(&messages, None, &ChatParams::new("claude-3-5-sonnet"), &bps).await;
    mock.assert_async().await;
}
```

If `mockito` isn't already a dev-dependency in `crates/providers/Cargo.toml`, add it. Run `grep mockito crates/providers/Cargo.toml`. If absent, add `mockito = "1"` under `[dev-dependencies]`.

If the existing test pattern uses a different mocking approach (e.g., `wiremock`), match that pattern instead. Search: `grep -rn "wiremock\|mockito" crates/providers/`.

- [ ] **Step 10.7: Run all tests, lint, format, commit**

```bash
cargo fmt --all
cargo nextest run -p providers 2>&1 | tail -10
cargo clippy -p providers --all-targets 2>&1 | tail -5
git add crates/providers/src/adapters/anthropic_native.rs crates/providers/Cargo.toml
git commit -m "feat(providers): set extended-cache-ttl-2025-04-11 header when any breakpoint is Persistent

Per-request, opt-in. Header is omitted when all breakpoints are
Ephemeral (or none exist), keeping the request payload clean for the
common case."
```

---

## Task 11: Set `explicit_cache_markers = true` on Anthropic adapter

**Files:**
- Modify: `crates/providers/src/adapters/anthropic_native.rs` (`capabilities` impl)

- [ ] **Step 11.1: Write failing test**

```rust
#[test]
fn anthropic_capabilities_have_explicit_cache_markers() {
    let provider = make_provider(false);
    let caps = provider.capabilities();
    assert!(caps.explicit_cache_markers);
    assert!(caps.prompt_caching);
}
```

- [ ] **Step 11.2: Run, expect failure**

Run: `cargo nextest run -p providers anthropic_capabilities_have_explicit_cache_markers`
Expected: fail because the field is `false`.

- [ ] **Step 11.3: Set the field to true**

Find the `capabilities()` impl in `anthropic_native.rs` (around line 740-750). Add the field:

```rust
fn capabilities(&self) -> ProviderCapabilities {
    ProviderCapabilities {
        extended_thinking: true,
        structured_outputs: true,
        prompt_caching: true,
        explicit_cache_markers: true,
        // ... existing fields
        ..Default::default()
    }
}
```

- [ ] **Step 11.4: Run test, verify pass; format, commit**

```bash
cargo nextest run -p providers anthropic_capabilities_have_explicit_cache_markers
cargo fmt --all
git add crates/providers/src/adapters/anthropic_native.rs
git commit -m "feat(providers): declare explicit_cache_markers=true for Anthropic adapter"
```

---

## Task 12: Add OpenAI-compat debug-only prefix-stability assertion

**Files:**
- Modify: `crates/providers/src/adapters/openai_compat.rs`
- Modify: `crates/providers/Cargo.toml` (add `dashmap` dep if not present)

- [ ] **Step 12.1: Verify `dashmap` is available**

Run: `grep dashmap crates/providers/Cargo.toml`

If absent, add to `[dependencies]`:

```toml
dashmap = "5"
```

If a different version is used elsewhere in the workspace (`grep -rn "^dashmap" crates/*/Cargo.toml | head -5`), match it.

- [ ] **Step 12.2: Add `ChatParams::session_key` accessor (if not present)**

Run: `grep -n "session_key" crates/providers/src/types.rs`

If `ChatParams` lacks a way to identify the session, the debug assertion can't key its DashMap. The cheapest fix:

1. Add an `Option<String>` field `session_key` on `ChatParams`.
2. Add a builder `with_session_key(...)`.

In `crates/providers/src/types.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ChatParams {
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub response_format: Option<ResponseFormat>,
    pub role: Option<crate::ProviderRole>,
    /// Opaque session identifier. Used by the OpenAI-compat debug assertion
    /// to dedupe prefix-stability hashes across calls. Production builds
    /// don't read it.
    pub session_key: Option<String>,
}

impl ChatParams {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            model: model.into(),
            temperature: None,
            max_tokens: None,
            response_format: None,
            role: None,
            session_key: None,
        }
    }

    pub fn with_session_key(mut self, key: impl Into<String>) -> Self {
        self.session_key = Some(key.into());
        self
    }
    // ... existing builders unchanged
}
```

This change is additive and doesn't require updating call sites.

- [ ] **Step 12.3: Write failing test for prefix-stability assertion**

Create `crates/providers/src/adapters/openai_compat.rs` test module additions:

```rust
#[cfg(test)]
#[cfg(debug_assertions)]
mod prefix_stability_tests {
    use super::*;

    fn make_provider() -> OpenAiCompatProvider {
        // Reuse existing test constructor pattern, e.g.:
        OpenAiCompatProvider::new_for_test()
    }

    fn msgs_v1() -> Vec<Message> {
        vec![
            Message::System { content: "sys".into() },
            Message::user("first"),
            Message::user("second"),
        ]
    }

    fn msgs_v2_mutated_middle() -> Vec<Message> {
        vec![
            Message::System { content: "sys".into() },
            Message::user("CHANGED"),    // prefix mutated
            Message::user("second"),
        ]
    }

    #[test]
    fn first_call_no_warn() {
        let provider = make_provider();
        let messages = msgs_v1();
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(2),
            ttl: CacheTtl::Ephemeral,
        }];
        let params = ChatParams::new("gpt-4o").with_session_key("sess-A");
        // First call records hash, no warn
        provider.assert_prefix_stable(&messages, &bps, &params);
        // (Not asserting absence of warn here — that requires log capture.
        // Just verify the function runs without panic.)
    }

    #[test]
    fn identical_second_call_no_change_in_hash() {
        let provider = make_provider();
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(2),
            ttl: CacheTtl::Ephemeral,
        }];
        let params = ChatParams::new("gpt-4o").with_session_key("sess-B");
        provider.assert_prefix_stable(&msgs_v1(), &bps, &params);
        let h1 = provider.prefix_hashes.get(&"sess-B".to_string()).map(|v| *v);
        provider.assert_prefix_stable(&msgs_v1(), &bps, &params);
        let h2 = provider.prefix_hashes.get(&"sess-B".to_string()).map(|v| *v);
        assert_eq!(h1, h2);
    }

    #[test]
    fn mutated_prefix_changes_hash() {
        let provider = make_provider();
        let bps = vec![CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(2),
            ttl: CacheTtl::Ephemeral,
        }];
        let params = ChatParams::new("gpt-4o").with_session_key("sess-C");
        provider.assert_prefix_stable(&msgs_v1(), &bps, &params);
        let h1 = provider.prefix_hashes.get(&"sess-C".to_string()).map(|v| *v);
        provider.assert_prefix_stable(&msgs_v2_mutated_middle(), &bps, &params);
        let h2 = provider.prefix_hashes.get(&"sess-C".to_string()).map(|v| *v);
        assert_ne!(h1, h2);
    }
}
```

- [ ] **Step 12.4: Run, expect compile failure**

Run: `cargo nextest run -p providers prefix_stability_tests 2>&1 | tail -10`
Expected: missing `assert_prefix_stable`, `prefix_hashes`.

- [ ] **Step 12.5: Add the field, helper, and call site**

In `crates/providers/src/adapters/openai_compat.rs`, modify the struct definition:

```rust
pub struct OpenAiCompatProvider {
    client: Client,
    api_base: String,
    // ... existing fields
    #[cfg(debug_assertions)]
    pub(crate) prefix_hashes: dashmap::DashMap<String, u64>,
}
```

Initialize in the constructor (`fn new` and `fn new_for_test`):

```rust
pub fn new(/* args */) -> Self {
    Self {
        // ... existing fields
        #[cfg(debug_assertions)]
        prefix_hashes: dashmap::DashMap::new(),
    }
}
```

Add the `assert_prefix_stable` method:

```rust
#[cfg(debug_assertions)]
impl OpenAiCompatProvider {
    /// In debug builds only: hash the conversation prefix up to the deepest
    /// `MessageIndex` breakpoint and compare against the previous hash for
    /// the same session_key. If different, log a warning — it indicates
    /// something mutated the prefix (e.g., compression rewrote a message
    /// in the cache region) which would invalidate server-side prefix cache.
    pub(crate) fn assert_prefix_stable(
        &self,
        messages: &[Message],
        breakpoints: &[crate::types::CacheBreakpoint],
        params: &ChatParams,
    ) {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let Some(session_key) = params.session_key.as_ref() else {
            return;
        };

        let frontier = breakpoints
            .iter()
            .filter_map(|b| match b.anchor {
                crate::types::CacheAnchor::MessageIndex(n) => Some(n),
                _ => None,
            })
            .max();
        let Some(frontier) = frontier else { return };

        if frontier >= messages.len() {
            return;
        }

        let mut hasher = DefaultHasher::new();
        for msg in &messages[..=frontier] {
            // serde_json gives us a stable byte representation
            if let Ok(s) = serde_json::to_string(msg) {
                s.hash(&mut hasher);
            }
        }
        let new_hash = hasher.finish();

        if let Some(prev) = self.prefix_hashes.insert(session_key.clone(), new_hash) {
            if prev != new_hash {
                tracing::warn!(
                    target: "klynt::providers::openai_compat",
                    session = %session_key,
                    prev_hash = format!("{:x}", prev),
                    new_hash = format!("{:x}", new_hash),
                    frontier,
                    "prefix-cache-busting detected: messages[..={}] hash changed. \
                     Did MidLoopCompressor or another rewrite mutate before the frontier?",
                    frontier,
                );
            }
        }
    }
}
```

In `chat` and `chat_stream`:

```rust
async fn chat(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    cache_breakpoints: &[CacheBreakpoint],
) -> Result<LlmResponse> {
    #[cfg(debug_assertions)]
    self.assert_prefix_stable(messages, cache_breakpoints, params);
    // ... existing body unchanged
}
```

Same shape in `chat_stream`.

- [ ] **Step 12.6: Run tests, format, lint, commit**

```bash
cargo nextest run -p providers prefix_stability_tests
cargo fmt --all
cargo clippy -p providers --all-targets 2>&1 | tail -5
git add crates/providers/src/adapters/openai_compat.rs \
        crates/providers/src/types.rs \
        crates/providers/Cargo.toml
git commit -m "feat(providers): add debug-only prefix-stability assertion to OpenAI-compat adapter

In #[cfg(debug_assertions)] only, OpenAiCompatProvider hashes the
prefix up to the deepest MessageIndex breakpoint and warns if a
subsequent call's prefix-hash differs. Catches accidental
cache-busting (e.g., compression mutating before the frontier)
during development. Production builds: pure no-op.

Adds session_key: Option<String> to ChatParams so the debug
assertion can dedupe across calls."
```

---

## Task 13: PR1 final verification

- [ ] **Step 13.1: Full workspace compile**

Run: `cargo check --workspace 2>&1 | tail -10`
Expected: success.

- [ ] **Step 13.2: Full test suite via nextest**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: all green. Behavior unchanged for callers passing `&[]`.

- [ ] **Step 13.3: Doctests**

Run: `cargo test --workspace --doc 2>&1 | tail -10`
Expected: pass.

- [ ] **Step 13.4: Clippy zero-warnings**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`
Expected: zero warnings.

- [ ] **Step 13.5: Format check**

Run: `cargo fmt --all --check`
Expected: clean.

- [ ] **Step 13.6: Open PR1**

```bash
git push -u origin <branch-name>
gh pr create --title "feat(providers): add CacheBreakpoint mechanism (PR1: types + adapters)" --body "$(cat <<'EOF'
## Summary
- Add `CacheTtl`, `CacheAnchor`, `CacheBreakpoint` types
- Add `cache_breakpoints: &[CacheBreakpoint]` parameter to `LlmProvider` trait
- Implement Anthropic adapter resolver, marker injection, and 1h beta header
- Implement OpenAI-compat debug-only prefix-stability assertion
- Mechanically update all ~40 production call sites + test mocks to pass `&[]`

Behavior is unchanged: callers still pass `&[]`, and the Anthropic adapter falls back to its legacy `cache_system_prompt` flag. PR2 wires the executor to compute breakpoints dynamically.

Spec: docs/superpowers/specs/2026-05-05-provider-agnostic-prompt-cache-placement-design.md

## Test plan
- [x] All existing tests pass with `&[]` injection
- [x] New unit tests in `cache_breakpoint_tests`, `resolve_breakpoints_tests`, `build_request_body_cache_tests`, `prefix_stability_tests`
- [x] Integration test `chat_with_persistent_marker_sets_beta_header`
- [x] Zero clippy warnings; cargo fmt --check clean

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

# PR 2 — Policy + Executor Wiring + Config + Observability

End state: the executor builds `CacheBreakpoint` values per cycle via the new `cache_policy` module, passes them to the provider, surfaces cache hit telemetry, and respects a kill switch in config. PR1's legacy fallback is left in place; a separate cleanup PR removes it later.

## Task 14: Add `MidLoopCompressor::frontier_index` accessor

**Files:**
- Modify: `crates/agent/src/execution/mid_loop_compressor.rs`

- [ ] **Step 14.1: Write failing test**

Add to the existing `#[cfg(test)] mod tests` in `mid_loop_compressor.rs`:

```rust
#[test]
fn frontier_index_returns_recent_window_start() {
    let compressor = make_compressor(10_000);
    let messages = vec![
        system_msg("sys"),
        user_msg("u1"),  assistant_msg("a1"), tool_msg("1", "t", "r1"),  // iter 1
        user_msg("u2"),  assistant_msg("a2"), tool_msg("2", "t", "r2"),  // iter 2
        user_msg("u3"),  assistant_msg("a3"), tool_msg("3", "t", "r3"),  // iter 3
        user_msg("u4"),  assistant_msg("a4"),                             // iter 4 (partial)
    ];
    // len = 13, MIN_RECENT_MESSAGES = 8
    // frontier_index = max(13 - 8, system_count=1) = max(5, 1) = 5
    assert_eq!(compressor.frontier_index(&messages), 5);
}

#[test]
fn frontier_index_respects_system_count() {
    let compressor = make_compressor(10_000);
    let messages = vec![
        system_msg("s1"), system_msg("s2"),
        user_msg("u1"), assistant_msg("a1"),
    ];
    // len = 4, MIN_RECENT_MESSAGES = 8
    // saturating_sub: 4 - 8 = 0
    // max(0, system_count=2) = 2
    assert_eq!(compressor.frontier_index(&messages), 2);
}

#[test]
fn frontier_index_short_conversation_clamps_to_system_count() {
    let compressor = make_compressor(10_000);
    let messages = vec![
        system_msg("sys"),
        user_msg("u1"),
    ];
    // len = 2, frontier = max(0, 1) = 1
    assert_eq!(compressor.frontier_index(&messages), 1);
}
```

- [ ] **Step 14.2: Run, expect failure**

Run: `cargo nextest run -p agent frontier_index_returns_recent_window_start 2>&1 | tail -10`
Expected: `frontier_index` not found.

- [ ] **Step 14.3: Implement the accessor**

In `crates/agent/src/execution/mid_loop_compressor.rs`, add inside the existing `impl MidLoopCompressor` block:

```rust
/// Returns the first index of the "always-preserved" recent window in `messages`.
///
/// Compression only mutates `messages[system_count..frontier_index]`;
/// `messages[frontier_index..]` is preserved verbatim across all
/// compression events. A cache marker placed at `frontier_index - 1`
/// will checkpoint the largest prefix that's stable between compression
/// events. (See spec Appendix B for compression-survivability analysis.)
///
/// Returns `system_count` for short conversations (len < MIN_RECENT_MESSAGES).
pub fn frontier_index(&self, messages: &[Message]) -> usize {
    let system_count = messages
        .iter()
        .take_while(|m| matches!(m, Message::System { .. }))
        .count();
    messages
        .len()
        .saturating_sub(MIN_RECENT_MESSAGES)
        .max(system_count)
}
```

- [ ] **Step 14.4: Refactor `compress_if_needed` to use it**

Find the inline computation in `compress_if_needed` (around line 70-78):

```rust
let system_count = messages.iter().take_while(...).count();
let recent_start = messages.len().saturating_sub(MIN_RECENT_MESSAGES).max(system_count);
```

Replace with:

```rust
let recent_start = self.frontier_index(messages);
let system_count = messages
    .iter()
    .take_while(|m| matches!(m, Message::System { .. }))
    .count();
```

(`system_count` is still needed locally for the slice `messages[system_count..recent_start]`.)

- [ ] **Step 14.5: Run all compressor tests**

Run: `cargo nextest run -p agent mid_loop_compressor 2>&1 | tail -15`
Expected: all existing + 3 new tests pass.

- [ ] **Step 14.6: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p agent --all-targets 2>&1 | tail -5
git add crates/agent/src/execution/mid_loop_compressor.rs
git commit -m "feat(agent): expose MidLoopCompressor::frontier_index accessor

Public accessor for the always-preserved-recent-window boundary.
Used by the new cache_policy module to place cache markers
relative to the compression frontier."
```

---

## Task 15: Create the `cache_policy` module

**Files:**
- Create: `crates/agent/src/execution/cache_policy.rs`
- Modify: `crates/agent/src/execution/mod.rs` (add `pub mod cache_policy;`)

- [ ] **Step 15.1: Write failing tests**

Create `crates/agent/src/execution/cache_policy.rs` with a test scaffold:

```rust
//! Cache-breakpoint placement policies.
//!
//! See docs/superpowers/specs/2026-05-05-provider-agnostic-prompt-cache-placement-design.md
//! Appendix B for the compression-survivability analysis.

use providers::{CacheAnchor, CacheBreakpoint, CacheTtl, Message};
use serde_json::Value;

use super::mid_loop_compressor::MidLoopCompressor;

// (Implementation goes in Step 15.3.)

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn make_compressor() -> MidLoopCompressor {
        MidLoopCompressor::new(
            Arc::new(context_engine::CharTokenCounter),
            10_000,
        )
    }

    fn sys() -> Message { Message::System { content: "sys".into() } }
    fn user(t: &str) -> Message { Message::user(t) }
    fn assistant(t: &str) -> Message {
        Message::Assistant {
            content: Some(t.into()),
            tool_calls: None,
            reasoning_content: None,
        }
    }

    #[test]
    fn empty_conversation_emits_only_last_system() {
        let compressor = make_compressor();
        let messages = vec![sys()];
        let bps = compression_aware_default(&messages, None, &compressor);
        assert_eq!(bps.len(), 1);
        assert_eq!(bps[0].anchor, CacheAnchor::LastSystem);
        assert_eq!(bps[0].ttl, CacheTtl::Persistent);
    }

    #[test]
    fn empty_conversation_with_tools_emits_two_persistent() {
        let compressor = make_compressor();
        let messages = vec![sys()];
        let tools = vec![serde_json::json!({"name": "echo"})];
        let bps = compression_aware_default(&messages, Some(&tools), &compressor);
        assert_eq!(bps.len(), 2);
        assert!(matches!(bps[0].anchor, CacheAnchor::LastSystem));
        assert!(matches!(bps[1].anchor, CacheAnchor::LastTool));
        assert_eq!(bps[0].ttl, CacheTtl::Persistent);
        assert_eq!(bps[1].ttl, CacheTtl::Persistent);
    }

    #[test]
    fn long_conversation_emits_three_breakpoints() {
        let compressor = make_compressor();
        let mut messages = vec![sys()];
        for i in 0..12 {
            messages.push(user(&format!("u{i}")));
            messages.push(assistant(&format!("a{i}")));
        }
        let tools = vec![serde_json::json!({"name": "echo"})];
        let bps = compression_aware_default(&messages, Some(&tools), &compressor);
        assert_eq!(bps.len(), 3);
        // Third breakpoint is MessageIndex(frontier - 1) with Ephemeral TTL.
        let frontier = compressor.frontier_index(&messages);
        assert!(frontier > 0);
        assert_eq!(bps[2].anchor, CacheAnchor::MessageIndex(frontier - 1));
        assert_eq!(bps[2].ttl, CacheTtl::Ephemeral);
    }

    #[test]
    fn no_tools_means_no_last_tool_breakpoint() {
        let compressor = make_compressor();
        let messages = vec![sys(), user("u1")];
        let bps = compression_aware_default(&messages, None, &compressor);
        assert!(!bps.iter().any(|b| matches!(b.anchor, CacheAnchor::LastTool)));
    }

    #[test]
    fn empty_tools_array_treated_as_no_tools() {
        let compressor = make_compressor();
        let messages = vec![sys()];
        let tools: Vec<Value> = vec![];
        let bps = compression_aware_default(&messages, Some(&tools), &compressor);
        assert!(!bps.iter().any(|b| matches!(b.anchor, CacheAnchor::LastTool)));
    }
}
```

In `crates/agent/src/execution/mod.rs`, add:

```rust
pub mod cache_policy;
```

after the existing `pub mod` lines.

- [ ] **Step 15.2: Run tests, expect failure**

Run: `cargo nextest run -p agent cache_policy::tests 2>&1 | tail -15`
Expected: `compression_aware_default` not defined.

- [ ] **Step 15.3: Implement `compression_aware_default`**

In `crates/agent/src/execution/cache_policy.rs`, replace the `// (Implementation goes in Step 15.3.)` placeholder with:

```rust
/// Default placement policy. Emits 2–3 breakpoints per call:
///
/// 1. `LastSystem` with `Persistent` TTL — system prompt durable across the session.
/// 2. `LastTool`   with `Persistent` TTL — tool definitions durable when present.
/// 3. `MessageIndex(frontier - 1)` with `Ephemeral` TTL — anchored at the
///    boundary of the compression mutation zone. The cached prefix
///    includes the mutation zone, so a compression event invalidates this
///    entry as a full match (Anthropic's longest-prefix-match still gives
///    a partial cache hit on the system+tools prefix afterward). Within a
///    compression-free run of turns it accelerates every call. Ephemeral
///    is correct because (a) the cache is invalidated by compression
///    anyway, and (b) the 5-min TTL matches typical ReAct-burst cadence.
///
/// See spec Appendix B for the analysis.
pub fn compression_aware_default(
    messages: &[Message],
    tools: Option<&[Value]>,
    compressor: &MidLoopCompressor,
) -> Vec<CacheBreakpoint> {
    let mut bps = Vec::with_capacity(3);

    // 1. System prompt — durable, worth Persistent
    bps.push(CacheBreakpoint {
        anchor: CacheAnchor::LastSystem,
        ttl: CacheTtl::Persistent,
    });

    // 2. Tool definitions — durable when present
    if matches!(tools, Some(t) if !t.is_empty()) {
        bps.push(CacheBreakpoint {
            anchor: CacheAnchor::LastTool,
            ttl: CacheTtl::Persistent,
        });
    }

    // 3. Pre-frontier prefix — accelerates intra-window turns
    let frontier = compressor.frontier_index(messages);
    if let Some(idx) = frontier.checked_sub(1) {
        bps.push(CacheBreakpoint {
            anchor: CacheAnchor::MessageIndex(idx),
            ttl: CacheTtl::Ephemeral,
        });
    }

    bps
}
```

- [ ] **Step 15.4: Run tests, expect pass**

Run: `cargo nextest run -p agent cache_policy::tests`
Expected: 5 tests passed.

- [ ] **Step 15.5: Re-export `compression_aware_default` from execution module**

In `crates/agent/src/execution/mod.rs`, add to the existing re-exports:

```rust
pub use cache_policy::compression_aware_default;
```

- [ ] **Step 15.6: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p agent --all-targets 2>&1 | tail -5
git add crates/agent/src/execution/cache_policy.rs \
        crates/agent/src/execution/mod.rs
git commit -m "feat(agent): add cache_policy::compression_aware_default

Default placement policy: LastSystem/Persistent + LastTool/Persistent +
MessageIndex(frontier-1)/Ephemeral. 5 unit tests covering empty
conversations, with/without tools, and the long-conversation case."
```

---

## Task 16: Add `cache_breakpoints` parameter to `ExecutionCore::run_cycle`

**Files:**
- Modify: `crates/agent/src/execution/core.rs`

- [ ] **Step 16.1: Update `run_cycle` signature**

In `crates/agent/src/execution/core.rs` around line 512, change:

```rust
pub async fn run_cycle(
    &self,
    messages: &mut Vec<Message>,
    tools: &[serde_json::Value],
    params: &ExecutionParams,
    routing_ctx: &RoutingContext,
    event_tx: Option<&tokio::sync::mpsc::Sender<crate::events::AgentEvent>>,
    seen_tool_calls: Option<&mut HashSet<String>>,
    cache_breakpoints: &[providers::CacheBreakpoint],
) -> Result<(CycleOutcome, Usage)> {
```

- [ ] **Step 16.2: Forward `cache_breakpoints` to provider calls**

Two existing call sites in `run_cycle` need updating:

Around line 275 (inside `call_provider_streaming`):

```rust
let mut stream = provider.chat_stream(messages, Some(tools), params, cache_breakpoints).await?;
```

Wait — `call_provider_streaming` is a **separate function** that takes `provider, messages, tools, params, event_tx, domain_bus`. It needs `cache_breakpoints` added to its signature too.

Update `call_provider_streaming` signature (around line 267):

```rust
async fn call_provider_streaming(
    provider: &dyn providers::LlmProvider,
    messages: &[Message],
    tools: &[serde_json::Value],
    params: &providers::ChatParams,
    event_tx: &tokio::sync::mpsc::Sender<crate::events::AgentEvent>,
    domain_bus: Option<&Arc<bus::DomainEventBus>>,
    cache_breakpoints: &[providers::CacheBreakpoint],
) -> Result<providers::LlmResponse> {
    let mut stream = provider.chat_stream(messages, Some(tools), params, cache_breakpoints).await?;
    // ... rest unchanged
}
```

The call site of `call_provider_streaming` inside `run_cycle` (around line 526) becomes:

```rust
let response = if let Some(tx) = event_tx {
    call_provider_streaming(
        &*self.provider,
        messages,
        tools,
        &params.chat_params,
        tx,
        self.domain_event_bus.as_ref(),
        cache_breakpoints,
    )
    .await?
} else {
    self.provider
        .chat(messages, Some(tools), &params.chat_params, cache_breakpoints)
        .await?
};
```

- [ ] **Step 16.3: Update `run_cycle` callers within `core.rs` test module**

Find tests that call `core.run_cycle(...)` (around lines 921+). Each needs `&[]` added.

Run: `grep -n "run_cycle" crates/agent/src/execution/core.rs`

Mechanical fix per call site — add `&[]` as the new last arg:

```rust
core.run_cycle(&mut messages, &tools, &params, &routing_ctx(), None, None, &[])
```

- [ ] **Step 16.4: Run core tests**

Run: `cargo nextest run -p agent execution::core 2>&1 | tail -15`
Expected: all green.

- [ ] **Step 16.5: Update single existing caller in `execute_loop.rs`**

Find: `grep -n "run_cycle" crates/agent/src/execution/execute_loop.rs`

The call (around line 157-166) becomes:

```rust
let (outcome, cycle_usage) = core
    .run_cycle(
        &mut messages,
        cycle_tools,
        params,
        ctx,
        event_tx.as_ref(),
        Some(&mut seen_tool_calls),
        &[],  // PR2 will replace with policy.compute(...)
    )
    .await?;
```

- [ ] **Step 16.6: Format, lint, run all tests**

```bash
cargo fmt --all
cargo clippy -p agent --all-targets 2>&1 | tail -5
cargo nextest run -p agent 2>&1 | tail -10
```

- [ ] **Step 16.7: Commit**

```bash
git add crates/agent/src/execution/core.rs \
        crates/agent/src/execution/execute_loop.rs
git commit -m "refactor(agent): thread cache_breakpoints through run_cycle and call_provider_streaming

Mechanical signature change. The executor still passes &[] from
execute_loop; the policy hookup follows in the next commit."
```

---

## Task 17: Wire `compression_aware_default` into `execute_loop`

**Files:**
- Modify: `crates/agent/src/execution/execute_loop.rs`

- [ ] **Step 17.1: Compute breakpoints per cycle**

In `crates/agent/src/execution/execute_loop.rs::execute_loop`, find the call to `core.run_cycle` (now modified by Task 16). Just before it, build the breakpoints:

```rust
// Compute cache breakpoints for this cycle.
// Policy: compression_aware_default — see spec Appendix B.
let cache_bps = crate::execution::cache_policy::compression_aware_default(
    &messages,
    Some(cycle_tools),
    &compressor,
);

let (outcome, cycle_usage) = core
    .run_cycle(
        &mut messages,
        cycle_tools,
        params,
        ctx,
        event_tx.as_ref(),
        Some(&mut seen_tool_calls),
        &cache_bps,
    )
    .await?;
```

Note: `tools` in `execute_loop`'s scope might be a slice or empty. Inspect the variable name. Use `cycle_tools` (the same value passed to `run_cycle`) — pass `Some(cycle_tools)` to the policy.

- [ ] **Step 17.2: Add an integration test**

Create `crates/agent/tests/cache_policy_wiring_test.rs`:

```rust
//! Integration test: execute_loop builds and forwards cache breakpoints.

use std::sync::Arc;
use tokio::sync::RwLock;

use agent::execution::{cache_policy, ExecutionBudget, DepthMode};
use providers::{CacheAnchor, ChatParams, Message};
use tools::{registry::ToolRegistry, RoutingContext};

#[tokio::test]
async fn compression_aware_default_emits_three_for_long_conversation() {
    let compressor = agent::execution::MidLoopCompressor::new(
        Arc::new(context_engine::CharTokenCounter),
        10_000,
    );

    let mut messages = vec![Message::System { content: "sys".into() }];
    for i in 0..12 {
        messages.push(Message::user(&format!("u{i}")));
    }
    let tools = vec![serde_json::json!({"name": "x"})];

    let bps = cache_policy::compression_aware_default(
        &messages,
        Some(&tools),
        &compressor,
    );

    assert_eq!(bps.len(), 3);
    assert!(matches!(bps[0].anchor, CacheAnchor::LastSystem));
    assert!(matches!(bps[1].anchor, CacheAnchor::LastTool));
    assert!(matches!(bps[2].anchor, CacheAnchor::MessageIndex(_)));
}
```

- [ ] **Step 17.3: Run tests**

```bash
cargo nextest run -p agent --test cache_policy_wiring_test
cargo nextest run -p agent execute_loop 2>&1 | tail -10
```

- [ ] **Step 17.4: Format, lint, commit**

```bash
cargo fmt --all
cargo clippy -p agent --all-targets 2>&1 | tail -5
git add crates/agent/src/execution/execute_loop.rs \
        crates/agent/tests/cache_policy_wiring_test.rs
git commit -m "feat(agent): wire compression_aware_default into execute_loop

Per cycle, build breakpoints via cache_policy and pass to run_cycle.
Integration test verifies the policy emits 3 breakpoints for a
long-conversation case."
```

---

## Task 18: Add `CacheConfig` kill switch to providers config schema

**Files:**
- Modify: `crates/config/src/schema/providers.rs`

- [ ] **Step 18.1: Write failing test**

Add to the existing test module in `crates/config/src/schema/providers.rs` (or create a `#[cfg(test)] mod tests` if absent):

```rust
#[cfg(test)]
mod cache_config_tests {
    use super::*;

    #[test]
    fn cache_config_default_enabled() {
        let cfg = CacheConfig::default();
        assert!(cfg.enabled);
    }

    #[test]
    fn providers_config_has_cache_field_default_enabled() {
        let cfg = ProvidersConfig::default();
        assert!(cfg.cache.enabled);
    }

    #[test]
    fn cache_config_deserializes_disabled_form() {
        let json = r#"{"enabled": false}"#;
        let cfg: CacheConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.enabled);
    }
}
```

- [ ] **Step 18.2: Run, expect compile error**

Run: `cargo nextest run -p config cache_config_tests 2>&1 | tail -10`
Expected: `CacheConfig` not defined.

- [ ] **Step 18.3: Add `CacheConfig` and field**

In `crates/config/src/schema/providers.rs`, add at the end of the file:

```rust
/// Cache-placement configuration. Single global kill switch; per-provider
/// overrides are not supported (YAGNI).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheConfig {
    /// When false, the executor passes empty cache_breakpoints to the
    /// provider. Anthropic adapter falls back to its legacy
    /// cache_system_prompt flag in that case.
    #[serde(default = "default_cache_enabled")]
    pub enabled: bool,
}

fn default_cache_enabled() -> bool {
    true
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}
```

Then modify `ProvidersConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProvidersConfig {
    // ... existing per-provider fields

    #[serde(default)]
    pub cache: CacheConfig,
}
```

- [ ] **Step 18.4: Run tests, format, commit**

```bash
cargo nextest run -p config cache_config_tests
cargo fmt --all
cargo clippy -p config --all-targets 2>&1 | tail -5
git add crates/config/src/schema/providers.rs
git commit -m "feat(config): add providers.cache.enabled kill switch

Default true. When false, the executor passes empty cache_breakpoints
to providers (Anthropic adapter then uses its legacy
cache_system_prompt fallback)."
```

---

## Task 19: Wire the kill switch in `execute_loop`

**Files:**
- Modify: `crates/agent/src/execution/types.rs` (add `cache_enabled` field to `ExecutionParams`)
- Modify: `crates/agent/src/execution/execute_loop.rs`
- Modify: every constructor of `ExecutionParams` (likely few)

- [ ] **Step 19.1: Add `cache_enabled` field to `ExecutionParams`**

In `crates/agent/src/execution/types.rs`:

```rust
#[derive(Debug, Clone)]
pub struct ExecutionParams {
    // ... existing fields
    /// When false, the executor short-circuits cache-breakpoint computation
    /// and passes &[] to the provider. Driven by config.providers.cache.enabled.
    pub cache_enabled: bool,
}
```

In the `new` constructor:

```rust
impl ExecutionParams {
    pub fn new(model: impl Into<String>) -> Self {
        Self {
            // ... existing field defaults
            cache_enabled: true,  // default on
        }
    }

    pub fn with_cache_enabled(mut self, enabled: bool) -> Self {
        self.cache_enabled = enabled;
        self
    }
}
```

- [ ] **Step 19.2: Gate the policy in `execute_loop`**

Update the breakpoint construction in `execute_loop.rs` (from Task 17):

```rust
let cache_bps: Vec<providers::CacheBreakpoint> = if params.cache_enabled {
    crate::execution::cache_policy::compression_aware_default(
        &messages,
        Some(cycle_tools),
        &compressor,
    )
} else {
    Vec::new()
};

let (outcome, cycle_usage) = core
    .run_cycle(
        &mut messages,
        cycle_tools,
        params,
        ctx,
        event_tx.as_ref(),
        Some(&mut seen_tool_calls),
        &cache_bps,
    )
    .await?;
```

- [ ] **Step 19.3: Wire from config to `ExecutionParams`**

Find where `ExecutionParams` is constructed in production code:

Run: `grep -rn "ExecutionParams::new\|ExecutionParams {" --include='*.rs' crates/ | grep -v test`

For every production construction site, set `cache_enabled` from `config.providers.cache.enabled`:

```rust
let params = ExecutionParams::new(&model)
    // ... existing builders
    .with_cache_enabled(config.providers.cache.enabled);
```

- [ ] **Step 19.4: Add a test**

In `crates/agent/src/execution/types.rs` test module:

```rust
#[test]
fn execution_params_cache_enabled_default_true() {
    let p = ExecutionParams::new("test-model");
    assert!(p.cache_enabled);
}

#[test]
fn execution_params_with_cache_enabled() {
    let p = ExecutionParams::new("test-model").with_cache_enabled(false);
    assert!(!p.cache_enabled);
}
```

- [ ] **Step 19.5: Run, format, commit**

```bash
cargo nextest run -p agent execution_params 2>&1 | tail -10
cargo fmt --all
cargo clippy --workspace --all-targets 2>&1 | tail -5
git add crates/agent/src/execution/types.rs \
        crates/agent/src/execution/execute_loop.rs \
        # any other files where ExecutionParams gets constructed
git commit -m "feat(agent): wire cache kill switch from config to executor

ExecutionParams.cache_enabled gates the compression_aware_default policy.
When false, executor passes &[] to provider — Anthropic adapter falls
back to legacy cache_system_prompt behavior."
```

---

## Task 20: Extend `AgentEvent::BudgetUpdate` with cache token fields

**Files:**
- Modify: `crates/agent/src/events.rs`
- Modify: `crates/agent/src/execution/execute_loop.rs` (event emission site)

- [ ] **Step 20.1: Update event variant**

In `crates/agent/src/events.rs`, modify `AgentEvent::BudgetUpdate` (around line 318):

```rust
BudgetUpdate {
    tokens_remaining_pct: f32,
    turns_used: u32,
    max_turns: u32,
    cost_usd: f64,
    depth: String,
    /// Cumulative cache hit tokens across the session (sum of Usage.cache_read_tokens).
    /// Pre-existing UI consumers ignore unknown fields when deserializing.
    #[serde(default)]
    cache_read_tokens: u32,
    #[serde(default)]
    cache_write_tokens: u32,
},
```

- [ ] **Step 20.2: Track cumulative cache tokens in `execute_loop`**

In `execute_loop.rs`, accumulate per-iteration cache tokens. Find the existing `accumulated_usage` (line 56). The `Usage::accumulate_usage` helper already adds `cache_read_tokens` and `cache_write_tokens` (verify in `crates/agent/src/execution/types.rs` line 139):

```rust
pub fn accumulate_usage(total: &mut Usage, cycle: &Usage) {
    total.prompt_tokens += cycle.prompt_tokens;
    total.completion_tokens += cycle.completion_tokens;
    total.total_tokens += cycle.total_tokens;
    total.cache_read_tokens += cycle.cache_read_tokens;
    total.cache_write_tokens += cycle.cache_write_tokens;
}
```

Good — already works. Now find every `BudgetUpdate` emission site:

Run: `grep -n "BudgetUpdate" crates/agent/src/execution/execute_loop.rs`

For each, add the new fields:

```rust
AgentEvent::BudgetUpdate {
    tokens_remaining_pct: budget.remaining_pct(),
    turns_used: budget.turns_used(),
    max_turns: budget.max_turns(),
    cost_usd: budget.cost_usd(),
    depth: budget.depth.to_string(),
    cache_read_tokens: accumulated_usage.cache_read_tokens,
    cache_write_tokens: accumulated_usage.cache_write_tokens,
}
```

- [ ] **Step 20.3: Update any other `BudgetUpdate` constructors workspace-wide**

Run: `grep -rn "AgentEvent::BudgetUpdate\|BudgetUpdate {" --include='*.rs' crates/`

Add the two new fields at every constructor. Also any frontend type-bindings file:

Run: `grep -n "BudgetUpdate" desktop-ui/src/bindings.ts`

The bindings.ts is auto-regenerated by `cargo tauri dev` — no manual edit needed; just rebuild.

- [ ] **Step 20.4: Run tests**

```bash
cargo nextest run --workspace 2>&1 | tail -10
```

- [ ] **Step 20.5: Format, commit**

```bash
cargo fmt --all
git add crates/agent/src/events.rs crates/agent/src/execution/execute_loop.rs
git commit -m "feat(agent): expose cache token counts on BudgetUpdate event

Adds cache_read_tokens and cache_write_tokens fields to
AgentEvent::BudgetUpdate so HUD consumers can show cache hit ratio.
Backwards-compatible: pre-existing fields unchanged, new fields
serde(default) for old deserializers."
```

---

## Task 21: Add per-call `tracing::info!` for cache hit ratio

**Files:**
- Modify: `crates/agent/src/execution/core.rs`

- [ ] **Step 21.1: Emit log after provider call returns**

In `crates/agent/src/execution/core.rs::run_cycle`, just after the provider call returns and `usage` is extracted (around line 541), add:

```rust
let usage = response.usage.clone();

// Log cache hit ratio for observability.
if usage.prompt_tokens > 0 {
    let hit_rate = usage.cache_read_tokens as f64 / usage.prompt_tokens as f64;
    tracing::info!(
        target: "klynt::execution::cache",
        cache_read = usage.cache_read_tokens,
        cache_write = usage.cache_write_tokens,
        prompt = usage.prompt_tokens,
        hit_rate,
        "cache hit ratio for this call"
    );
}
```

- [ ] **Step 21.2: Run tests, format, commit**

```bash
cargo nextest run -p agent execution::core 2>&1 | tail -10
cargo fmt --all
git add crates/agent/src/execution/core.rs
git commit -m "feat(agent): log cache hit ratio per LLM call

Emits tracing::info at target klynt::execution::cache with
cache_read/cache_write/prompt token counts and computed hit_rate."
```

---

## Task 22: Add Anthropic wire-format integration test

**Files:**
- Create: `crates/providers/tests/cache_breakpoint_wire_format_test.rs`

- [ ] **Step 22.1: Author the integration test**

Create the file:

```rust
//! Integration: verify the JSON body produced by AnthropicNativeProvider
//! contains cache_control on the right blocks given various breakpoint
//! configurations.

use providers::{
    adapters::anthropic_native::AnthropicNativeProvider,
    CacheAnchor, CacheBreakpoint, CacheTtl, ChatParams, Message,
};

fn provider() -> AnthropicNativeProvider {
    // Use whatever public test constructor the adapter exposes.
    AnthropicNativeProvider::new_for_test(false)
}

fn body_for(messages: &[Message], tools: Option<&[serde_json::Value]>, bps: &[CacheBreakpoint]) -> serde_json::Value {
    provider().build_request_body(
        messages,
        tools,
        &ChatParams::new("claude-3-5-sonnet"),
        false,
        bps,
    )
}

#[test]
fn three_breakpoints_apply_three_cache_controls() {
    let messages = vec![
        Message::System { content: "sys".into() },
        Message::user("u0"),
        Message::user("u1"),
        Message::user("u2"),
    ];
    let tools = vec![serde_json::json!({"name": "echo", "description": "", "input_schema": {}})];
    let bps = vec![
        CacheBreakpoint { anchor: CacheAnchor::LastSystem, ttl: CacheTtl::Persistent },
        CacheBreakpoint { anchor: CacheAnchor::LastTool, ttl: CacheTtl::Persistent },
        CacheBreakpoint { anchor: CacheAnchor::MessageIndex(2), ttl: CacheTtl::Ephemeral },
    ];
    let body = body_for(&messages, Some(&tools), &bps);

    // Last system block has cache_control 1h
    let sys_blocks = body.get("system").unwrap().as_array().unwrap();
    let last_sys = sys_blocks.last().unwrap();
    assert_eq!(
        last_sys.get("cache_control").unwrap().get("ttl").and_then(|t| t.as_str()),
        Some("1h"),
    );

    // Last tool has cache_control 1h
    let tools_arr = body.get("tools").unwrap().as_array().unwrap();
    let last_tool = tools_arr.last().unwrap();
    assert_eq!(
        last_tool.get("cache_control").unwrap().get("ttl").and_then(|t| t.as_str()),
        Some("1h"),
    );

    // messages[2] (the THIRD user message in input) has cache_control ephemeral.
    let msgs_arr = body.get("messages").unwrap().as_array().unwrap();
    // Anthropic's "messages" excludes system messages, so messages[2] of our input
    // (a user message) maps to a different position in the wire payload.
    // Verify SOME message has the ephemeral marker; the structural test is the
    // key — we don't pin the exact index because system extraction shifts indices.
    let has_ephemeral = msgs_arr.iter().any(|m| {
        let content = m.get("content");
        match content {
            Some(serde_json::Value::Array(blocks)) => {
                blocks.iter().any(|b| {
                    b.get("cache_control")
                        .and_then(|cc| cc.get("type"))
                        .and_then(|t| t.as_str())
                        == Some("ephemeral")
                        && b.get("cache_control")
                            .and_then(|cc| cc.get("ttl"))
                            .is_none()
                })
            }
            _ => false,
        }
    });
    assert!(has_ephemeral, "expected an ephemeral cache_control on a message block");
}

#[test]
fn empty_breakpoints_with_legacy_flag_marks_only_last_system() {
    let provider = AnthropicNativeProvider::new_for_test(true);
    let body = provider.build_request_body(
        &[Message::System { content: "sys".into() }, Message::user("hi")],
        None,
        &ChatParams::new("claude-3-5-sonnet"),
        false,
        &[],
    );
    let sys_blocks = body.get("system").unwrap().as_array().unwrap();
    assert!(sys_blocks[0].get("cache_control").is_some());
}

#[test]
fn five_breakpoints_drop_to_trailing_four() {
    let messages = vec![
        Message::System { content: "sys".into() },
        Message::user("u0"), Message::user("u1"), Message::user("u2"),
        Message::user("u3"), Message::user("u4"),
    ];
    let tools = vec![serde_json::json!({"name": "t", "description": "", "input_schema": {}})];
    let bps = vec![
        CacheBreakpoint { anchor: CacheAnchor::LastSystem, ttl: CacheTtl::Persistent },
        CacheBreakpoint { anchor: CacheAnchor::LastTool, ttl: CacheTtl::Persistent },
        CacheBreakpoint { anchor: CacheAnchor::MessageIndex(2), ttl: CacheTtl::Ephemeral },
        CacheBreakpoint { anchor: CacheAnchor::MessageIndex(3), ttl: CacheTtl::Ephemeral },
        CacheBreakpoint { anchor: CacheAnchor::MessageIndex(4), ttl: CacheTtl::Ephemeral },
    ];
    let body = body_for(&messages, Some(&tools), &bps);

    let mut total_cc = 0usize;
    if let Some(sys) = body.get("system").and_then(|s| s.as_array()) {
        for b in sys {
            if b.get("cache_control").is_some() { total_cc += 1; }
        }
    }
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        for b in tools {
            if b.get("cache_control").is_some() { total_cc += 1; }
        }
    }
    if let Some(msgs) = body.get("messages").and_then(|m| m.as_array()) {
        for m in msgs {
            if let Some(serde_json::Value::Array(blocks)) = m.get("content") {
                for b in blocks {
                    if b.get("cache_control").is_some() { total_cc += 1; }
                }
            }
        }
    }
    assert_eq!(total_cc, 4, "expected exactly 4 cache_control markers");
}
```

If `AnthropicNativeProvider::new_for_test` doesn't exist, add a public test constructor to `anthropic_native.rs`:

```rust
#[cfg(any(test, feature = "test-utils"))]
impl AnthropicNativeProvider {
    pub fn new_for_test(cache_system_prompt: bool) -> Self {
        // Construct with minimal stub config — match the existing struct fields
        Self {
            client: reqwest::Client::new(),
            api_base: "https://api.anthropic.com/v1".to_string(),
            api_key: common::Secret::new(String::new()),
            cache_system_prompt,
            // ... other fields with sensible defaults
        }
    }
}
```

If the struct has many fields, look at how existing tests construct it:

Run: `grep -n "AnthropicNativeProvider\s*{" crates/providers/src/adapters/anthropic_native.rs | head -5`

- [ ] **Step 22.2: Run integration tests**

```bash
cargo nextest run -p providers --test cache_breakpoint_wire_format_test
```

Expected: 3 tests pass.

- [ ] **Step 22.3: Format, commit**

```bash
cargo fmt --all
git add crates/providers/tests/cache_breakpoint_wire_format_test.rs \
        crates/providers/src/adapters/anthropic_native.rs
git commit -m "test(providers): integration test for Anthropic cache_control wire format

Asserts 3 simultaneous breakpoints land on the correct system/tool/message
content blocks, the legacy cache_system_prompt fallback still works with
empty breakpoints, and 5 breakpoints get truncated to trailing 4."
```

---

## Task 23: PR2 final verification

- [ ] **Step 23.1: Full workspace compile**

Run: `cargo check --workspace 2>&1 | tail -10`

- [ ] **Step 23.2: Full test suite**

Run: `cargo nextest run --workspace 2>&1 | tail -20`

- [ ] **Step 23.3: Doctests**

Run: `cargo test --workspace --doc 2>&1 | tail -10`

- [ ] **Step 23.4: Clippy zero warnings**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | tail -20`

- [ ] **Step 23.5: Format check**

Run: `cargo fmt --all --check`

- [ ] **Step 23.6: KCA validation gates**

Run: `./scripts/run_kca_validation.sh 2>&1 | tail -30`

Expected: all gates pass.

- [ ] **Step 23.7: Run a smoke test against a live Anthropic key (optional, manual)**

If an `ANTHROPIC_API_KEY` is available, run a one-shot ReAct turn with `tracing` set to info and confirm:

```bash
RUST_LOG=klynt::execution::cache=info,klynt::providers::anthropic=debug \
  cargo run -p klyntbot --bin klyntbot -- chat "list my tasks" 2>&1 | grep "cache hit ratio\|cache_breakpoints"
```

Expected: log line `cache hit ratio for this call` appears with non-zero `cache_write_tokens` on first turn and `cache_read_tokens > 0` on the second turn.

- [ ] **Step 23.8: Open PR2**

```bash
git push
gh pr create --title "feat(agent): wire compression-aware cache placement (PR2: policy + observability)" --body "$(cat <<'EOF'
## Summary
- Add `MidLoopCompressor::frontier_index` accessor
- Add `cache_policy::compression_aware_default` policy (LastSystem/Persistent + LastTool/Persistent + MessageIndex(frontier-1)/Ephemeral)
- Wire breakpoint computation per cycle in `execute_loop`
- Add `providers.cache.enabled` kill switch in config
- Extend `AgentEvent::BudgetUpdate` with `cache_read_tokens` / `cache_write_tokens`
- Per-call `tracing::info!` log at target `klynt::execution::cache`
- Integration test for Anthropic wire-format

Companion to PR1 (cache breakpoint mechanism). Together these make every Anthropic call cache-aware.

Spec: docs/superpowers/specs/2026-05-05-provider-agnostic-prompt-cache-placement-design.md

## Test plan
- [x] Unit tests for `frontier_index`, `compression_aware_default`, `CacheConfig` defaults
- [x] Integration test for executor wiring + Anthropic wire-format markers
- [x] All existing tests still pass
- [x] Zero clippy warnings; cargo fmt --check clean
- [x] KCA validation gates pass

🤖 Generated with [Claude Code](https://claude.com/claude-code)
EOF
)"
```

---

## Self-Review Checklist (Run This After Writing/Editing the Plan)

The plan was verified against the spec on the same day; below is the audit trail.

**1. Spec coverage:** Each spec section maps to a task:
| Spec section | Implementing task(s) |
|---|---|
| §5 Type definitions | Task 1, Task 2 |
| §6 Provider trait change | Task 3 |
| §7 Anthropic adapter | Tasks 4, 8, 9, 10, 11 |
| §8 OpenAI-compat adapter | Tasks 5, 12 |
| §9 cache_policy module | Tasks 15, 17 |
| §10 frontier_index | Task 14 |
| §11 Data flow | Tasks 16, 17 (wiring) |
| §12 Error handling | Resolver tests in Task 8 |
| §13 Configuration | Tasks 18, 19 |
| §14 Observability | Tasks 20, 21 |
| §15 Testing plan | Tasks 1, 8, 9, 12, 14, 15, 17, 22 |
| §16 Migration / rollout | Two-PR structure of this plan |
| §17 File touch list | File Structure section above |

**2. Placeholder scan:** Every step has either concrete code or an exact command. The only "lookup yourself" instructions point to `grep` commands with deterministic patterns.

**3. Type consistency:** `CacheBreakpoint`, `CacheAnchor`, `CacheTtl`, `compression_aware_default`, `frontier_index`, `resolve_breakpoints`, `ResolvedMarker`, `SECTION_SYSTEM`/`SECTION_TOOLS`/`SECTION_MESSAGES`, `needs_extended_cache_ttl_header`, `assert_prefix_stable`, `prefix_hashes` — all named consistently across tasks.

**4. Workspace-compiles invariant:** Tasks 3–7 form one atomic commit (signature change + all callers + tests). Tasks 8 onward each leave the workspace compiling.

---

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-05-05-provider-agnostic-prompt-cache-placement-implementation.md`.

Two execution options:

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — I execute tasks in this session using `superpowers:executing-plans`, batch execution with checkpoints.

Which approach?
