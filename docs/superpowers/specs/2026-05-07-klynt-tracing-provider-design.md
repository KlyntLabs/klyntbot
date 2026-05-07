# Klynt Tracing Provider — Wire Klynt's Coding Mode Into the Existing Tracing Pipeline

**Status:** Design (approved by user, pending review)
**Date:** 2026-05-07
**Scope:** Wire Klynt's coding-mode runtime events into the existing `coding-ingest` + `tracing` pipeline so Klynt sessions appear in the same Tracing UI that already serves Claude Code, Codex, Kimi CLI, and OpenCode.
**Crates touched:** `app-core`, `coding-memory`, `desktop`, `desktop-ui`
**Companion docs:**
- `docs/superpowers/specs/2026-04-29-klynt-coding-in-chat-design.md` §10 (event vocabulary), §12 (Distiller integration)
- `docs/superpowers/specs/2026-04-23-klynt-cli-design.md` (deprecated; event-vocabulary decisions carried forward)
- `docs/superpowers/notes/2026-05-07-long-running-task-comparative-analysis.md` (background and rationale)

---

## Problem

Klynt's coding-mode sessions are persisted to SQLite per-iteration but **do not appear in the Tracing UI** at `desktop-ui/src/tracing/`. The Tracing UI today is hardcoded to the `kimi` provider (`desktop-ui/src/tracing/lib/api.ts:7`) and renders only Kimi CLI sessions, even though it has a generic provider abstraction (`TracingProvider` trait + registry) that already supports four external CLIs (ClaudeCode, Codex, KimiCli, OpenCode).

The cognitive subsystems (Distiller, Mirror, Reforge) cannot consume Klynt's own coding-mode events at the structured-event fidelity that external-CLI adapters produce, because the `turn_handler.rs` event loop translates `agent::AgentEvent` into UI-bound `ThreadEvent`s only — there is no parallel path into `coding_ingest::AgentEvent` and `Distiller::accept_event`.

## Goals

1. Klynt coding-mode sessions appear in the Tracing UI alongside (or instead of) external CLIs, with no loss of existing kimi/claude-code/codex/opencode functionality.
2. Klynt's runtime `agent::AgentEvent`s flow into the same `coding-ingest` store that external CLIs feed, tagged with `AgentSource::KlyntCli`.
3. Distiller / Mirror / Reforge receive the rich structured-event vocabulary from Klynt sessions at ≥5x the volume external-CLI adapters produce (per the deprecated 2026-04-23 spec §1 vision).
4. The cross-CLI normalization invariant (`parse(serialize(event)) == event`) holds for the new Klynt source.
5. Frontend provider selection: user can switch between providers (or see all) via the Tracing UI.

## Non-goals

- Replacing or modifying the existing kimi/claude-code/codex/opencode adapters.
- Adding a new file format (no `wire.jsonl` per session — the existing `coding-ingest` SQLite store is authoritative).
- Multi-process coordination (the desktop process is the runtime per active spec §12).
- Backwards compatibility for an external `klynt-cli` binary — the deprecated 2026-04-23 architecture for that is not in scope.
- Phase 2+ runtime variants (LSP diagnostics, full coverage parsing, mirror-learned approval) — see active spec §10 phase-1 stub-fields note.

## Background — the existing infrastructure

The architecture was built over the past month to support multi-source agent observability. As of 2026-05-07:

```
                 ┌─────────────────────────────────────────────┐
                 │      External coding agents (existing)      │
                 │                                             │
                 │   Claude Code session JSONL                 │
                 │   Codex session JSONL                       │
                 │   Kimi CLI wire.jsonl                       │
                 │   OpenCode SQLite WAL                       │
                 └─────────────┬───────────────────────────────┘
                               │ poll
                               ▼
                 ┌─────────────────────────────────────────────┐
                 │   crates/coding-ingest/src/adapters/        │
                 │   ┌─────────┐ ┌──────┐ ┌────────┐ ┌────────┐│
                 │   │claude_  │ │codex │ │kimi_cli│ │opencode││
                 │   │code     │ │      │ │        │ │        ││
                 │   └────┬────┘ └──┬───┘ └───┬────┘ └───┬────┘│
                 │        └─────────┴─────────┴──────────┘     │
                 │                  │                          │
                 │                  ▼                          │
                 │    coding_ingest::AgentEvent (V1)           │
                 │    { id, source: AgentSource, session_id,   │
                 │      turn_id, cwd, repo, occurred_at, kind }│
                 └──────────────────┬──────────────────────────┘
                                    │
                                    ▼
                 ┌─────────────────────────────────────────────┐
                 │   coding-memory store (SQLite)              │
                 │   coding_ingest_events table                │
                 └──────────────────┬──────────────────────────┘
                                    │
                                    ▼
                 ┌─────────────────────────────────────────────┐
                 │   crates/app-core/src/tracing/              │
                 │   - TracingProvider trait (13 methods)      │
                 │   - TracingRegistry                         │
                 │   - providers/kimi/ (full impl, 16 files)   │
                 │   - providers/claude_code/ ...              │
                 └──────────────────┬──────────────────────────┘
                                    │
                                    ▼
                 ┌─────────────────────────────────────────────┐
                 │   crates/desktop/src/commands/tracing.rs    │
                 │   tracing_list_sessions(provider_id)        │
                 │   tracing_load_session(provider_id, ...)    │
                 │   tracing_load_context(provider_id, ...)    │
                 │   ... etc                                   │
                 └──────────────────┬──────────────────────────┘
                                    │
                                    ▼
                 ┌─────────────────────────────────────────────┐
                 │   desktop-ui/src/tracing/                   │
                 │   - WireViewer (718 lines, virtualized)     │
                 │   - ContextViewer, StateViewer, DualView    │
                 │   - AgentsPanel, Statistics                 │
                 │   - 8 sub-features under wire-viewer/       │
                 └─────────────────────────────────────────────┘

KEY MISSING PIECES (this spec adds):

  ┌──────────────────────────────────┐
  │   Klynt's own coding mode        │  ← Gap 1: feed events here
  │   crates/app-core/src/coding/    │
  │     turn_handler.rs              │
  │     (currently emits only        │
  │      AgentEvent → ThreadEvent    │
  │      for the UI; no path to      │
  │      coding_ingest::AgentEvent)  │
  └──────────────────────────────────┘

  ┌──────────────────────────────────┐
  │   crates/app-core/src/tracing/   │  ← Gap 2: new provider
  │     providers/klynt/             │
  │     (does not exist yet)         │
  └──────────────────────────────────┘
```

## Architecture overview — the four gaps

### Gap 1 — Wire `turn_handler.rs` into the Translator

The existing `turn_handler.rs:163` matches on `agent::AgentEvent` to translate into UI-bound `ThreadEvent`s. We add a parallel call path: `agent::AgentEvent` → `RuntimeEvent` → `Translator::translate()` → `coding_ingest::AgentEvent` → `Distiller::accept_event()`.

The active 2026-04-29 spec §12 says: *"There is no `MemorySink` trait abstraction needed in this spec (unlike the superseded klynt-cli design): the desktop process is the runtime, so emission is always in-process."* — so we call `Distiller::accept_event` directly rather than going through `MemorySink`.

The Translator and Aggregator at `crates/coding-memory/src/sink/translator.rs` already handle the bulk of variants. Two missing variants must be added:

- `RuntimeEvent::MidLoopCompressionTriggered` → `EventKind::CompressionApplied`
- (Optional Phase 2+) `RuntimeEvent::TurnInterrupted` — runtime-only, no ingest counterpart

A new helper module `crates/app-core/src/coding/runtime_event_translator.rs` provides:
```rust
pub fn agent_event_to_runtime_event(evt: &agent::AgentEvent) -> Option<RuntimeEvent>;
```
Returns `None` for runtime-only variants (`IterationStart`, `ToolCallStreamChunk`, `PowerModeToggled` per active spec §10).

### Gap 2 — `KlyntTracingProvider`

Mirror the directory structure of `crates/app-core/src/tracing/providers/kimi/` but adapted for SQLite-resident data:

```
crates/app-core/src/tracing/providers/klynt/
├── mod.rs            // pub use provider_impl::KlyntTracingProvider
├── provider_impl.rs  // impl TracingProvider for KlyntTracingProvider
├── discovery.rs      // SELECT * FROM sessions WHERE conversation_type = 'coding'
├── loader.rs         // SELECT * FROM coding_ingest_events WHERE session_id = ? AND source = 'klynt-cli'
├── context_loader.rs // SELECT * FROM chat_messages WHERE session_id = ?
├── state_loader.rs   // session row → SessionState (cwd, repo_id, repo_branch, tool_profile, approval_mode)
├── subagent_loader.rs// existing SubagentManager state, queried by session
├── summary.rs        // per-session aggregate (turns, steps, tools, errors, tokens)
└── stats.rs          // cross-session aggregate
```

Estimated: ~600 lines total. Each file is straightforward SQLite queries against the existing schema.

### Gap 3 — Registration

