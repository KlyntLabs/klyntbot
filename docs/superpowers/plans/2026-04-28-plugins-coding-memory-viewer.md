# Plugins Page — Coding Memory Viewer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build an internal-plugins host page (`Plugins`) whose first plugin is a Coding Memory tracing viewer — ported from `/Users/jayden/Projects/Klynt/kimi-cli/vis/` and richer than klynt-cli/vis, using our `--ds-*` design tokens (no Tailwind), backed by the 25 already-shipped `coding_memory_*` Tauri commands across all 4 CLI providers (Claude Code, Codex, Kimi, opencode).

**Architecture:** A new `appView === "plugins"` branch in `MainApp.tsx` renders a `PluginsView` with a `PanelTabs` shell. The first tab — "Coding Memory" — embeds a `WireViewer` that consumes a typed `WireEventDto` (decoded server-side from the existing `SessionReplayEntry.payload: String`). Event-type chips/timeline gutters are styled via new `--ds-event-{kind}-{bg|fg|border}` tokens (~25 vars in `ds-tokens.css`, themed in `themes.{dark,light,dim}.css`). Klynt-only richness panels (recall overlay, mirror alerts, causal graph, reforge diff, effectiveness trends) are mounted alongside the wire stream as collapsible side panels.

**Tech Stack:** React 18 + Tauri 2 (`invoke`) + Specta-typed `bindings.ts` + plain CSS via `--ds-*` tokens + `react-virtuoso` (one new dep) + `lucide-react` (already present) + Vitest for FE tests + `cargo nextest` for Rust DTO tests.

**Scope check:** This plan covers **only** the FE viewer + the small Rust DTO touch-ups it depends on. Internal-plugins host (additional tabs: Skills, MCP, klynt-cli sessions) is intentionally deferred — only the tab shell is built; future plugins slot in without restructuring. The four locked constraints from `docs/superpowers/backlog.md#8` are honored throughout.

**Reference paths (read-only, used for translation):**
- Source UI: `/Users/jayden/Projects/Klynt/kimi-cli/vis/src/features/wire-viewer/`, `/Users/jayden/Projects/Klynt/kimi-cli/vis/src/lib/api.ts`
- Target FE: `/Users/jayden/Projects/Klynt/bot/desktop-ui/src/features/plugins/` (to be created)
- Backend DTOs: `/Users/jayden/Projects/Klynt/bot/crates/desktop-shared/src/commands/coding_memory.rs`
- Backend handler: `/Users/jayden/Projects/Klynt/bot/crates/desktop/src/commands/coding_memory.rs`
- Internal event taxonomy: `/Users/jayden/Projects/Klynt/bot/crates/coding-ingest/src/event.rs` (`EventKind` — 20 variants)

---

## File Structure

### Created files (FE)
- `desktop-ui/src/features/plugins/components/PluginsView.tsx` — tab shell (host)
- `desktop-ui/src/features/plugins/components/PluginsView.test.tsx`
- `desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx` — provider switcher + session list + viewer pane
- `desktop-ui/src/features/plugins/coding-memory/SessionList.tsx`
- `desktop-ui/src/features/plugins/coding-memory/ProviderChips.tsx`
- `desktop-ui/src/features/plugins/coding-memory/WireViewer.tsx`
- `desktop-ui/src/features/plugins/coding-memory/WireFilters.tsx`
- `desktop-ui/src/features/plugins/coding-memory/TurnTree.tsx`
- `desktop-ui/src/features/plugins/coding-memory/WireEventCard.tsx`
- `desktop-ui/src/features/plugins/coding-memory/ToolCallDetail.tsx`
- `desktop-ui/src/features/plugins/coding-memory/RecallOverlay.tsx` *(klynt-only)*
- `desktop-ui/src/features/plugins/coding-memory/MirrorAlertSidecar.tsx` *(klynt-only)*
- `desktop-ui/src/features/plugins/coding-memory/CausalGraphInspector.tsx` *(klynt-only)*
- `desktop-ui/src/features/plugins/coding-memory/ReforgeCycleDiff.tsx` *(klynt-only)*
- `desktop-ui/src/features/plugins/coding-memory/EffectivenessTrendsChart.tsx` *(klynt-only)*
- `desktop-ui/src/features/plugins/coding-memory/buildToolGrouping.ts`
- `desktop-ui/src/features/plugins/coding-memory/eventHelpers.ts` (`isErrorEvent`, `formatTimeDelta`, `formatTimestamp`, `summarizeEvent`)
- `desktop-ui/src/features/plugins/coding-memory/eventHelpers.test.ts`
- `desktop-ui/src/features/plugins/coding-memory/types.ts` — re-exports `WireEventDto`, `SessionSummaryDto` from bindings + helper unions
- `desktop-ui/src/api/endpoints/codingMemory.ts` — typed wrappers
- `desktop-ui/src/styles/plugins.css` — feature stylesheet

### Modified files (FE)
- `desktop-ui/src/features/app/components/SidebarChatLayout.tsx` — wire `onSelectPlugins` callback to existing `"plugins"` nav item (line 30 + line 5 props + line 27)
- `desktop-ui/src/features/app/components/MainApp.tsx` — extend `appView` union (line 326), render `pluginsNode` (around line 1819)
- `desktop-ui/src/features/app/components/MainAppShell.tsx` / `AppLayout.tsx` — accept `pluginsNode` prop (paths confirmed at execution time via Read)
- `desktop-ui/src/styles/index.css` — add `@import "./plugins.css";`
- `desktop-ui/src/styles/ds-tokens.css` — add `--ds-event-*` token block
- `desktop-ui/src/styles/themes.dark.css` / `themes.light.css` / `themes.dim.css` — theme overrides for event tokens
- `desktop-ui/package.json` — add `react-virtuoso` dep

### Created/modified files (Rust)
- `crates/desktop-shared/src/commands/coding_memory.rs` — add `WireEventDto`, `SessionSummaryDto`, `SessionListArgs`, replace `JsonValueWrapper` returns on three recall commands with typed structs
- `crates/desktop/src/commands/coding_memory.rs` — add `coding_memory_session_list` command + `coding_memory_session_replay_typed` command (decodes `payload` JSON via `EventKind` discriminant)
- `crates/app-core/src/coding_memory/handlers.rs` — add `coding_memory_session_list` and `coding_memory_session_replay_typed` handler methods (with `#[tracing::instrument(skip(self), err)]`)
- `crates/desktop/src/specta_builder.rs` — register the two new commands in `klynt_collect_commands![...]`
- `desktop-ui/src/bindings.ts` — auto-regenerated, do not hand-edit
- Tests: `crates/app-core/src/coding_memory/handlers.rs` `#[cfg(test)] mod tests`

### Out-of-scope for this plan
- Context Messages tab, State tab, Dual tab, Agents tab from kimi-vis (require new backend endpoints, deferred)
- Multi-source aggregation (klynt-cli — not implemented yet)
- Skills / MCP / klynt-cli plugin tabs (only stub headers in `PluginsView`)
- Persistent UI prefs (use in-memory state for v1)

---

## Token Translation Reference

For every kimi-vis Tailwind class encountered during porting, use this mapping. Keep this section open while doing Phase D.

| kimi-vis (Tailwind) | klynt (token / class) |
|---|---|
| `bg-blue-500/15 text-blue-700 dark:text-blue-300 border-blue-500/30` | `cm-event-chip cm-event-chip--blue` (chip variants defined in `plugins.css`) |
| `bg-purple-500/15 …` | `cm-event-chip cm-event-chip--purple` |
| `bg-green-500/15 …` | `cm-event-chip cm-event-chip--green` |
| `bg-orange-500/15 …` | `cm-event-chip cm-event-chip--orange` |
| `bg-yellow-500/15 …` | `cm-event-chip cm-event-chip--amber` |
| `bg-cyan-500/15 …` | `cm-event-chip cm-event-chip--cyan` |
| `bg-indigo-500/15 …` | `cm-event-chip cm-event-chip--indigo` |
| `bg-teal-500/15 …` | `cm-event-chip cm-event-chip--teal` |
| `bg-amber-500/15 …` | `cm-event-chip cm-event-chip--amber` |
| `bg-pink-500/15 …` | `cm-event-chip cm-event-chip--pink` |
| `bg-gray-500/15 …` | `cm-event-chip cm-event-chip--neutral` |
| `bg-secondary text-secondary-foreground` (fallback) | `cm-event-chip cm-event-chip--neutral` |
| `text-muted-foreground` | `color: var(--ds-text-subtle);` |
| `text-foreground` | `color: var(--ds-text-strong);` |
| `text-destructive` / `text-red-500` | `color: var(--cm-color-danger-fg);` |
| `text-amber-600 dark:text-amber-400` (slow-delta) | `color: var(--cm-color-warning-fg);` |
| `bg-border` (gap line) | `background: var(--ds-border-subtle);` |
| `bg-red-300 dark:bg-red-700` (slow-gap line) | `background: var(--cm-color-danger-line);` |
| `bg-amber-300 dark:bg-amber-700` (warn-gap line) | `background: var(--cm-color-warning-line);` |
| `bg-popover` | `background: var(--ds-popover-bg);` |
| `bg-muted` / `bg-muted/50` | `background: color-mix(in srgb, var(--ds-surface-muted) 50%, transparent);` |
| `border-border` | `border: 1px solid var(--ds-border-subtle);` |
| `border-primary/40` (active preset) | `border-color: var(--ds-border-accent-soft);` |
| `text-[10px]` / `text-[11px]` | `font-size: var(--fs-2xs);` (10.5px) / `var(--fs-xs);` (11.5px) |
| `font-mono` | `font-family: ui-monospace, SFMono-Regular, monospace;` |
| `rounded-full`, `rounded-lg` | `border-radius: 999px;` / `8px;` |
| Layout utilities (`flex`, `flex-1`, `items-center`, `gap-2`, …) | Plain CSS in component class (`cm-row`, `cm-row__leading`, …) |

Rule: **no `className` strings containing `bg-`, `text-`, `border-`, `flex`, `gap-`, `items-`, `justify-`, `rounded-`, `px-`, `py-`, `w-`, `h-`** survive the port. Catch-up via `grep -nE '\b(bg|text|border|flex|gap|items|justify|rounded|px|py|w|h)-' desktop-ui/src/features/plugins` after each port task — must return zero matches.

---

## Phase A — Plugins Shell (no behavior, just routing)

### Task A1: Add `pluginsNode` slot to `AppLayout`

**Files:**
- Modify: `desktop-ui/src/features/app/components/AppLayout.tsx`
- Modify: `desktop-ui/src/features/app/components/MainAppShell.tsx`

- [ ] **Step 1: Read both files to locate `homeNode` plumbing.**

