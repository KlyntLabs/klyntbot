# Provider-Agnostic Compression-Aware Prompt-Cache Placement

- **Status:** Draft (approved in brainstorm 2026-05-05)
- **Author:** Jayden + Claude (Opus 4.7, explanatory mode)
- **Type:** Design / Spec
- **Scope:** `crates/providers`, `crates/agent/src/execution`, `crates/config`
- **Estimated effort:** 1.5–2 days, ~600 LOC net change including tests
- **Implementation handoff:** writing-plans skill

---

## 1. Motivation

Klynt's ReAct loop today does prompt-cache placement **only** in the Anthropic adapter, only on the last `System` block, and only when a hardcoded `cache_system_prompt: bool` flag is true (`crates/providers/src/anthropic_native.rs:452-465`). This misses three categories of savings:

1. **Tool-definition caching.** The tools array is large (Klynt registers 20+ tools), stable across a session, and currently uncached on every Anthropic request.
2. **Conversation-prefix caching.** Early-turn user/assistant/tool messages are stable but never marked, so Anthropic re-tokenizes them on every iteration.
3. **Compression-aware placement.** `MidLoopCompressor` rewrites older `Tool` results in place. Today this silently invalidates any prior server-side cache hit on **all** providers (Anthropic and OpenAI-style alike). Two of our three default markers — `LastSystem` and `LastTool` — are placed *outside* the mutation zone by construction, so they survive compression unconditionally. The third marker (at the compression frontier) does *not* survive a compression event, but it accelerates every call between compressions and benefits from Anthropic's longest-prefix-match on partially-cached prefixes after a compression. See Appendix B for the analysis.

We are pre-release. This is the right time to fix it for **every** provider, not just Anthropic.

The reference implementation we benchmarked against — `opencode` (`/Users/jayden/Projects/Klynt/opencode/internal/llm/provider/anthropic.go:135-139`) — uses static markers on the last 2 messages plus the last tool. That's a useful baseline but doesn't account for compression. We can do better.

## 2. Goals and non-goals

### Goals

1. **Provider-agnostic API.** Every `LlmProvider` implementation accepts the same cache-breakpoint metadata. Anthropic acts on it; OpenAI-compatible providers ignore it (relying on server-side automatic prefix caching) and validate prefix stability in debug builds.
2. **Executor-driven dynamic placement.** The executor — not the provider — decides where to mark each call, because the executor is the only layer that knows about compression boundaries.
3. **Compression-aware placement.** The default policy places `LastSystem` and `LastTool` markers — both outside the compression mutation zone — so the system+tools prefix stays cache-warm unconditionally across compression events. A third frontier-anchored marker accelerates intra-compression-window turns. See Appendix B.
4. **Two TTLs.** Support both Anthropic ephemeral (~5 min) and persistent (~1 h via `extended-cache-ttl-2025-04-11`) caches; pick TTL by anchor type so durable prefixes get longer life.
5. **Observable.** Surface `cache_read_tokens / total_input_tokens` ratio per call via existing `tracing` and `AgentEvent::BudgetUpdate` paths.
6. **Safe migration.** Existing call sites that pass no breakpoints fall back to today's behavior (last-system-block ephemeral). No big-bang cutover.

### Non-goals

1. **Caching for non-Anthropic providers via explicit headers.** OpenAI/Gemini/DeepSeek do this server-side based on prefix-byte equality; the client doesn't speak markers to them.
2. **Persistence of cache state across process restarts.** Anthropic and OpenAI both manage cache lifetime server-side. We don't replicate it.
3. **A separate cache layer at the application level.** No L1 / L2 caches in Klynt itself. We're driving server-side caches; that's the entire feature.
4. **Adaptive / cache-feedback policies.** We document where they could plug in but don't ship them in v1.
5. **Per-provider configuration knobs.** One global kill switch. YAGNI.

## 3. Decisions locked in (from brainstorm 2026-05-05)

