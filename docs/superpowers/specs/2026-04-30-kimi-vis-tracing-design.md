# Kimi-vis–quality Agent Tracing — Design

**Date:** 2026-04-30
**Status:** Spec, awaiting plan
**Goal:** Bring the rich, faithful agent-tracing experience that `kimi-cli` ships in its `kimi vis` web UI into the desktop app's `Plugins → Coding Memory` page, with Kimi as the only provider in v1 but with all data-flow, types, and UI components designed so adding Claude Code, Codex, opencode, and future CLIs later is contained inside one new provider implementation.

---

## 1. Problem

The current `Plugins → Coding Memory → Sessions` view shows two raw JSON event types — `userPrompt` and `assistantMsg` — for a Kimi session that, in `kimi vis`, displays 13 distinct event types organized into turns and steps with header aggregates, scope selectors, navigation tree, filters, search, error navigation, and tool-call detail panels. The data is on disk at `~/.kimi/sessions/<workdir-hash>/<session-uuid>/wire.jsonl`; it just isn't reaching the UI because the ingestion pipeline normalizes the wire stream into nine `EventKind` variants for cross-CLI cognitive use (Reforge, Mirror, Recall) and discards the rest.

Reforge needs that normalized lane. The tracing viewer needs the raw lane. They are different consumers with different needs and should remain independent.

## 2. Scope

**In v1.** Match `kimi vis` quality for Kimi sessions, end to end:

- Sessions list with project grouping by cwd basename, per-session cards, search, sort, group toggle, list/grid toggle, Import button.
- Single-session detail view with the five top-level tabs (Wire Events, Context Messages, State, Dual, Agents).
- Wire Events tab with header counts, scope selector (main + subagents), filter pills, search, error navigation, type chips, navigation tree, virtualized event list with per-event-type cards.
- Statistics tab (per-project totals, tool-use heatmap, errors-by-tool, daily token chart, subagent-type breakdown).
- Manual refresh button. No live tail in v1.

**Out of scope for v1.**

- Live file-watch / tail. Future work; the pluggable refresh path exists.
- Cross-CLI providers (Claude Code, Codex, opencode). The trait and DTOs are defined; only the Kimi provider is implemented.
- DB persistence of raw wire envelopes. Live-read from disk only. Optional file copy via Import.

**Non-goals.**

- Replacing the existing `Plugins → Coding Memory → Sessions` tab. That tab continues to show the normalized AgentEventV1 stream and remains the substrate for Reforge debugging. The new Tracing view is a sibling sub-tab.
- Generalizing the existing `WireViewer.tsx` (which renders normalized DTOs). It is left untouched. The new tracing viewer is a parallel implementation operating on raw provider DTOs.

## 3. Architecture

### 3.1 Live read, no DB writes

All tracing reads happen at view time against `~/.kimi/sessions/`. No new SQLite tables, no migrations, no backfill. The existing `coding-ingest` adapters are not touched.

### 3.2 Provider abstraction

A single `TracingProvider` trait, implemented per CLI. v1 implements it for Kimi only. Adding a CLI later is bounded to one new submodule plus a registration line.

```rust
#[async_trait]
pub trait TracingProvider: Send + Sync {
    fn id(&self) -> &'static str;                                  // "kimi" | "claudeCode" | "codex" | …
    fn display_name(&self) -> &'static str;                        // "Kimi"
    async fn list_sessions(&self) -> Result<Vec<SessionSummary>>;
    async fn load_session(&self, sid: &str, scope: Scope) -> Result<SessionDetail>;
    async fn load_context(&self, sid: &str, scope: Scope) -> Result<Vec<ContextMessage>>;
    async fn load_state(&self, sid: &str) -> Result<SessionState>;
    async fn list_subagents(&self, sid: &str) -> Result<Vec<SubagentSummary>>;
    async fn import_from_file(&self, path: &Path) -> Result<String>;
    async fn open_dir(&self, sid: &str) -> Result<PathBuf>;
    async fn stats(&self) -> Result<StatsBundle>;
}
```

A `TracingRegistry` (held by `AppCore`) holds `HashMap<&'static str, Arc<dyn TracingProvider>>`. All Tauri commands take a `provider_id` and dispatch through the registry.

### 3.3 Provider-agnostic DTOs

Every event flows through a uniform envelope tagged with a closed `SemanticCategory` enum. The UI dispatches by category; provider-specific payload details are read inside the per-category card components.

