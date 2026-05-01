# Global Assistant ⇄ Code mode switch — UI design

**Date:** 2026-05-01
**Status:** Design — ready for plan
**Scope:** Frontend (`desktop-ui/`) only

## Summary

Add a global app-level mode switch that toggles the main content area between today's **Assistant** experience (unchanged) and a new **Code** experience: a Codex-style landing with a hero task input and a project grid. The switch sits in the very top row of the left panel — aligned with the macOS traffic-light reserve and the existing left-panel collapse toggle — rendered as a small segmented pill.

This pass is UI-only. It does **not** change skill routing, agent prompts, tool gating, or any backend wiring. Mode is a frontend preference persisted to `localStorage`.

## User-visible behavior

1. The top row of the sidebar — currently a 32px spacer reserving room for the macOS traffic lights — becomes a real row holding (left → right): traffic-light reserve, the existing left-panel collapse toggle, a flexible spacer, the new **Assistant | Code** segmented pill.
2. Clicking a segment switches the mode globally, persists to `localStorage`, and survives app restart. Default on first launch is `assistant`.
3. The sidebar nav below the top row (New chat, Search, Calendar, Plugins, Automations, Project) and the Chats / Settings / Upgrade footer are **identical in both modes**.
4. **Assistant mode** renders the main content area exactly as today — no visual change to anything below the top row.
5. **Code mode** replaces the main content area with `<CodeLanding />`:
   - **Hero block.** Centered column, max-width ~640px. `<h1>What should we build today?</h1>` (28px), subtitle, and a rounded task input ("Describe a task…") with a small ↑ send affordance. The input collects a string; wiring to actual task creation is **out of scope** for this pass.
   - **Projects section.** Header reads `Projects` + count, with right-aligned `Import` and `+ New` ghost buttons. Below: a 3-column responsive grid of project cards (drops to 2 / 1 below ~640px / ~420px). Each card shows `📦 name`, a meta line (`N sessions` or `edited Xh ago`), and the current branch in cyan mono at the bottom. Last cell is a dashed `+ New project` card.
   - **Empty state** (zero workspaces). Hero collapses to a single `Get started` line. The grid is replaced by a centered pair of large CTAs: a filled white **Create first project** button and an outlined **Import existing repo** button. Wire-up calls the existing add-workspace and import handlers.

## Non-goals

- No skill-routing, agent-prompt, model-gating, or tool-gating changes based on mode.
- No new entity (`CodeProject`, etc.). Projects in the grid are existing `WorkspaceInfo` records, just rendered as cards.
- No changes to the sidebar nav below the top row.
- No settings UI for the mode (the segmented pill is the only control).
- No telemetry, mirror signal, or analytics on mode changes.
- No context pills under the task input (`in: <workspace>`, `branch: <name>`) in this pass — that's a follow-up.

## State & persistence

- New hook **`useAppMode()`** at `desktop-ui/src/features/app/hooks/useAppMode.ts`.
- Backed by a small singleton store exposed via `useSyncExternalStore`.
- Persisted to `localStorage` under the key `klynt.appMode`.
- Returns `{ mode, setMode }`. `mode` is `"assistant" | "code"`. Default `"assistant"`.
- No `config.json` change. No Tauri command. No backend involvement.

## Files

### New

| Path | Purpose |
|---|---|
| `desktop-ui/src/features/app/components/AppModeSwitch.tsx` | Segmented-pill mode switch component. |
| `desktop-ui/src/features/app/hooks/useAppMode.ts` | Mode store + hook + localStorage persistence. |
| `desktop-ui/src/features/app/hooks/useAppMode.test.ts` | Hydration & persistence tests. |
| `desktop-ui/src/features/app/components/AppModeSwitch.test.tsx` | Render + interaction tests. |
| `desktop-ui/src/features/coding/components/CodeLanding.tsx` | Hero + project grid + empty state for Code mode. |
| `desktop-ui/src/features/coding/components/CodeLanding.test.tsx` | Render, click, empty-state tests. |
| `desktop-ui/src/styles/app-mode-switch.css` | Switch styles (BEM, design-token-driven). |
| `desktop-ui/src/styles/code-landing.css` | Landing-page styles. |