| Concern | Decision |
|---|---|
| Placement strategy | **(D)** executor-driven dynamic |
| API surface | **(C)** parallel `Vec<CacheBreakpoint>` parameter on `chat()` / `chat_stream()` |
| Default policy | **(B)** compression-aware: emits 2–3 breakpoints per call |
| TTL coverage | **(B)** both `Ephemeral` and `Persistent` |
| Non-Anthropic behavior | **(Q)** silent no-op + debug-only prefix-stability hash check |
| 1-hour beta header | Per-request, only when any `Persistent` breakpoint is present |
| Legacy `cache_system_prompt` | Keep as fallback when `cache_breakpoints == &[]`; remove in a follow-up PR after 1–2 weeks of stability |
| Anthropic 4-marker limit | Sort by absolute position, **keep trailing 4** (later positions imply earlier ones) |
| Frontier marker placement | On `messages[frontier_index - 1]` (the last pre-frontier message that compression won't touch) |

## 4. Architecture

```
┌──────────────────────────────────────────────────────────────┐
│  EXECUTOR  (crates/agent/src/execution/)                     │
│                                                              │
│  execute_loop ──▶ run_cycle                                  │
│       │                                                      │
│       │  per turn:                                           │
│       │   1. cache_policy::compression_aware_default(...)    │
│       │   2. core.run_cycle(... &bps)                        │
│       │                                                      │
│  cache_policy.rs                  ◀── NEW MODULE             │
│  ├─ compression_aware_default()                              │
│                                                              │
│  mid_loop_compressor.rs                                      │
│  └─ frontier_index(messages) -> usize  ◀── NEW ACCESSOR      │
└──────────────────────────────────────────────────────────────┘
                            │
                            ▼ &[CacheBreakpoint]
┌──────────────────────────────────────────────────────────────┐
│  PROVIDERS  (crates/providers/)                              │
│                                                              │
│  trait LlmProvider {                                         │
│    async fn chat(.., bps: &[CacheBreakpoint]) -> ...         │
│    async fn chat_stream(.., bps: &[CacheBreakpoint]) -> ...  │
│  }                                                           │
│                                                              │
│  AnthropicNativeProvider                                     │
│    ├─ resolve_breakpoints(messages, tools, bps)              │
│    ├─ sort + keep trailing 4                                 │
│    ├─ apply cache_control:{type,ttl}                         │
│    └─ optionally add anthropic-beta:extended-cache-ttl-      │
│       2025-04-11 header (if any Persistent)                  │
│                                                              │
│  OpenAiCompatProvider                                        │
│    ├─ release: no-op                                         │
│    └─ debug:    DashMap<(channel,chat_id),u64> prefix-hash   │
│                 + tracing::warn! on mismatch                 │
└──────────────────────────────────────────────────────────────┘
```

Two crates change. Feature crates, tools, MCP layer, UI: untouched.

## 5. Type definitions (`crates/providers/src/types.rs`)

```rust
/// Cache lifetime hint. Picked by the policy that emits the breakpoint;
/// honored by providers that support explicit cache-control markers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheTtl {
    /// ~5 minutes. Anthropic default. Right for transient prefixes
    /// (e.g. the message-frontier marker that survives one compression
    /// burst but probably won't be reused tomorrow).
    Ephemeral,
    /// ~1 hour. Anthropic via `extended-cache-ttl-2025-04-11` beta.
    /// Right for system prompts and tool definitions that are stable
    /// for the whole session and likely reused after lunch breaks.
    Persistent,
}

impl Default for CacheTtl {
    fn default() -> Self { Self::Ephemeral }
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

`ProviderCapabilities` gains one bit:

```rust
pub struct ProviderCapabilities {
    // … existing fields
    pub prompt_caching: bool,           // already exists
    pub explicit_cache_markers: bool,   // NEW: true for Anthropic, false elsewhere
}
```

The new bit is informational. We expose it for future telemetry and for the OpenAI-compat adapter's debug assertion to skip work cleanly when it's `false`.

## 6. Provider trait change

```rust
#[async_trait]
pub trait LlmProvider: Send + Sync {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],   // NEW
    ) -> Result<LlmResponse>;

    async fn chat_stream(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],   // NEW
    ) -> Result<BoxStream<'_, Result<LlmStreamChunk>>> {
        // Default impl wraps `chat` (unchanged behavior). Adapters that
        // stream natively override.
    }
}
```

**Migration discipline.** Every existing call site must compile with this change. The mechanical fix is to add `&[]` as the last argument. Test fixtures, sub-agent dispatches, mock providers, the simulator — all get `&[]`. The executor is the only caller that builds a real breakpoints vec.

## 7. Anthropic adapter (`crates/providers/src/anthropic_native.rs`)

The `build_request_body` function gains a `cache_breakpoints` parameter and applies markers in three steps:

```rust
fn build_request_body(
    &self,
    messages: &[Message],
    tools: Option<&[Value]>,
    params: &ChatParams,
    cache_breakpoints: &[CacheBreakpoint],
) -> Value {
    // 1) Synthesize fallback if caller passed no breakpoints AND legacy flag is on.
    //    Logged at debug level so we can see when the fallback fires during rollout.
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

    // 2) Resolve anchors → concrete (block_kind, payload_position, ttl) tuples.
    let mut resolved = resolve_breakpoints(messages, tools, bps);

    // 3) Anthropic permits at most 4 cache_control blocks per request.
    //    Caching at position N implicitly caches everything before N, so
    //    later positions dominate earlier ones. When in doubt, drop the
    //    earliest. (Stable sort by absolute payload position; keep trailing 4.)
    resolved.sort_by_key(|r| r.absolute_position);
    if resolved.len() > 4 {
        resolved.drain(..resolved.len() - 4);
    }

    // 4) Build payload, injecting cache_control on the resolved blocks.
    //    Inject `anthropic-beta: extended-cache-ttl-2025-04-11` header iff
    //    any resolved breakpoint has CacheTtl::Persistent.
    // …
}
```

`absolute_position` is a private ordering helper: tools come before all messages in the cache-key-impact sense (tools form part of the system prefix that Anthropic hashes), so a tool-anchored marker at position `tool_count - 1` sorts as `(SECTION_TOOLS, tool_count - 1)`, and a message-anchored marker at `messages[k]` sorts as `(SECTION_MESSAGES, k)` with `SECTION_MESSAGES > SECTION_TOOLS`.

The wire-format injection is per Anthropic's tool-and-system caching convention:

- **`LastSystem`** → on the last `system` block: `{ "type": "text", "text": "...", "cache_control": { "type": "ephemeral", "ttl": "1h"? } }`
- **`LastTool`** → on the last tool definition: same shape, on the tool object.
- **`MessageIndex(n)`** → on the last content block of `messages[n]`.

`ttl` is omitted from the JSON when `CacheTtl::Ephemeral` (Anthropic's default); set to `"1h"` when `CacheTtl::Persistent`.

`ProviderCapabilities.explicit_cache_markers = true`.

## 8. OpenAI-compat adapter (`crates/providers/src/openai_compat.rs`)

```rust
pub struct OpenAiCompatProvider {
    // … existing fields
    #[cfg(debug_assertions)]
    prefix_hashes: dashmap::DashMap<(ChannelName, ChatId), u64>,
}