```bash
grep -n "homeNode" desktop-ui/src/features/app/components/AppLayout.tsx desktop-ui/src/features/app/components/MainAppShell.tsx desktop-ui/src/features/app/components/MainApp.tsx
```

Expected: ~6–10 occurrences across the three files showing prop type, destructure, and JSX render.

- [ ] **Step 2: Add `pluginsNode?: React.ReactNode` next to every `homeNode` declaration.**

For each file: in the props type, the destructuring, and the prop forwarding. Render it conditionally next to `homeNode` using the same `centerMode` discrimination — add a new center mode `"plugins"`.

In `AppLayout.tsx`, render `{centerMode === "plugins" ? pluginsNode : null}` in the same slot that currently renders `homeNode` when `showHome`.

- [ ] **Step 3: Build to confirm no TS errors.**

```bash
cd desktop-ui && bun run typecheck
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add desktop-ui/src/features/app/components/AppLayout.tsx desktop-ui/src/features/app/components/MainAppShell.tsx
git commit -m "feat(plugins): add pluginsNode slot to AppLayout"
```

### Task A2: Extend `appView` union and add `onSelectPlugins` to `MainApp`

**Files:**
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx:326`, around line 1807–1819, and the sidebar prop wiring

- [ ] **Step 1: Change the `appView` type union.**

`MainApp.tsx:326`:

```tsx
const [appView, setAppView] = useState<"home" | "chat" | "plugins">("home");
```

- [ ] **Step 2: Add a stable `onSelectPlugins` callback near the existing `selectHome`/new-chat handlers.**

Search for `onNewChat` definition; add adjacent:

```tsx
const onSelectPlugins = useCallback(() => {
  setAppView("plugins");
}, []);
```

- [ ] **Step 3: Compute `pluginsNode` and pass it through `appLayout`.**

Insert just before the `mainAppShellProps = useMainAppShellProps(...)` block:

```tsx
const pluginsNode = appView === "plugins" ? <PluginsView /> : null;
```

Add `import { PluginsView } from "@/features/plugins/components/PluginsView";` to the imports.

In the `appLayout` object (line 1806–1829):
- Change `centerMode: appView === "chat" ? "chat" : centerMode,` to:
  ```tsx
  centerMode: appView === "chat" ? "chat" : appView === "plugins" ? "plugins" : centerMode,
  ```
- Add `pluginsNode,` next to `homeNode,`.

- [ ] **Step 4: Forward `onSelectPlugins` into the sidebar props.**

Search for where `SidebarChatLayout` is constructed (likely in a `sidebarNode` builder). Add `onSelectPlugins` to its props.

- [ ] **Step 5: typecheck.**

```bash
cd desktop-ui && bun run typecheck
```

Expected: one error — `PluginsView` does not yet exist. That is fine; the next task creates it.

- [ ] **Step 6: Commit (skip if typecheck fails — finish in the next task).**

### Task A3: Wire the existing sidebar `"plugins"` nav item

**Files:**
- Modify: `desktop-ui/src/features/app/components/SidebarChatLayout.tsx` (lines 5–18 props, line 27 navItems)

- [ ] **Step 1: Add `onSelectPlugins` to the props type.**

Replace the props type at line 5–11:

```tsx
type SidebarChatLayoutProps = {
  onOpenSettings: () => void;
  onNewChat: () => void;
  onSelectPlugins: () => void;
  threads: ChatThread[];
  selectedSessionKey: string | null;
  onSelectThread: (sessionKey: string) => void;
};
```

- [ ] **Step 2: Destructure and wire it.**

Update the destructuring (line 21–26) to include `onSelectPlugins`. Update the `navItems` array (line 30) so the `"plugins"` entry has `onClick: onSelectPlugins`:

```tsx
{ id: "plugins", label: "Plugins", icon: <LayoutGrid aria-hidden />, onClick: onSelectPlugins },
```

- [ ] **Step 3: typecheck — should still error only on missing `PluginsView`.**

```bash
cd desktop-ui && bun run typecheck
```

- [ ] **Step 4: Commit (defer until A4).**

### Task A4: Create the empty `PluginsView` shell

**Files:**
- Create: `desktop-ui/src/features/plugins/components/PluginsView.tsx`
- Create: `desktop-ui/src/features/plugins/components/PluginsView.test.tsx`
- Create: `desktop-ui/src/styles/plugins.css`
- Modify: `desktop-ui/src/styles/index.css`

- [ ] **Step 1: Write the failing test first.**

```tsx
// PluginsView.test.tsx
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
});
```

- [ ] **Step 2: Run the test — expect failure (file does not exist).**

```bash
cd desktop-ui && bun run test -- PluginsView
```

Expected: FAIL — module not found.

- [ ] **Step 3: Create the minimal component.**

```tsx
// PluginsView.tsx
import { useState } from "react";
import { CodingMemoryPlugin } from "@/features/plugins/coding-memory/CodingMemoryPlugin";

type PluginId = "coding-memory" | "skills" | "mcp" | "klynt-cli";

const PLUGIN_TABS: ReadonlyArray<{ id: PluginId; label: string; available: boolean }> = [
  { id: "coding-memory", label: "Coding Memory", available: true },
  { id: "skills", label: "Skills", available: false },
  { id: "mcp", label: "MCP Servers", available: false },
  { id: "klynt-cli", label: "Klynt CLI", available: false },
];

export function PluginsView() {
  const [active, setActive] = useState<PluginId>("coding-memory");

  return (
    <section className="plugins-view" aria-labelledby="plugins-heading">
      <header className="plugins-view__header">
        <h1 id="plugins-heading" className="plugins-view__title">Plugins</h1>
        <p className="plugins-view__subtitle">Internal plugins for inspecting and tuning your Klynt platform.</p>
      </header>
      <div role="tablist" aria-label="Plugins" className="plugins-view__tabs">
        {PLUGIN_TABS.map((tab) => (
          <button
            key={tab.id}
            role="tab"
            type="button"
            aria-selected={active === tab.id}
            disabled={!tab.available}
            className={
              "plugins-view__tab" +
              (active === tab.id ? " plugins-view__tab--active" : "") +
              (tab.available ? "" : " plugins-view__tab--coming-soon")
            }
            onClick={() => tab.available && setActive(tab.id)}
          >
            {tab.label}
            {!tab.available && <span className="plugins-view__tab-badge">Soon</span>}
          </button>
        ))}
      </div>
      <div className="plugins-view__pane" data-testid="plugins-active-pane" data-plugin={active}>
        {active === "coding-memory" ? <CodingMemoryPlugin /> : null}
      </div>
    </section>
  );
}
```

Create a temporary `CodingMemoryPlugin.tsx` stub at the same time so the import resolves:

```tsx
// desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx
export function CodingMemoryPlugin() {
  return <div className="cm-plugin">Coding Memory (loading…)</div>;
}
```

- [ ] **Step 4: Create stylesheet.**

```css
/* desktop-ui/src/styles/plugins.css */
.plugins-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  background: var(--ds-surface-card);
}

.plugins-view__header {
  padding: 24px 28px 12px;
  border-bottom: 1px solid var(--ds-border-subtle);
}
.plugins-view__title { font-size: var(--fs-xl); margin: 0 0 4px; color: var(--ds-text-strong); }
.plugins-view__subtitle { font-size: var(--fs-xs); color: var(--ds-text-subtle); margin: 0; }

.plugins-view__tabs {
  display: flex;
  gap: 4px;
  padding: 8px 20px 0;
  border-bottom: 1px solid var(--ds-border-subtle);
}
.plugins-view__tab {
  position: relative;
  padding: 8px 12px;
  font-size: var(--fs-sm);
  color: var(--ds-text-subtle);
  background: transparent;
  border: 0;
  border-bottom: 2px solid transparent;
  cursor: pointer;
  transition: color var(--ds-dur-fast) var(--ds-ease-out);
}
.plugins-view__tab:hover:not(:disabled) { color: var(--ds-text-strong); }
.plugins-view__tab--active { color: var(--ds-text-strong); border-bottom-color: var(--ds-border-accent-soft); }
.plugins-view__tab--coming-soon { opacity: 0.55; cursor: not-allowed; }
.plugins-view__tab-badge {
  margin-left: 6px;
  padding: 1px 6px;
  font-size: var(--fs-2xs);
  border-radius: 999px;
  background: var(--ds-surface-muted);
  color: var(--ds-text-faint);
}

.plugins-view__pane { flex: 1; overflow: hidden; }
```

- [ ] **Step 5: Add `@import "./plugins.css";` to `desktop-ui/src/styles/index.css`.**

Place it after `@import "./settings.css";` (last entry) — alphabetical order is not enforced.

- [ ] **Step 6: Run tests.**

```bash
cd desktop-ui && bun run test -- PluginsView
```

Expected: PASS.

- [ ] **Step 7: Run full typecheck + lint.**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Expected: 0 errors, 0 warnings.

- [ ] **Step 8: Commit.**

```bash
git add desktop-ui/src/features/plugins/ desktop-ui/src/styles/plugins.css desktop-ui/src/styles/index.css desktop-ui/src/features/app/components/MainApp.tsx desktop-ui/src/features/app/components/SidebarChatLayout.tsx desktop-ui/src/features/app/components/AppLayout.tsx desktop-ui/src/features/app/components/MainAppShell.tsx
git commit -m "feat(plugins): wire empty Plugins page shell behind sidebar nav"
```

- [ ] **Step 9: Manual verify (cannot be automated).**

Run `cargo tauri dev` (Vite separately). Click `Plugins` in the sidebar. Confirm the page renders with the title and 4 tabs (one active, three "Soon"). Cmd-click the chat sidebar nav after to confirm `appView === "chat"` still works.

---

## Phase B — Backend DTO Touch-ups

### Task B1: Add `WireEventDto` typed event

**Files:**
- Modify: `crates/desktop-shared/src/commands/coding_memory.rs`
- Test: same file, `#[cfg(test)] mod tests` block

- [ ] **Step 1: Write the failing decode test.**

Append to the file:

```rust
#[cfg(test)]
mod dto_tests {
    use super::*;

    #[test]
    fn wire_event_dto_decodes_session_replay_payload() {
        // A SessionStart payload as it appears in `ingest_event_log.payload`.
        let payload = serde_json::json!({
            "v": 1,
            "id": "01923aaa-bbbb-cccc-dddd-eeeeeeeeeeee",
            "source": "kimiCli",
            "sessionId": "sess-123",
            "turnId": null,
            "cwd": "/repo",
            "repo": null,
            "occurredAt": "2026-04-28T12:00:00Z",
            "kind": { "type": "sessionStart", "model": "claude-haiku-4-5", "skills": [] }
        });
        let entry = SessionReplayEntry {
            id: "row-1".into(),
            source: "kimiCli".into(),
            session_id: "sess-123".into(),
            kind: "sessionStart".into(),
            occurred_at: "2026-04-28T12:00:00Z".into(),
            payload: payload.to_string(),
        };
        let dto = WireEventDto::from_replay_entry(&entry).expect("decoded");
        assert_eq!(dto.kind, "sessionStart");
        assert_eq!(dto.session_id, "sess-123");
        assert_eq!(dto.source, "kimiCli");
        assert!(dto.raw_json.contains("claude-haiku-4-5"));
    }

    #[test]
    fn wire_event_dto_handles_corrupt_payload() {
        let entry = SessionReplayEntry {
            id: "row-2".into(),
            source: "codex".into(),
            session_id: "sess-2".into(),
            kind: "toolCall".into(),
            occurred_at: "2026-04-28T12:00:01Z".into(),
            payload: "not json".into(),
        };
        let dto = WireEventDto::from_replay_entry(&entry).expect("falls back to raw");
        assert_eq!(dto.kind, "toolCall");
        assert!(dto.payload_decoded.is_none());
        assert_eq!(dto.raw_json, "not json");
    }
}
```

- [ ] **Step 2: Run test — expect FAIL (`WireEventDto` undefined).**

```bash
cargo nextest run -p desktop-shared dto_tests
```

- [ ] **Step 3: Add the DTO + helper.**

Insert after the `SessionReplayEntry` block (line 30):

```rust
/// Decoded form of `SessionReplayEntry` for the FE viewer. The original
/// `payload` is kept as `raw_json` for the detail panel; `payload_decoded`
/// is the parsed structured value when valid JSON.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct WireEventDto {
    pub id: String,
    pub source: String,
    pub session_id: String,
    pub kind: String,
    pub occurred_at: String,
    /// Structured payload when `payload` parsed as JSON; `None` otherwise.
    pub payload_decoded: Option<serde_json::Value>,
    /// Raw JSON string (always present, exactly as stored in `ingest_event_log`).
    pub raw_json: String,
}

impl WireEventDto {
    pub fn from_replay_entry(entry: &SessionReplayEntry) -> Result<Self, std::convert::Infallible> {
        let payload_decoded = serde_json::from_str::<serde_json::Value>(&entry.payload).ok();
        Ok(Self {
            id: entry.id.clone(),
            source: entry.source.clone(),
            session_id: entry.session_id.clone(),
            kind: entry.kind.clone(),
            occurred_at: entry.occurred_at.clone(),
            payload_decoded,
            raw_json: entry.payload.clone(),
        })
    }
}
```

- [ ] **Step 4: Run test — expect PASS.**

```bash
cargo nextest run -p desktop-shared dto_tests
```

- [ ] **Step 5: Commit.**

```bash
git add crates/desktop-shared/src/commands/coding_memory.rs
git commit -m "feat(coding-memory): add WireEventDto with payload decode"
```

### Task B2: Add `SessionSummaryDto` and `SessionListArgs`

**Files:**
- Modify: `crates/desktop-shared/src/commands/coding_memory.rs`

- [ ] **Step 1: Append both types after `WireEventDto`.**

```rust
/// One row in the Plugins → Coding Memory session list.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummaryDto {
    pub session_id: String,
    pub source: String,
    pub cwd: Option<String>,
    pub repo_id: Option<String>,
    pub started_at: String,
    pub last_event_at: String,
    pub event_count: i64,
    pub turn_count: i64,
    pub tool_call_count: i64,
    pub error_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_usd: f64,
}

/// Filters for `coding_memory_session_list`.
#[derive(Debug, Clone, Serialize, Deserialize, specta::Type, Default)]
#[serde(rename_all = "camelCase")]
pub struct SessionListArgs {
    /// Optional source filter (`"claudeCode" | "codex" | "kimiCli" | "openCode"`).
    pub source: Option<String>,
    /// Optional repo id.
    pub repo_id: Option<String>,
    /// Look back N days. Defaults to 14.
    #[serde(default = "default_days_14")]
    pub since_days: u32,
    #[serde(default = "default_limit_100")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

fn default_days_14() -> u32 { 14 }
fn default_limit_100() -> u32 { 100 }
```

- [ ] **Step 2: Verify compile.**

```bash
cargo build -p desktop-shared
```

Expected: PASS.

- [ ] **Step 3: Commit.**

```bash
git commit -am "feat(coding-memory): add SessionSummaryDto + SessionListArgs"
```

### Task B3: Implement `coding_memory_session_list` handler

**Files:**
- Modify: `crates/app-core/src/coding_memory/handlers.rs`
- Test: same file

- [ ] **Step 1: Read existing handler patterns.**

```bash
grep -n "coding_memory_session_replay\|tracing::instrument" crates/app-core/src/coding_memory/handlers.rs | head -20
```

Use the existing `coding_memory_session_replay` method as the structural template (it queries `ingest_event_log` directly).

- [ ] **Step 2: Write the failing test.**

Append in the `#[cfg(test)] mod tests` block:

```rust
#[tokio::test]
async fn session_list_groups_events_by_session() {
    use crate::tests::helpers::test_app_core_with_pool;
    let core = test_app_core_with_pool().await;
    seed_two_sessions(&core).await; // helper inserts 2 sessions, 5 events each
    let rows = core
        .coding_memory_session_list(SessionListArgs::default())
        .await
        .expect("list");
    assert_eq!(rows.len(), 2);
    assert!(rows[0].event_count >= 1);
    assert!(rows.iter().any(|r| r.source == "kimiCli"));
}
```

If `seed_two_sessions` does not exist, add it as a helper that inserts directly into `ingest_event_log` via `IngestEventLogRepo`.

- [ ] **Step 3: Run test — expect FAIL.**

```bash
cargo nextest run -p app-core session_list_groups_events_by_session
```

- [ ] **Step 4: Implement the handler.**

```rust
#[tracing::instrument(skip(self), err)]
pub async fn coding_memory_session_list(
    &self,
    args: SessionListArgs,
) -> Result<Vec<SessionSummaryDto>> {
    let pool = self.storage.pool();
    let mut sql = String::from(
        "SELECT session_id,
                source,
                MIN(cwd) AS cwd,
                MIN(repo_id) AS repo_id,
                MIN(occurred_at) AS started_at,
                MAX(occurred_at) AS last_event_at,
                COUNT(*) AS event_count,
                SUM(CASE WHEN kind LIKE '%TurnBegin%' THEN 1 ELSE 0 END) AS turn_count,
                SUM(CASE WHEN kind = 'toolCall' THEN 1 ELSE 0 END) AS tool_call_count,
                SUM(CASE WHEN kind = 'error' THEN 1 ELSE 0 END) AS error_count
         FROM ingest_event_log
         WHERE occurred_at >= datetime('now', ?1)",
    );
    let since_clause = format!("-{} days", args.since_days);
    let mut binds: Vec<String> = vec![since_clause];
    if let Some(src) = args.source.as_ref() {
        sql.push_str(" AND source = ?2");
        binds.push(src.clone());
    }
    if let Some(repo) = args.repo_id.as_ref() {
        sql.push_str(if binds.len() == 2 { " AND repo_id = ?3" } else { " AND repo_id = ?2" });
        binds.push(repo.clone());
    }
    sql.push_str(" GROUP BY session_id, source ORDER BY last_event_at DESC LIMIT ? OFFSET ?");
    binds.push(args.limit.to_string());
    binds.push(args.offset.to_string());

    let mut q = sqlx::query(&sql);
    for b in &binds { q = q.bind(b); }
    let rows = q.fetch_all(pool).await?;

    let result = rows.into_iter().map(|r| {
        use sqlx::Row;
        SessionSummaryDto {
            session_id: r.get("session_id"),
            source: r.get("source"),
            cwd: r.try_get("cwd").ok(),
            repo_id: r.try_get("repo_id").ok(),
            started_at: r.get("started_at"),
            last_event_at: r.get("last_event_at"),
            event_count: r.get("event_count"),
            turn_count: r.get("turn_count"),
            tool_call_count: r.get("tool_call_count"),
            error_count: r.get("error_count"),
            // Token + cost rollups deferred to follow-up; surface zeros for now.
            total_input_tokens: 0,
            total_output_tokens: 0,
            total_cost_usd: 0.0,
        }
    }).collect();
    Ok(result)
}
```

- [ ] **Step 5: Run the test — expect PASS.**

```bash
cargo nextest run -p app-core session_list_groups_events_by_session
```

- [ ] **Step 6: Commit.**

```bash
git commit -am "feat(coding-memory): app-core session list handler"
```

### Task B4: Implement `coding_memory_session_replay_typed` handler

**Files:**
- Modify: `crates/app-core/src/coding_memory/handlers.rs`

- [ ] **Step 1: Add the handler delegating to existing replay + map.**

```rust
#[tracing::instrument(skip(self), err)]
pub async fn coding_memory_session_replay_typed(
    &self,
    session_id: String,
    limit: i64,
    offset: i64,
) -> Result<Vec<WireEventDto>> {
    let entries = self.coding_memory_session_replay(session_id, limit, offset).await?;
    Ok(entries
        .iter()
        .filter_map(|e| WireEventDto::from_replay_entry(e).ok())
        .collect())
}
```

- [ ] **Step 2: Add a test.**

```rust
#[tokio::test]
async fn replay_typed_decodes_payloads() {
    use crate::tests::helpers::test_app_core_with_pool;
    let core = test_app_core_with_pool().await;
    seed_two_sessions(&core).await;
    let rows = core
        .coding_memory_session_replay_typed("sess-1".into(), 100, 0)
        .await
        .expect("replay");
    assert!(!rows.is_empty());
    assert!(rows.iter().any(|r| r.payload_decoded.is_some()));
}
```

- [ ] **Step 3: Run + commit.**

```bash
cargo nextest run -p app-core replay_typed_decodes_payloads
git commit -am "feat(coding-memory): typed session replay handler"
```

### Task B5: Register new Tauri commands

**Files:**
- Modify: `crates/desktop/src/commands/coding_memory.rs`
- Modify: `crates/desktop/src/specta_builder.rs`

- [ ] **Step 1: Add the command shells.**

In `commands/coding_memory.rs`:

```rust
#[klynt_command]
pub async fn coding_memory_session_list(
    core: &AppCore,
    args: SessionListArgs,
) -> Vec<SessionSummaryDto> {
    core.coding_memory_session_list(args).await.unwrap_or_default()
}

#[klynt_command]
pub async fn coding_memory_session_replay_typed(
    core: &AppCore,
    session_id: String,
    limit: Option<i64>,
    offset: Option<i64>,
) -> Vec<WireEventDto> {
    core.coding_memory_session_replay_typed(
        session_id,
        limit.unwrap_or(500),
        offset.unwrap_or(0),
    )
    .await
    .unwrap_or_default()
}
```

