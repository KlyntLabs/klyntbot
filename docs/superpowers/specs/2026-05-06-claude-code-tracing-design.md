# Claude Code Agent Tracing — Design

**Date:** 2026-05-06
**Status:** Spec, awaiting plan
**Goal:** Add Claude Code as a second `TracingProvider` alongside Kimi, exposing Claude Code's native session data (`~/.claude/projects/<encoded-cwd>/*.jsonl` + `<session-uuid>/subagents/`) through the existing Plugins → Tracing screen. Reuse the outer chrome (sidebar, provider chips, Sessions list, Statistics, session-detail header layout). Render the inner tabs **provider-natively** — Claude Code shows what Claude Code has; Kimi shows what Kimi has; neither pretends to be the other.

---

## 1. Problem

The Plugins → Tracing page already has provider chips for Claude Code / Codex / Kimi / opencode / Klynt CLI, but only Kimi is registered with the `TracingRegistry`. The Claude Code chip shows zero sessions even though `~/.claude/projects/` contains 80 sessions for this user (35 371 wire lines, 194 subagent files).

The `TracingProvider` trait and provider-agnostic DTOs introduced for Kimi (spec `2026-04-30-kimi-vis-tracing-design.md`, port spec `2026-05-02-tracing-ui-port-design.md`) are deliberately CLI-agnostic — adding a CLI is meant to be one new submodule plus a registration line. Two things prevent that today:

1. **The trait assumes Kimi-shaped semantics.** `SessionState`, `ContextMessage`, `Dual` view all model concepts Claude Code does not have. Synthesizing them produces partially-empty tabs that lie about the underlying data.
2. **The session-detail UI hardcodes Kimi's five tabs and Kimi's header chip set.** A provider with a different shape has no way to declare what it actually has.

This spec resolves both: extends the trait with two declarative methods (`supported_tabs`, `header_layout`), refactors the FE shell to dispatch by provider, and adds the Claude Code provider implementation.

## 2. Scope

**In v1.** End-to-end Claude Code support in the Tracing screen:

- Discovery of native sessions under `~/.claude/projects/<encoded-cwd>/*.jsonl` plus imported sessions.
- Sessions list (existing UI, no changes) populated from `SessionSummary` rows.
- Session detail page with **two tabs declared by Claude Code**: `Wire`, `Agents`. (No Context, no State, no Dual.)
- Wire tab: full categorization of all 22 distinct line shapes observed in real data, native cards per shape, virtualized list with the existing filter/search/error-nav primitives.
- Agents tab: lists `agent-<id>.jsonl` + `agent-<id>.meta.json` sibling files; selecting one re-runs the wire viewer scoped to that subagent.
- Statistics tab populated when "Claude Code" is selected in the provider chip row.
- Manual refresh. No live tail.
- Import via file picker — copy a `.jsonl` into `{klyntbot_data}/coding_memory/imported_claude_code/<uuid>/`.

**Out of scope for v1.**

- **Tree tab** (parentUuid visualization). 0 of 35 371 lines had `isSidechain: true`; the parentUuid graph is mostly linear. Can revisit if usage emerges.
- **Cross-session stitching.** Every session has cross-file `parentUuid` references (avg ~10 per session) from `--continue`/compaction-fork. v1 treats each `.jsonl` as one independent session and documents the limitation.
- **Live file watching / tail.** Manual refresh only.
- **Sidechain virtualization.** Inline badge if encountered; no separate scope.
- **PR-link chip on the session card** in the Sessions list. PR-link still renders inline as a card in Wire view.
- **DB persistence of raw events.** Live read only. Optional copy via Import.

**Non-goals.**

- Sharing card components between Kimi and Claude Code. Each provider gets its own native cards. Outer chrome is shared; rich rendering is not.
- Generalizing Kimi's existing components. They stay where they are; we add siblings.

## 3. Architecture

### 3.1 Live read, no DB writes

Reads happen at view time against `~/.claude/projects/`. No new SQLite tables, no migrations. Mirrors Kimi's approach.

### 3.2 Trait change — declarative tabs and header layout

Two new methods on `TracingProvider`. Default impls preserve Kimi's current behavior.