```rust
pub struct TraceEvent {
    pub seq: u64,
    pub provider_id: &'static str,
    pub raw_kind: String,                  // "ContentPart" | "TurnBegin" | …
    pub payload: serde_json::Value,        // verbatim, untouched
    pub occurred_at: Timestamp,
    pub category: SemanticCategory,
    pub turn_index: Option<u32>,
    pub step_index: Option<u32>,
    pub parent_subagent_id: Option<String>,
}

pub enum SemanticCategory {
    TurnBegin, TurnEnd,
    StepBegin, StepInterrupted,
    Thinking, AssistantText, UserInput,
    ToolCall, ToolCallStream, ToolResult,
    StatusUpdate, Subagent,
    CompactionBegin, CompactionEnd,
    Error, Other,
}
```

`SessionSummary`, `SessionDetail`, `HeaderStats`, `ContextMessage`, `SessionState`, `SubagentSummary`, `Scope`, `StatsBundle` are all provider-agnostic and live in `tracing/types.rs`.

### 3.4 Categorization is the only Kimi-specific UI input

A function `kimi_kind_to_category(raw_kind: &str, payload: &Value) -> SemanticCategory` is the single point where Kimi vocabulary maps onto the shared categories. Example mappings:

| Kimi raw_kind | Payload condition | SemanticCategory |
| --- | --- | --- |
| `TurnBegin` | — | `TurnBegin` |
| `TurnEnd` | — | `TurnEnd` |
| `StepBegin` | — | `StepBegin` |
| `StepInterrupted` | — | `StepInterrupted` |
| `ContentPart` | `payload.type == "think"` | `Thinking` |
| `ContentPart` | `payload.type == "text"` | `AssistantText` |
| `ToolCall` | — | `ToolCall` |
| `ToolCallPart` | — | `ToolCallStream` |
| `ToolResult` | `payload.is_error == true` | `Error` |
| `ToolResult` | otherwise | `ToolResult` |
| `StatusUpdate` | — | `StatusUpdate` |
| `SubagentEvent` | — | `Subagent` |
| `CompactionBegin` | — | `CompactionBegin` |
| `CompactionEnd` | — | `CompactionEnd` |
| `metadata` | — | `Other` (filtered) |
| anything unknown | — | `Other` |

Future providers ship their own `categorize.rs` with the same shape.

### 3.5 Path resolution and discovery

`~/.kimi/sessions/<hash>/<uuid>/` where `<hash> = md5(work_dir)` for `kaos == "local"`, prefixed for non-local. The existing `crates/coding-ingest/src/adapters/kimi_cli/workdir.rs::WorkdirIndex` already implements this and reads `~/.kimi/kimi.json`. We extend it with a public `entries() -> Vec<(String, PathBuf)>` accessor and reuse it.

Sessions are discovered by globbing for any `<hash>/<uuid>/wire.jsonl`. Subagents are discovered by globbing `<session>/subagents/<id>/`.

Imported sessions live under `{klyntbot_data}/coding_memory/imported_wire/<uuid>/` with a sentinel `meta.json` recording the original path. Discovery scans both locations.

### 3.6 Header-stat derivation

Computed in a single streaming pass over `wire.jsonl` during `load_session`:

- `turn_count` = number of `TurnBegin` events
- `step_count` = number of `StepBegin` events
- `tool_call_count` = number of `ToolCall` events
- `error_count` = number of `ToolResult` with `payload.is_error == true`
- `compaction_count` = number of `CompactionBegin` events
- `agent_count` = number of distinct `subagents/<id>/` directories
- `total_duration_ms` = `(last event timestamp) - (first event timestamp)`
- `total_input_tokens` = sum of `StatusUpdate.token_usage.input_other + input_cache_read + input_cache_creation` across the session
- `total_output_tokens` = sum of `StatusUpdate.token_usage.output`
- `cache_hit_pct` = `input_cache_read / (input_cache_read + input_other + input_cache_creation) * 100` taken from the most recent `StatusUpdate`
- `model` is `null` in v1 (Kimi's `StatusUpdate` carries `message_id` but not a model name; deferring to a future `metadata` line parse)

For a 2229-line wire file this pass is microseconds. Sessions over 50MB stream-parse and cap to the most recent 50k events, surfacing a "showing recent N of M" banner.

### 3.7 Caching

`list_sessions` results are cached per `(wire_path, mtime)` so repeated FE polls don't re-scan unchanged files. Cache invalidates on file modification or explicit refresh. Single-session detail loads are not cached — they are cheap and the user expects the refresh button to be authoritative.

## 4. UI

### 4.1 Location

New sub-tab under `Plugins → Coding Memory`, sitting between `Sessions` and `Reforge`. The Tracing sub-tab itself contains two top-level tabs: `Sessions` (default) and `Statistics`, mirroring `kimi vis`.

### 4.2 Sessions list view

Toolbar:

- Search input (substring match against title, session id, cwd).
- Sort dropdown: `Recent` (default, last_event_at desc), `Created`, `Size`, `Errors`.
- `Imported` toggle (filters to imported sessions only).
- `Group` toggle (group by project, on by default).
- list/grid toggle (grid by default).
- `Import` button (file picker → copy → refresh).
- Right-aligned session count.

Body, grouped mode: collapsible folder cards per project. Header shows folder icon, project basename, count, full cwd path right-aligned. Body is a 3-column responsive grid of session cards.

Session card:

- Top row: 8-char id, download icon, relative time ("1d ago").
- Title (single line, ellipsis): first user prompt truncated; "Untitled Session" if none.
- Badges: `wire`, `context` (green when each file exists).
- Size right-aligned (sum of `wire.jsonl` + `context.jsonl` bytes, KB/MB).
- Counts row: `N turn · N step · N tool` with per-metric color accents.
- Bottom row: `⏱ duration · ⓘ N error` (error count omitted when zero).

Footer: `N sessions · N projects · N MB total`.

### 4.3 Single-session detail view

Header bar:

- Back arrow + title `Kimi Agent Tracing`. (Title is provider-driven: `<displayName> Agent Tracing`.)
- Below: truncated session id, counts (`N turns · N steps · N tool calls · N errors · N compactions · N agents`), wallclock total, in/out tokens, cache %, action buttons: `Open Dir`, `Copy DIR`, download (saves wire.jsonl), refresh.

Five top-level tabs:

1. **Wire Events** (default) — see 4.4.
2. **Context Messages** — virtualized list of `context.jsonl` rows. Role chip per row (`system_prompt | checkpoint | user | assistant | tool | usage`), expandable content. Markdown render for assistant/user; monospace for tool/system. `_usage` rows show token + cost deltas inline. Search input at top.
3. **State** — pretty-printed `state.json` with two sections: Session (custom_title, plan_mode, archived flags) and Todos (checkbox list with status pills: done / in_progress / cancelled).
4. **Dual** — split view: Wire Events left, Context Messages right, scroll-locked by occurrence time. Hover on a wire event highlights its context counterpart.
5. **Agents** — subagent timeline. Gantt rows, x-axis is wallclock, each row spans `created_at → last event` with the subagent's `meta.description` as label. Click → opens that subagent's wire view via the scope selector.

### 4.4 Wire Events tab

Scope selector (top): "Main Agent" pill plus chips per subagent labeled with `meta.description`. Selecting a subagent re-loads from that subagent's wire file. Chip colors hashed from `agent_id` for stable identity.

Filter row:

- Quick presets: `All Events`, `Errors Only`, `Tool Calls`, `Thinking`, `Approvals`, `Sub-agents`. Each sets the underlying type-chip selection.
- Search field (`/` shortcut).
- Error counter with up/down navigation (`e` shortcut).
- Total event counter.
- Expand/collapse all toggle.
- View-mode toggles: `Chart`, `Events` (default), `Timeline`, `Decisions`. Chart = events/sec sparkline; Timeline = same data with type bands; Decisions = pre-applied filter to ToolCall + Approval events.
- Completeness % indicator.

Type-chip row: every distinct `raw_kind` found in the session, multi-select, color-coded per kimi vis palette.

Left navigation tree: collapsible Turn list. Each Turn label is its truncated user prompt. Expanded shows Step 1, Step 2, … with tool count in parens. Click → scroll-to in the event list.

Event list (virtualized, react-virtuoso): one card per event, dispatched by `event.category`:

- `TurnBegin` — pill with full user prompt content.
- `TurnEnd` — closing rule with total turn duration.
- `StepBegin` — separator with "Step N" + `+Δ` since previous event.
- `StepInterrupted` — red bar with reason if any.
- `Thinking` — gray italic block; collapsible.
- `AssistantText` — assistant message text, markdown-rendered via existing `Markdown` component.
- `UserInput` — same as TurnBegin in v1 (Kimi doesn't emit standalone UserInput; reserved for future providers).
- `ToolCall` — tool name + args preview; click opens a new tracing-local `ToolCallDetail` sidecar (a fresh component under `tracing/event-cards/`, not the existing one which is bound to `WireEventDto`). Pairs the call with its matching `ToolResult` by `payload.id`.
- `ToolCallStream` — small grey "…streaming arguments…" inline marker.
- `ToolResult` — paired with its ToolCall; ok/error icon, duration, result preview (markdown for `Read`, monospace for `Shell`).
- `StatusUpdate` — compact one-liner: `ctx: 4.5% · 11.7k/262k · in: 11.7k (79% cache) · out: 0.1k`.
- `Subagent` — nested card showing the wrapped event with `↳ <subagent description>` prefix and click-through to that subagent's full view.
- `CompactionBegin` / `CompactionEnd` — bracket markers visually spanning compressed events.
- `Error` — red card with tool name and message.
- `Other` — generic raw-payload card (JSON dump, collapsed by default).

Each card has timestamp left-aligned and `+Δ` right-aligned, formatted `+5.45s` / `+66ms` / `+800ms`.

### 4.5 Statistics tab

Cross-session aggregates for the selected provider:

- Per-project totals table (sessions, turns, tool calls, errors, total tokens).
- Tool-use heatmap (tool × session, success-rate-tinted cells).
- Errors-by-tool table, sorted descending.
- Daily token-spend line chart.
- Subagent-type frequency bar chart.

Charts use `recharts` if already a dependency; otherwise plain SVG. To be confirmed during plan.

## 5. File layout

### 5.1 New Rust files

```
crates/app-core/src/tracing/
  mod.rs                       # public surface
  provider.rs                  # TracingProvider trait
  registry.rs                  # TracingRegistry
  types.rs                     # SessionSummary, TraceEvent, SemanticCategory, etc.
  providers/
    kimi/
      mod.rs                   # KimiTracingProvider impl
      discovery.rs             # session + subagent discovery
      loader.rs                # wire.jsonl streaming + header stats
      context_loader.rs        # context.jsonl streaming
      state_loader.rs          # state.json parser
      subagent_loader.rs       # meta.json + subagent wire counts
      import.rs                # file copy into imported_wire/
      cache.rs                 # mtime-keyed memoization
      stats.rs                 # cross-session aggregations
      categorize.rs            # kimi_kind_to_category
```

### 5.2 New shared DTO file

```
crates/desktop-shared/src/commands/tracing.rs   # specta::Type DTOs
```

### 5.3 New desktop command file

```
crates/desktop/src/commands/tracing.rs          # #[klynt_command] adapters
```

Register each command path in `klynt_collect_commands![…]` in `specta_builder.rs`.

### 5.4 New frontend files

```
desktop-ui/src/features/plugins/coding-memory/tracing/
  TracingTab.tsx
  providers.ts                  # provider chrome registry (display name, accent)
  SessionsListView.tsx
  SessionsToolbar.tsx
  SessionCard.tsx
  ProjectGroup.tsx
  SessionDetailView.tsx
  DetailHeader.tsx
  ScopeSelector.tsx
  WireEventsTab.tsx
  WireFilters.tsx               # tracing-local; not the existing one
  TurnTree.tsx                  # tracing-local
  EventTypeChip.tsx
  event-cards/
    TurnBeginCard.tsx
    TurnEndCard.tsx
    StepBeginCard.tsx
    StepInterruptedCard.tsx
    ThinkingCard.tsx
    AssistantTextCard.tsx
    ToolCallCard.tsx
    ToolCallStreamCard.tsx
    ToolResultCard.tsx
    StatusUpdateCard.tsx
    SubagentEventCard.tsx
    CompactionMarkerCard.tsx
    ErrorCard.tsx
    OtherEventCard.tsx
  ContextMessagesTab.tsx
  StateTab.tsx
  DualTab.tsx
  AgentsTab.tsx
  StatisticsView.tsx
  useTracingSessions.ts
  useTracingSession.ts
  types.ts                      # re-export from bindings
  tracing.css
```

`tracing.css` is `@import`-ed from `desktop-ui/src/styles/index.css`. All sizes use `--fs-*` tokens.

### 5.5 Modifications to existing files

- `desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx` — add `'tracing'` to the `secondaryTab` union, render `<TracingTab />` when active.
- `crates/coding-ingest/src/adapters/kimi_cli/workdir.rs` — add `pub async fn entries(&self) -> Vec<(String, PathBuf)>`.
- `crates/coding-ingest/src/adapters/kimi_cli/wire_file.rs` — already exposes `parse_line` and `collect_events`; the loader composes them line-by-line. No change required unless an iterator helper proves useful during implementation.
- `crates/desktop/src/specta_builder.rs` — register new command paths.
- `crates/app-core/src/lib.rs` and init wiring — instantiate `TracingRegistry` with the Kimi provider during setup; store on `AppCore`.

The existing `crates/coding-ingest/src/adapters/kimi_cli/mapper.rs` (the normalization mapper) and `crates/desktop-shared/src/commands/coding_memory.rs` are **not** touched.

## 6. Tauri commands

All in `crates/desktop/src/commands/tracing.rs`, all `#[klynt_command]`:

| Command | Args | Returns |
| --- | --- | --- |
| `tracing_list_providers` | — | `Vec<ProviderInfo>` |
| `tracing_list_sessions` | `provider_id` | `Vec<SessionSummary>` |
| `tracing_load_session` | `provider_id, session_id, scope` | `SessionDetail` |
| `tracing_load_context` | `provider_id, session_id, scope` | `Vec<ContextMessage>` |
| `tracing_load_state` | `provider_id, session_id` | `SessionState` |
| `tracing_list_subagents` | `provider_id, session_id` | `Vec<SubagentSummary>` |
| `tracing_import` | `provider_id, file_path` | `String` (new session id) |
| `tracing_open_dir` | `provider_id, session_id` | `()` |
| `tracing_stats` | `provider_id` | `StatsBundle` |

Each adapter delegates to `AppCore::tracing_registry().get(provider_id)?.method(...)` and is one-line. Every `app-core` handler method has `#[tracing::instrument(skip(self), err)]` per CLAUDE.md.

## 7. Build sequence

1. Trait, types, registry — compile-only skeleton with empty Kimi impl.
2. Kimi provider — discovery, loader, categorize. Unit tests against the example session at `~/.kimi/sessions/89cf5562870a0854ac789a2b74eae924/e3bc9d64-60fc-4e56-8cec-36491a6a2439/wire.jsonl`.
3. Tauri commands wired; `cargo tauri dev` once to regenerate `desktop-ui/src/bindings.ts`.
4. Sessions list view end-to-end.
5. Detail header + Wire Events list rendering for all categories.
6. Wire Events filters / search / nav / tree / scope selector.
7. Context Messages, State, Dual, Agents tabs.
8. Statistics tab.
9. Import button.
10. Visual polish to match `kimi vis` palette and typography.

Each step is one or more commits; the implementation plan turns each into tasks with verification criteria.

## 8. Testing

- Unit tests in `app-core/src/tracing/providers/kimi/` for discovery, loader header-stat aggregation, categorize mapping (table-driven), import.
- Integration test: load the example session above, assert header counts match the known truth (17 turns, 299 steps, 407 tool calls, 22 errors, 1 compaction, 5 agents).
- Frontend unit tests with Vitest for `SessionCard` rendering, event-card dispatch by category, filter-preset application.
- Manual smoke test: open desktop, navigate to `Plugins → Coding Memory → Tracing`, verify sessions list, open a session, walk through all five tabs, verify scope selector switches subagents, verify Import accepts a foreign `wire.jsonl`.

## 9. Performance and limits

- Single-pass streaming over `wire.jsonl`; 5MB files load in well under 100ms in development.
- Files larger than 50MB stream-parse and cap to the most recent 50,000 events with a "showing recent N of M" banner.
- Sessions list cached per `(wire_path, mtime)`.
- Single-session detail is not cached; refresh button is authoritative.
- Frontend uses `react-virtuoso` for both the events list and the context list.

## 10. Extending to other CLIs

Adding a CLI later is bounded:

1. Implement `XTracingProvider: TracingProvider` under `app-core/src/tracing/providers/x/` (with that CLI's discovery, loader, categorize).
2. Register in `AppCore::init`.
3. Add an entry to `desktop-ui/src/features/plugins/coding-memory/tracing/providers.ts` with display name and accent.
4. If the CLI introduces a new event kind that does not map onto an existing `SemanticCategory`, add one variant to the enum, add one new card under `event-cards/`, register in the dispatch table. Roughly 30 lines of UI work.

No changes to `TracingTab`, `SessionsListView`, `WireEventsTab`, `WireFilters`, `TurnTree`, header, scope selector, or any other shared UI file.

## 11. Open questions

- **Charting library.** Whether to add `recharts` for the Statistics tab or stick to plain SVG. To be resolved during the plan.
- **Color palette.** Match `kimi vis` exact hex values or adapt to the existing klyntbot dark/light themes? Visual polish step (10) decides.

## 12. Risks

- **Kimi format drift.** A future kimi-cli release may add event kinds. The `Other` category absorbs unknowns gracefully; the categorize table is the single point of update.
- **Wire file rotation.** Kimi-cli does not currently rotate session files. If it begins doing so, the Import button is the recovery path; durable mirroring would be a future v2.
- **Subagent depth.** v1 supports one level of subagents (Kimi's current model). Recursive subagents would need a tree-shaped scope selector; out of scope.