### Changed

| Path | Change |
|---|---|
| `desktop-ui/src/features/app/components/SidebarChatLayout.tsx` | Replace the `.sidebar-chat__topbar` spacer with a real row containing traffic-light reserve, existing collapse toggle, flexible spacer, and `<AppModeSwitch />` right-aligned. The rest of the file is unchanged. |
| `desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx` | Extend to assert the top row contains a `role="tablist"` with `aria-label="App mode"` and that the nav below renders identically in both modes. |
| `desktop-ui/src/features/app/components/MainApp.tsx` | Branch the main content area on `useAppMode()`: `assistant` → existing render (untouched); `code` → `<CodeLanding workspaces={...} onSelectWorkspace={...} onAddWorkspace={...} onImportWorkspace={...} />`. |
| `desktop-ui/src/styles/sidebar-chat.css` | Update `.sidebar-chat__topbar` from a 32px spacer to a flex row that hosts the switch (preserving the macOS traffic-light reserve). |
| `desktop-ui/src/styles/index.css` | Add `@import` lines for `app-mode-switch.css` and `code-landing.css`. |

## Component contracts

### `AppModeSwitch`

```ts
type AppMode = "assistant" | "code";

type AppModeSwitchProps = {
  mode: AppMode;
  onChange: (mode: AppMode) => void;
};
```

- Renders `<div class="app-mode-switch" role="tablist" aria-label="App mode">` with two `<button role="tab">` children.
- BEM: `.app-mode-switch`, `.app-mode-switch__btn`, `.app-mode-switch__btn--active`.
- Visual: outer track `background: var(--surface-card-muted)` (`rgba(255,255,255,0.06)`), 7px outer radius, 2px padding. Each button is 22px tall, 5px inner radius, padding `3px 10px`, font `var(--fs-xs)` weight 500, color `var(--text-muted)`. Active button: `background: rgba(255,255,255,0.14)`, color `var(--text-strong)`.
- `data-tauri-drag-region="false"` on both the wrapper and the buttons; ensures clicks land on the control inside the drag-strip area. Z-index sits above `.sidebar-chat__drag-strip`.
- Keyboard: ←/→ arrows move active tab; `Tab` enters/leaves the group.

### `useAppMode`

```ts
function useAppMode(): { mode: AppMode; setMode: (mode: AppMode) => void };
```

- On mount, reads `localStorage["klynt.appMode"]`. If present and valid, hydrates; otherwise defaults to `"assistant"`.
- On `setMode`, writes synchronously to `localStorage` and notifies subscribers via the singleton store.
- Implemented with `useSyncExternalStore` so multiple consumers stay in sync without prop-drilling.

### `CodeLanding`

```ts
type CodeLandingProps = {
  workspaces: WorkspaceInfo[];
  onSelectWorkspace: (id: string) => void;
  onAddWorkspace: () => void;        // create flow
  onImportWorkspace: () => void;     // import flow
};
```

- When `workspaces.length > 0`: hero + grid as described under "User-visible behavior".
- When `workspaces.length === 0`: hero collapses to `<h1>Get started</h1>`, grid is replaced by the two centered CTAs.
- BEM: `.code-landing`, `.code-landing__hero`, `.code-landing__input`, `.code-landing__projects`, `.code-landing__grid`, `.code-landing__card`, `.code-landing__card--add`, `.code-landing__empty`.
- All colors come from existing design tokens (`--surface-card`, `--surface-card-muted`, `--border-subtle`, `--text-strong`, `--text-muted`, `--text-faint`).
- Per the project's typography rules, no hardcoded `font-size` in pixels — uses the existing `--fs-*` tokens. The hero `<h1>` uses `var(--fs-display-sm)` (28px, already defined in `ds-tokens.css`).
- Project-card branch text uses a soft cyan readable on dark surfaces — `color: rgba(120, 200, 255, 0.85)`, sourced from the same hue family as `--surface-active` (`rgba(100, 200, 255, 0.14)`). If a follow-up wants a token, define `--text-accent-cyan` in `themes.{dark,light,dim,system}.css` rather than reusing `--cm-color-blue-fg` (which is tuned for light backgrounds and reads as dark navy on dark themes).