```rust
#[async_trait]
pub trait TracingProvider: Send + Sync {
    // existing methods unchanged...

    fn supported_tabs(&self) -> &'static [SessionTab] {
        &[SessionTab::Wire, SessionTab::Context, SessionTab::State,
          SessionTab::Dual, SessionTab::Agents]
    }

    fn header_layout(&self) -> &'static [HeaderChip] {
        &[HeaderChip::Turns, HeaderChip::Steps, HeaderChip::ToolCalls,
          HeaderChip::Errors, HeaderChip::Compactions, HeaderChip::Agents,
          HeaderChip::Duration, HeaderChip::Tokens, HeaderChip::CacheHitPct]
    }
}

#[derive(Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum SessionTab { Wire, Tree, Context, State, Dual, Agents }

#[derive(Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub enum HeaderChip {
    Turns, Steps, Messages, ToolCalls, Errors, Compactions, Agents,
    Duration, Tokens, CacheHitPct, Model,
}
```

Methods that don't apply to a provider (`load_context`, `load_state`, `load_subagent_context` for Claude Code) return `KlyntbotError::Unsupported`. The FE never calls them because it consults `supported_tabs()` first.

### 3.3 Claude Code declares

```rust
fn supported_tabs(&self) -> &'static [SessionTab] {
    &[SessionTab::Wire, SessionTab::Agents]
}

fn header_layout(&self) -> &'static [HeaderChip] {
    &[HeaderChip::Turns, HeaderChip::ToolCalls, HeaderChip::Errors,
      HeaderChip::Compactions, HeaderChip::Agents,
      HeaderChip::Model, HeaderChip::Tokens,
      HeaderChip::CacheHitPct, HeaderChip::Duration]
}
```

Notably absent: `Steps` (no Claude Code analog), `Messages` (we use Turns instead).

### 3.4 New `TraceEvent` field

```rust
pub struct TraceEvent {
    // existing fields...
    pub meta: bool,   // NEW — true when source line had `isMeta: true`
}
```

Kimi sets `meta = false` for all events (no equivalent flag). Claude Code propagates the source flag. UI default-filters `meta = true` events with a toggle to reveal.

### 3.5 New module layout

```
crates/app-core/src/tracing/providers/claude_code/
├── mod.rs
├── provider_impl.rs        ── ClaudeCodeTracingProvider, TracingProvider impl
├── discovery.rs            ── scan ~/.claude/projects/<encoded-cwd>/*.jsonl + imported
├── categorize.rs           ── (raw_kind, payload) → SemanticCategory
├── loader.rs               ── stream JSONL → Vec<TraceEvent> + HeaderStats (single pass)
├── summary.rs              ── SessionSummary, cwd resolution from JSONL content
├── subagent_loader.rs      ── readdir <session>/subagents/, parse .meta.json
├── stats.rs                ── StatsBundle aggregation across all Claude Code sessions
├── cache.rs                ── (file_path, mtime)-keyed cache for list_sessions
└── import.rs               ── copy picked file into imported_claude_code/<uuid>/
```

No `state_loader.rs`, no `context_loader.rs` — those tabs are not declared.

### 3.6 Data on disk (Claude Code)

```
~/.claude/projects/
  <encoded-cwd>/                          ← cwd with '/' replaced by '-'
    <session-uuid>.jsonl                  ← one session per file
    <session-uuid>/
      subagents/
        agent-<id>.jsonl                  ← subagent transcript
        agent-<id>.meta.json              ← {agentType, description}

{klyntbot_data}/coding_memory/imported_claude_code/
  <new-uuid>/
    <new-uuid>.jsonl                      ← copied from picked file; renamed for stable session_id
    meta.json                             ← {imported_at, original_path}
```

`<encoded-cwd>` is lossy (original `-` chars in path become indistinguishable from separators). We resolve cwd by reading the first JSONL line's `cwd` field; fall back to dash-decoding only if no cwd field is present.

## 4. Wire categorization

All 22 distinct line shapes observed across 80 real sessions (35 371 lines):