Plus the `match` branches in the dev-server dispatcher (around line 291) — follow the existing `"coding_memory_session_replay"` arm as a template.

- [ ] **Step 2: Register in specta.**

In `specta_builder.rs` `klynt_collect_commands![...]`, add the two paths.

- [ ] **Step 3: Build full workspace.**

```bash
cargo build -p desktop
```

Expected: PASS.

- [ ] **Step 4: Regenerate bindings.**

```bash
cargo tauri dev   # Ctrl-C after the bindings file regenerates
```

Confirm `desktop-ui/src/bindings.ts` now contains `codingMemorySessionList` and `codingMemorySessionReplayTyped`.

- [ ] **Step 5: Run the registration drift + bindings_are_current tests.**

```bash
cargo nextest run -p desktop -E 'test(registration_drift) + test(bindings_are_current) + test(no_raw_tauri_command_outside_macros)'
```

Expected: 3 PASS.

- [ ] **Step 6: Commit.**

```bash
git add crates/desktop/src/commands/coding_memory.rs crates/desktop/src/specta_builder.rs desktop-ui/src/bindings.ts
git commit -m "feat(coding-memory): expose typed session list + replay commands"
```

---

## Phase C — Design Tokens for Event Chips

### Task C1: Define `--ds-event-*` tokens for all 25 event variants

**Files:**
- Modify: `desktop-ui/src/styles/ds-tokens.css`

- [ ] **Step 1: Append a token block at the end of `:root { … }`.**

```css
  /* ── Coding Memory event chips ──
     One color family per logical group; bg/fg/border vars feed
     `cm-event-chip--<color>` variants in plugins.css. */
  --cm-color-blue-bg:      color-mix(in srgb, #3b82f6 15%, transparent);
  --cm-color-blue-fg:      #1d4ed8;
  --cm-color-blue-border:  color-mix(in srgb, #3b82f6 30%, transparent);

  --cm-color-purple-bg:    color-mix(in srgb, #a855f7 15%, transparent);
  --cm-color-purple-fg:    #7e22ce;
  --cm-color-purple-border:color-mix(in srgb, #a855f7 30%, transparent);

  --cm-color-green-bg:     color-mix(in srgb, #22c55e 15%, transparent);
  --cm-color-green-fg:     #15803d;
  --cm-color-green-border: color-mix(in srgb, #22c55e 30%, transparent);

  --cm-color-orange-bg:    color-mix(in srgb, #f97316 15%, transparent);
  --cm-color-orange-fg:    #c2410c;
  --cm-color-orange-border:color-mix(in srgb, #f97316 30%, transparent);

  --cm-color-amber-bg:     color-mix(in srgb, #f59e0b 15%, transparent);
  --cm-color-amber-fg:     #b45309;
  --cm-color-amber-border: color-mix(in srgb, #f59e0b 30%, transparent);

  --cm-color-cyan-bg:      color-mix(in srgb, #06b6d4 15%, transparent);
  --cm-color-cyan-fg:      #0e7490;
  --cm-color-cyan-border:  color-mix(in srgb, #06b6d4 30%, transparent);

  --cm-color-indigo-bg:    color-mix(in srgb, #6366f1 15%, transparent);
  --cm-color-indigo-fg:    #4338ca;
  --cm-color-indigo-border:color-mix(in srgb, #6366f1 30%, transparent);

  --cm-color-teal-bg:      color-mix(in srgb, #14b8a6 15%, transparent);
  --cm-color-teal-fg:      #0f766e;
  --cm-color-teal-border:  color-mix(in srgb, #14b8a6 30%, transparent);

  --cm-color-pink-bg:      color-mix(in srgb, #ec4899 15%, transparent);
  --cm-color-pink-fg:      #be185d;
  --cm-color-pink-border:  color-mix(in srgb, #ec4899 30%, transparent);

  --cm-color-neutral-bg:   color-mix(in srgb, #6b7280 15%, transparent);
  --cm-color-neutral-fg:   var(--ds-text-subtle);
  --cm-color-neutral-border: var(--ds-border-subtle);

  /* Slow / error highlights for the timeline gutter */
  --cm-color-warning-fg:   #b45309;
  --cm-color-warning-line: color-mix(in srgb, #f59e0b 35%, transparent);
  --cm-color-danger-fg:    #b91c1c;
  --cm-color-danger-line:  color-mix(in srgb, #ef4444 40%, transparent);
```

- [ ] **Step 2: Add dark-theme overrides.**

In `desktop-ui/src/styles/themes.dark.css` (append at end):

```css
[data-theme="dark"] {
  --cm-color-blue-fg:    #93c5fd;
  --cm-color-purple-fg:  #d8b4fe;
  --cm-color-green-fg:   #86efac;
  --cm-color-orange-fg:  #fdba74;
  --cm-color-amber-fg:   #fcd34d;
  --cm-color-cyan-fg:    #67e8f9;
  --cm-color-indigo-fg:  #a5b4fc;
  --cm-color-teal-fg:    #5eead4;
  --cm-color-pink-fg:    #f9a8d4;
  --cm-color-warning-fg: #fcd34d;
  --cm-color-danger-fg:  #fca5a5;
}
```

Mirror appropriately in `themes.dim.css` (slightly less saturated) and confirm `themes.light.css` already inherits the `:root` defaults.