impl LlmProvider for OpenAiCompatProvider {
    async fn chat(
        &self,
        messages: &[Message],
        tools: Option<&[Value]>,
        params: &ChatParams,
        cache_breakpoints: &[CacheBreakpoint],   // honored as no-op
    ) -> Result<LlmResponse> {
        // RELEASE BUILDS: ignore breakpoints entirely. Nothing else changes.
        // OpenAI / Gemini / DeepSeek / etc. cache prefixes >1024 tokens
        // server-side based on byte equality; client signals nothing.

        #[cfg(debug_assertions)]
        self.assert_prefix_stable(messages, cache_breakpoints, params);

        // … existing request-build path, unchanged.
    }
}

#[cfg(debug_assertions)]
impl OpenAiCompatProvider {
    fn assert_prefix_stable(
        &self,
        messages: &[Message],
        bps: &[CacheBreakpoint],
        params: &ChatParams,
    ) {
        // Pull the session key. If the caller didn't supply one, skip — we
        // can't dedupe across requests anyway.
        let key = match params.session_key() {
            Some(k) => k,
            None => return,
        };

        // Find the deepest MessageIndex breakpoint; that's our prefix-end.
        let frontier = bps.iter().filter_map(|b| match b.anchor {
            CacheAnchor::MessageIndex(n) => Some(n),
            _ => None,
        }).max();

        let Some(frontier) = frontier else { return };
        let hash = hash_messages_prefix(&messages[..=frontier.min(messages.len() - 1)]);

        if let Some(prev) = self.prefix_hashes.insert(key.clone(), hash) {
            if prev != hash {
                tracing::warn!(
                    target: "klynt::providers::openai_compat",
                    session = ?key,
                    "prefix-cache-busting detected: messages[..={}] hash changed \
                     ({:x} -> {:x}). Did MidLoopCompressor mutate before the frontier?",
                    frontier, prev, hash,
                );
            }
        }
    }
}
```

The debug-assertion uses `DashMap` instead of `thread_local!` so it works correctly when the executor spawns multiple sessions on different tokio worker threads (per `ActiveStreams` in `crates/agent/src/agent_loop/streaming.rs:27`).

`ProviderCapabilities.explicit_cache_markers = false`.

## 9. Executor — `cache_policy` module (`crates/agent/src/execution/cache_policy.rs`)

```rust
//! Cache-breakpoint placement policies.
//!
//! The default policy ("compression-aware") places markers thoughtfully
//! relative to the compression mutation zone:
//!   - LastSystem and LastTool sit OUTSIDE the mutation zone, so their
//!     cache survives every compression event.
//!   - A frontier-anchored MessageIndex marker sits AT the boundary of
//!     the mutation zone. Its cache is invalidated by a compression event
//!     but accelerates every call between compressions and gets a partial
//!     hit via Anthropic's longest-prefix-match after one.
//!
//! See docs/superpowers/specs/2026-05-05-provider-agnostic-prompt-cache-placement-design.md
//! (Appendix B) for the full analysis.