The current registry init point (likely in `crates/app-core/src/init/`) registers `KimiTracingProvider`. We add a sibling registration:
```rust
registry.register(Arc::new(KlyntTracingProvider::new(repos.clone())));
```

### Gap 4 — Frontend provider selector

`desktop-ui/src/tracing/lib/api.ts:7` is currently:
```typescript
const PROVIDER_ID = "kimi";
```

Replace with URL-param-driven selection plus a header chip-row:
```typescript
function getCurrentProviderId(): string {
  const params = new URLSearchParams(window.location.search);
  return params.get("provider") ?? "klynt"; // default to klynt
}
```

The header gains a provider chip-row using `tracing_list_providers` (registry's `list_providers()` method, exposed via a new Tauri command). Each chip shows display name + session count badge.

## Detailed design

### 4.1 RuntimeEvent enum extension

`crates/coding-memory/src/sink/translator.rs` — add at the end of the existing `RuntimeEvent` enum:

```rust
/// Mid-loop compression was triggered.
MidLoopCompressionTriggered {
    /// Tokens before compression.
    before_tokens: u32,
    /// Tokens after compression.
    after_tokens: u32,
    /// Number of messages condensed.
    messages_condensed: u32,
},
```

And the translator match arm:

```rust
RuntimeEvent::MidLoopCompressionTriggered {
    before_tokens,
    after_tokens,
    messages_condensed,
} => vec![EventKind::CompressionApplied {
    before_tokens: *before_tokens,
    after_tokens: *after_tokens,
    messages_condensed: *messages_condensed,
}],
```

### 4.2 `agent_event_to_runtime_event` mapping

`crates/app-core/src/coding/runtime_event_translator.rs` (new file):

```rust
//! Maps runtime agent::AgentEvent into the coding-memory RuntimeEvent shape.
//!
//! Returns None for runtime-only variants that have no ingest counterpart
//! (per active spec §10).

use agent::events::AgentEvent;
use coding_memory::sink::translator::RuntimeEvent;

#[must_use]
pub fn agent_event_to_runtime_event(evt: &AgentEvent) -> Option<RuntimeEvent> {
    match evt {
        AgentEvent::ContentChunk { data, .. } => Some(RuntimeEvent::ContentChunk {
            text: data.clone(),
        }),
        AgentEvent::ToolStart { call_id, name, args, .. } => Some(RuntimeEvent::ToolStart {
            call_id: call_id.clone(),
            name: name.clone(),
            args: args.clone(),
        }),
        AgentEvent::ToolEnd { call_id, success, output, duration_ms, .. } => {
            Some(RuntimeEvent::ToolEnd {
                call_id: call_id.clone(),
                success: *success,
                output: output.clone(),
                duration_ms: *duration_ms,
            })
        }
        AgentEvent::ContextCompressed { before_tokens, after_tokens, messages_condensed, .. } => {
            Some(RuntimeEvent::MidLoopCompressionTriggered {
                before_tokens: *before_tokens,
                after_tokens: *after_tokens,
                messages_condensed: *messages_condensed,
            })
        }
        AgentEvent::FileEditWithSymbols { path, op, bytes, anchored_symbols, lsp_diagnostics_delta, .. } => {
            Some(RuntimeEvent::FileEditWithSymbols {
                path: path.clone(),
                op: op.clone(),
                bytes: *bytes,
                anchored_symbols: anchored_symbols.clone(),
                lsp_diagnostics_delta: lsp_diagnostics_delta.clone(),
            })
        }
        // Runtime-only — no ingest counterpart per active spec §10:
        AgentEvent::ReasoningChunk { .. }
        | AgentEvent::Done { .. }
        | AgentEvent::TurnComplete { .. }
        | AgentEvent::UsageReport { .. }
        | AgentEvent::Error { .. } => None,
        _ => None,
    }
}
```

### 4.3 Wiring in `turn_handler.rs`

In the existing match block at `crates/app-core/src/coding/turn_handler.rs:163`, add a parallel path:

```rust
// New: at the top of the bridge task, alongside existing translator
let mut ingest_translator = coding_memory::sink::translator::Translator::new();

while let Some(ag_evt) = event_rx.recv().await {
    // Existing: AgentEvent → ThreadEvent (UI emit)
    // [existing code unchanged]

    // NEW: AgentEvent → RuntimeEvent → IngestEvt → Distiller
    if let Some(rt_evt) = runtime_event_translator::agent_event_to_runtime_event(&ag_evt) {
        match ingest_translator.translate(&rt_evt) {
            Ok(ingest_events) => {
                for mut evt in ingest_events {
                    // Stamp session_id (translator returns String::new() for it)
                    if let coding_ingest::event::AgentEvent::V1(ref mut v1) = evt {
                        v1.session_id = session_id.clone();
                        v1.turn_id = Some(turn_id.clone());
                        v1.cwd = cwd.clone();
                    }
                    if let Err(e) = distiller.accept_event(evt).await {
                        tracing::warn!(?e, "distiller rejected ingest event; continuing");
                    }
                }
            }
            Err(e) => tracing::warn!(?e, "translator error; skipping event"),
        }
    }
}
```

### 4.4 KlyntTracingProvider — discovery query

`crates/app-core/src/tracing/providers/klynt/discovery.rs`:

```rust
pub async fn list_klynt_sessions(repos: &Repos) -> Result<Vec<SessionSummary>> {
    let pool = repos.pool();
    let rows = sqlx::query!(
        r#"
        SELECT
          s.id as session_id,
          s.title,
          s.cwd,
          s.repo_id,
          s.created_at,
          s.updated_at,
          (SELECT COUNT(*) FROM coding_ingest_events e
           WHERE e.session_id = s.id AND e.source = 'klynt-cli') as event_count,
          (SELECT COUNT(*) FROM coding_ingest_events e
           WHERE e.session_id = s.id AND e.source = 'klynt-cli'
             AND e.kind = 'TurnBegin') as turn_count
        FROM sessions s
        WHERE s.conversation_type = 'coding'
        ORDER BY s.updated_at DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows.into_iter().map(|r| SessionSummary {
        session_id: r.session_id,
        title: r.title.unwrap_or_else(|| "Untitled session".to_string()),
        // ... map other fields
    }).collect())
}
```

### 4.5 KlyntTracingProvider — event loader

`crates/app-core/src/tracing/providers/klynt/loader.rs`:

```rust
pub async fn load_klynt_session(
    repos: &Repos,
    session_id: &str,
    scope: Scope,
) -> Result<SessionDetail> {
    let pool = repos.pool();
    let agent_id_filter = match &scope {
        Scope::Main => None,
        Scope::Subagent { agent_id } => Some(agent_id.clone()),
    };

    let rows = sqlx::query!(
        r#"
        SELECT
          seq, kind as "raw_kind", payload, occurred_at, turn_id, parent_subagent_id
        FROM coding_ingest_events
        WHERE session_id = ? AND source = 'klynt-cli'
          AND (? IS NULL OR parent_subagent_id = ?)
        ORDER BY seq ASC
        LIMIT 5000
        "#,
        session_id,
        agent_id_filter,
        agent_id_filter,
    )
    .fetch_all(pool)
    .await?;

    let events = rows.into_iter().map(|r| TraceEvent {
        seq: r.seq as u64,
        provider_id: "klynt".to_string(),
        raw_kind: r.raw_kind,
        payload: serde_json::from_str(&r.payload).unwrap_or(serde_json::Value::Null),
        occurred_at: r.occurred_at,
        category: categorize(&r.raw_kind),
        turn_index: None, // computed below
        step_index: None,
        parent_subagent_id: r.parent_subagent_id,
    }).collect();

    Ok(SessionDetail { /* ... */ events, /* ... */ })
}
```

### 4.6 Frontend provider chip-row

`desktop-ui/src/tracing/lib/api.ts` — replace the constant with a function:

```typescript
let _providerId: string | null = null;

export function getCurrentProviderId(): string {
  if (_providerId) return _providerId;
  const params = new URLSearchParams(window.location.search);
  _providerId = params.get("provider") ?? "klynt";
  return _providerId;
}

export function setCurrentProviderId(id: string): void {
  _providerId = id;
  const url = new URL(window.location.href);
  url.searchParams.set("provider", id);
  window.history.pushState({}, "", url.toString());
  apiCache.clear(); // invalidate cached sessions/wire/etc.
}

export interface ProviderInfo {
  id: string;
  display_name: string;
  session_count: number;
}

export async function listProviders(): Promise<ProviderInfo[]> {
  return invoke<ProviderInfo[]>("tracing_list_providers");
}
```

Replace every occurrence of `providerId: PROVIDER_ID` with `providerId: getCurrentProviderId()`.

`desktop-ui/src/tracing/index.tsx` — add a chip-row in the header:

```tsx
function ProviderChips() {
  const [providers, setProviders] = useState<ProviderInfo[]>([]);
  const current = getCurrentProviderId();

  useEffect(() => {
    listProviders().then(setProviders).catch(console.error);
  }, []);

  return (
    <div className="flex gap-1 px-3 py-1 border-b">
      {providers.map((p) => (
        <button
          key={p.id}
          type="button"
          onClick={() => {
            setCurrentProviderId(p.id);
            window.location.reload(); // or trigger a re-render via state lift
          }}
          className={`text-xs px-2 py-1 rounded-md ${
            current === p.id
              ? "bg-accent text-foreground"
              : "text-muted-foreground hover:bg-accent/50"
          }`}
        >
          {p.display_name} <span className="opacity-60">({p.session_count})</span>
        </button>
      ))}
    </div>
  );
}
```

### 4.7 New Tauri command — `tracing_list_providers`

`crates/desktop/src/commands/tracing.rs`:

```rust
#[klynt_command]
pub async fn tracing_list_providers(state: State<'_, AppCore>) -> Result<Vec<ProviderInfo>> {
    state.tracing_list_providers().await
}
```

Then list the path in `desktop_macros::klynt_collect_commands![...]` in `specta_builder.rs` and run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts` per the CLAUDE.md gotcha.

`crates/app-core/src/tracing_handlers.rs` adds:

```rust
#[tracing::instrument(skip(self), err)]
pub async fn tracing_list_providers(&self) -> Result<Vec<ProviderInfo>> {
    self.tracing_registry.list_providers().await
}
```

## Schema changes

**None.** The `sessions` table already has `conversation_type` (per active spec §11). The `coding_ingest_events` table already has `source` and `session_id` columns and is indexed accordingly. The `chat_messages` table already stores per-iteration message parts.

## Property test extension

`crates/coding-ingest/tests/cross_cli_normalization.rs` — extend the existing proptest to include `AgentSource::KlyntCli`. Generate sample events with the new source, serialize, parse, assert equality. Same property as the four existing sources.

## Verification criteria

- [ ] Open a coding-mode chat thread in the desktop app.
- [ ] Send a message that triggers tool calls.
- [ ] Open the Tracing tab.
- [ ] Confirm the session appears under provider "Klynt".
- [ ] Confirm the wire viewer shows ContentChunk → ToolCall → ToolResult events in chronological order with proper grouping.
- [ ] Confirm the Statistics tab counts Klynt's session in the aggregates.
- [ ] Switch to the Kimi provider chip — confirm Kimi sessions still load correctly.
- [ ] Run `cargo nextest run -p coding-ingest cross_cli_normalization` — all sources pass round-trip.
- [ ] Run `cargo nextest run -p app-core tracing` — all provider tests pass.
- [ ] Run `cargo nextest run -p coding-memory translator` — translator tests pass with new `MidLoopCompressionTriggered` variant.
- [ ] Run `cd desktop-ui && bun run typecheck && bun run lint` — both clean.

## Open questions

1. **Distiller availability at `turn_handler.rs`**: Is `Distiller` already injected into the bridge task's scope, or does it need to be plumbed via `AppCore`? (To verify: read `crates/app-core/src/coding/turn_handler.rs:1–100` for handle imports.)
2. **`coding_ingest_events` schema**: Confirm column names match the queries in §4.4–4.5. (To verify: read the migration file at `crates/coding-memory/migrations/`.)
3. **Subagent event filtering**: How are Klynt's subagent events tagged? `parent_subagent_id` column or a separate `agent_id` field? (To verify against `crates/coding-ingest/src/event.rs`.)
4. **Default provider**: Should the Tracing UI default to `klynt` or aggregate across all? Recommendation: default `klynt` so users see their own work first; chip-row exposes others.
5. **Tracking which Klynt sessions are "active"**: The Tracing UI's "running" indicator (today via `session.has_wire = true && wire_mtime within 30s`) doesn't apply to Klynt's in-process model. We can derive "active" from the existing `ActiveStreams` DashMap or just leave the indicator off for Klynt sessions in v1.

## Future work

- **Phase 2:** Map `agent::AgentEvent::ReasoningChunk` to a new `RuntimeEvent::ReasoningChunk` + `EventKind::ThinkChunk` for full think-block visibility in the wire viewer.
- **Phase 2:** Subagent rendering parity — if Klynt's subagents emit events, the AgentsPanel's nested rendering should mirror Kimi's pattern.
- **Phase 2:** Per-Klynt-session export: download a self-contained replay file (mirrors Kimi's existing `tracing_import` flow in reverse).
- **Phase 3:** Aggregate-across-providers view — Statistics tab can sum across `[claude_code, codex, kimi, opencode, klynt]` for cross-source comparisons.

---

*End of design.*