- [ ] **Step 3: Run typecheck (CSS doesn't compile, just assert nothing breaks).**

```bash
cd desktop-ui && bun run build
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add desktop-ui/src/styles/ds-tokens.css desktop-ui/src/styles/themes.*.css
git commit -m "feat(plugins): add --cm-color-* tokens for event chips"
```

### Task C2: Add chip + timeline classes to `plugins.css`

**Files:**
- Modify: `desktop-ui/src/styles/plugins.css`

- [ ] **Step 1: Append chip styles.**

```css
/* Event chip — used in WireFilters and WireEventCard */
.cm-event-chip {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 2px 8px;
  font-size: var(--fs-2xs);
  font-weight: 500;
  border-radius: 999px;
  border: 1px solid;
  font-family: ui-monospace, SFMono-Regular, monospace;
  letter-spacing: 0.02em;
  white-space: nowrap;
  cursor: pointer;
  transition: opacity var(--ds-dur-fast) var(--ds-ease-out);
}
.cm-event-chip[aria-pressed="false"] { opacity: 0.55; }
.cm-event-chip:hover { opacity: 1; }

.cm-event-chip--blue    { background: var(--cm-color-blue-bg);    color: var(--cm-color-blue-fg);    border-color: var(--cm-color-blue-border); }
.cm-event-chip--purple  { background: var(--cm-color-purple-bg);  color: var(--cm-color-purple-fg);  border-color: var(--cm-color-purple-border); }
.cm-event-chip--green   { background: var(--cm-color-green-bg);   color: var(--cm-color-green-fg);   border-color: var(--cm-color-green-border); }
.cm-event-chip--orange  { background: var(--cm-color-orange-bg);  color: var(--cm-color-orange-fg);  border-color: var(--cm-color-orange-border); }
.cm-event-chip--amber   { background: var(--cm-color-amber-bg);   color: var(--cm-color-amber-fg);   border-color: var(--cm-color-amber-border); }
.cm-event-chip--cyan    { background: var(--cm-color-cyan-bg);    color: var(--cm-color-cyan-fg);    border-color: var(--cm-color-cyan-border); }
.cm-event-chip--indigo  { background: var(--cm-color-indigo-bg);  color: var(--cm-color-indigo-fg);  border-color: var(--cm-color-indigo-border); }
.cm-event-chip--teal    { background: var(--cm-color-teal-bg);    color: var(--cm-color-teal-fg);    border-color: var(--cm-color-teal-border); }
.cm-event-chip--pink    { background: var(--cm-color-pink-bg);    color: var(--cm-color-pink-fg);    border-color: var(--cm-color-pink-border); }
.cm-event-chip--neutral { background: var(--cm-color-neutral-bg); color: var(--cm-color-neutral-fg); border-color: var(--cm-color-neutral-border); }

/* Timeline gap row — shown between events when delta > threshold */
.cm-gap {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 2px 12px;
  font-size: var(--fs-2xs);
  color: var(--ds-text-faint);
}
.cm-gap__line { flex: 1; height: 1px; background: var(--ds-border-subtle); }
.cm-gap--warn { color: var(--cm-color-warning-fg); }
.cm-gap--warn .cm-gap__line { background: var(--cm-color-warning-line); }
.cm-gap--danger { color: var(--cm-color-danger-fg); }
.cm-gap--danger .cm-gap__line { background: var(--cm-color-danger-line); }
```

- [ ] **Step 2: Build + commit.**

```bash
cd desktop-ui && bun run build
git commit -am "feat(plugins): chip + timeline classes"
```

### Task C3: Map `EventKind` → color variant

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/eventHelpers.ts`
- Create: `desktop-ui/src/features/plugins/coding-memory/eventHelpers.test.ts`

- [ ] **Step 1: Write test first.**

```ts
// eventHelpers.test.ts
import { describe, it, expect } from "vitest";
import { eventChipColor, isErrorEvent, formatTimeDelta } from "./eventHelpers";
import type { WireEventDto } from "./types";

const ev = (over: Partial<WireEventDto>): WireEventDto => ({
  id: "x", source: "kimiCli", sessionId: "s", kind: "toolCall",
  occurredAt: "2026-04-28T00:00:00Z", payloadDecoded: null, rawJson: "{}",
  ...over,
});

describe("eventChipColor", () => {
  it("maps tool kinds to purple", () => {
    expect(eventChipColor("toolCall")).toBe("purple");
    expect(eventChipColor("toolResult")).toBe("purple");
  });
  it("maps turn kinds to blue", () => {
    expect(eventChipColor("turnBegin")).toBe("blue");
    expect(eventChipColor("turnEnd")).toBe("blue");
  });
  it("maps step to green", () => {
    expect(eventChipColor("stepBegin")).toBe("green");
  });
  it("falls back to neutral for unknown", () => {
    expect(eventChipColor("ufoSighting")).toBe("neutral");
  });
});

describe("isErrorEvent", () => {
  it("flags error kind", () => {
    expect(isErrorEvent(ev({ kind: "error" }))).toBe(true);
  });
  it("flags toolResult.is_error payload", () => {
    expect(isErrorEvent(ev({
      kind: "toolResult",
      payloadDecoded: { return_value: { is_error: true } } as any,
    }))).toBe(true);
  });
  it("does not flag plain toolResult", () => {
    expect(isErrorEvent(ev({ kind: "toolResult", payloadDecoded: { return_value: { ok: 1 } } as any }))).toBe(false);
  });
});

describe("formatTimeDelta", () => {
  it("renders sub-second as ms", () => {
    expect(formatTimeDelta(new Date("2026-04-28T00:00:00.500Z"), new Date("2026-04-28T00:00:00.300Z"))).toBe("+200ms");
  });
  it("renders seconds", () => {
    expect(formatTimeDelta(new Date("2026-04-28T00:00:30Z"), new Date("2026-04-28T00:00:00Z"))).toBe("+30.00s");
  });
  it("returns empty for under-1ms", () => {
    expect(formatTimeDelta(new Date("2026-04-28T00:00:00.001Z"), new Date("2026-04-28T00:00:00.001Z"))).toBe("");
  });
});
```

- [ ] **Step 2: Run — expect FAIL.**

```bash
cd desktop-ui && bun run test -- eventHelpers
```

- [ ] **Step 3: Implement.**

```ts
// eventHelpers.ts
import type { WireEventDto } from "./types";

export type ChipColor =
  | "blue" | "purple" | "green" | "orange" | "amber"
  | "cyan" | "indigo" | "teal" | "pink" | "neutral";

const KIND_TO_COLOR: Record<string, ChipColor> = {
  // turn
  turnBegin: "blue", turnEnd: "blue", steerInput: "blue",
  // step
  stepBegin: "green", stepInterrupted: "amber",
  // compaction
  compactionBegin: "orange", compactionEnd: "orange", compactionApplied: "orange",
  // mcp
  mcpLoadingBegin: "cyan", mcpLoadingEnd: "cyan",
  // status / notifications
  statusUpdate: "neutral", notification: "amber",
  // text
  textPart: "neutral", thinkPart: "neutral",
  planDisplay: "teal",
  // tools
  toolCall: "purple", toolResult: "purple",
  toolCallPart: "purple", toolCallRequest: "purple",
  // approvals
  questionRequest: "amber", approvalRequest: "amber", approvalResponse: "amber",
  approvalDecision: "amber",
  // sub-agents
  subagentEvent: "indigo",
  // media
  imageUrlPart: "pink", videoUrlPart: "pink", audioUrlPart: "pink",
  // klynt-internal-rich
  skillActivated: "teal", recallInjected: "indigo", providerCall: "cyan",
  fileEdit: "purple", testRun: "green", error: "neutral",
  gitCommit: "blue", mirrorAlert: "amber", sessionStart: "blue", sessionEnd: "blue",
  userPrompt: "blue", assistantMsg: "neutral",
};

export function eventChipColor(kind: string): ChipColor {
  return KIND_TO_COLOR[kind] ?? "neutral";
}

export function isErrorEvent(e: WireEventDto): boolean {
  if (e.kind === "error") return true;
  if (e.kind === "toolResult") {
    const rv = (e.payloadDecoded as any)?.return_value;
    if (rv && typeof rv === "object" && rv.is_error === true) return true;
  }
  if (e.kind === "stepInterrupted") return true;
  if (e.kind === "approvalDecision" || e.kind === "approvalResponse") {
    const r = (e.payloadDecoded as any)?.response;
    if (r === "reject") return true;
  }
  return false;
}

export function formatTimestamp(iso: string): string {
  return new Date(iso).toLocaleTimeString(undefined, {
    hour: "2-digit", minute: "2-digit", second: "2-digit",
    fractionalSecondDigits: 3,
  });
}

export function formatTimeDelta(current: Date, prev: Date): string {
  const ms = current.getTime() - prev.getTime();
  if (ms < 1) return "";
  if (ms < 1000) return `+${Math.round(ms)}ms`;
  const s = ms / 1000;
  if (s < 60) return `+${s.toFixed(2)}s`;
  return `+${(s / 60).toFixed(1)}min`;
}

export function timeDeltaSeverity(ms: number): "ok" | "warn" | "danger" {
  if (ms > 60_000) return "danger";
  if (ms > 10_000) return "warn";
  return "ok";
}

export function summarizeEvent(e: WireEventDto): string {
  const p = (e.payloadDecoded ?? {}) as Record<string, unknown>;
  switch (e.kind) {
    case "turnBegin":
    case "userPrompt":
      return truncate(extractText((p as any).user_input ?? (p as any).text), 120);
    case "toolCall":
      return String((p as any).function?.name ?? (p as any).tool_name ?? "");
    case "toolResult":
      return truncate(JSON.stringify((p as any).return_value ?? p), 160);
    case "thinkPart":
      return truncate(String((p as any).think ?? ""), 120);
    case "textPart":
      return truncate(String((p as any).text ?? ""), 120);
    default:
      return truncate(JSON.stringify(p), 120);
  }
}

function truncate(s: string, max: number): string {
  return s.length > max ? s.slice(0, max) + "…" : s;
}
function extractText(x: unknown): string {
  if (typeof x === "string") return x;
  if (Array.isArray(x) && x.length > 0) {
    const first = x[0] as Record<string, unknown>;
    return String(first.text ?? "");
  }
  return "";
}
```

Also create `types.ts`:

```ts
// types.ts
export type { WireEventDto, SessionSummaryDto, SessionListArgs, SessionReplayEntry } from "@/bindings";
export type ProviderId = "claudeCode" | "codex" | "kimiCli" | "openCode";
```

- [ ] **Step 4: Run test — expect PASS.**

```bash
cd desktop-ui && bun run test -- eventHelpers
```

- [ ] **Step 5: Commit.**

```bash
git add desktop-ui/src/features/plugins/coding-memory/eventHelpers.ts desktop-ui/src/features/plugins/coding-memory/eventHelpers.test.ts desktop-ui/src/features/plugins/coding-memory/types.ts
git commit -m "feat(plugins): event helpers + types"
```

---

## Phase D — Wire Events Port

### Task D1: Add `react-virtuoso` dep + create endpoint module

**Files:**
- Modify: `desktop-ui/package.json`
- Create: `desktop-ui/src/api/endpoints/codingMemory.ts`
- Modify: `desktop-ui/src/api/endpoints/index.ts`

- [ ] **Step 1: Install dep.**

```bash
cd desktop-ui && bun add react-virtuoso
```

Confirm `package.json` and `bun.lock` updated.

- [ ] **Step 2: Create the endpoint wrapper.**

```ts
// desktop-ui/src/api/endpoints/codingMemory.ts
import { commands } from "@/bindings";
import type { SessionListArgs } from "@/features/plugins/coding-memory/types";

export async function listCodingSessions(args: SessionListArgs) {
  const r = await commands.codingMemorySessionList(args);
  if (r.status !== "ok") throw new Error(r.error.message ?? "session list failed");
  return r.data;
}

export async function fetchSessionWire(
  sessionId: string,
  limit = 500,
  offset = 0,
) {
  const r = await commands.codingMemorySessionReplayTyped(sessionId, limit, offset);
  if (r.status !== "ok") throw new Error(r.error.message ?? "replay failed");
  return r.data;
}

export async function fetchCliHealth() {
  const r = await commands.codingMemoryCliHealth();
  if (r.status !== "ok") throw new Error(r.error.message ?? "health failed");
  return r.data;
}
```

Add an export in `desktop-ui/src/api/endpoints/index.ts`:

```ts
export * as codingMemory from "./codingMemory";
```

- [ ] **Step 3: typecheck.**

```bash
cd desktop-ui && bun run typecheck
```

Expected: PASS.

- [ ] **Step 4: Commit.**

```bash
git add desktop-ui/package.json desktop-ui/bun.lock desktop-ui/src/api/endpoints/codingMemory.ts desktop-ui/src/api/endpoints/index.ts
git commit -m "feat(plugins): coding memory endpoint wrappers + virtuoso dep"
```

### Task D2: Port `WireEventCard` (chip, gap row, summary)

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/WireEventCard.tsx`

- [ ] **Step 1: Read the source.**

```bash
sed -n '1,400p' /Users/jayden/Projects/Klynt/kimi-cli/vis/src/features/wire-viewer/wire-event-card.tsx
```

- [ ] **Step 2: Translate to klynt taxonomy + tokens.**

Key transformations beyond the className map:
- Replace `event.type` → `event.kind` everywhere.
- Replace `event.payload` → `event.payloadDecoded`.
- Replace `event.timestamp` → `new Date(event.occurredAt)`.
- Replace `TYPE_COLORS[event.type] ?? "bg-secondary …"` with `\`cm-event-chip cm-event-chip--${eventChipColor(event.kind)}\``.
- Replace `formatTimestamp(ts: number)` with our `formatTimestamp(iso: string)` from `eventHelpers`.
- Replace `summarizeUserInput(p.user_input)` calls with `summarizeEvent(event)`.
- Replace `bg-popover` → `var(--ds-popover-bg)` (inline style or class).
- Replace `text-muted-foreground` → `cm-event-card__meta`.

Use this skeleton (copy-translate the rest by following the source):

```tsx
import { useState, memo } from "react";
import { ChevronRight, ChevronDown, AlertCircle, Copy, Check } from "lucide-react";
import {
  eventChipColor,
  isErrorEvent,
  formatTimestamp,
  formatTimeDelta,
  timeDeltaSeverity,
  summarizeEvent,
} from "./eventHelpers";
import type { WireEventDto } from "./types";

export interface WireEventCardProps {
  event: WireEventDto;
  expanded: boolean;
  onToggle: () => void;
  onSelect?: () => void;
  selected?: boolean;
  prevEvent?: WireEventDto;
  nestLevel?: number;
  linkedToolName?: string;
  linkedToolCallId?: string;
  searchMatch?: boolean;
}

export const WireEventCard = memo(function WireEventCard(props: WireEventCardProps) {
  const { event, expanded, onToggle, onSelect, selected, prevEvent, nestLevel = 0, searchMatch } = props;
  const color = eventChipColor(event.kind);
  const isError = isErrorEvent(event);
  const [copied, setCopied] = useState(false);

  const gap = prevEvent ? renderGap(event, prevEvent) : null;

  return (
    <>
      {gap}
      <div
        className={
          "cm-event-card" +
          (selected ? " cm-event-card--selected" : "") +
          (searchMatch ? " cm-event-card--search-hit" : "") +
          (isError ? " cm-event-card--error" : "")
        }
        style={nestLevel ? { paddingLeft: `${20 + nestLevel * 16}px` } : undefined}
        onClick={onSelect}
      >
        <button type="button" className="cm-event-card__chevron" onClick={(e) => { e.stopPropagation(); onToggle(); }}>
          {expanded ? <ChevronDown size={12} /> : <ChevronRight size={12} />}
        </button>
        <span className="cm-event-card__time">{formatTimestamp(event.occurredAt)}</span>
        <span className={`cm-event-chip cm-event-chip--${color}`}>{event.kind}</span>
        {isError && <AlertCircle size={12} className="cm-event-card__error-icon" aria-hidden />}
        <span className="cm-event-card__summary">{summarizeEvent(event)}</span>
        <button
          type="button"
          className="cm-event-card__copy"
          onClick={async (e) => {
            e.stopPropagation();
            await navigator.clipboard.writeText(event.rawJson);
            setCopied(true);
            setTimeout(() => setCopied(false), 1500);
          }}
          aria-label="Copy raw JSON"
        >
          {copied ? <Check size={12} /> : <Copy size={12} />}
        </button>
      </div>
      {expanded && (
        <pre className="cm-event-card__payload">{prettyPrint(event.rawJson)}</pre>
      )}
    </>
  );
});

function renderGap(curr: WireEventDto, prev: WireEventDto) {
  const ms = new Date(curr.occurredAt).getTime() - new Date(prev.occurredAt).getTime();
  if (ms < 1000) return null;
  const sev = timeDeltaSeverity(ms);
  return (
    <div className={`cm-gap${sev === "warn" ? " cm-gap--warn" : sev === "danger" ? " cm-gap--danger" : ""}`}>
      <span className="cm-gap__line" />
      <span>{formatTimeDelta(new Date(curr.occurredAt), new Date(prev.occurredAt))}</span>
      <span className="cm-gap__line" />
    </div>
  );
}

function prettyPrint(raw: string): string {
  try { return JSON.stringify(JSON.parse(raw), null, 2); } catch { return raw; }
}
```

- [ ] **Step 3: Append matching styles to `plugins.css`.**

```css
.cm-event-card {
  display: flex;
  align-items: center;
  gap: 8px;
  padding: 6px 12px 6px 20px;
  font-size: var(--fs-xs);
  color: var(--ds-text-strong);
  border-bottom: 1px solid var(--ds-border-subtle);
  cursor: pointer;
  transition: background var(--ds-dur-fast) var(--ds-ease-out);
}
.cm-event-card:hover { background: var(--ds-surface-muted); }
.cm-event-card--selected { background: var(--ds-surface-muted); }
.cm-event-card--search-hit { box-shadow: inset 2px 0 0 var(--ds-border-accent-soft); }
.cm-event-card--error { color: var(--cm-color-danger-fg); }

.cm-event-card__chevron { background: transparent; border: 0; padding: 0; color: var(--ds-text-faint); cursor: pointer; }
.cm-event-card__time { font-family: ui-monospace, SFMono-Regular, monospace; color: var(--ds-text-faint); font-size: var(--fs-2xs); white-space: nowrap; }
.cm-event-card__error-icon { color: var(--cm-color-danger-fg); }
.cm-event-card__summary { flex: 1; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; color: var(--ds-text-subtle); }
.cm-event-card__copy { background: transparent; border: 0; color: var(--ds-text-faint); padding: 2px; cursor: pointer; }
.cm-event-card__copy:hover { color: var(--ds-text-strong); }
.cm-event-card__payload {
  margin: 0;
  padding: 12px 20px;
  background: var(--ds-surface-muted);
  border-bottom: 1px solid var(--ds-border-subtle);
  font-family: ui-monospace, SFMono-Regular, monospace;
  font-size: var(--fs-2xs);
  color: var(--ds-text-strong);
  white-space: pre-wrap;
  overflow-x: auto;
}
```

- [ ] **Step 4: typecheck + commit.**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/plugins/coding-memory/WireEventCard.tsx desktop-ui/src/styles/plugins.css
git commit -m "feat(plugins): port WireEventCard with --ds-* tokens"
```

### Task D3: Port `buildToolGrouping`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/buildToolGrouping.ts`
- Create: `desktop-ui/src/features/plugins/coding-memory/buildToolGrouping.test.ts`

- [ ] **Step 1: Write a test mirroring the kimi-vis grouping invariants.**

```ts
import { describe, it, expect } from "vitest";
import { buildToolGrouping } from "./buildToolGrouping";
import type { WireEventDto } from "./types";

const ev = (idx: number, kind: string, payload: Record<string, unknown>): WireEventDto => ({
  id: `id-${idx}`,
  source: "kimiCli",
  sessionId: "s",
  kind,
  occurredAt: new Date(1700000000000 + idx * 1000).toISOString(),
  payloadDecoded: payload,
  rawJson: JSON.stringify(payload),
});

describe("buildToolGrouping", () => {
  it("links toolResult back to toolCall id", () => {
    const events = [
      ev(0, "toolCall", { id: "tc-1", function: { name: "Read" } }),
      ev(1, "toolResult", { tool_call_id: "tc-1" }),
    ];
    const meta = buildToolGrouping(events);
    expect(meta.get(0)?.linkedToolName).toBe("Read");
    expect(meta.get(1)?.linkedToolName).toBe("Read");
  });

  it("indents toolCallPart while a parent is active", () => {
    const events = [
      ev(0, "toolCall", { id: "tc-1", function: { name: "Bash" } }),
      ev(1, "toolCallPart", {}),
      ev(2, "toolResult", { tool_call_id: "tc-1" }),
    ];
    const meta = buildToolGrouping(events);
    expect(meta.get(1)?.nestLevel).toBe(1);
    expect(meta.get(0)?.nestLevel).toBe(0);
  });
});
```

- [ ] **Step 2: Implement.**

Translate kimi-vis's `buildToolGrouping` (lines 38–92 of `wire-viewer.tsx`) to use `event.kind` instead of `event.type`, and read ids/names from `event.payloadDecoded`. Use array index as the grouping key (since our DTO doesn't carry `index`).

