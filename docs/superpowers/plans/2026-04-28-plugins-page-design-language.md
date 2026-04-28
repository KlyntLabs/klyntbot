# Plugins Page Design-Language Alignment — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Plugins page render full-bleed (full width, full height) with a visual language identical to the chat page and right-panel — single near-black surface, two-tone foreground, pill segmented controls — and demote `Klynt CLI` from a primary plugin tab to an agent filter pill alongside Claude Code / Codex / Kimi / opencode.

**Architecture:** UI-only changes in the `desktop-ui` Tauri renderer. One union-type extension (`ProviderId`), one new state-derived classname on the app shell (`.app.is-plugins`), one new CSS rule on `.main`, one markup swap from underline-style tabs to the existing `.panel-tabs` / `.panel-tab` primitive, and a localized restyle of the agent filter pills. No new components, no new files (except a new test file for `ProviderChips`), no backend changes.

**Tech Stack:** React 18 + TypeScript, plain CSS (no Tailwind, no CSS-in-JS), Vitest + `@testing-library/react`, `bun` as the package manager. Path aliases: `@/*` → `src/*`, `@app/*` → `src/features/app/*`.

**Spec:** `docs/superpowers/specs/2026-04-28-plugins-page-design-language-design.md`

---

## File Map

**Modified files (8):**

| Path | Responsibility |
|------|---------------|
| `desktop-ui/src/features/plugins/coding-memory/types.ts` | Add `klyntCli` to `ProviderId` union |
| `desktop-ui/src/features/plugins/coding-memory/ProviderChips.tsx` | Add Klynt CLI entry to `PROVIDERS` array |
| `desktop-ui/src/features/plugins/components/PluginsView.tsx` | Narrow `PluginId`, swap markup to `.panel-tabs` primitive |
| `desktop-ui/src/features/plugins/components/PluginsView.test.tsx` | Update assertions: no Klynt CLI tab, panel-tabs markup |
| `desktop-ui/src/styles/plugins.css` | Surface/token swaps, retire `__tab*` selectors, restyle `.cm-provider-chip` |
| `desktop-ui/src/styles/panel-tabs.css` | Add disabled state + text-mode modifier for text-bearing tabs |
| `desktop-ui/src/styles/main.css` | Add `.app.is-plugins .main { … }` rule |
| `desktop-ui/src/features/app/orchestration/useLayoutOrchestration.ts` | Accept `appView`, append `is-plugins` to `appClassName` |
| `desktop-ui/src/features/app/components/MainApp.tsx` | Pass `appView` to `useAppShellOrchestration` |

**New file (1):**

| Path | Responsibility |
|------|---------------|
| `desktop-ui/src/features/plugins/coding-memory/ProviderChips.test.tsx` | Verify Klynt CLI pill renders and is selectable |

---

## Task 1: Extend `ProviderId` union with `klyntCli`

**Why first:** Pure type-level change. Prep for Task 2 (the chip array entry references this variant). Catches any unhandled exhaustive switches at compile time before we add UI for it.

**Files:**
- Modify: `desktop-ui/src/features/plugins/coding-memory/types.ts`

- [ ] **Step 1: Read current type**

Run: `cat desktop-ui/src/features/plugins/coding-memory/types.ts`

Expected current content:

```ts
export type { WireEventDto, SessionSummaryDto, SessionListArgs, SessionReplayEntry } from "@/bindings";
export type ProviderId = "claudeCode" | "codex" | "kimiCli" | "openCode";
```

- [ ] **Step 2: Update the union**

Edit `desktop-ui/src/features/plugins/coding-memory/types.ts` so it reads:

```ts
export type { WireEventDto, SessionSummaryDto, SessionListArgs, SessionReplayEntry } from "@/bindings";
export type ProviderId = "claudeCode" | "codex" | "kimiCli" | "openCode" | "klyntCli";
```