use providers::{CacheAnchor, CacheBreakpoint, CacheTtl, Message};
use serde_json::Value;

use super::mid_loop_compressor::MidLoopCompressor;

/// Default placement policy. Emits 2–3 breakpoints per call:
///
/// 1. `LastSystem` with `Persistent` TTL — the system prompt is durable
///    across the whole session and worth keeping for ~1h.
/// 2. `LastTool`   with `Persistent` TTL — tool definitions rarely change
///    within a session, same reasoning as above.
/// 3. `MessageIndex(frontier - 1)` with `Ephemeral` TTL — anchored at the
///    boundary of the compression mutation zone. The cached prefix
///    includes the mutation zone, so a compression event invalidates this
///    entry as a full match (Anthropic's longest-prefix-match still gives
///    a partial cache hit on the system+tools prefix afterward). Within a
///    compression-free run of turns it accelerates every call. Ephemeral
///    is correct because (a) the cache is invalidated by compression
///    anyway, and (b) the 5-min TTL matches typical ReAct-burst cadence.
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

    // 3. Pre-frontier prefix — accelerates intra-window turns; partial-hit only after compression
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

Wired into `execute_loop.rs::execute_loop`: built once per cycle right before `core.run_cycle`, passed straight through. `core.run_cycle` forwards to the provider's streaming path.

## 10. `MidLoopCompressor` accessor (`crates/agent/src/execution/mid_loop_compressor.rs`)