| Line | Condition | `SemanticCategory` | `meta` |
|---|---|---|---|
| `assistant` content[].type | `thinking` | `Thinking` | from line |
| `assistant` content[].type | `text` | `AssistantText` | from line |
| `assistant` content[].type | `tool_use` | `ToolCall` | from line |
| `user` content[].type | `text` | `UserInput` | from line |
| `user` content[].type | `image` | `UserInput` | from line |
| `user` content[].type | `tool_result` & `is_error == true` | `Error` | from line |
| `user` content[].type | `tool_result` else | `ToolResult` | from line |
| `user` content as top-level string | (slash-command echoes) | `UserInput` | from line |
| `system` `subtype` | `compact_boundary` | `CompactionBegin` | from line |
| `system` `subtype` | `api_error` | `Error` | from line |
| `system` `subtype` | `turn_duration` | `StatusUpdate` | from line |
| `system` `subtype` | `stop_hook_summary` | `StatusUpdate` | from line |
| `system` `subtype` | `away_summary` | `StatusUpdate` | from line |
| `system` `subtype` | `local_command` | `StatusUpdate` | from line |
| `system` `subtype` | `scheduled_task_fire` | `StatusUpdate` | from line |
| `system` `subtype` | `bridge_status` | `Other` | from line |
| `pr-link` (top-level) | — | `StatusUpdate` | false |
| `attachment` | any of 22 subtypes | `Other` | from line |
| `permission-mode` | — | `Other` | false |
| `ai-title` | — | `Other` | false |
| `last-prompt` | — | `Other` | false |
| `file-history-snapshot` | — | `Other` | false |
| `queue-operation` | — | `Other` | false |

`Other` events default-filtered in Wire view; toggle reveals them.

### 4.1 Synthetic `TurnBegin`

The only inferred event we emit. Single rule, applied during the streaming pass:

> For each `user` line whose content includes a `text` block (real prompt, not just `tool_result`s), if `message.promptId` differs from the last-seen turn promptId, emit a synthetic `TraceEvent { category: TurnBegin, turn_index: N+1, … }` immediately before the line.

`turn_index` propagates to all subsequent events until the next `TurnBegin`. We do not emit `StepBegin`, `StepInterrupted`, `TurnEnd`, or `CompactionEnd` (no honest signal in Claude Code).

### 4.2 Header stat derivation

| Field | Source |
|---|---|
| `turn_count` | distinct `promptId` on user-text lines |
| `step_count` | 0 (not declared in `header_layout`) |
| `tool_call_count` | `tool_use` content blocks |
| `error_count` | `tool_result.is_error == true` + `system.subtype == api_error` |
| `compaction_count` | `system.subtype == compact_boundary` |
| `agent_count` | `agent-*.jsonl` files in `<session>/subagents/` |
| `total_duration_ms` | last timestamp − first timestamp |
| `total_input_tokens` | sum of `input_tokens + cache_creation_input_tokens` across `assistant.message.usage` |
| `total_output_tokens` | sum of `output_tokens` |
| `cache_read_tokens` | sum of `cache_read_input_tokens` |
| `cache_hit_pct` | `cache_read / (cache_read + input + cache_creation)` over session totals |
| `model` | most recent `assistant.message.model` excluding `<synthetic>` |

`<synthetic>` rows are compaction-generated and represent Claude Code's internal reflexes, not user-facing model selection.

## 5. UI

### 5.1 Shared (provider-agnostic)

Unchanged from today, still drives both providers:

- Left sidebar, provider chip row, top tab row (`Sessions / Reforge / Tracing`).
- Sessions list page: search, sort, project grouping, list/grid toggle, Import button, session cards.
- Statistics tab.
- Session-detail page **shell** (header layout primitive + tabs primitive).
- Wire-viewer **shell** (filter pills, search, error nav, virtualized list, scope selector). Shells operate on `TraceEvent[]` and `SemanticCategory`; cards are provider-supplied.

### 5.2 New file layout (frontend)

```
desktop-ui/src/tracing/
  features/
    sessions-explorer/         ← shared (unchanged)
    statistics/                ← shared (unchanged)
    session-picker/            ← shared (unchanged)
    session-detail/            ← shell, dispatches tabs by provider_id
    providers/
      kimi/                    ← MOVED from features/wire-viewer, etc.
        wire-viewer/
        context-viewer/
        state-viewer/
        dual-view/
        agents-panel/
      claude-code/             ← NEW
        wire-viewer/
          claude-code-wire-viewer.tsx
          cards/
            tool-call-card.tsx
            tool-result-card.tsx
            thinking-card.tsx
            assistant-text-card.tsx
            user-input-card.tsx
            compaction-card.tsx
            api-error-card.tsx
            status-update-card.tsx
            pr-link-card.tsx
            other-card.tsx
        agents-panel/
          claude-code-agents-panel.tsx
```

