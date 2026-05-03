# Tracing UI Port — Design

**Date:** 2026-05-02
**Status:** Spec
**Supersedes:** `2026-04-30-kimi-vis-tracing-plan.md` frontend phases (23–47)
**Backend foundation kept:** Phases 1–22 of the prior plan remain valid; this doc only redoes the UI plus a small backend extension set.

## 1. Goal

Replace the existing Coding Memory plugin's tracing surface with a verbatim port of the upstream Apache-2.0 tracing visualization SPA found in `references/kimi-cli/vis/`. Port covers all features (sessions explorer, wire viewer, agents panel, context viewer, dual view, state viewer, statistics, session picker). The port lives as a self-contained Tailwind v4 island inside `desktop-ui/`; the rest of the app remains plain CSS / BEM.

## 2. Success criteria

1. The Coding Memory plugin route renders the ported SPA as its only surface — no nested tabs, no legacy `WireViewer` co-existing.
2. Sessions discovered by `KimiTracingProvider` show up in the ported sessions explorer with correct totals, turns, sizes.
3. Wire events, context messages, state, subagents, and aggregate statistics all render with parity to upstream behavior.
4. Theme switches across klyntbot's `dark` / `light` / `dim` / `system` re-theme the tracing surface — no hardcoded colors leak through.
5. `cargo nextest run -p app-core` and `cd desktop-ui && bun run typecheck && bun run lint && bun run test` all pass with zero warnings inside `src/tracing/`.
6. License compliance: Apache-2.0 NOTICE retained, attribution lives in three contained locations (project-root `THIRD_PARTY_NOTICES.md`, `desktop-ui/LICENSES/apache-2.0-tracing-ui.txt`, and an SPDX header per ported file). No upstream project-name strings inside identifiers, component names, file paths, BEM classes, or comments outside the SPDX line.
7. Backend has zero clippy warnings inside `crates/app-core/src/tracing/`.

## 3. Architecture

### 3.1 Tailwind island

A single Tailwind v4 island at `desktop-ui/src/tracing/`. The `@tailwindcss/vite` plugin is configured with a `content` glob restricted to `src/tracing/**/*.{ts,tsx,css}`. No Tailwind utilities land in CSS for any other folder. The rest of `desktop-ui` is unaffected.

The root `vite.config.ts` adds the plugin, but the Tailwind config + entry CSS live entirely under `src/tracing/`.

### 3.2 Mounting point

The Coding Memory plugin's route (currently `desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx`) is replaced. The plugin entry now imports and renders `<TracingApp />` from `src/tracing/index.tsx`. No wrapper tabs.

### 3.3 Backend integration

The ported app talks to klyntbot via `src/tracing/lib/api.ts`. This module exports the same function surface the upstream SPA expects but re-implements each call against `@tauri-apps/api/core`'s `invoke()`. No HTTP, no `fetch`. The cache module is preserved unchanged.

The adapter pins to `KimiTracingProvider` — the upstream SPA is single-provider by design. Provider switching is out of scope.

## 4. File layout

```
desktop-ui/
├── src/
│   └── tracing/
│       ├── index.tsx                  # Mount: renders <TracingApp />
│       ├── App.tsx                    # Top-level shell (replaces upstream App)
│       ├── components/
│       │   ├── markdown.tsx
│       │   └── ui/                    # Radix-based primitives (shadcn-style)
│       ├── features/
│       │   ├── sessions-explorer/
│       │   ├── wire-viewer/
│       │   ├── agents-panel/
│       │   ├── context-viewer/
│       │   ├── dual-view/
│       │   ├── state-viewer/
│       │   ├── statistics/
│       │   └── session-picker/
│       ├── hooks/
│       ├── lib/
│       │   ├── api.ts                 # Tauri-backed; same function surface
│       │   ├── cache.ts               # Ported as-is
│       │   └── utils.ts               # Ported as-is
│       ├── styles/
│       │   ├── tracing.css            # Tailwind entry + theme bridge
│       │   └── theme-bridge.css       # CSS that maps klyntbot tokens → Tailwind theme vars
│       └── tailwind.config.ts         # Content glob: ./src/tracing/**
├── LICENSES/
│   └── apache-2.0-tracing-ui.txt      # Apache-2.0 license text
THIRD_PARTY_NOTICES.md                 # Project-root attribution file
```