```rust
impl MidLoopCompressor {
    /// First index of the "always-preserved" recent window in `messages`.
    ///
    /// Compression only mutates `messages[system_count..frontier_index]`;
    /// `messages[frontier_index..]` is preserved verbatim across all
    /// compression events. A cache marker placed at `frontier_index - 1`
    /// (i.e. on the last message that compression cannot touch) will
    /// therefore survive any future compression.
    ///
    /// Returns 0 only when the message vec is shorter than `MIN_RECENT_MESSAGES`.
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
}
```

The body is the existing private logic in `compress_if_needed` (`mid_loop_compressor.rs:70-78`), promoted to a public accessor. `compress_if_needed` is updated to call `self.frontier_index(messages)` so we have one source of truth.

## 11. Data flow per call

```
execute_loop iteration N:
  ├─ messages = current vec  (from prior turns)
  ├─ tools    = current schema slice
  ├─ bps = cache_policy::compression_aware_default(&messages, &tools, &compressor)
  │     example: [
  │       LastSystem  / Persistent,
  │       LastTool    / Persistent,
  │       MessageIndex(7) / Ephemeral,    // = frontier_index - 1
  │     ]
  └─ core.run_cycle(messages, tools, params, ctx, event_tx, seen_tool_calls)
       │   bps is passed as a separate parameter on core.run_cycle (NOT
       │   folded into ExecutionParams — keeping that struct free of
       │   provider-layer concepts; bps is a per-cycle, executor-built value)
       ↓
ExecutionCore::run_cycle:
  └─ provider.chat_stream(messages, tools, &params.chat_params, bps)
       ↓
AnthropicNativeProvider::chat_stream → build_request_body:
  ├─ resolve LastSystem        → messages.iter().rposition(System) ⇒ idx 0
  ├─ resolve LastTool          → tools[len-1]
  ├─ resolve MessageIndex(7)   → messages[7]
  ├─ sort by absolute_position, keep trailing 4 (here: 3)
  ├─ inject cache_control on each resolved block
  ├─ if any Persistent → add header anthropic-beta:extended-cache-ttl-2025-04-11
  └─ POST to Anthropic API
       ↓
Response parsed:
  ├─ Usage { cache_read_tokens, cache_write_tokens, ... } populated as today
  └─ AgentEvent::BudgetUpdate emitted with new cache_read_tokens field
```

## 12. Error handling

| Condition | Behavior |
|---|---|
| `MessageIndex(n)` where `n >= messages.len()` | `tracing::warn!` once, skip the breakpoint, continue |
| `LastSystem` with no System messages | Silent skip (legitimate: a stateless turn) |
| `LastTool` with `tools.is_none()` or empty | Silent skip |
| More than 4 resolved breakpoints | Sort + keep trailing 4 (no error, no warning) |
| `CacheTtl::Persistent` on non-Anthropic provider | Silent no-op (whole breakpoint ignored) |
| Anthropic API rejects the cache_control payload | Surfaces as a normal provider error; not retried specially. Cache markers are best-effort and shouldn't block forward progress. |
| `frontier_index` returns 0 (very short conversation) | `checked_sub(1) → None`, the third breakpoint is omitted |

## 13. Configuration

One new config field in `crates/config/src/schema/providers.rs`:

```rust
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct CacheConfig {
    pub enabled: bool,    // default: true
}

impl Default for CacheConfig {
    fn default() -> Self { Self { enabled: true } }
}

// Lives at config.providers.cache.enabled
```

When `enabled = false`, `execute_loop` short-circuits to passing `&[]` to the provider. The Anthropic adapter then synthesizes its legacy fallback (LastSystem/Ephemeral) — caching still works at the system-prompt level, exactly as today.

This is a **kill switch**, not a tuning knob. There's no per-provider knob, no TTL override, no policy selector. YAGNI.

## 14. Observability