- [ ] **Step 3: Run typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS (no errors). The only consumer of the type today is `CodingMemoryPlugin.tsx` and it treats the value as a string passthrough, so no exhaustive-switch errors are expected.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/types.ts
git commit -m "feat(plugins): add klyntCli variant to ProviderId union

Aligns frontend ProviderId with backend AgentSource::KlyntCli (already
present in crates/coding-ingest/src/event.rs:55). Sets up the agent
filter pill addition in the next commit.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 2: Add Klynt CLI to `ProviderChips`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/ProviderChips.test.tsx`
- Modify: `desktop-ui/src/features/plugins/coding-memory/ProviderChips.tsx`

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/plugins/coding-memory/ProviderChips.test.tsx` with:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { ProviderChips } from "./ProviderChips";

describe("ProviderChips", () => {
  it("renders all five agent sources plus All", () => {
    render(<ProviderChips active="all" onChange={() => {}} />);
    expect(screen.getByRole("tab", { name: /^all$/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /claude code/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /codex/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /kimi/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /opencode/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /klynt cli/i })).toBeInTheDocument();
  });

  it("emits klyntCli when the Klynt CLI pill is clicked", () => {
    const onChange = vi.fn();
    render(<ProviderChips active="all" onChange={onChange} />);
    fireEvent.click(screen.getByRole("tab", { name: /klynt cli/i }));
    expect(onChange).toHaveBeenCalledWith("klyntCli");
  });

  it("marks the active pill with aria-selected=true", () => {
    render(<ProviderChips active="klyntCli" onChange={() => {}} />);
    expect(screen.getByRole("tab", { name: /klynt cli/i })).toHaveAttribute("aria-selected", "true");
    expect(screen.getByRole("tab", { name: /^all$/i })).toHaveAttribute("aria-selected", "false");
  });
});
```

- [ ] **Step 2: Run the failing test**

Run: `cd desktop-ui && bun run test ProviderChips`
Expected: FAIL — "Klynt CLI" pill is not in the rendered output (3 of 3 assertions fail to find it).

- [ ] **Step 3: Add the entry to `PROVIDERS`**

Edit `desktop-ui/src/features/plugins/coding-memory/ProviderChips.tsx`. Replace the `PROVIDERS` constant:

```tsx
const PROVIDERS: { id: ProviderId | "all"; label: string }[] = [
  { id: "all", label: "All" },
  { id: "claudeCode", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "kimiCli", label: "Kimi" },
  { id: "openCode", label: "opencode" },
  { id: "klyntCli", label: "Klynt CLI" },
];
```

(All other code in the file is unchanged.)

- [ ] **Step 4: Run the test, verify pass**

Run: `cd desktop-ui && bun run test ProviderChips`
Expected: PASS — all three tests green.

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/features/plugins/coding-memory/ProviderChips.tsx \
        desktop-ui/src/features/plugins/coding-memory/ProviderChips.test.tsx
git commit -m "feat(plugins): add Klynt CLI agent filter pill

Surfaces klyntCli sessions in the filter row alongside Claude Code,
Codex, Kimi, and opencode.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 3: Narrow `PluginId` and remove Klynt CLI from primary tabs

**Files:**
- Modify: `desktop-ui/src/features/plugins/components/PluginsView.tsx`
- Modify: `desktop-ui/src/features/plugins/components/PluginsView.test.tsx`

- [ ] **Step 1: Update the failing test first**

Edit `desktop-ui/src/features/plugins/components/PluginsView.test.tsx` to add a negative assertion:

```tsx
import { render, screen } from "@testing-library/react";
import { describe, it, expect } from "vitest";
import { PluginsView } from "./PluginsView";