Component file names are ported verbatim (e.g., `wire-viewer.tsx`, `tool-call-detail.tsx`, `turn-tree.tsx`, `session-card.tsx`) — they describe what the file does.

## 5. Dependency additions

Added to `desktop-ui/package.json`:

```
tailwindcss            ^4
@tailwindcss/vite      ^4
@radix-ui/react-collapsible
@radix-ui/react-scroll-area
@radix-ui/react-select
@radix-ui/react-separator
@radix-ui/react-tabs
@radix-ui/react-tooltip
@radix-ui/react-alert-dialog
radix-ui
lucide-react
streamdown
class-variance-authority
clsx
tailwind-merge
tw-animate-css
@fontsource-variable/inter
```

`react-virtuoso` is already a dependency. No new dev dependencies.

## 6. Backend extensions

The current backend exposes 10 Tauri commands. The port needs three more plus schema additions on `SessionSummary`.

### 6.1 New Tauri commands

| Command | Args | Returns | Notes |
|---|---|---|---|
| `tracing_load_subagent_session` | `provider_id, session_id, agent_id` | `SessionDetail` (wire events for the subagent) | Per-subagent wire stream |
| `tracing_load_subagent_context` | `provider_id, session_id, agent_id` | `Vec<ContextMessage>` | Per-subagent context |
| `tracing_session_summary` | `provider_id, session_id` | `SessionSummary` | Per-session aggregate (turns/steps/tool_calls/errors/compactions/duration/tokens/sizes) |

Each goes through `app-core::tracing_handlers` with `#[tracing::instrument(skip(self), err)]`, then a thin `klynt_command` adapter in `crates/desktop/src/commands/tracing.rs`, then specta registration, then `bun run tauri dev` to regenerate `bindings.ts`.

`KimiTracingProvider` gains the corresponding methods. Session-summary aggregation reuses `stats::aggregate` logic scoped to one session.

### 6.2 SessionSummary field additions

The existing `SessionSummary` DTO (in `crates/app-core/src/tracing/types.rs`) adds:

- `work_dir: Option<String>`
- `work_dir_hash: String`
- `has_wire: bool`
- `has_context: bool`
- `has_state: bool`
- `wire_size: u64`
- `context_size: u64`
- `state_size: u64`
- `total_size: u64`
- `subagent_count: u32`
- `imported: bool`
- `metadata: Option<SessionMetadataInfo>` — see 6.3

### 6.3 SessionMetadataInfo (new struct)

```
pub struct SessionMetadataInfo {
    pub session_id: String,
    pub title: String,
    pub title_generated: bool,
    pub archived: bool,
    pub archived_at: Option<i64>,
    pub auto_archive_exempt: bool,
    pub wire_mtime: Option<i64>,
}
```

For v1, the Kimi provider returns `archived: false`, `auto_archive_exempt: false`, `archived_at: None`, `title_generated: false`, `title` = filesystem-derived. Archive functionality is deferred. The struct exists so the adapter can populate the upstream-expected shape without `null` blowups.

### 6.4 Endpoints we do not implement

- **Session download** — upstream uses an HTTP URL; we'd need a Tauri file-dialog flow. Defer; UI button shows but is disabled.
- **Session delete** — upstream calls `DELETE`. We do not expose deletion. UI button hidden.
- **`getVisCapabilities`** — adapter returns a hardcoded `{ open_in_supported: true }`.

## 7. Adapter layer (`src/tracing/lib/api.ts`)

The adapter exports the same function names the upstream SPA imports — the ported components remain unchanged. Each function is a thin wrapper around `invoke()`:

| Upstream-shaped function | Backing Tauri command | Reshape work |
|---|---|---|
| `listSessions()` | `tracing_list_sessions` | Map `SessionSummary` → upstream `SessionInfo` shape |
| `getWireEvents(sessionId)` | `tracing_load_session` | Wrap event list in `{ total, events }` |
| `getContextMessages(sessionId)` | `tracing_load_context` | Wrap in `{ total, messages }` |
| `getSessionState(sessionId)` | `tracing_load_state` | Pass through |
| `getSessionSummary(sessionId)` | `tracing_session_summary` | Pass through |
| `getSubagents(sessionId)` | `tracing_list_subagents` | Map `SubagentSummary` → upstream `SubagentInfo` |
| `getSubagentWireEvents(sid, aid)` | `tracing_load_subagent_session` | Wrap |
| `getSubagentContextMessages(sid, aid)` | `tracing_load_subagent_context` | Wrap |
| `getAggregateStats()` | `tracing_stats` | Field-name remap (`tool_usage`, `daily_usage`, `per_project`) |
| `getVisCapabilities()` | n/a | Return hardcoded `{ open_in_supported: true }` |
| `openInPath("finder", path)` | `tracing_open_dir` | Pass through |
| `importSession(file)` | `tracing_import` | File goes via Tauri file-dialog before invoke; adapter handles the dialog |
| `getSessionDownloadUrl()` | n/a | Returns `""`; the download button is disabled in the adapter layer |
| `deleteSession()` | n/a | Throws "not supported"; the delete button is hidden in the adapter layer |