No new infrastructure. Two small additions to existing paths:

1. **Per-call `tracing::info!`** in `core.rs::run_cycle` after the provider returns:
   ```rust
   if usage.prompt_tokens > 0 {
       let hit_rate = usage.cache_read_tokens as f64 / usage.prompt_tokens as f64;
       tracing::info!(
           target: "klynt::execution::cache",
           cache_read = usage.cache_read_tokens,
           prompt = usage.prompt_tokens,
           hit_rate,
           "cache hit rate for this call"
       );
   }
   ```
2. **`AgentEvent::BudgetUpdate` payload extension.** Add `cache_read_tokens: u32` and `cache_write_tokens: u32` fields. Existing UI consumers ignore unknown fields.

`Usage.cache_read_tokens` and `Usage.cache_write_tokens` are already populated end-to-end by both adapters. We're surfacing what's already collected.

Honors CLAUDE.md non-goal "structured observability (OpenTelemetry, Prometheus, metrics dashboards) — single-user local app, existing tracing logs and PipelineEvent SSE stream are sufficient."

## 15. Testing plan

### Unit — providers crate
- `resolve_breakpoints` correctness for each `CacheAnchor` variant.
- Trailing-4 dedup when given 5, 6, 7 breakpoints.
- Out-of-range `MessageIndex(n)` is skipped, not errored.
- `Persistent` TTL ⇒ `extended-cache-ttl-2025-04-11` header present.
- All `Ephemeral` ⇒ header absent.
- Empty `cache_breakpoints` AND `cache_system_prompt = true` ⇒ legacy fallback synthesized + debug log fires.
- Empty `cache_breakpoints` AND `cache_system_prompt = false` ⇒ no markers at all.

### Unit — executor
- `compression_aware_default` returns `[LastSystem]` when conversation has only a system prompt.
- Returns `[LastSystem, LastTool]` when tools are present, no user messages yet.
- Returns 3 breakpoints with `MessageIndex(frontier - 1)` after several turns.
- `MessageIndex` correctly equals `frontier_index - 1` after `MIN_RECENT_MESSAGES` messages have accumulated.

### Unit — `MidLoopCompressor`
- `frontier_index` matches the existing computed value in all 5 fixtures of `mid_loop_compressor.rs::tests`.
- `compress_if_needed` still produces the same compression output before and after the refactor (existing tests must pass unchanged).

### Integration — `OpenAiCompatProvider` debug assertion
- First call → no warn (no prior hash).
- Second call with byte-identical prefix → no warn.
- Second call where `messages[3]` was mutated → exactly one warn.
- Across two different `(channel, chat_id)` pairs → independent state.

### Integration — Anthropic mock
- A stub `LlmProvider` that captures the request body. Assert `cache_control` JSON on the right blocks.
- Assert the beta header is present iff any breakpoint is `Persistent`.

### Regression
- All existing tests gain `&[]` at provider call sites and continue to pass.

## 16. Migration / rollout

Phased over two PRs to keep each diff reviewable:

**PR 1 — mechanism only.**
- Add types (`CacheTtl`, `CacheAnchor`, `CacheBreakpoint`).
- Update trait signature.
- Add Anthropic adapter resolver + Anthropic header logic.
- Add OpenAI-compat debug assertion.
- Mechanically add `&[]` to every existing call site.
- All existing tests still pass; behavior unchanged for any caller passing `&[]` (legacy fallback covers Anthropic).

**PR 2 — policy wiring.**
- Add `cache_policy` module.
- Add `MidLoopCompressor::frontier_index` accessor.
- Wire `execute_loop` to call the policy and pass breakpoints through.
- Add `Usage` surfacing in events + tracing.
- Add config kill switch.
- Add unit + integration tests for the policy.

**Follow-up — cleanup.**
- After 1–2 weeks of stable operation in dev, delete the legacy `cache_system_prompt: bool` fallback in the Anthropic adapter; require all callers to go through breakpoints.