## Data flow

```
useAppMode() ─┬─► <AppModeSwitch mode setMode/>   (sidebar top row)
              │
              └─► MainApp:
                    mode === "assistant" → existing render (no change)
                    mode === "code"      → <CodeLanding
                                              workspaces={fromExistingStore}
                                              onSelectWorkspace={existingHandler}
                                              onAddWorkspace={existingHandler}
                                              onImportWorkspace={existingHandler}
                                            />
```

`CodeLanding` is a different visual lens over the same `WorkspaceInfo[]` already used by the sidebar. No new endpoint, no new state shape, no new IPC.

## Testing

- **`useAppMode.test.ts`** — default is `"assistant"`; hydrates valid localStorage value on mount; ignores invalid values and falls back to default; `setMode` updates state and writes to localStorage; multiple subscribers stay in sync.
- **`AppModeSwitch.test.tsx`** — renders both buttons; the active button has `aria-selected="true"` and the active class; clicking the inactive button calls `onChange` with the new mode; ←/→ arrows move active selection; `data-tauri-drag-region` is false on the wrapper.
- **`CodeLanding.test.tsx`** — populated state renders one card per workspace with name, meta, and branch; clicking a card calls `onSelectWorkspace(id)`; the `+ New` card calls `onAddWorkspace`; the header `Import` and `+ New` buttons call their handlers; empty state renders the centered Create/Import pair, both buttons wire to the right handlers.
- **`SidebarChatLayout.test.tsx`** (extended) — top row contains a `role="tablist"` labelled `App mode`; the existing nav (`New chat`, `Search`, `Calendar`, `Plugins`, `Automations`, `Project`) renders identically when `mode === "assistant"` and `mode === "code"`; the `Chats` section and footer render identically in both modes.
- **No backend tests** — there's no backend change.

## Risks & open questions

- **Drag-region clickability.** The top row sits inside the macOS drag strip area. Verification step: run `cargo tauri dev` and confirm both segments accept clicks and don't drag the window. The `data-tauri-drag-region="false"` attribute on the switch and a positive `z-index` above `.sidebar-chat__drag-strip` should be sufficient; if not, narrow the drag strip's pointer-events to bypass the switch region.
- **Sidebar `Project` nav item.** Today's sidebar already has a `Project` nav item. In Code mode this is visually redundant with the main-panel project grid. Per scope, we leave it untouched; a follow-up pass may hide or relabel it. Flagged here so the next plan owner knows this is intentional.
- **Mode-aware nav (deferred).** Some sidebar items (e.g. `New chat`) may eventually route differently in Code mode (e.g. "New coding session"). Not in scope; the current pass keeps them identical.
- **Hero input wire-up (deferred).** The task input collects a string but does nothing with it in this pass. The next pass will route the string into the existing task-creation pipeline, scoped to the most recently active or selected project.

## Out-of-scope follow-ups (named for future planning)

1. Wire the hero task input to actual task creation (likely scoped to a chosen workspace).
2. Mode-aware sidebar items (`New chat` → `New coding session` in Code mode, etc.).
3. Hide or relabel the `Project` sidebar nav item in Code mode.
4. Mirror signal for mode changes (procedural-memory candidate).
5. Per-workspace "last active mode" memory if users want different defaults per project.