describe("PluginsView", () => {
  it("renders the plugins host with a Coding Memory tab", () => {
    render(<PluginsView />);
    expect(screen.getByRole("heading", { name: /plugins/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /coding memory/i })).toBeInTheDocument();
  });

  it("renders Coding Memory plugin by default", () => {
    render(<PluginsView />);
    expect(screen.getByTestId("plugins-active-pane")).toHaveAttribute("data-plugin", "coding-memory");
  });

  it("does not render Klynt CLI as a primary plugin tab", () => {
    render(<PluginsView />);
    expect(screen.queryByRole("tab", { name: /klynt cli/i })).not.toBeInTheDocument();
  });

  it("renders Skills and MCP Servers as Soon-badged tabs", () => {
    render(<PluginsView />);
    expect(screen.getByRole("tab", { name: /skills/i })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: /mcp servers/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run the failing test**

Run: `cd desktop-ui && bun run test PluginsView`
Expected: FAIL — `does not render Klynt CLI` fails (it currently does render).

- [ ] **Step 3: Narrow `PluginId` and drop the Klynt CLI entry**

Edit `desktop-ui/src/features/plugins/components/PluginsView.tsx`. Replace lines 1–11 (imports + types + tabs) with:

```tsx
import { useState } from "react";
import { CodingMemoryPlugin } from "@/features/plugins/coding-memory/CodingMemoryPlugin";

type PluginId = "coding-memory" | "skills" | "mcp";

const PLUGIN_TABS: ReadonlyArray<{ id: PluginId; label: string; available: boolean }> = [
  { id: "coding-memory", label: "Coding Memory", available: true },
  { id: "skills", label: "Skills", available: false },
  { id: "mcp", label: "MCP Servers", available: false },
];
```

(The `PluginsView` function body and JSX remain unchanged in this task — Task 5 will swap the markup to `.panel-tabs`.)

- [ ] **Step 4: Run tests, verify pass**

Run: `cd desktop-ui && bun run test PluginsView`
Expected: PASS — all four tests green.

- [ ] **Step 5: Run typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS — `PluginId` is local to `PluginsView.tsx` and has no external consumers.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/plugins/components/PluginsView.tsx \
        desktop-ui/src/features/plugins/components/PluginsView.test.tsx
git commit -m "refactor(plugins): drop Klynt CLI from primary plugin tabs

Klynt CLI is an agent source, not a plugin — it now lives in the
agent filter pill row inside Coding Memory. Primary tabs narrow to
Coding Memory / Skills / MCP Servers.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 4: Add disabled state and text-mode modifier to `.panel-tab`

**Why this task:** `panel-tabs.css` was built for icon-only tabs (the right-panel git/files/prompts toggle). The plugins page reuses the primitive but renders text labels and one disabled state ("Soon"). We extend the existing rules — non-invasive — rather than fork them.

**Files:**
- Modify: `desktop-ui/src/styles/panel-tabs.css`

- [ ] **Step 1: Read the current file**

Run: `cat desktop-ui/src/styles/panel-tabs.css`

Confirm contents match the file shown in the spec discovery (`.panel-tabs` container, `.panel-tab` button, `.panel-tab.is-active`, `.panel-tab:focus-visible`, `.panel-tab-icon`).

- [ ] **Step 2: Append the new rules**

Add the following at the end of `desktop-ui/src/styles/panel-tabs.css`:

```css
.panel-tab--text {
  padding: 6px 12px;
  font-size: var(--fs-sm);
  font-weight: 500;
  gap: 6px;
}

.panel-tab:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.panel-tab__badge {
  font-size: var(--fs-2xs);
  font-weight: 500;
  color: var(--text-faint);
  letter-spacing: 0.02em;
}
```

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint`
Expected: PASS — pure CSS append, no JS/TS files touched.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/panel-tabs.css
git commit -m "feat(layout): add text-mode + disabled state to .panel-tab

Extends the right-panel pill primitive so the plugins page can reuse
it for text-label tabs (Coding Memory / Skills / MCP Servers) with a
quiet 'Soon' badge for unavailable ones.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 5: Migrate primary tabs to `.panel-tabs` markup

**Files:**
- Modify: `desktop-ui/src/features/plugins/components/PluginsView.tsx`

- [ ] **Step 1: Replace the tab markup**

Edit `desktop-ui/src/features/plugins/components/PluginsView.tsx`. Replace the entire file with:

```tsx
import { useState } from "react";
import { CodingMemoryPlugin } from "@/features/plugins/coding-memory/CodingMemoryPlugin";

type PluginId = "coding-memory" | "skills" | "mcp";

const PLUGIN_TABS: ReadonlyArray<{ id: PluginId; label: string; available: boolean }> = [
  { id: "coding-memory", label: "Coding Memory", available: true },
  { id: "skills", label: "Skills", available: false },
  { id: "mcp", label: "MCP Servers", available: false },
];

export function PluginsView() {
  const [active, setActive] = useState<PluginId>("coding-memory");

  return (
    <section className="plugins-view" aria-labelledby="plugins-heading">
      <header className="plugins-view__header">
        <h1 id="plugins-heading" className="plugins-view__title">Plugins</h1>
        <p className="plugins-view__subtitle">Internal plugins for inspecting and tuning your Klynt platform.</p>
      </header>
      <div className="plugins-view__tabs-wrap">
        <div className="panel-tabs" role="tablist" aria-label="Plugins" aria-orientation="horizontal">
          {PLUGIN_TABS.map((tab) => {
            const isActive = active === tab.id;
            return (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={isActive}
                disabled={!tab.available}
                className={`panel-tab panel-tab--text${isActive ? " is-active" : ""}`}
                onClick={() => tab.available && setActive(tab.id)}
              >
                {tab.label}
                {!tab.available && <span className="panel-tab__badge">Soon</span>}
              </button>
            );
          })}
        </div>
      </div>
      <div className="plugins-view__pane" data-testid="plugins-active-pane" data-plugin={active}>
        {active === "coding-memory" ? <CodingMemoryPlugin /> : null}
      </div>
    </section>
  );
}
```

- [ ] **Step 2: Run the existing tests**

Run: `cd desktop-ui && bun run test PluginsView`
Expected: PASS — all four tests green. ARIA roles (`tab`, `tablist`) are preserved so the existing `screen.getByRole("tab", …)` queries still match.

- [ ] **Step 3: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/plugins/components/PluginsView.tsx
git commit -m "refactor(plugins): adopt .panel-tabs primitive for primary tabs

Replaces the underline-style tab row with the rounded pill segmented
control already used by the right-panel git/files/prompts toggle.
Same markup, same a11y, shared visual language.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 6: Swap surface tokens on `.plugins-view` chrome

**Why this is its own task:** Pure CSS, easy to revert in isolation if a visual regression appears. Keeps the diff readable.

**Files:**
- Modify: `desktop-ui/src/styles/plugins.css`

- [ ] **Step 1: Edit the chrome rules**

In `desktop-ui/src/styles/plugins.css`, replace lines 1–13 (the top of the file) with:

```css
.plugins-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--surface-messages);
}

.plugins-view__header {
  padding: 24px 28px 12px;
  border-bottom: 1px solid var(--border-subtle);
}
.plugins-view__title { font-size: var(--fs-xl); margin: 0 0 4px; color: var(--text-stronger); }
.plugins-view__subtitle { font-size: var(--fs-xs); color: var(--text-muted); margin: 0; }
```

Then replace line 482 (the `.cm-plugin__sidebar` rule) so the border-right token routes through the chat-page vocabulary:

Old:
```css
.cm-plugin__sidebar { width: 320px; border-right: 1px solid var(--ds-border-subtle); display: flex; flex-direction: column; }
```

New:
```css
.cm-plugin__sidebar { width: 320px; border-right: 1px solid var(--border-subtle); display: flex; flex-direction: column; }
```

(Note: `--ds-border-subtle` resolves to `--border-subtle` today via `ds-tokens.css:19`, but routing through the chat vocabulary directly removes the indirection on the page chrome.)

- [ ] **Step 2: Visual smoke**

Run: `cd desktop-ui && bun run dev:vite` (in one terminal) and `cargo tauri dev` (in another).

Open the desktop app, click **Plugins** in the sidebar. Confirm:
- The plugins area background is now the same near-black as the chat slab (open a chat in the same session for comparison).
- Title "Plugins" reads in the brightest text tone (matches chat headings).
- Subtitle "Internal plugins for inspecting and tuning your Klynt platform" reads in the muted secondary tone.
- The hairline under the header is the same subtle line as the chat composer's borders.

If you can't run the desktop app, run `cd desktop-ui && bun run dev:vite` and open `localhost:1420` directly — the plugins view renders inside the same shell.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/styles/plugins.css
git commit -m "style(plugins): align page chrome with chat-page surface tokens

.plugins-view background swaps from --ds-surface-card (translucent
white, reads gray) to --surface-messages (solid near-black). Header
text adopts the chat page's two-tone (--text-stronger /
--text-muted) instead of the three-tone --ds-* scale.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 7: Retire the obsolete `__tab*` selectors

**Why:** Task 5 stopped using `.plugins-view__tabs`, `.plugins-view__tab`, `.plugins-view__tab--active`, `.plugins-view__tab--coming-soon`, and `.plugins-view__tab-badge`. Leaving them in `plugins.css` is dead CSS that will rot. Removing them is part of the "surgical changes" discipline in CLAUDE.md.

**Files:**
- Modify: `desktop-ui/src/styles/plugins.css`

- [ ] **Step 1: Confirm nothing else uses these selectors**

Run: `grep -rn "plugins-view__tab" desktop-ui/src/`
Expected: zero matches (Task 5 removed the JSX; this task removes the CSS).

- [ ] **Step 2: Delete the obsolete rules**

In `desktop-ui/src/styles/plugins.css`, remove the block previously at lines 15–42 (the `.plugins-view__tabs`, `.plugins-view__tab`, `.plugins-view__tab:hover:not(:disabled)`, `.plugins-view__tab--active`, `.plugins-view__tab--coming-soon`, and `.plugins-view__tab-badge` rules).

After deletion, the file's first sibling rule below `.plugins-view__subtitle` should be `.plugins-view__pane`. Add a single-line wrapper rule above `.plugins-view__pane` to position the panel-tabs row:

```css
.plugins-view__tabs-wrap {
  padding: 8px 20px 12px;
  border-bottom: 1px solid var(--border-subtle);
}

.plugins-view__pane { flex: 1; overflow: hidden; }
```

- [ ] **Step 3: Run lint**

Run: `cd desktop-ui && bun run lint`
Expected: PASS.

- [ ] **Step 4: Run tests**

Run: `cd desktop-ui && bun run test PluginsView`
Expected: PASS.

- [ ] **Step 5: Visual smoke**

Reload the desktop app or Vite dev server. Confirm the Plugins page still renders correctly — title row with hairline below, then the pill tab row, then the active plugin pane.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/styles/plugins.css
git commit -m "chore(plugins): retire .plugins-view__tab* dead CSS

Removes the underline-style tab rules superseded by .panel-tabs in
the previous commit. Adds a small .plugins-view__tabs-wrap layout
rule for the new pill row's outer padding + border.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 8: Redesign the agent filter pills

**Files:**
- Modify: `desktop-ui/src/styles/plugins.css`

- [ ] **Step 1: Replace the `.cm-provider-chips` and `.cm-provider-chip` rules**

In `desktop-ui/src/styles/plugins.css`, find the section labeled `/* ProviderChips */` (currently around line 429) and replace the four rules (`.cm-provider-chips`, `.cm-provider-chip`, `.cm-provider-chip:hover`, `.cm-provider-chip--active`, `.cm-provider-chip__count`) with:

```css
/* ProviderChips */
.cm-provider-chips {
  display: flex;
  gap: 6px;
  padding: 12px 20px;
  border-bottom: 1px solid var(--border-subtle);
  flex-wrap: wrap;
}
.cm-provider-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 3px 10px;
  font-size: var(--fs-xs);
  font-weight: 500;
  border-radius: 999px;
  border: 1px solid var(--border-subtle);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition:
    color var(--ds-dur-fast) var(--ds-ease-out),
    background var(--ds-dur-fast) var(--ds-ease-out);
}
.cm-provider-chip:hover { color: var(--text-stronger); }
.cm-provider-chip--active {
  color: var(--text-stronger);
  background: var(--surface-card-strong);
  border-color: transparent;
}
.cm-provider-chip__count {
  font-size: var(--fs-2xs);
  color: var(--text-faint);
  margin-left: 2px;
}
```

- [ ] **Step 2: Run tests**

Run: `cd desktop-ui && bun run test ProviderChips`
Expected: PASS — class swaps don't change DOM/ARIA, only styles.

- [ ] **Step 3: Visual smoke**

Reload the desktop app. Click through pills (All → Claude Code → Codex → Kimi → opencode → Klynt CLI). Confirm:
- Default state: pills read as quiet outlines on the near-black slab.
- Hover: text brightens, no background flicker.
- Active: text brightens **and** background fills with the same `surface-card-strong` used elsewhere; the previous accent-stroke ring is gone.
- Count badges (when sessions exist) sit flush to the label, fainter than the label, no separate pill.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/plugins.css
git commit -m "style(plugins): redesign agent filter pills

Tighter padding, smaller font, single-tone foreground, fill-only
selected state (no accent-color ring). Count badge tucks flush to
the label instead of a separate pill.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 9: Plumb `appView` through `useAppShellOrchestration`

**Why:** The full-bleed layout is conditional on `appView === "plugins"`. The shell hook produces `appClassName`, so it's the natural seam. We add an option, not a new state.

**Files:**
- Modify: `desktop-ui/src/features/app/orchestration/useLayoutOrchestration.ts`
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx`

- [ ] **Step 1: Extend the options type and the destructure**

In `desktop-ui/src/features/app/orchestration/useLayoutOrchestration.ts`, modify lines 5–21 to add `appView`:

```ts
type UseAppShellOrchestrationOptions = {
  sidebarCollapsed: boolean;
  rightPanelCollapsed: boolean;
  shouldReduceTransparency: boolean;
  isWorkspaceDropActive: boolean;
  centerMode: "chat" | "diff";
  appView: "home" | "chat" | "plugins";
  selectedDiffPath: string | null;
  showComposer: boolean;
  activeThreadId: string | null;
  sidebarWidth: number;
  rightPanelWidth: number;
  chatDiffSplitPositionPercent: number;
  planPanelHeight: number;
  terminalPanelHeight: number;
  debugPanelHeight: number;
  appSettings: Pick<AppSettings, "uiFontFamily" | "codeFontFamily" | "codeFontSize">;
};
```

Then update the destructure (line 23–39) to include `appView`:

```ts
export function useAppShellOrchestration({
  sidebarCollapsed,
  rightPanelCollapsed,
  shouldReduceTransparency,
  isWorkspaceDropActive,
  centerMode,
  appView,
  selectedDiffPath,
  showComposer,
  activeThreadId,
  sidebarWidth,
  rightPanelWidth,
  chatDiffSplitPositionPercent,
  planPanelHeight,
  terminalPanelHeight,
  debugPanelHeight,
  appSettings,
}: UseAppShellOrchestrationOptions) {
```

- [ ] **Step 2: Append the modifier to `appClassName`**

In the same file, replace the `appClassName` template (lines 44–48) with:

```ts
  const appClassName = `app layout-desktop${
    shouldReduceTransparency ? " reduced-transparency" : ""
  }${sidebarCollapsed ? " sidebar-collapsed" : ""}${
    rightPanelCollapsed ? " right-panel-collapsed" : ""
  }${appView === "plugins" ? " is-plugins" : ""}${isWindows ? " is-windows" : ""}`;
```

- [ ] **Step 3: Pass `appView` from `MainApp.tsx`**

In `desktop-ui/src/features/app/components/MainApp.tsx`, find the `useAppShellOrchestration` call at line 1366. Add `appView` to the options object (alongside `centerMode`):

```tsx
  const { isThreadOpen, dropOverlayActive, dropOverlayText, appClassName, appStyle } =
    useAppShellOrchestration({
      sidebarCollapsed,
      rightPanelCollapsed,
      shouldReduceTransparency,
      isWorkspaceDropActive,
      centerMode,
      appView,
      selectedDiffPath,
      showComposer,
      activeThreadId,
      sidebarWidth,
      chatDiffSplitPositionPercent,
      rightPanelWidth,
      planPanelHeight,
      terminalPanelHeight,
      debugPanelHeight,
      appSettings,
    });
```

`appView` is already declared in `MainApp.tsx:327` as `useState<"home" | "chat" | "plugins">("home")` — no new state needed.

- [ ] **Step 4: Typecheck**

Run: `cd desktop-ui && bun run typecheck`
Expected: PASS — `appView` is `"home" | "chat" | "plugins"` at the call site, matching the new option type exactly.

- [ ] **Step 5: Lint**

Run: `cd desktop-ui && bun run lint`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/app/orchestration/useLayoutOrchestration.ts \
        desktop-ui/src/features/app/components/MainApp.tsx
git commit -m "feat(layout): expose appView to the shell classname

Adds an 'is-plugins' modifier on .app when appView === 'plugins',
purely derived from existing state. Used by the next commit to drop
the right-panel column reservation in the .main grid.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 10: Add `.app.is-plugins .main` rule for full-bleed layout

**Files:**
- Modify: `desktop-ui/src/styles/main.css`

- [ ] **Step 1: Append the rule**

In `desktop-ui/src/styles/main.css`, find the rule starting at line 32 (`.app.sidebar-collapsed .main { … }`). Add the new rule immediately after it:

```css
.app.sidebar-collapsed .main {
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
  box-shadow: none;
}

.app.is-plugins .main {
  grid-template-columns: 1fr;
  border-top-left-radius: 0;
  border-bottom-left-radius: 0;
  box-shadow: none;
}
```

(The `.app.sidebar-collapsed .main` rule is unchanged; the new rule sits below it.)

- [ ] **Step 2: Visual smoke — full-bleed layout**

Reload the desktop app. Click **Plugins**. Confirm:
- The plugins area extends from the right edge of the sidebar all the way to the right edge of the window — no empty band on the right.
- The top-left and bottom-left corners are square (no rounded floating-card affordance).
- Click **Home** or open a chat. Confirm the chat workspace's rounded corners and right-panel return immediately, with no flicker.

- [ ] **Step 3: Typecheck + lint + tests (full check)**

Run all three in parallel:

```bash
cd desktop-ui && bun run typecheck
cd desktop-ui && bun run lint
cd desktop-ui && bun run test
```

Expected: all PASS.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/styles/main.css
git commit -m "feat(layout): full-bleed .main when appView is plugins

Drops the right-panel column reservation and the rounded-corner
floating-card affordance on the plugins view. Plugins page now
fills the full area between sidebar and window edge.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>"
```

---

## Task 11: Final verification

**Files:** none (verification only)

- [ ] **Step 1: Run the full frontend check**

```bash
cd desktop-ui && bun run typecheck
cd desktop-ui && bun run lint
cd desktop-ui && bun run test
```

Expected: all PASS, no warnings.

- [ ] **Step 2: Manual end-to-end smoke**

Run `cd desktop-ui && bun run dev:vite` in one terminal, then `cargo tauri dev` in another. Walk this checklist in the live app:

| Check | Pass criterion |
|-------|---------------|
| Sidebar → Plugins | Plugins page fills the full window width minus the sidebar; no empty band on the right. |
| Plugins surface color | Background is the same near-black as the chat slab. |
| Primary tab row | Three pills: **Coding Memory** (active), **Skills · Soon**, **MCP Servers · Soon**. Pill style matches the right-panel diff/plan toggle. |
| Disabled tabs | Hovering Skills or MCP Servers shows a `not-allowed` cursor. Clicking does nothing. |
| Agent filter row | Six pills: All · Claude Code · Codex · Kimi · opencode · **Klynt CLI** (last). Selecting any updates the session list filter. |
| Active pill state | Selected pill has a filled background + brighter text, no colored stroke ring. |
| Switch back to chat | Click sidebar → New Chat. Right-panel returns; rounded-corner floating-card returns; no flicker on transition. |
| Window resize 1440 → 800 | Plugins page reflows; no horizontal scrollbar; tab pills wrap if needed. |

- [ ] **Step 3: Workspace clippy + nextest (defensive — nothing Rust changed, but the convention)**

```bash
cargo nextest run --workspace
cargo clippy --workspace --all-targets --all-features
```

Expected: PASS — this plan touches no Rust code, so this is a sanity check that we didn't accidentally break workspace builds via, e.g., a stray binding regeneration.

- [ ] **Step 4: Final commit (only if any housekeeping changes accumulated)**

If the previous tasks left no outstanding diff:

```bash
git status
```

Expected: clean working tree, branch ahead of `main` by 9 commits (Tasks 1, 2, 3, 4, 5, 6, 7, 8, 9, 10 = 10 commits, but Task 1 + Task 2 may have been combined depending on execution path).

If you find any incidental dead code that this work made obsolete (e.g., an unused import in `MainApp.tsx`), commit it as a separate `chore(plugins):` cleanup.

---

## Self-Review

**1. Spec coverage** — every spec section has a task:

| Spec section | Implemented in |
|-------------|----------------|
| Goals: full-width / full-height | Task 9 + Task 10 |
| Goals: shared design vocabulary | Task 4 + Task 5 + Task 6 + Task 8 |
| Goals: Klynt CLI as filter pill | Task 1 + Task 2 + Task 3 |
| Goals: no regressions in chat/right-panel | Task 9 (`is-plugins` only when `appView === "plugins"`) + Task 11 (smoke check) |
| Design 1 (Layout) | Task 9 + Task 10 |
| Design 2 (Primary tabs pill segmented) | Task 4 + Task 5 |
| Design 3 (Agent filter pills extended + restyled) | Task 1 + Task 2 + Task 8 |
| Design 4 (Surface tokens) | Task 6 + (`.cm-plugin__sidebar` line in Task 6) |
| Design 5 (Tests) | Task 2 (ProviderChips) + Task 3 (PluginsView) |
| Risks: ProviderId consumers | Task 1 Step 3 (typecheck) |
| Risks: is-plugins persistence | Task 9 Step 2 (derived from `appView` directly, no flag) |
| Risks: empty centerMode plugins rendering | Task 10 Step 2 (visual smoke) |
| Risks: right-panel resizer conflict | Task 11 Step 2 (smoke checklist row "Switch back to chat") |
| Verification (cargo tauri dev, manual checks) | Task 11 Step 2 |
| Verification (typecheck/lint/test) | Task 11 Step 1 |

**2. Placeholder scan** — no "TBD", "TODO", "implement later", "add appropriate error handling", or "similar to Task N" anywhere. Every CSS block, JSX block, and command is concrete.

**3. Type consistency** — `ProviderId` is referenced consistently across Tasks 1, 2, and 8. `PluginId` is local to `PluginsView.tsx` (Task 3 + Task 5) and never crosses files. The new `useAppShellOrchestration` option `appView: "home" | "chat" | "plugins"` matches the type of `appView` state in `MainApp.tsx:327` exactly. The `.panel-tab--text` and `.panel-tab__badge` classnames are introduced in Task 4 and consumed in Task 5 with identical spelling.

No corrections needed.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-04-28-plugins-page-design-language.md`. Two execution options:**

**1. Subagent-Driven (recommended)** — I dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using executing-plans, batch execution with checkpoints.

**Which approach?**