The existing Kimi-specific components (decision-path, turn-tree, tool-stats-dashboard, integrity-check) move into `providers/kimi/wire-viewer/` and stay Kimi-only.

### 5.3 Tab dispatch table

```ts
const TAB_RENDERERS: Record<ProviderId, Partial<Record<SessionTab, React.FC<TabProps>>>> = {
  kimi: {
    wire: KimiWireViewer,
    context: KimiContextViewer,
    state: KimiStateViewer,
    dual: KimiDualView,
    agents: KimiAgentsPanel,
  },
  claudeCode: {
    wire: ClaudeCodeWireViewer,
    agents: ClaudeCodeAgentsPanel,
  },
};
```

The session-detail shell calls `tracing_supported_tabs(providerId)` once on mount, renders only those tabs, and dispatches each tab through the table.

### 5.4 Header layout primitive

The session-detail header takes `chips: HeaderChip[]` and `stats: HeaderStats` and renders chips in declared order, formatting each per its kind (`Tokens` → `1.2M / 18K`, `Duration` → `4m 21s`, `CacheHitPct` → `87%`, `Model` → `claude-opus-4-7`, etc.). One component, two providers.

### 5.5 Claude Code Wire cards

- **Compaction card** — `Conversation compacted · trigger: manual · 308 887 → 14 033 tokens · 2m 28s` with expandable `preCompactDiscoveredTools` chip list.
- **API error card** — `API error · retry 1/10 · in 590 ms` with optional `error.type` badge.
- **Stop-hook-summary card** — `Hooks ran (2) · 10 048 ms total` expandable to per-hook command + duration.
- **Tool-call card** — tool name + truncated input JSON, click → detail panel with full input/output and linked `tool_result`.
- **Tool-result card** — rendered text or content-blocks list; red border when `is_error`.
- **Thinking card** — collapsed by default; expand to read.
- **PR-link card** — repository / PR number / clickable URL.
- **Status-update card** — generic key-value rendering for the various `system.subtype`s.
- **Other card** — type label + raw payload viewer (filtered out by default).

## 6. Wiring

### 6.1 Backend registration

In `crates/app-core/src/init/mod.rs` near line 1126 (next to where `KimiTracingProvider::new(&home, &data_dir, widx)` is registered):

```rust
let claude_root = home_dir.join(".claude/projects");
let imported_claude = data_dir.join("coding_memory/imported_claude_code");
registry.register(Arc::new(ClaudeCodeTracingProvider::new(claude_root, imported_claude)));
```

### 6.2 Tauri commands

**Reused (no change):** `tracing_list_providers`, `tracing_list_sessions`, `tracing_load_session`, `tracing_load_subagent_session`, `tracing_stats`, `tracing_open_dir`, `tracing_import`. All take `provider_id` and dispatch through `TracingRegistry`.

**Returns `Unsupported` for Claude Code (FE never calls):** `tracing_load_context`, `tracing_load_state`, `tracing_load_subagent_context`.

**New:**

```rust
#[klynt_command]
async fn tracing_supported_tabs(app: ..., provider_id: String) -> Vec<SessionTab>;

#[klynt_command]
async fn tracing_header_layout(app: ..., provider_id: String) -> Vec<HeaderChip>;
```

Both register in `desktop_macros::klynt_collect_commands![...]`. After wiring, run `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`.

## 7. Caching

- `list_sessions` cached per `(file_path, mtime)`. Cache invalidates on mtime bump or explicit refresh.
- Single-session detail loads uncached.
- `stats()` bundle cached for ~30 s to absorb tab re-renders.

## 8. Testing

### 8.1 Backend unit tests (colocated)