## 17. Files touched

| File | Change | Approx LOC |
|---|---|---|
| `crates/providers/src/types.rs` | + enums + trait sig + `explicit_cache_markers` field | +50 |
| `crates/providers/src/anthropic_native.rs` | resolver + apply + header | +120 |
| `crates/providers/src/openai_compat.rs` | debug-only prefix-stability hash check | +60 |
| `crates/providers/src/factory.rs` | populate `explicit_cache_markers` for each adapter | +5 |
| `crates/providers/tests/*` | new unit tests | +120 |
| `crates/agent/src/execution/cache_policy.rs` | NEW module | +80 |
| `crates/agent/src/execution/mod.rs` | export | +2 |
| `crates/agent/src/execution/execute_loop.rs` | call policy + forward bps | +10 |
| `crates/agent/src/execution/core.rs` | `bps` parameter on `run_cycle`; forward to provider | +5 |
| `crates/agent/src/execution/mid_loop_compressor.rs` | promote `frontier_index` to public | +15 |
| `crates/agent/src/execution/types.rs` (or new param on `run_cycle`) | wiring | +5 |
| `crates/agent/src/events.rs` | add cache fields to `BudgetUpdate` | +5 |
| `crates/agent/tests/*` | policy + frontier tests | +130 |
| `crates/config/src/schema/providers.rs` | `CacheConfig` | +10 |
| **All existing call sites** | mechanical `&[]` insert | trivial |

**Estimated net change:** ~600 LOC including tests. No UI changes. No DB migrations. No new dependencies.

## 18. Open questions / future work

1. **Adaptive policy (the future (C) option).** Track `cache_read_tokens > 0` per breakpoint position over the last N turns; promote consistently warm positions, drop cold ones. Plug-in shape:
   ```rust
   pub trait CachePolicy {
       fn compute(&self, ctx: &PolicyContext) -> Vec<CacheBreakpoint>;
   }
   ```
   The default `CompressionAwarePolicy` becomes one impl; an `AdaptiveCachePolicy` becomes another. Out of scope for v1.

2. **Multi-turn cache analytics.** Aggregate `cache_read_tokens / prompt_tokens` per session, expose as a HUD chip. Out of scope.

3. **OpenAI-compat prefix-stability enforcement (production).** If we ever observe real cache-busting in the wild, promote the debug-only `assert_prefix_stable` to a production warning (rate-limited) or even a metric.

4. **Image-bearing tool results and compression.** CLAUDE.md notes: `MidLoopCompressor` drops images on compression. The cache-marker mechanism doesn't change this, but it interacts with it: if a future `Computer Use` turn injects images into `Message::Tool`, compression invalidates them, and any cache marker placed *after* the image was injected is wasted. Defer to the procedural-memory design.

5. **Persistent caching across CLI boundaries.** The `klyntbot mcp serve --stdio` subcommand spawns a fresh process per Claude Code session. Anthropic's server-side cache is keyed on prompt content, not process identity, so this works automatically — but worth confirming with a real measurement during rollout.

---

## Appendix A — Why the trailing-4 dedup rule is correct

Anthropic semantics: a `cache_control` marker at position N caches **everything from the start of the request up to and including position N**. So:

- A marker at `LastSystem` (position 0 in payload terms) caches just the system prompt.
- A marker at `LastTool` (position 1) caches system + tools.
- A marker at `MessageIndex(7)` (position 2) caches system + tools + messages[..=7].

If we have 5 markers and must drop 1, dropping the *latest* throws away the largest cached prefix; dropping the *earliest* loses nothing because everything it covered is already covered by the next-deepest marker. Therefore: sort ascending by absolute position, drop the head, keep the trailing 4.

## Appendix B — What each marker actually does

This appendix is the source of truth for the policy. Every claim about "compression-aware" elsewhere in the spec must be consistent with what's written here.

### B.1 The mutation zone

