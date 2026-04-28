# Plugins Page — Design-Language Alignment

**Status:** Draft
**Date:** 2026-04-28
**Owner:** Jayden
**Surface:** `desktop-ui` (Tauri renderer)

## Summary

The plugins page currently sits inside the chat workspace's two-column grid, leaving a ~230px empty band on the right where the right-panel would normally dock, and uses a parallel `--ds-*` token vocabulary that produces a translucent gray-blue surface against the near-black chat slab next to it. This design realigns the plugins page with the chat page's design language: a single full-bleed slab, two-tone foreground, and pill segmented controls reused from the right-panel primitive.

The Klynt CLI plugin tab is also being demoted from a top-level plugin tab to an agent filter pill alongside Claude Code, Codex, Kimi, and opencode — closing a frontend/backend drift where `AgentSource::KlyntCli` exists in the Rust enum but is missing from the frontend `ProviderId` union.

## Problem

Two distinct issues:

1. **Layout:** `.main` (`desktop-ui/src/styles/main.css:3`) is a CSS grid with `grid-template-columns: minmax(0, 1fr) var(--right-panel-width, 230px)`. The right column is always reserved. On the plugins view, `showWorkspace` is forced false (`MainApp.tsx:1818`) so the right panel never renders — but the grid still allocates the column, producing the empty band visible in the screenshot. The `.main` panel also keeps its rounded top-left/bottom-left corners and inset hairline, which read as a chat-floating-card affordance the plugins page doesn't need.

2. **Surface and chrome divergence:** The plugins page uses `--ds-surface-card` (= `--surface-card-strong` = `rgba(255,255,255,0.04)` — translucent white) for its background, while the chat page uses `--surface-messages` (= `rgb(8,9,12)` — solid near-black). Side by side they look like two different products. The plugins page also uses three text shades (`--ds-text-strong` / `--ds-text-subtle` / `--ds-text-faint`) where the chat page uses two (`--text-stronger` / `--text-muted`), and renders its primary tabs as an underlined row where the right-panel uses pill segmented controls.

3. **Frontend/backend drift:** `crates/coding-ingest/src/event.rs:55` defines five `AgentSource` variants (`ClaudeCode`, `Codex`, `KimiCli`, `OpenCode`, `KlyntCli`); `desktop-ui/src/features/plugins/coding-memory/types.ts:2` defines four `ProviderId` variants. Sessions emitted by Klynt CLI cannot currently be filtered in the UI.

## Goals

- Plugins page fills the full area between the sidebar and the window edge, top to bottom, on every viewport.
- A single design vocabulary across the chat page, right panel, and plugins page: near-black surface, two-tone foreground, pill segmented controls.
- Klynt CLI surfaces as an agent filter pill, not a top-level plugin.
- No regressions to the chat or right-panel layout when switching between views.

## Non-goals

- Redesigning the colored event chips inside `WireViewer`/`WireFilters` — they encode domain meaning and stay.
- Restyling the secondary "Sessions / Reforge" tabs — they're already discrete and the hierarchy reads correctly.
- Touching the right-panel itself.
- Building a real Skills or MCP Servers plugin pane — they remain "Soon" placeholders.
- Backend changes to the coding-ingest pipeline — the `KlyntCli` source already flows end-to-end on the Rust side.

## Design

### 1. Layout — full width and full height

Add an `is-plugins` modifier on the `.app` root element when `appView === "plugins"`.

CSS rules added to `main.css`:

```css
.app.is-plugins .main {
  grid-template-columns: 1fr;
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
  box-shadow: none;
}
```

Wiring: extend `useAppShellOrchestration` with an `appView: "home" | "chat" | "plugins"` parameter, and append `" is-plugins"` to `appClassName` when `appView === "plugins"`. `MainApp.tsx` passes the existing `appView` state through.

The plugins pane already uses `height: 100%`, so dropping the second column gives it the full row.

The `app.is-plugins` class is removed when `appView` changes back, restoring the chat layout exactly.

### 2. Primary tabs — pill segmented control

Replace the underlined-row tabs in `PluginsView.tsx` with the existing `.panel-tabs` / `.panel-tab` primitive (`panel-tabs.css`).

Final tab set:

- **Coding Memory** — available, default selected
- **Skills** — `Soon` badge, disabled
- **MCP Servers** — `Soon` badge, disabled

Removed: ~~Klynt CLI~~ (moves to agent filter pills).

The `Soon` badge styling is tuned to read as a quiet meta-tag, not a button: low-contrast text (`--text-faint`), no border, no background — just text after the label. Disabled tabs render at `opacity: 0.55` and ignore clicks.

`PluginId` type narrows from `"coding-memory" | "skills" | "mcp" | "klynt-cli"` to `"coding-memory" | "skills" | "mcp"`.

### 3. Agent filter pills — extended set, redesigned

Update `ProviderId` in `desktop-ui/src/features/plugins/coding-memory/types.ts`:

```ts
export type ProviderId = "claudeCode" | "codex" | "kimiCli" | "openCode" | "klyntCli";
```

Update the `PROVIDERS` constant in `ProviderChips.tsx` to add `{ id: "klyntCli", label: "Klynt CLI" }` after `openCode`.

Visual redesign of `.cm-provider-chips` and `.cm-provider-chip` in `plugins.css`:

- Tighter padding (3px 10px vs 4px 10px) and slightly smaller font (`--fs-xs`).
- Default state: `color: var(--text-muted)`, `background: transparent`, `border: 1px solid var(--border-subtle)`.
- Hover: `color: var(--text-stronger)`, no background change, no border change — text-weight transition is the affordance.
- Active: `color: var(--text-stronger)`, `background: var(--surface-card-strong)`, `border-color: transparent`. No accent-color stroke.
- Count badge (`.cm-provider-chip__count`): tucked flush against the label with 4px left margin, color `--text-faint`, no separate pill.

### 4. Surface colors and tokens

In `plugins.css`:

- `.plugins-view` background: `var(--ds-surface-card)` → `var(--surface-messages)`.
- `.plugins-view__header` border-bottom: `var(--ds-border-subtle)` → `var(--border-subtle)`.
- `.plugins-view__title` color: `var(--ds-text-strong)` → `var(--text-stronger)`.
- `.plugins-view__subtitle` color: `var(--ds-text-subtle)` → `var(--text-muted)`.
- `.cm-plugin__sidebar` border-right: `var(--ds-border-subtle)` → `var(--border-subtle)`.
- Internal Coding Memory components (`SessionList`, `WireViewer`, `WireFilters`, etc.) keep their `--ds-*` tokens — those tokens already alias to `--text-strong`/`--text-subtle` etc. in the dark theme, so the visual change is concentrated on the chrome surrounding them.

The `--ds-*` indirection stays available codebase-wide; this redesign just stops using it on the plugins-page chrome where it produces a translucent surface that misreads on the near-black slab.

### 5. Tests

- `PluginsView.test.tsx` — assert that the tab list contains exactly Coding Memory, Skills, MCP Servers (no Klynt CLI), and that Coding Memory is the default selection.
- New assertion: when the component renders, the primary tab role is `tab` and the active one has `aria-selected="true"`.
- `ProviderChips` — extend the existing test (or add one) to verify the Klynt CLI pill renders and is selectable.
- Manual smoke (documented in implementation plan): verify the plugins page fills the full width on a 1440-wide viewport in `cargo tauri dev`, and that switching back to chat restores the right-panel column without flicker.

## Architecture impact

- `MainApp.tsx`: passes `appView` (or a derived boolean) to `useAppShellOrchestration`. No new state.
- `useAppShellOrchestration.ts`: appends `is-plugins` to `appClassName` when on the plugins view. No change to `appStyle`.
- `PluginsView.tsx`: tab list shrinks; tab markup switches to `.panel-tabs` / `.panel-tab` classes. Logic unchanged.
- `ProviderChips.tsx`: array gains one entry; markup unchanged.
- `types.ts`: union gains one variant.
- `plugins.css`: token swaps and pill restyling. No new selectors except for the layout modifier.
- `main.css`: one new rule (`.app.is-plugins .main { … }`).

No new components, no new files, no new dependencies.

## Risks

- **Other consumers of `ProviderId`:** Adding `"klyntCli"` to the union forces every exhaustive switch to handle it. Today only `CodingMemoryPlugin.tsx` consumes the type, and its only branch (`provider === "all" ? undefined : provider`) is a string passthrough — no compiler errors expected. Verify with `bun run typecheck`.
- **`is-plugins` class persistence:** If the modifier isn't removed when the user switches away from plugins, the chat view would render in single-column mode. Mitigation: derive the class purely from current `appView`, not a flag that needs separate cleanup.
- **Empty `centerMode === "plugins"` rendering:** `DesktopLayout.tsx:159` renders `pluginsNode` when `centerMode === "plugins"`, but only inside `<section className="main">`. Once `.main` is single-column, the plugins node uses the full row; no other layout side effects.
- **The right-panel resizer:** `.right-panel-resizer` is positioned via `right: calc(var(--right-panel-width) - 4px)`. With the right panel hidden on plugins view, the resizer is also hidden (`showWorkspace=false` excludes the markup). No conflict.

## Verification

After implementation:

- `cargo tauri dev` opens the desktop app; navigate to Plugins. The page fills the full width to the window edge, full height to the dock. Switching back to Home or Chat restores the previous layout instantly.
- The Plugins title row and tab row use the same near-black background as the chat page when chat is open in another window.
- The primary tab row reads as the same control style as the right-panel diff/plan toggles.
- Klynt CLI appears as the rightmost agent filter pill. Selecting it filters the session list (will show "no sessions" until a Klynt CLI session is recorded — that's expected; the UI plumbing is what's being verified).
- `bun run typecheck` passes.
- `bun run lint` passes.
- `bun run test` passes (vitest).