- `discovery.rs` — fixture dirs cover: native session, imported session, missing root, files-without-jsonl-extension, subagent files at top level (must be skipped).
- `categorize.rs` — one fixture per row of the table in §4.
- `loader.rs` — `turn_count` from distinct promptIds, tool_results don't bump turn_count, tool_call_count counts `tool_use` blocks, `error_count` includes both error sources, `compaction_count` from `compact_boundary`, token aggregation, cache_hit_pct formula, synthetic `TurnBegin` placement, model excludes `<synthetic>`, `meta` flag propagation.
- `subagent_loader.rs` — pairs of `agent-X.jsonl` + `.meta.json` listed; orphan jsonls skipped silently; subagent loading uses same pipeline.
- `summary.rs` — cwd from first JSONL line; `custom_title` from `ai-title` event with fallback to first user-text content; `last_event_at` handles trailing partial writes.
- `cache.rs` — mtime-based invalidation.
- `import.rs` — copy + meta.json round-trip.

### 8.2 Backend integration test

`crates/app-core/tests/claude_code_tracing.rs` — uses a hand-crafted minimal fixture in `crates/app-core/tests/fixtures/claude_code/` exercising every category. Asserts:

- `list_sessions` discovers the fixture
- `load_session` returns expected event count, turn count, tool-call count, error count, compaction count
- `HeaderStats` fields match hand-computed expectations
- `list_subagents` returns expected types and descriptions
- `load_subagent_session` works with the same DTO contract
- `stats()` aggregation matches expected per-project / tool-usage / token-series numbers
- `import_from_file` copies and re-discovers via `list_sessions`
- `Unsupported` returned from `load_state`, `load_context`, `load_subagent_context`

### 8.3 Frontend tests (Vitest)

- `claude-code/wire-viewer/cards/*.test.tsx` — render-snapshot per card with realistic payload fixtures.
- `session-detail/tab-dispatch.test.tsx` — declared tabs render; non-declared tabs hidden.
- `session-header/header-chip-rendering.test.tsx` — chips render in declared order with declared values.

### 8.4 Manual verification

- Open Tracing page → Claude Code chip shows non-zero count.
- Click chip → Sessions list scoped to Claude Code with project grouping by cwd.
- Open a session → header shows Claude Code chip set (no Steps, has Model).
- Wire tab renders Claude Code-specific cards (compaction card visible if session was compacted).
- Agents tab lists subagents from disk; click a subagent → wire scoped to it.
- Tabs not declared (Context / State / Dual) are not visible for Claude Code.
- Statistics tab populated when Claude Code chip is selected.
- Import → file picker → adds to list after refresh.
- Switch back to Kimi chip — Kimi UI is unchanged.

### 8.5 KCA gates

`./scripts/run_kca_validation.sh` must pass: 0 clippy warnings, fmt clean, all tests green, no doctest regressions.

## 9. Known limitations

1. **Cross-session continuity not stitched.** `--continue` and compaction-fork produce cross-file `parentUuid` references (avg ~10 per session in real data). v1 treats each `.jsonl` as one session. Future: add `predecessor_session_id` to `SessionSummary` and a "View ancestry" affordance.
2. **Sidechains rendered inline only.** `isSidechain: true` events would get a small badge in Wire view; no separate scope. Zero observed in 35 371 lines, so the inline path is not exercised in v1; if encountered after release, the badge appears.
3. **No live tail.** Manual refresh button only.
4. **Compaction has no End event.** `compact_boundary` is a single point; we render `CompactionBegin` only; `compaction_count` counts boundaries.
5. **`<synthetic>` model excluded from Model chip.** Compaction-generated reflexes, not user-facing.
6. **Lossy `<encoded-cwd>` decode.** Original `-` chars in cwd are ambiguous after encoding. We resolve cwd from inside the JSONL; the directory name is used only as a group key (mirrors Kimi's md5 hash usage).

## 10. Deferred

- **Tree tab** (parentUuid graph). Revisit if sidechains become common.
- **Cross-session ancestry stitching.**
- **Live tail** via file watcher.
- **PR-link chip on session card** in Sessions list.
- **Sidechain virtualization** as ephemeral subagent.
- **Stitching subagent transcripts** back into the parent timeline.

## 11. References

- `docs/superpowers/specs/2026-04-30-kimi-vis-tracing-design.md` — the originating spec; defines the trait, DTOs, and Sessions/Statistics layout reused here.
- `docs/superpowers/specs/2026-05-02-tracing-ui-port-design.md` — the FE port that introduced the current Tracing screen.
- `crates/app-core/src/tracing/` — current trait + Kimi provider.
- `desktop-ui/src/tracing/` — current FE.