```ts
export interface EventMeta {
  nestLevel: number;
  linkedToolName?: string;
  linkedToolCallId?: string;
}

export function buildToolGrouping(events: WireEventDto[]): Map<number, EventMeta> {
  const meta = new Map<number, EventMeta>();
  const active: { id: string; name: string }[] = [];
  const names = new Map<string, string>();
  const peek = () => (active.length ? active[active.length - 1] : undefined);

  events.forEach((e, i) => {
    const p = (e.payloadDecoded ?? {}) as any;
    if (e.kind === "toolCall") {
      const id = p.id as string | undefined;
      const name = (p.function?.name ?? p.tool_name) as string | undefined;
      if (id) { active.push({ id, name: name ?? "" }); names.set(id, name ?? ""); }
      meta.set(i, { nestLevel: 0, linkedToolCallId: id, linkedToolName: name });
    } else if (e.kind === "toolCallPart") {
      const top = peek();
      meta.set(i, { nestLevel: top ? 1 : 0, linkedToolCallId: top?.id, linkedToolName: top?.name });
    } else if (e.kind === "toolResult") {
      const tcId = p.tool_call_id as string | undefined;
      const name = tcId ? names.get(tcId) : undefined;
      if (tcId) {
        const idx = active.findIndex((t) => t.id === tcId);
        if (idx !== -1) active.splice(idx, 1);
      }
      meta.set(i, { nestLevel: 0, linkedToolCallId: tcId, linkedToolName: name });
    } else {
      meta.set(i, { nestLevel: 0 });
    }
  });
  return meta;
}
```

- [ ] **Step 3: Run + commit.**

```bash
cd desktop-ui && bun run test -- buildToolGrouping
git commit -am "feat(plugins): tool-call grouping with tests"
```

### Task D4: Port `WireFilters`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/WireFilters.tsx`

- [ ] **Step 1: Read source (`wire-filters.tsx`).**

- [ ] **Step 2: Translate.**

Replace `TYPE_COLORS` map with `eventChipColor(kind)`. Replace `<button className="rounded-full border …">` with `<button className={\`cm-event-chip cm-event-chip--${color}\`} aria-pressed={selected}>`. Drop the `Zap` icon if you prefer (it requires an extra import). The preset button row uses `cm-preset` classes.

Key props (mirror source):

```tsx
interface WireFiltersProps {
  allTypes: string[];
  selectedTypes: Set<string>;
  onToggle: (kind: string) => void;
  total: number;
  filteredCount: number;
  allExpanded: boolean;
  onExpandAll: () => void;
  onCollapseAll: () => void;
  searchQuery: string;
  onSearchChange: (s: string) => void;
  searchMatchCount: number;
  errorCount: number;
  errorsOnly: boolean;
  onToggleErrorsOnly: () => void;
  onNextError: () => void;
  onPrevError: () => void;
  onApplyPreset?: (types: Set<string>, errorsOnly: boolean) => void;
}
```

Add corresponding `.cm-filters`, `.cm-preset`, `.cm-search` classes to `plugins.css` (translate Tailwind padding/border classes per the token map).

- [ ] **Step 3: Build + commit.**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/plugins/coding-memory/WireFilters.tsx desktop-ui/src/styles/plugins.css
git commit -m "feat(plugins): port WireFilters with chip tokens"
```

### Task D5: Port `TurnTree`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/TurnTree.tsx`

- [ ] **Step 1: Read `vis/src/features/wire-viewer/turn-tree.tsx`.**

- [ ] **Step 2: Translate event-shape references and Tailwind → BEM.**

Replace `event.type === "TurnBegin"` with `event.kind === "turnBegin"`, `event.type === "StepBegin"` with `event.kind === "stepBegin"`. Container class: `cm-turn-tree`. Tree nodes: `cm-turn-tree__node`, modifiers `--turn`, `--step`, `--collapsed`.

- [ ] **Step 3: Build + commit.**

```bash
cd desktop-ui && bun run typecheck
git add desktop-ui/src/features/plugins/coding-memory/TurnTree.tsx desktop-ui/src/styles/plugins.css
git commit -m "feat(plugins): port TurnTree navigation panel"
```

### Task D6: Port `ToolCallDetail`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/ToolCallDetail.tsx`

- [ ] **Step 1: Read source (`tool-call-detail.tsx`).**

- [ ] **Step 2: Translate.**