`MidLoopCompressor::compress_if_needed` rewrites `Message::Tool` content in the range `messages[system_count..frontier_index]` where `frontier_index = max(messages.len() - MIN_RECENT_MESSAGES, system_count)`. Messages outside that range are preserved byte-identical:
- `messages[..system_count]` — the system prompt(s).
- `messages[frontier_index..]` — the recent-window (last `MIN_RECENT_MESSAGES = 8` messages).
- `tools` — never touched by compression; tools live alongside the messages array, not inside it.

### B.2 Anthropic's cache key

A `cache_control` marker at position N caches the **entire prefix from the start of the request through position N inclusive**. The cache key is the bytewise content of that prefix. Subsequent requests use **longest-prefix-match**: the server returns cache savings for whatever cached prefix matches the start of the new request, even if shorter than what was originally cached.

### B.3 Marker-by-marker behavior

| Marker | Cached prefix | Compression invalidates? |
|---|---|---|
| `LastSystem` | system messages only | **No** — system messages are outside the mutation zone. Cache survives every compression event unconditionally. |
| `LastTool`   | system messages + tools array | **No** — tools live outside the messages array. Cache survives every compression event unconditionally. |
| `MessageIndex(frontier_index - 1)` | system + tools + `messages[..frontier_index]` | **Yes** — the cached prefix includes the mutation zone `messages[system_count..frontier_index]`. After compression, the bytes change and the original cache entry is no longer reachable as a full match. |

### B.4 So why include the third marker at all?

Two reasons.

**Reason 1: intra-compression-window acceleration.** Compression only fires when total tokens exceed 70% of the context window. Between compression events there can be many ReAct turns (3, 5, 10+) in which the conversation grows by 1–2 messages per turn. With a marker at `frontier_index - 1`, each subsequent turn finds a long cached prefix that matches up to the previous turn's marker (via longest-prefix-match). Without this marker, each turn pays full input-token cost for the entire conversation prefix.

**Reason 2: partial-prefix recovery after compression.** When compression does fire, the marker's cache is invalidated as a full match — but the start of the prefix (system + tools + any pre-mutation-zone messages, which in practice is just `messages[..system_count]` since the mutation zone starts immediately after system messages) still matches. Anthropic's longest-prefix-match returns a cache hit for that shorter prefix. We don't get the full prefix savings post-compression, but we don't pay full cost either.

### B.5 Why `frontier_index - 1` and not `len - 2` (opencode-style)

Functionally similar — both cache the conversation prefix up to a stable point. The frontier-anchored placement has two minor advantages:
- The marker position is **deterministic relative to compression**: it always sits exactly at the mutation-zone boundary. This makes the policy easier to reason about and easier to test.
- The marker position is **stable across small turn additions**: the frontier shifts only when message count crosses the `MIN_RECENT_MESSAGES` threshold. Opencode-style `len - 2` shifts every turn, so the cache key changes every turn (longest-prefix-match still helps, but the marker itself moves more).

### B.6 What we are NOT claiming

We are not claiming the third marker "survives compression" in the literal sense of its full cache entry being reusable post-compression. We are claiming the policy as a whole is **compression-aware**: it places durable markers (`LastSystem`, `LastTool`) outside the mutation zone where they're guaranteed to keep paying off, and a single accelerator marker at the mutation-zone boundary where it's positioned to give the maximum within-window speedup with minimum interaction cost.

## Appendix C — References

- Anthropic prompt caching: https://docs.anthropic.com/en/docs/build-with-claude/prompt-caching
- Anthropic 1h cache beta: `anthropic-beta: extended-cache-ttl-2025-04-11`
- opencode reference: `/Users/jayden/Projects/Klynt/opencode/internal/llm/provider/anthropic.go:135-139`
- Klynt's existing Anthropic caching: `crates/providers/src/anthropic_native.rs:452-465`
- Klynt's compressor: `crates/agent/src/execution/mid_loop_compressor.rs:70-78`