Provider ID is hardcoded to `"kimi"` everywhere in the adapter — no upstream call has a provider concept.

## 8. Theme mapping

Klyntbot's design tokens (`desktop-ui/src/styles/ds-tokens.css`) are the source of truth. The tracing island bridges them into Tailwind theme variables.

`src/tracing/styles/theme-bridge.css` declares CSS variables in the tracing root selector that Tailwind v4 reads via `@theme inline`:

```
.tracing-root {
  --color-background: var(--ds-bg);
  --color-foreground: var(--ds-fg);
  --color-card: var(--ds-surface);
  --color-card-foreground: var(--ds-fg);
  --color-muted: var(--ds-bg-muted);
  --color-muted-foreground: var(--ds-fg-muted);
  --color-accent: var(--ds-accent);
  --color-border: var(--ds-border);
  --color-ring: var(--ds-accent);
  --color-destructive: var(--ds-error);
  --color-primary: var(--ds-accent);
  --color-secondary: var(--ds-bg-elevated);
  /* + radius, font, spacing tokens */
}
```

Tailwind v4's `@theme inline { --color-background: var(...); }` directive in `tracing.css` references these. When klyntbot's theme switches (root class on `<html>`), the `--ds-*` values change → Tailwind utility colors re-render automatically.

Token mapping table (klyntbot → Tailwind):

| Tailwind token | Klyntbot token |
|---|---|
| `background` | `--ds-bg` |
| `foreground` | `--ds-fg` |
| `card` | `--ds-surface` |
| `popover` | `--ds-surface-elevated` |
| `muted` | `--ds-bg-muted` |
| `muted-foreground` | `--ds-fg-muted` |
| `accent` | `--ds-accent` |
| `border` | `--ds-border` |
| `input` | `--ds-border` |
| `ring` | `--ds-accent` |
| `destructive` | `--ds-error` |
| `primary` | `--ds-accent` |
| `secondary` | `--ds-bg-elevated` |

Any tokens upstream uses that have no direct klyntbot equivalent get added to `ds-tokens.css` rather than hardcoded in the bridge. The audit will surface those during port.

Typography: upstream uses Inter Variable. We add it via `@fontsource-variable/inter` and apply it only inside `.tracing-root`. The rest of klyntbot keeps its system font stack.

## 9. Files to delete

When the port lands, the following are removed in the same commit:

- `desktop-ui/src/features/plugins/coding-memory/tracing/` — entire folder (replaced)
- `desktop-ui/src/features/plugins/coding-memory/WireViewer.tsx`
- `desktop-ui/src/features/plugins/coding-memory/TurnTree.tsx`
- `desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx` — rewritten as a 5-line shell rendering `<TracingApp />`
- `desktop-ui/src/styles/tracing.css` — replaced by `src/tracing/styles/tracing.css`
- The `@import "./tracing.css"` line in `desktop-ui/src/styles/index.css`

Backend tracing module under `crates/app-core/src/tracing/` is **kept** with the additions in §6. Existing fixtures kept; we add a richer fixture for visual testing of statistics (see §11).

## 10. Attribution scheme

### 10.1 Project root

`THIRD_PARTY_NOTICES.md` — single canonical attribution file. Lists each third-party component, license, source URL, and which subtree contains the port. Apache-2.0 NOTICE text retained verbatim where required.

### 10.2 License copy

`desktop-ui/LICENSES/apache-2.0-tracing-ui.txt` — copy of the Apache-2.0 license text. Required by the license.

### 10.3 Per-file SPDX header

Every ported `.ts`/`.tsx` file starts with:

```
// SPDX-License-Identifier: Apache-2.0
// Derived from upstream Apache-2.0 source. See THIRD_PARTY_NOTICES.md.
```

No upstream project name in the header.

### 10.4 Identifiers

Component names, types, functions, variables, file paths, BEM/Tailwind class names, comments inside the file body — all generic. The folder is `tracing/`. The root component is `<TracingApp />`. The API module is `lib/api.ts`. No upstream project name appears anywhere except the SPDX header line and the two attribution files.

## 11. Fixture additions

Existing fixture (`crates/app-core/tests/fixtures/kimi/sessions/abc123hash/sess-fixture-001/`) has 10 wire events — enough for unit tests, not enough for visual smoke testing.

Add a richer fixture: `sess-fixture-rich/` with ~150 wire events spanning multiple turns, multiple tool calls including errors, a compaction marker, two subagents, and a state.json with active todos. This drives visual verification of statistics charts, agents panel Gantt, and integrity check.

## 12. Out of scope

- Live tail / file watcher (manual refresh button only)
- Scroll-locked dual-view sync (port the layout, not the sync)
- Session download
- Session delete
- Provider switcher (single-provider)
- Cross-CLI provider expansion
- Archive functionality (`metadata.archived` always false from Kimi provider)

## 13. Risks

1. **Tailwind v4 + plain-CSS coexistence.** Tailwind v4 uses `@theme` and `@layer` directives. We must verify that the Tailwind plugin's emitted CSS doesn't leak utility classes into other parts of the app. Mitigation: strict `content` glob limited to `src/tracing/**`; smoke-test by inspecting the production bundle.
2. **shadcn-style Radix wrappers depend on Tailwind utility classes.** The ported `components/ui/*` files use Tailwind classes directly. They only render correctly inside the island. Mitigation: never import from `src/tracing/` outside the island.
3. **Theme-bridge token gaps.** Some upstream colors (e.g., chart palettes, syntax-highlight colors) don't have klyntbot equivalents. We'll add new `--ds-chart-*` tokens to `ds-tokens.css` rather than hardcode.
4. **Backend session metadata is incomplete.** Upstream expects `archived`, `auto_archive_exempt`, `wire_mtime`. We stub these. UI features that depend on them (e.g., archive filter in the toolbar) will appear functional but be no-ops until backend support lands.
5. **Bindings regen flow.** Adding 3 new Tauri commands requires `cargo tauri dev` to regenerate `desktop-ui/src/bindings.ts`. Skipping the regen breaks the `bindings_are_current` test.
6. **Bundle size.** Tailwind + 7 Radix packages + lucide-react + streamdown adds ~80–120 KB gzipped to the desktop bundle. Acceptable for a desktop app; flagged for awareness.

## 14. Implementation phasing

The detailed plan is the writing-plans output. High-level phasing intent:

1. **Backend extensions** — 3 new commands + `SessionSummary` field additions + bindings regen + tests.
2. **Tailwind island setup** — config, theme bridge, smoke-test.
3. **Adapter layer** — `src/tracing/lib/api.ts` with all upstream-shaped functions wired to `invoke()`. Unit-tested with mocked invoke.
4. **Component port — bottom-up** — `lib/utils.ts`, `lib/cache.ts`, `components/ui/*`, `components/markdown.tsx`, `hooks/use-theme.ts`. Each verbatim with SPDX header.
5. **Feature port — leaf to root** — sessions-explorer first (most visible entry point), then wire-viewer (largest), then agents-panel, context-viewer, dual-view, state-viewer, statistics, session-picker.
6. **Mount + cleanup** — replace `CodingMemoryPlugin.tsx`, delete legacy files in the same commit.
7. **Fixture enrichment** — add `sess-fixture-rich/` for visual smoke.
8. **Polish** — theme-bridge token gaps, attribution files, lint/clippy/typecheck pass.

## 15. Verification gates

Before merging:

- `cargo nextest run -p app-core` green
- `cargo clippy -p app-core --all-targets` zero warnings inside `tracing/`
- `cd desktop-ui && bun run typecheck` green
- `cd desktop-ui && bun run lint` zero new errors
- `cd desktop-ui && bun run test` green
- Manual smoke: every feature surface renders against the rich fixture; theme switch (dark/light/dim/system) re-themes the tracing surface; subagent navigation works; statistics renders with non-empty charts.