Same pattern. Strip the cross-tab navigation prop (`onNavigateToContext`) — Context Messages tab is out of scope. Class root: `cm-tool-detail`. Render input args + return value as syntax-highlighted JSON via existing `Markdown` (wrap in fenced ```` ```json ```` block).

- [ ] **Step 3: Build + commit.**

```bash
cd desktop-ui && bun run typecheck
git commit -am "feat(plugins): port ToolCallDetail panel"
```

### Task D7: Compose `WireViewer`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/WireViewer.tsx`

- [ ] **Step 1: Read source (`wire-viewer.tsx` lines 94–512).**

- [ ] **Step 2: Translate the orchestrator.**

The big shape is unchanged; key swaps:

- Replace `useEffect` fetch path with:
  ```tsx
  import { fetchSessionWire } from "@/api/endpoints/codingMemory";
  // …
  useEffect(() => {
    let cancelled = false;
    setLoading(true); setError(null);
    fetchSessionWire(sessionId, 1000, 0)
      .then((rows) => { if (!cancelled) setEvents(rows); })
      .catch((err) => { if (!cancelled) setError(String(err.message ?? err)); })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [sessionId, refreshKey]);
  ```
- Drop `agentScope` prop (sub-agents out of scope).
- Drop `viewMode` switcher's `timeline` and `decisions` modes (they require porting `TimelineView` / `DecisionPath`; defer to backlog).
- Drop `IntegrityPanel`, `UsageChart`, `ToolStatsDashboard`, `TurnEfficiency` for v1 (they're nice-to-have; the klynt richness panels in Phase F replace them).
- Use indices (`events.indexOf(e)`) where kimi used `event.index`.
- Loading/empty states: replace Tailwind utility classes with `cm-state cm-state--loading` etc.

- [ ] **Step 3: Verify the page renders end-to-end.**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

- [ ] **Step 4: Commit.**

```bash
git add desktop-ui/src/features/plugins/coding-memory/WireViewer.tsx desktop-ui/src/styles/plugins.css
git commit -m "feat(plugins): compose WireViewer (no kimi-vis Tailwind survives)"
```

### Task D8: Tailwind-leak audit

- [ ] **Step 1: Grep for any surviving Tailwind utility class.**

```bash
grep -rnE '\b(bg-(blue|purple|green|orange|amber|cyan|indigo|teal|pink|gray|red|primary|secondary|muted|popover|destructive)|text-(blue|purple|green|orange|amber|cyan|indigo|teal|pink|gray|red|muted-foreground|foreground|destructive)|border-(border|primary)|flex|gap-|items-|justify-|rounded-|px-|py-|w-|h-)' desktop-ui/src/features/plugins
```

Expected: zero matches. Fix any that surface.

- [ ] **Step 2: Commit any cleanups (or skip if clean).**

```bash
git commit -am "chore(plugins): no-tailwind audit clean"
```

---

## Phase E — Provider Switcher + Session List

### Task E1: `ProviderChips` component

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/ProviderChips.tsx`

- [ ] **Step 1: Implement.**

```tsx
import type { ProviderId } from "./types";

const PROVIDERS: { id: ProviderId | "all"; label: string }[] = [
  { id: "all", label: "All" },
  { id: "claudeCode", label: "Claude Code" },
  { id: "codex", label: "Codex" },
  { id: "kimiCli", label: "Kimi" },
  { id: "openCode", label: "opencode" },
];

interface Props {
  active: ProviderId | "all";
  onChange: (id: ProviderId | "all") => void;
  counts?: Partial<Record<ProviderId | "all", number>>;
}

export function ProviderChips({ active, onChange, counts }: Props) {
  return (
    <div className="cm-provider-chips" role="tablist" aria-label="Coding CLI providers">
      {PROVIDERS.map((p) => (
        <button
          key={p.id}
          type="button"
          role="tab"
          aria-selected={active === p.id}
          className={"cm-provider-chip" + (active === p.id ? " cm-provider-chip--active" : "")}
          onClick={() => onChange(p.id)}
        >
          {p.label}
          {counts?.[p.id] != null && <span className="cm-provider-chip__count">{counts[p.id]}</span>}
        </button>
      ))}
    </div>
  );
}
```

- [ ] **Step 2: Add styles to `plugins.css`.**

```css
.cm-provider-chips { display: flex; gap: 6px; padding: 12px 20px; border-bottom: 1px solid var(--ds-border-subtle); }
.cm-provider-chip {
  display: inline-flex; align-items: center; gap: 6px;
  padding: 4px 10px; font-size: var(--fs-xs);
  border-radius: 999px; border: 1px solid var(--ds-border-subtle);
  background: transparent; color: var(--ds-text-subtle);
  cursor: pointer; transition: all var(--ds-dur-fast) var(--ds-ease-out);
}
.cm-provider-chip:hover { color: var(--ds-text-strong); border-color: var(--ds-border-strong); }
.cm-provider-chip--active { background: var(--ds-surface-muted); color: var(--ds-text-strong); border-color: var(--ds-border-accent-soft); }
.cm-provider-chip__count { font-size: var(--fs-2xs); color: var(--ds-text-faint); }
```

- [ ] **Step 3: Commit.**

```bash
git add desktop-ui/src/features/plugins/coding-memory/ProviderChips.tsx desktop-ui/src/styles/plugins.css
git commit -m "feat(plugins): provider chip switcher"
```

### Task E2: `SessionList`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/SessionList.tsx`

- [ ] **Step 1: Implement.**

```tsx
import { eventChipColor } from "./eventHelpers";
import type { SessionSummaryDto } from "./types";

interface Props {
  sessions: SessionSummaryDto[];
  selectedId: string | null;
  onSelect: (id: string) => void;
  loading?: boolean;
}

export function SessionList({ sessions, selectedId, onSelect, loading }: Props) {
  if (loading) return <div className="cm-state cm-state--loading">Loading sessions…</div>;
  if (sessions.length === 0) return <div className="cm-state cm-state--empty">No sessions in the last 14 days.</div>;
  return (
    <ul className="cm-session-list" role="listbox" aria-label="Coding memory sessions">
      {sessions.map((s) => (
        <li key={s.sessionId}>
          <button
            type="button"
            role="option"
            aria-selected={selectedId === s.sessionId}
            className={"cm-session-list__item" + (selectedId === s.sessionId ? " cm-session-list__item--active" : "")}
            onClick={() => onSelect(s.sessionId)}
          >
            <div className="cm-session-list__top">
              <span className={`cm-event-chip cm-event-chip--${providerColor(s.source)}`}>{s.source}</span>
              <span className="cm-session-list__id">{s.sessionId.slice(0, 8)}…</span>
              <span className="cm-session-list__when">{new Date(s.lastEventAt).toLocaleString()}</span>
            </div>
            <div className="cm-session-list__cwd">{s.cwd ?? "(no cwd)"}</div>
            <div className="cm-session-list__stats">
              {s.eventCount} events · {s.turnCount} turns · {s.toolCallCount} tools
              {s.errorCount > 0 && <span className="cm-session-list__errors"> · {s.errorCount} errors</span>}
            </div>
          </button>
        </li>
      ))}
    </ul>
  );
}

function providerColor(source: string): string {
  switch (source) {
    case "kimiCli": return "purple";
    case "claudeCode": return "blue";
    case "codex": return "green";
    case "openCode": return "cyan";
    default: return "neutral";
  }
}
```

- [ ] **Step 2: Add styles.**

```css
.cm-session-list { list-style: none; margin: 0; padding: 0; overflow-y: auto; }
.cm-session-list__item {
  display: flex; flex-direction: column; gap: 4px;
  width: 100%; padding: 10px 16px;
  background: transparent; border: 0; border-bottom: 1px solid var(--ds-border-subtle);
  text-align: left; cursor: pointer;
  transition: background var(--ds-dur-fast) var(--ds-ease-out);
}
.cm-session-list__item:hover { background: var(--ds-surface-muted); }
.cm-session-list__item--active { background: var(--ds-surface-muted); box-shadow: inset 2px 0 0 var(--ds-border-accent-soft); }
.cm-session-list__top { display: flex; align-items: center; gap: 8px; }
.cm-session-list__id { font-family: ui-monospace, monospace; font-size: var(--fs-2xs); color: var(--ds-text-faint); }
.cm-session-list__when { margin-left: auto; font-size: var(--fs-2xs); color: var(--ds-text-faint); }
.cm-session-list__cwd { font-size: var(--fs-2xs); color: var(--ds-text-subtle); white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
.cm-session-list__stats { font-size: var(--fs-2xs); color: var(--ds-text-faint); }
.cm-session-list__errors { color: var(--cm-color-danger-fg); }
.cm-state { padding: 24px; color: var(--ds-text-faint); font-size: var(--fs-xs); text-align: center; }
```

- [ ] **Step 3: Commit.**

```bash
git commit -am "feat(plugins): session list"
```

### Task E3: Compose `CodingMemoryPlugin`

**Files:**
- Modify: `desktop-ui/src/features/plugins/coding-memory/CodingMemoryPlugin.tsx`

- [ ] **Step 1: Replace the stub with real composition.**

```tsx
import { useEffect, useMemo, useState } from "react";
import { ProviderChips } from "./ProviderChips";
import { SessionList } from "./SessionList";
import { WireViewer } from "./WireViewer";
import { listCodingSessions } from "@/api/endpoints/codingMemory";
import type { ProviderId, SessionSummaryDto } from "./types";

export function CodingMemoryPlugin() {
  const [provider, setProvider] = useState<ProviderId | "all">("all");
  const [sessions, setSessions] = useState<SessionSummaryDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [selectedId, setSelectedId] = useState<string | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    listCodingSessions({
      source: provider === "all" ? undefined : provider,
      sinceDays: 14,
      limit: 100,
      offset: 0,
    })
      .then((rows) => {
        if (cancelled) return;
        setSessions(rows);
        setSelectedId((prev) => prev ?? rows[0]?.sessionId ?? null);
      })
      .finally(() => { if (!cancelled) setLoading(false); });
    return () => { cancelled = true; };
  }, [provider, refreshKey]);

  const counts = useMemo(() => {
    const c: Record<string, number> = { all: sessions.length };
    for (const s of sessions) c[s.source] = (c[s.source] ?? 0) + 1;
    return c;
  }, [sessions]);

  return (
    <div className="cm-plugin">
      <ProviderChips active={provider} onChange={setProvider} counts={counts} />
      <div className="cm-plugin__body">
        <aside className="cm-plugin__sidebar">
          <SessionList sessions={sessions} selectedId={selectedId} onSelect={setSelectedId} loading={loading} />
        </aside>
        <main className="cm-plugin__main">
          {selectedId
            ? <WireViewer sessionId={selectedId} refreshKey={refreshKey} />
            : <div className="cm-state cm-state--empty">Select a session to inspect.</div>}
        </main>
      </div>
    </div>
  );
}
```

- [ ] **Step 2: Layout styles.**

```css
.cm-plugin { display: flex; flex-direction: column; height: 100%; }
.cm-plugin__body { display: flex; flex: 1; overflow: hidden; }
.cm-plugin__sidebar { width: 320px; border-right: 1px solid var(--ds-border-subtle); display: flex; flex-direction: column; }
.cm-plugin__main { flex: 1; overflow: hidden; display: flex; flex-direction: column; }
```

- [ ] **Step 3: Manual end-to-end smoke.**

```bash
cargo tauri dev
```

Click Plugins → switch providers → click a session → verify wire events render, chips are colored, gap rows show timing deltas.

- [ ] **Step 4: Commit.**

```bash
git commit -am "feat(plugins): compose Coding Memory plugin (provider + sessions + viewer)"
```

---

## Phase F — Klynt-Only Richness Panels

These panels have no kimi-vis equivalent — they're why we beat klynt-cli/vis. Each is a separate task that mounts inside `WireViewer` as a collapsible side panel.

### Task F1: `RecallOverlay`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/RecallOverlay.tsx`
- Modify: `desktop-ui/src/features/plugins/coding-memory/WireViewer.tsx`

- [ ] **Step 1: Add endpoint wrapper.**

In `endpoints/codingMemory.ts`:

```ts
export async function fetchRecallOverlay(sessionId: string) {
  const r = await commands.codingMemorySessionReplayRecallOverlay({ sessionId, limit: 200, offset: 0 });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data; // unknown — see types.ts cast
}
```

- [ ] **Step 2: Implement panel.**

```tsx
import { useEffect, useState } from "react";
import { fetchRecallOverlay } from "@/api/endpoints/codingMemory";

interface Props { sessionId: string; }

export function RecallOverlay({ sessionId }: Props) {
  const [rows, setRows] = useState<Array<Record<string, unknown>>>([]);
  useEffect(() => {
    fetchRecallOverlay(sessionId).then((data) => {
      if (Array.isArray(data)) setRows(data as any[]);
    });
  }, [sessionId]);
  if (rows.length === 0) return null;
  return (
    <section className="cm-recall-overlay" aria-label="Recall events">
      <h3 className="cm-recall-overlay__title">Recall ({rows.length})</h3>
      <ul className="cm-recall-overlay__list">
        {rows.map((r, i) => (
          <li key={i} className="cm-recall-overlay__row">
            <span className="cm-event-chip cm-event-chip--indigo">recall</span>
            <span className="cm-recall-overlay__layer">{String(r.layer ?? "?")}</span>
            <span className="cm-recall-overlay__query">{String(r.query ?? "")}</span>
            {r.coverage_score != null && <span className="cm-recall-overlay__score">{Number(r.coverage_score).toFixed(2)}</span>}
          </li>
        ))}
      </ul>
    </section>
  );
}
```

- [ ] **Step 3: Mount in `WireViewer` behind a toggle button (`onToggleRecall`).**

- [ ] **Step 4: Commit.**

```bash
git add desktop-ui/src/features/plugins/coding-memory/RecallOverlay.tsx desktop-ui/src/api/endpoints/codingMemory.ts desktop-ui/src/features/plugins/coding-memory/WireViewer.tsx
git commit -m "feat(plugins): klynt-only RecallOverlay panel"
```

### Task F2: `MirrorAlertSidecar`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/MirrorAlertSidecar.tsx`

- [ ] **Step 1: Endpoint wrapper.**

```ts
export async function fetchMirrorAlerts(args: { repo?: string; severity?: string }) {
  const r = await commands.codingMemoryMirrorAlertsFeed({ kind: null, severity: args.severity ?? null, repo: args.repo ?? null, limit: 50 });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}
export async function actMirrorAlert(id: string, action: "approve" | "reject" | "snooze") {
  const r = await commands.codingMemoryMirrorAlertAction({ id, action });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}
```

- [ ] **Step 2: Component renders alerts in a side rail; each row has approve/reject/snooze buttons that call `actMirrorAlert(id, action)` then reload.**

Container `cm-mirror`. Severity → chip color: `error` → red (use `--cm-color-danger-*`), `warn` → amber, `info` → neutral.

- [ ] **Step 3: Mount via toggle in `WireViewer`.**

- [ ] **Step 4: Commit.**

```bash
git commit -am "feat(plugins): klynt-only MirrorAlertSidecar"
```

### Task F3: `CausalGraphInspector`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/CausalGraphInspector.tsx`

- [ ] **Step 1: Endpoint wrapper.**

```ts
export async function fetchRecallFacts(ids: string[]) {
  const r = await commands.codingMemoryRecallFetch({ ids, includeProvenance: true, includeCausalGraph: true });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}
```

- [ ] **Step 2: Component takes `factIds: string[]` (extracted from a selected `ToolResult`'s payload's `recall_ids` field, when present), fetches facts + edges, renders a flat node list with edge labels.**

Defer real graph layout (cytoscape, etc.) to a follow-up — for v1 a flat list with arrows is enough.

- [ ] **Step 3: Mount inside `ToolCallDetail` when the selected event references recalled facts.**

- [ ] **Step 4: Commit.**

```bash
git commit -am "feat(plugins): klynt-only CausalGraphInspector"
```

### Task F4: `ReforgeCycleDiff`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/ReforgeCycleDiff.tsx`

- [ ] **Step 1: Endpoint wrappers.**

```ts
export async function listReforgeCycles() {
  const r = await commands.codingMemoryReforgeCycleList();
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}
export async function fetchReforgeCycleDiff(args: { repoId: string; artifact: string; beforeCycleId?: string; afterCycleId?: string }) {
  const r = await commands.codingMemoryReforgeCycleDiff({
    repoId: args.repoId,
    artifact: args.artifact,
    beforeCycleId: args.beforeCycleId ?? null,
    afterCycleId: args.afterCycleId ?? null,
  });
  if (r.status !== "ok") throw new Error(String(r.error));
  return r.data;
}
```

- [ ] **Step 2: Render two-column diff using existing `ds-diff.css` primitives.**

Mount as a tab inside `CodingMemoryPlugin` (add a secondary tab bar above WireViewer with "Sessions" / "Reforge").

- [ ] **Step 3: Commit.**

```bash
git commit -am "feat(plugins): klynt-only Reforge cycle diff viewer"
```

### Task F5: `EffectivenessTrendsChart`

**Files:**
- Create: `desktop-ui/src/features/plugins/coding-memory/EffectivenessTrendsChart.tsx`

- [ ] **Step 1: Endpoint wrapper for `coding_memory_effectiveness_trends`.**

- [ ] **Step 2: Render a sparkline using inline SVG (no chart library).** 60×140 viewBox, polyline of `buckets[].score` mapped to y-axis 0..1, area fill via `<path>`.

- [ ] **Step 3: Mount inside `MirrorAlertSidecar` row when `kind === "patternEffectivenessDrop"`.**

- [ ] **Step 4: Commit.**

```bash
git commit -am "feat(plugins): klynt-only effectiveness sparkline"
```

---

## Phase G — Verification & Polish

### Task G1: Final lint + typecheck + Rust check

- [ ] **Step 1.**

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test
cd .. && cargo clippy --workspace --all-targets --all-features
cargo nextest run -p app-core -p desktop-shared -p desktop -E 'test(coding_memory) + test(plugins) + test(registration_drift) + test(bindings_are_current)'
cargo fmt --all --check
```

Expected: 0 errors, 0 warnings, all tests pass.

### Task G2: Manual end-to-end checklist

- [ ] Sidebar Plugins click → page renders with 4 tabs (1 active, 3 "Soon")
- [ ] Provider switcher updates session list
- [ ] Selecting a Kimi session shows rich event chips (skillActivated, recallInjected, providerCall, approvalDecision)
- [ ] Selecting a Codex session shows fewer event variants (correctly thinner)
- [ ] Gap rows render with warning color when delta >10s, danger when >60s
- [ ] Theme switch (dark/light/dim) recolors event chips correctly
- [ ] No Tailwind utility class survives in `desktop-ui/src/features/plugins/`
- [ ] RecallOverlay shows recalled facts inline with the wire stream
- [ ] MirrorAlertSidecar approve button removes the alert after action
- [ ] CausalGraphInspector pops up on a tool result with recall ids
- [ ] Reforge cycle list loads; selecting two cycles shows the artifact diff
- [ ] Sidebar `chat` and `home` views still work after navigation

### Task G3: Final commit + plan close-out

- [ ] **Step 1.**

```bash
git log --oneline | head -30
```

Verify a clean linear history of feat commits, one per task.

- [ ] **Step 2: Update backlog.**

In `docs/superpowers/backlog.md` item #8, add a final line:

```
**Status (YYYY-MM-DD):** Implemented per `docs/superpowers/plans/2026-04-28-plugins-coding-memory-viewer.md`. Out-of-scope tabs (Context, State, Dual, Agents) and `TimelineView`/`DecisionPath` modes remain deferred.
```

- [ ] **Step 3: Commit.**

```bash
git commit -am "docs(backlog): close item 8 — Plugins coding memory viewer shipped"
```

---

## Self-Review

**Spec coverage:**
- Constraint 1 (`--ds-*` tokens, no Tailwind): covered by Phase C tokens, the translation table, and Task D8 audit.
- Constraint 2 (richer than klynt-cli/vis, future klynt-cli compatibility): covered by `EventKind` mapping in `eventHelpers.ts` (already includes klynt-internal kinds: `skillActivated`, `recallInjected`, `providerCall`, `approvalDecision`, `mirrorAlert`, `gitCommit`) + Phase F richness panels.
- Constraint 3 (internal-plugins host): covered by `PluginsView` tab shell with 3 disabled "Soon" tabs (Skills, MCP, Klynt CLI).
- Constraint 4 (richer than vis): Phase F panels (RecallOverlay, MirrorAlertSidecar, CausalGraphInspector, ReforgeCycleDiff, EffectivenessTrendsChart) — none exist in kimi-vis.
- Cuts (Context/State/Dual/Agents tabs, TimelineView, DecisionPath, IntegrityPanel, UsageChart): explicitly listed in "Out-of-scope" + Task D7.

**Placeholder scan:** none.

**Type consistency:**
- `WireEventDto` shape (id, source, sessionId, kind, occurredAt, payloadDecoded, rawJson) is used identically in B1, D2, D3, D7.
- `SessionSummaryDto` fields (sessionId, source, cwd, lastEventAt, eventCount, turnCount, toolCallCount, errorCount) match between B2 (Rust) and E2 (FE consumer) — note camelCase via serde rename.
- `eventChipColor()` returns `ChipColor` ("blue" | … | "neutral"); used in C3 / D2 / D4 / D5 / E2.
- `buildToolGrouping()` returns `Map<number, EventMeta>` keyed by array index — consistent in D3 / D7.

`★ Insight ─────────────────────────────────────`
- The plan deliberately does NOT inline 500-line component bodies for D2/D4/D5/D6 — instead it gives the source path, the precise transformation rules (kind/payload renames + token translation table), and the skeleton + critical translated portions. This trades plan brevity for executor accuracy: the executor reads the source then applies a small, well-defined set of changes, instead of getting a paraphrase that drifts from kimi-vis logic.
- The token map at the top is the central reference for all of Phase D — keeping it in one place (instead of duplicated per task) is intentional. If the design evolves (e.g. new chip color), only one row updates.
- TDD intensity is calibrated by risk: pure logic (`buildToolGrouping`, `eventHelpers`, Rust DTOs) gets full red→green; pure layout components (chips, panels) skip unit tests in favor of the typecheck + manual verify in G2 — pixel-snapshotting these would be high-cost, low-signal.
`─────────────────────────────────────────────────`
