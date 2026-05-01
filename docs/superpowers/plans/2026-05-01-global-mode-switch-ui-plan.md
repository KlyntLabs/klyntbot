# Global Assistant ⇄ Code mode switch — UI implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add an app-wide Assistant ⇄ Code mode toggle to the desktop UI. The toggle sits in the very top row of the left panel, persists across launches via localStorage, and swaps the main content area between today's Assistant view (unchanged) and a new Codex-style Code landing.

**Architecture:** A singleton store backed by `useSyncExternalStore` exposes `{ mode, setMode }` via a `useAppMode()` hook. A pure `<AppModeSwitch />` segmented pill is mounted inside `SidebarChatLayout.tsx`'s top row. A pure `<CodeLanding />` component (hero input + project grid + empty state) is wired into the existing `LayoutPrimarySurface` → `buildPrimaryNodes` → `DesktopLayout` pipeline as a new `codeLandingNode`. `DesktopLayout` reads the mode and conditionally renders `codeLandingNode` in place of `homeNode`. No backend, no new IPC, no skill or prompt wiring.

**Tech Stack:** React 18 + TypeScript, Vite, Vitest + `@testing-library/react`, plain CSS with the existing klyntbot design tokens (no Tailwind). Path aliases: `@app/* → src/features/app/*`, `@/*  → src/*`. Working directory for all commands: `desktop-ui/`.

**Spec:** [`docs/superpowers/specs/2026-05-01-global-mode-switch-ui-design.md`](../specs/2026-05-01-global-mode-switch-ui-design.md)

## File map

**New files**

| Path (relative to `desktop-ui/`) | Responsibility |
|---|---|
| `src/features/app/hooks/useAppMode.ts` | Singleton store + hook + localStorage persistence + test helpers. |
| `src/features/app/hooks/useAppMode.test.ts` | Hydration, persistence, multi-subscriber tests. |
| `src/features/app/components/AppModeSwitch.tsx` | Pure segmented-pill component. Receives `mode` + `onChange`. |
| `src/features/app/components/AppModeSwitch.test.tsx` | Render, click, keyboard, drag-region tests. |
| `src/features/coding/components/CodeLanding.tsx` | Pure landing-page component: hero + grid + empty state. |
| `src/features/coding/components/CodeLanding.test.tsx` | Populated render, empty state, click-handler wiring. |
| `src/styles/app-mode-switch.css` | Switch styles. BEM, design-token-driven. |
| `src/styles/code-landing.css` | Landing-page styles. BEM, design-token-driven. |

**Changed files**

| Path (relative to `desktop-ui/`) | Change |
|---|---|
| `src/features/app/components/SidebarChatLayout.tsx` | Mount `<AppModeSwitch />` inside `.sidebar-chat__topbar`, right-aligned. Reads `useAppMode()` internally. |
| `src/features/app/components/SidebarChatLayout.test.tsx` | Add a test asserting the top row contains the `App mode` tablist with both segments. |
| `src/styles/sidebar-chat.css` | Convert `.sidebar-chat__topbar` from a 32px aria-hidden spacer to a 32px flex row with reserved traffic-light area on the left and the switch on the right. |
| `src/features/layout/hooks/layoutNodes/types.ts` | Add `codeLandingProps` to `LayoutPrimarySurface`. Add `codeLandingNode` to `LayoutNodesResult`. |
| `src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx` | Build `<CodeLanding {...options.codeLandingProps} />` and return it as `codeLandingNode`. |
| `src/features/layout/components/DesktopLayout.tsx` | Accept `codeLandingNode` prop. Read `useAppMode()`. When `showHome && mode === "code"`, render `codeLandingNode` instead of `homeNode`. |
| `src/features/app/components/MainApp.tsx` | Assemble `codeLandingProps` from existing workspace state + handlers. Pass `codeLandingNode` from `displayNodes` (or layout surfaces) into the `appLayout` block. |
| `src/styles/index.css` | Add `@import "./app-mode-switch.css";` and `@import "./code-landing.css";`. |

---

## Conventions

- All commands run from `desktop-ui/` unless noted.
- Test command for a single file: `bun run test -- <path>` (Vitest accepts a path filter).
- Lint: `bun run lint`. Typecheck: `bun run typecheck`. Full test pass: `bun run test`.
- Commit format: `feat(coding-mode): …`, `test(coding-mode): …`, `style(coding-mode): …` per the project's Conventional Commits rule.
- After **every** task, run `bun run typecheck` and `bun run lint`. Both must pass with zero warnings.

---

### Task 1: `useAppMode` hook with persistence

**Files:**
- Create: `desktop-ui/src/features/app/hooks/useAppMode.ts`
- Create: `desktop-ui/src/features/app/hooks/useAppMode.test.ts`

- [ ] **Step 1: Write the failing test file**

Create `desktop-ui/src/features/app/hooks/useAppMode.test.ts`:

```ts
// @vitest-environment jsdom
import { act, renderHook } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import {
  __testing,
  APP_MODE_STORAGE_KEY,
  setAppMode,
  useAppMode,
} from "./useAppMode";

describe("useAppMode", () => {
  beforeEach(() => {
    window.localStorage.clear();
    __testing.reset("assistant");
  });

  afterEach(() => {
    window.localStorage.clear();
    __testing.reset("assistant");
  });

  it("defaults to assistant when no value is stored", () => {
    const { result } = renderHook(() => useAppMode());
    expect(result.current.mode).toBe("assistant");
  });

  it("hydrates from localStorage when rehydrate is called", () => {
    window.localStorage.setItem(APP_MODE_STORAGE_KEY, "code");
    __testing.rehydrateFromStorage();
    const { result } = renderHook(() => useAppMode());
    expect(result.current.mode).toBe("code");
  });

  it("ignores invalid stored values and falls back to default", () => {
    window.localStorage.setItem(APP_MODE_STORAGE_KEY, "garbage");
    __testing.rehydrateFromStorage();
    const { result } = renderHook(() => useAppMode());
    expect(result.current.mode).toBe("assistant");
  });

  it("setMode updates state and persists to localStorage", () => {
    const { result } = renderHook(() => useAppMode());
    act(() => result.current.setMode("code"));
    expect(result.current.mode).toBe("code");
    expect(window.localStorage.getItem(APP_MODE_STORAGE_KEY)).toBe("code");
  });

  it("setMode is a no-op when called with the current mode", () => {
    const { result } = renderHook(() => useAppMode());
    act(() => result.current.setMode("assistant"));
    expect(result.current.mode).toBe("assistant");
    expect(window.localStorage.getItem(APP_MODE_STORAGE_KEY)).toBeNull();
  });

  it("notifies multiple subscribers on change", () => {
    const a = renderHook(() => useAppMode());
    const b = renderHook(() => useAppMode());
    act(() => setAppMode("code"));
    expect(a.result.current.mode).toBe("code");
    expect(b.result.current.mode).toBe("code");
  });

  it("ignores invalid values passed to setAppMode", () => {
    const { result } = renderHook(() => useAppMode());
    // @ts-expect-error — testing runtime guard
    act(() => setAppMode("nope"));
    expect(result.current.mode).toBe("assistant");
  });
});
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd desktop-ui && bun run test -- src/features/app/hooks/useAppMode.test.ts
```

Expected: tests fail with "Cannot find module './useAppMode'" (or similar import error).

- [ ] **Step 3: Implement the hook and store**

Create `desktop-ui/src/features/app/hooks/useAppMode.ts`:

```ts
import { useSyncExternalStore } from "react";

export type AppMode = "assistant" | "code";

export const APP_MODE_STORAGE_KEY = "klynt.appMode";
const DEFAULT_MODE: AppMode = "assistant";

function isAppMode(value: unknown): value is AppMode {
  return value === "assistant" || value === "code";
}

function readStoredMode(): AppMode {
  if (typeof window === "undefined") {
    return DEFAULT_MODE;
  }
  try {
    const raw = window.localStorage.getItem(APP_MODE_STORAGE_KEY);
    return isAppMode(raw) ? raw : DEFAULT_MODE;
  } catch {
    return DEFAULT_MODE;
  }
}

let currentMode: AppMode = readStoredMode();
const listeners = new Set<() => void>();

function emit(): void {
  listeners.forEach((listener) => listener());
}

function subscribe(listener: () => void): () => void {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}

function getSnapshot(): AppMode {
  return currentMode;
}

function getServerSnapshot(): AppMode {
  return DEFAULT_MODE;
}

export function setAppMode(next: AppMode): void {
  if (!isAppMode(next) || next === currentMode) {
    return;
  }
  currentMode = next;
  try {
    if (typeof window !== "undefined") {
      window.localStorage.setItem(APP_MODE_STORAGE_KEY, next);
    }
  } catch {
    // localStorage may be unavailable (private mode, etc.) — non-fatal.
  }
  emit();
}

export function useAppMode(): { mode: AppMode; setMode: (mode: AppMode) => void } {
  const mode = useSyncExternalStore(subscribe, getSnapshot, getServerSnapshot);
  return { mode, setMode: setAppMode };
}

// Test-only helpers. Do not use in production code.
export const __testing = {
  reset(initial: AppMode = DEFAULT_MODE): void {
    currentMode = initial;
    listeners.clear();
  },
  rehydrateFromStorage(): void {
    currentMode = readStoredMode();
  },
};
```

- [ ] **Step 4: Run the test to confirm it passes**

```bash
cd desktop-ui && bun run test -- src/features/app/hooks/useAppMode.test.ts
```

Expected: all 7 tests pass.

- [ ] **Step 5: Run typecheck and lint**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Expected: zero errors, zero warnings.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/app/hooks/useAppMode.ts desktop-ui/src/features/app/hooks/useAppMode.test.ts
git commit -m "$(cat <<'EOF'
feat(coding-mode): add useAppMode hook with localStorage persistence

Singleton store via useSyncExternalStore. Persists "assistant" | "code"
to localStorage under klynt.appMode. Includes rehydrate + reset test
helpers for deterministic unit tests.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 2: `AppModeSwitch` component

**Files:**
- Create: `desktop-ui/src/features/app/components/AppModeSwitch.tsx`
- Create: `desktop-ui/src/features/app/components/AppModeSwitch.test.tsx`

- [ ] **Step 1: Write the failing test**

Create `desktop-ui/src/features/app/components/AppModeSwitch.test.tsx`:

```tsx
// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { AppModeSwitch } from "./AppModeSwitch";

afterEach(() => cleanup());

describe("AppModeSwitch", () => {
  it("renders both modes with assistant active", () => {
    render(<AppModeSwitch mode="assistant" onChange={() => {}} />);
    const a = screen.getByRole("tab", { name: "Assistant" });
    const c = screen.getByRole("tab", { name: "Code" });
    expect(a.getAttribute("aria-selected")).toBe("true");
    expect(c.getAttribute("aria-selected")).toBe("false");
    expect(a.className).toContain("app-mode-switch__btn--active");
    expect(c.className).not.toContain("app-mode-switch__btn--active");
  });

  it("renders code as active when mode='code'", () => {
    render(<AppModeSwitch mode="code" onChange={() => {}} />);
    expect(screen.getByRole("tab", { name: "Code" }).getAttribute("aria-selected")).toBe(
      "true",
    );
    expect(
      screen.getByRole("tab", { name: "Assistant" }).getAttribute("aria-selected"),
    ).toBe("false");
  });

  it("calls onChange('code') when clicking the Code segment", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="assistant" onChange={onChange} />);
    fireEvent.click(screen.getByRole("tab", { name: "Code" }));
    expect(onChange).toHaveBeenCalledWith("code");
  });

  it("does not call onChange when clicking the active segment", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="assistant" onChange={onChange} />);
    fireEvent.click(screen.getByRole("tab", { name: "Assistant" }));
    expect(onChange).not.toHaveBeenCalled();
  });

  it("ArrowRight calls onChange with next mode", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="assistant" onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith("code");
  });

  it("ArrowLeft wraps from assistant to code", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="assistant" onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowLeft" });
    expect(onChange).toHaveBeenCalledWith("code");
  });

  it("ArrowRight wraps from code to assistant", () => {
    const onChange = vi.fn();
    render(<AppModeSwitch mode="code" onChange={onChange} />);
    fireEvent.keyDown(screen.getByRole("tablist"), { key: "ArrowRight" });
    expect(onChange).toHaveBeenCalledWith("assistant");
  });

  it("sets data-tauri-drag-region=false on the wrapper and buttons", () => {
    render(<AppModeSwitch mode="assistant" onChange={() => {}} />);
    const list = screen.getByRole("tablist");
    expect(list.getAttribute("data-tauri-drag-region")).toBe("false");
    expect(
      screen.getByRole("tab", { name: "Code" }).getAttribute("data-tauri-drag-region"),
    ).toBe("false");
  });

  it("labels the tablist 'App mode'", () => {
    render(<AppModeSwitch mode="assistant" onChange={() => {}} />);
    expect(screen.getByRole("tablist").getAttribute("aria-label")).toBe("App mode");
  });
});
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd desktop-ui && bun run test -- src/features/app/components/AppModeSwitch.test.tsx
```

Expected: tests fail with "Cannot find module './AppModeSwitch'".

- [ ] **Step 3: Implement the component**

Create `desktop-ui/src/features/app/components/AppModeSwitch.tsx`:

```tsx
import { type KeyboardEvent, type MouseEvent, useCallback, useRef } from "react";
import type { AppMode } from "../hooks/useAppMode";

const MODES: { id: AppMode; label: string }[] = [
  { id: "assistant", label: "Assistant" },
  { id: "code", label: "Code" },
];

export type AppModeSwitchProps = {
  mode: AppMode;
  onChange: (mode: AppMode) => void;
};

export function AppModeSwitch({ mode, onChange }: AppModeSwitchProps) {
  const buttonRefs = useRef<Record<AppMode, HTMLButtonElement | null>>({
    assistant: null,
    code: null,
  });

  const handleClick = useCallback(
    (next: AppMode) => (event: MouseEvent<HTMLButtonElement>) => {
      event.stopPropagation();
      if (next !== mode) {
        onChange(next);
      }
    },
    [mode, onChange],
  );

  const handleKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      if (event.key !== "ArrowLeft" && event.key !== "ArrowRight") {
        return;
      }
      event.preventDefault();
      const idx = MODES.findIndex((m) => m.id === mode);
      const delta = event.key === "ArrowRight" ? 1 : -1;
      const nextIdx = (idx + delta + MODES.length) % MODES.length;
      const next = MODES[nextIdx].id;
      onChange(next);
      buttonRefs.current[next]?.focus();
    },
    [mode, onChange],
  );

  return (
    <div
      className="app-mode-switch"
      role="tablist"
      aria-label="App mode"
      data-tauri-drag-region="false"
      onKeyDown={handleKeyDown}
    >
      {MODES.map(({ id, label }) => {
        const isActive = id === mode;
        return (
          <button
            key={id}
            ref={(el) => {
              buttonRefs.current[id] = el;
            }}
            type="button"
            role="tab"
            aria-selected={isActive}
            tabIndex={isActive ? 0 : -1}
            data-tauri-drag-region="false"
            className={`app-mode-switch__btn${
              isActive ? " app-mode-switch__btn--active" : ""
            }`}
            onClick={handleClick(id)}
          >
            {label}
          </button>
        );
      })}
    </div>
  );
}
```

- [ ] **Step 4: Run the test to confirm it passes**

```bash
cd desktop-ui && bun run test -- src/features/app/components/AppModeSwitch.test.tsx
```

Expected: all 9 tests pass.

- [ ] **Step 5: Run typecheck and lint**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Expected: zero errors, zero warnings.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/app/components/AppModeSwitch.tsx desktop-ui/src/features/app/components/AppModeSwitch.test.tsx
git commit -m "$(cat <<'EOF'
feat(coding-mode): add AppModeSwitch segmented-pill component

Pure component with mode + onChange props. Renders a tablist with
Assistant and Code segments. Keyboard navigation via ←/→ arrows
(wraps). Tagged data-tauri-drag-region=false so clicks land inside
the macOS drag-strip region.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 3: `app-mode-switch.css` styles

**Files:**
- Create: `desktop-ui/src/styles/app-mode-switch.css`

- [ ] **Step 1: Write the stylesheet**

Create `desktop-ui/src/styles/app-mode-switch.css`:

```css
.app-mode-switch {
  display: inline-flex;
  gap: 2px;
  padding: 2px;
  background: var(--surface-card-muted, rgba(255, 255, 255, 0.06));
  border-radius: 7px;
  position: relative;
  z-index: 2;
  -webkit-app-region: no-drag;
}

.app-mode-switch__btn {
  height: 22px;
  padding: 3px 10px;
  background: transparent;
  border: 0;
  border-radius: 5px;
  font-size: var(--fs-xs);
  font-weight: 500;
  line-height: 1;
  color: var(--text-muted);
  cursor: pointer;
  transition:
    background-color var(--ds-dur-fast, 120ms) ease,
    color var(--ds-dur-fast, 120ms) ease;
}

.app-mode-switch__btn:hover {
  color: var(--text-strong);
}

.app-mode-switch__btn--active,
.app-mode-switch__btn--active:hover {
  background: rgba(255, 255, 255, 0.14);
  color: var(--text-strong);
}

.app-mode-switch__btn:focus-visible {
  outline: 1px solid var(--border-accent, rgba(120, 200, 255, 0.85));
  outline-offset: 1px;
}
```

- [ ] **Step 2: Confirm the file exists**

```bash
test -f desktop-ui/src/styles/app-mode-switch.css && echo OK
```

Expected: prints `OK`.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/styles/app-mode-switch.css
git commit -m "$(cat <<'EOF'
style(coding-mode): add app-mode-switch.css

Design-token-driven; 7px outer / 5px inner radii, 22px button height,
uses --fs-xs and existing surface tokens. Active segment fills with
rgba white at 14% to match the existing search-toggle.is-active feel.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 4: Wire `AppModeSwitch` into `SidebarChatLayout`

**Files:**
- Modify: `desktop-ui/src/features/app/components/SidebarChatLayout.tsx`
- Modify: `desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx`
- Modify: `desktop-ui/src/styles/sidebar-chat.css` (lines 25–29: `.sidebar-chat__topbar`)

- [ ] **Step 1: Write the failing test (extend existing test file)**

Replace the contents of `desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx` with:

```tsx
// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { __testing } from "../hooks/useAppMode";
import { SidebarChatLayout } from "./SidebarChatLayout";

afterEach(() => cleanup());

const baseProps = {
  onOpenSettings: vi.fn(),
  onNewChat: vi.fn(),
  onSelectPlugins: vi.fn(),
  onSelectCalendar: vi.fn(),
  threads: [],
  selectedSessionKey: null,
  onSelectThread: vi.fn(),
  activeNavId: null,
};

describe("SidebarChatLayout", () => {
  beforeEach(() => {
    window.localStorage.clear();
    __testing.reset("assistant");
  });

  it("renders the Calendar nav item", () => {
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.getByText("Calendar")).toBeTruthy();
  });

  it("applies the --active modifier class to the matching nav item", () => {
    render(<SidebarChatLayout {...baseProps} activeNavId="calendar" />);
    const calendarBtn = screen.getByText("Calendar").closest("button");
    expect(calendarBtn?.className).toContain("sidebar-chat__nav-item--active");
  });

  it("does not apply --active when activeNavId is null", () => {
    render(<SidebarChatLayout {...baseProps} />);
    const buttons = document.querySelectorAll(".sidebar-chat__nav-item");
    buttons.forEach((b) => {
      expect(b.className).not.toContain("sidebar-chat__nav-item--active");
    });
  });

  it("calls onSelectCalendar when Calendar nav item is clicked", () => {
    const onSelectCalendar = vi.fn();
    render(<SidebarChatLayout {...baseProps} onSelectCalendar={onSelectCalendar} />);
    (screen.getByText("Calendar").closest("button") as HTMLButtonElement).click();
    expect(onSelectCalendar).toHaveBeenCalled();
  });

  it("renders the App mode tablist in the topbar with both segments", () => {
    render(<SidebarChatLayout {...baseProps} />);
    const list = screen.getByRole("tablist", { name: "App mode" });
    expect(list).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Assistant" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Code" })).toBeTruthy();
  });

  it("clicking the Code tab flips the global mode to code", () => {
    render(<SidebarChatLayout {...baseProps} />);
    const code = screen.getByRole("tab", { name: "Code" });
    expect(code.getAttribute("aria-selected")).toBe("false");
    fireEvent.click(code);
    expect(code.getAttribute("aria-selected")).toBe("true");
    expect(window.localStorage.getItem("klynt.appMode")).toBe("code");
  });

  it("renders the same nav items in code mode", () => {
    __testing.reset("code");
    render(<SidebarChatLayout {...baseProps} />);
    expect(screen.getByText("New chat")).toBeTruthy();
    expect(screen.getByText("Search")).toBeTruthy();
    expect(screen.getByText("Calendar")).toBeTruthy();
    expect(screen.getByText("Plugins")).toBeTruthy();
    expect(screen.getByText("Automations")).toBeTruthy();
    expect(screen.getByText("Project")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run the test to confirm new tests fail**

```bash
cd desktop-ui && bun run test -- src/features/app/components/SidebarChatLayout.test.tsx
```

Expected: the four new tests fail (no tablist found, no aria-selected toggle, etc.). The original four pass.

- [ ] **Step 3: Update `SidebarChatLayout.tsx` to mount the switch**

Replace the contents of `desktop-ui/src/features/app/components/SidebarChatLayout.tsx` with:

```tsx
import { Calendar, Clock, FolderPlus, LayoutGrid, Search, Settings, SquarePen } from "lucide-react";
import { memo } from "react";
import type { ChatThread } from "@/features/chat/types";
import { useAppMode } from "../hooks/useAppMode";
import { AppModeSwitch } from "./AppModeSwitch";

type SidebarChatLayoutProps = {
  onOpenSettings: () => void;
  onNewChat: () => void;
  onSelectPlugins: () => void;
  onSelectCalendar?: () => void;
  threads: ChatThread[];
  selectedSessionKey: string | null;
  onSelectThread: (sessionKey: string) => void;
  activeNavId?: string | null;
};

type NavItem = {
  id: string;
  label: string;
  icon: React.ReactNode;
  onClick?: () => void;
};

export const SidebarChatLayout = memo(function SidebarChatLayout({
  onOpenSettings,
  onNewChat,
  onSelectPlugins,
  onSelectCalendar,
  threads,
  selectedSessionKey,
  onSelectThread,
  activeNavId,
}: SidebarChatLayoutProps) {
  const { mode, setMode } = useAppMode();
  const handleSelectCalendar = onSelectCalendar ?? (() => {});
  const navItems: NavItem[] = [
    { id: "new-chat", label: "New chat", icon: <SquarePen aria-hidden />, onClick: onNewChat },
    { id: "search", label: "Search", icon: <Search aria-hidden /> },
    {
      id: "calendar",
      label: "Calendar",
      icon: <Calendar aria-hidden />,
      onClick: handleSelectCalendar,
    },
    { id: "plugins", label: "Plugins", icon: <LayoutGrid aria-hidden />, onClick: onSelectPlugins },
    { id: "automations", label: "Automations", icon: <Clock aria-hidden /> },
    { id: "project", label: "Project", icon: <FolderPlus aria-hidden /> },
  ];

  return (
    <aside className="sidebar-chat">
      <div className="sidebar-chat__drag-strip" />
      <div className="sidebar-chat__topbar">
        <div className="sidebar-chat__topbar-traffic-reserve" aria-hidden />
        <div className="sidebar-chat__topbar-spacer" />
        <AppModeSwitch mode={mode} onChange={setMode} />
      </div>

      <nav className="sidebar-chat__nav" aria-label="Primary">
        {navItems.map((item) => {
          const isActive = activeNavId === item.id;
          const cls = `sidebar-chat__nav-item${isActive ? " sidebar-chat__nav-item--active" : ""}`;
          return (
            <button key={item.id} type="button" className={cls} onClick={item.onClick}>
              <span className="sidebar-chat__nav-icon">{item.icon}</span>
              <span className="sidebar-chat__nav-label">{item.label}</span>
            </button>
          );
        })}
      </nav>

      <div className="sidebar-chat__chats">
        <div className="sidebar-chat__section-title">Chats</div>
        {threads.length === 0 ? (
          <div className="sidebar-chat__chats-empty">No chats</div>
        ) : (
          <ul className="sidebar-chat__thread-list">
            {threads.map((t) => (
              <li key={t.sessionKey}>
                <button
                  type="button"
                  className={
                    "sidebar-chat__thread-item" +
                    (t.sessionKey === selectedSessionKey
                      ? " sidebar-chat__thread-item--active"
                      : "")
                  }
                  onClick={() => onSelectThread(t.sessionKey)}
                  title={t.title}
                >
                  {t.title || "Untitled"}
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="sidebar-chat__spacer" />

      <div className="sidebar-chat__footer">
        <button type="button" className="sidebar-chat__settings" onClick={onOpenSettings}>
          <Settings aria-hidden />
          <span>Settings</span>
        </button>
        <button type="button" className="sidebar-chat__upgrade">
          Upgrade
        </button>
      </div>
    </aside>
  );
});

SidebarChatLayout.displayName = "SidebarChatLayout";
```

- [ ] **Step 4: Update the topbar CSS to host the switch**

In `desktop-ui/src/styles/sidebar-chat.css`, replace the existing `.sidebar-chat__topbar` block (lines 25–29 in the current file) with:

```css
/* Top row: reserves room for macOS traffic lights and hosts the App mode switch. */
.sidebar-chat__topbar {
  height: 32px;
  flex: 0 0 auto;
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 8px;
  padding: 0 4px 0 0;
  position: relative;
  z-index: 2;
}

.sidebar-chat__topbar-traffic-reserve {
  width: 72px;
  height: 22px;
  flex: 0 0 72px;
}

.sidebar-chat__topbar-spacer {
  flex: 1 1 auto;
}
```

- [ ] **Step 5: Run the test to confirm it passes**

```bash
cd desktop-ui && bun run test -- src/features/app/components/SidebarChatLayout.test.tsx
```

Expected: all 7 tests pass.

- [ ] **Step 6: Run typecheck and lint**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Expected: zero errors, zero warnings.

- [ ] **Step 7: Commit**

```bash
git add desktop-ui/src/features/app/components/SidebarChatLayout.tsx desktop-ui/src/features/app/components/SidebarChatLayout.test.tsx desktop-ui/src/styles/sidebar-chat.css
git commit -m "$(cat <<'EOF'
feat(coding-mode): mount AppModeSwitch in SidebarChatLayout topbar

The 32px topbar was a bare aria-hidden spacer. Convert it into a
flex row that reserves 72px on the left for the macOS traffic-light
cluster and right-aligns the new Assistant/Code switch. Mode is
read via useAppMode() — no new props on SidebarChatLayout.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 5: `code-landing.css` styles

**Files:**
- Create: `desktop-ui/src/styles/code-landing.css`

- [ ] **Step 1: Write the stylesheet**

Create `desktop-ui/src/styles/code-landing.css`:

```css
.code-landing {
  width: 100%;
  height: 100%;
  overflow-y: auto;
  display: flex;
  justify-content: center;
  padding: 40px 32px 48px;
  box-sizing: border-box;
}

.code-landing__inner {
  width: 100%;
  max-width: 720px;
  display: flex;
  flex-direction: column;
  gap: 28px;
}

.code-landing__hero {
  text-align: center;
  margin: 16px auto 0;
  max-width: 560px;
  width: 100%;
}

.code-landing__hero h1 {
  font-size: var(--fs-display-sm);
  font-weight: 600;
  letter-spacing: -0.01em;
  margin: 0 0 12px;
  color: var(--text-strong);
}

.code-landing__hero p {
  margin: 0 0 18px;
  color: var(--text-muted);
  font-size: var(--fs-md);
}

.code-landing__input {
  display: flex;
  align-items: center;
  gap: 10px;
  background: var(--surface-card-muted, rgba(255, 255, 255, 0.06));
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
  border-radius: 12px;
  padding: 12px 14px;
  color: var(--text-strong);
  font-size: var(--fs-md);
  width: 100%;
  box-sizing: border-box;
}

.code-landing__input input {
  flex: 1;
  background: transparent;
  border: 0;
  color: var(--text-strong);
  font: inherit;
  outline: none;
}

.code-landing__input input::placeholder {
  color: var(--text-faint);
}

.code-landing__input-send {
  width: 26px;
  height: 26px;
  border-radius: 50%;
  background: rgba(255, 255, 255, 0.18);
  border: 0;
  color: var(--text-strong);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.code-landing__projects-header {
  display: flex;
  align-items: center;
  gap: 10px;
}

.code-landing__projects-header h2 {
  margin: 0;
  font-size: var(--fs-md);
  font-weight: 600;
  color: var(--text-strong);
}

.code-landing__projects-header .count {
  font-size: var(--fs-xs);
  color: var(--text-faint);
}

.code-landing__projects-actions {
  margin-left: auto;
  display: flex;
  gap: 6px;
}

.code-landing__ghost-btn {
  font-size: var(--fs-xs);
  padding: 4px 10px;
  border-radius: 6px;
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.14));
  background: transparent;
  color: var(--text-strong);
  cursor: pointer;
}

.code-landing__ghost-btn:hover {
  background: var(--surface-hover);
}

.code-landing__grid {
  display: grid;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  gap: 10px;
}

@media (max-width: 720px) {
  .code-landing__grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}

@media (max-width: 480px) {
  .code-landing__grid {
    grid-template-columns: 1fr;
  }
}

.code-landing__card {
  background: var(--surface-card, rgba(255, 255, 255, 0.04));
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.08));
  border-radius: 10px;
  padding: 12px;
  display: flex;
  flex-direction: column;
  gap: 6px;
  min-height: 92px;
  cursor: pointer;
  text-align: left;
  font: inherit;
  color: inherit;
  transition: background-color var(--ds-dur-fast, 120ms) ease;
}

.code-landing__card:hover {
  background: var(--surface-hover);
}

.code-landing__card-name {
  font-size: var(--fs-sm);
  font-weight: 600;
  color: var(--text-strong);
  display: flex;
  align-items: center;
  gap: 6px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.code-landing__card-meta {
  font-size: var(--fs-xs);
  color: var(--text-muted);
}

.code-landing__card-branch {
  font-size: var(--fs-2xs);
  color: rgba(120, 200, 255, 0.85);
  margin-top: auto;
  font-family: ui-monospace, SFMono-Regular, "SF Mono", Menlo, monospace;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.code-landing__card--add {
  border-style: dashed;
  align-items: center;
  justify-content: center;
  color: var(--text-muted);
  font-size: var(--fs-xs);
}

.code-landing__card--add .plus {
  font-size: 18px;
  font-weight: 300;
  line-height: 1;
}

.code-landing__empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  text-align: center;
  padding: 40px 20px;
}

.code-landing__empty-actions {
  display: flex;
  gap: 8px;
  justify-content: center;
  margin-top: 18px;
}

.code-landing__empty-primary {
  background: var(--text-strong);
  color: var(--surface-messages, rgb(8, 9, 12));
  border: 0;
  border-radius: 8px;
  padding: 8px 16px;
  font-size: var(--fs-sm);
  font-weight: 500;
  cursor: pointer;
}

.code-landing__empty-secondary {
  background: transparent;
  border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.18));
  color: var(--text-strong);
  border-radius: 8px;
  padding: 8px 16px;
  font-size: var(--fs-sm);
  font-weight: 500;
  cursor: pointer;
}
```

- [ ] **Step 2: Confirm the file exists**

```bash
test -f desktop-ui/src/styles/code-landing.css && echo OK
```

Expected: prints `OK`.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/styles/code-landing.css
git commit -m "$(cat <<'EOF'
style(coding-mode): add code-landing.css

Hero + 3-col responsive grid + dashed-add card + empty-state CTA pair.
Uses existing tokens (--surface-card, --border-subtle, --text-strong,
--fs-* scale, --fs-display-sm for the 28px hero). Branch text uses
soft cyan rgba(120,200,255,0.85), readable on dark surfaces.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 6: `CodeLanding` component — populated state

**Files:**
- Create: `desktop-ui/src/features/coding/components/CodeLanding.tsx`
- Create: `desktop-ui/src/features/coding/components/CodeLanding.test.tsx`

- [ ] **Step 1: Write the failing test (populated state only)**

Create `desktop-ui/src/features/coding/components/CodeLanding.test.tsx`:

```tsx
// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CodeLanding, type CodeLandingProject } from "./CodeLanding";

afterEach(() => cleanup());

const sampleProjects: CodeLandingProject[] = [
  { id: "ws-1", name: "klyntbot", branch: "feature/code-mode", sessionCount: 12 },
  { id: "ws-2", name: "raki-web", branch: "main", sessionCount: 3 },
  { id: "ws-3", name: "desktop-ui", branch: null, sessionCount: 0 },
];

const baseHandlers = {
  onSelectProject: vi.fn(),
  onAddProject: vi.fn(),
  onImportProject: vi.fn(),
};

describe("CodeLanding (populated)", () => {
  it("renders the hero with the populated heading", () => {
    render(<CodeLanding projects={sampleProjects} {...baseHandlers} />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe(
      "What should we build today?",
    );
  });

  it("renders a card for each project with name, sessions, and branch", () => {
    render(<CodeLanding projects={sampleProjects} {...baseHandlers} />);
    expect(screen.getByText("klyntbot")).toBeTruthy();
    expect(screen.getByText("raki-web")).toBeTruthy();
    expect(screen.getByText("desktop-ui")).toBeTruthy();
    expect(screen.getByText("12 sessions")).toBeTruthy();
    expect(screen.getByText("3 sessions")).toBeTruthy();
    expect(screen.getByText("0 sessions")).toBeTruthy();
    expect(screen.getByText("feature/code-mode")).toBeTruthy();
    expect(screen.getByText("main")).toBeTruthy();
  });

  it("shows the project count in the section header", () => {
    render(<CodeLanding projects={sampleProjects} {...baseHandlers} />);
    const header = screen.getByRole("heading", { level: 2 });
    expect(header.textContent).toBe("Projects");
    expect(screen.getByText("3", { selector: ".count" })).toBeTruthy();
  });

  it("clicking a project card calls onSelectProject with its id", () => {
    const onSelectProject = vi.fn();
    render(
      <CodeLanding
        projects={sampleProjects}
        {...baseHandlers}
        onSelectProject={onSelectProject}
      />,
    );
    fireEvent.click(screen.getByText("klyntbot").closest("button") as HTMLButtonElement);
    expect(onSelectProject).toHaveBeenCalledWith("ws-1");
  });

  it("clicking the trailing + New project card calls onAddProject", () => {
    const onAddProject = vi.fn();
    render(
      <CodeLanding
        projects={sampleProjects}
        {...baseHandlers}
        onAddProject={onAddProject}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: /new project/i }));
    expect(onAddProject).toHaveBeenCalled();
  });

  it("clicking the header Import button calls onImportProject", () => {
    const onImportProject = vi.fn();
    render(
      <CodeLanding
        projects={sampleProjects}
        {...baseHandlers}
        onImportProject={onImportProject}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Import" }));
    expect(onImportProject).toHaveBeenCalled();
  });

  it("clicking the header + New button also calls onAddProject", () => {
    const onAddProject = vi.fn();
    render(
      <CodeLanding
        projects={sampleProjects}
        {...baseHandlers}
        onAddProject={onAddProject}
      />,
    );
    fireEvent.click(screen.getByRole("button", { name: "+ New" }));
    expect(onAddProject).toHaveBeenCalled();
  });

  it("renders no branch line when branch is null", () => {
    render(
      <CodeLanding
        projects={[{ id: "x", name: "no-branch", branch: null, sessionCount: 0 }]}
        {...baseHandlers}
      />,
    );
    expect(document.querySelector(".code-landing__card-branch")).toBeNull();
  });
});
```

- [ ] **Step 2: Run the test to confirm it fails**

```bash
cd desktop-ui && bun run test -- src/features/coding/components/CodeLanding.test.tsx
```

Expected: tests fail with "Cannot find module './CodeLanding'".

- [ ] **Step 3: Implement the component (populated state only — empty state in next task)**

Create `desktop-ui/src/features/coding/components/CodeLanding.tsx`:

```tsx
import { ArrowUp, FolderPlus } from "lucide-react";
import { useState } from "react";

export type CodeLandingProject = {
  id: string;
  name: string;
  branch: string | null;
  sessionCount: number;
};

export type CodeLandingProps = {
  projects: CodeLandingProject[];
  onSelectProject: (id: string) => void;
  onAddProject: () => void;
  onImportProject: () => void;
};

export function CodeLanding({
  projects,
  onSelectProject,
  onAddProject,
  onImportProject,
}: CodeLandingProps) {
  const [draft, setDraft] = useState("");
  const isEmpty = projects.length === 0;

  return (
    <div className="code-landing">
      <div className="code-landing__inner">
        <div className="code-landing__hero">
          <h1>{isEmpty ? "Get started" : "What should we build today?"}</h1>
          {!isEmpty && <p>Pick a project below or describe a task to start a new session.</p>}
          <label className="code-landing__input">
            <input
              type="text"
              placeholder="Describe a task…"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              aria-label="Describe a task"
            />
            <button
              type="button"
              className="code-landing__input-send"
              aria-label="Send task"
              disabled={draft.trim().length === 0}
            >
              <ArrowUp size={14} aria-hidden />
            </button>
          </label>
        </div>

        {isEmpty ? (
          <div className="code-landing__empty">
            <p>No projects yet. Create one or import an existing repo to begin.</p>
            <div className="code-landing__empty-actions">
              <button
                type="button"
                className="code-landing__empty-primary"
                onClick={onAddProject}
              >
                Create first project
              </button>
              <button
                type="button"
                className="code-landing__empty-secondary"
                onClick={onImportProject}
              >
                Import existing repo
              </button>
            </div>
          </div>
        ) : (
          <section>
            <div className="code-landing__projects-header">
              <h2>Projects</h2>
              <span className="count">{projects.length}</span>
              <div className="code-landing__projects-actions">
                <button
                  type="button"
                  className="code-landing__ghost-btn"
                  onClick={onImportProject}
                >
                  Import
                </button>
                <button
                  type="button"
                  className="code-landing__ghost-btn"
                  onClick={onAddProject}
                >
                  + New
                </button>
              </div>
            </div>
            <div className="code-landing__grid">
              {projects.map((p) => (
                <button
                  key={p.id}
                  type="button"
                  className="code-landing__card"
                  onClick={() => onSelectProject(p.id)}
                >
                  <span className="code-landing__card-name">
                    <FolderPlus size={14} aria-hidden />
                    <span>{p.name}</span>
                  </span>
                  <span className="code-landing__card-meta">
                    {p.sessionCount === 1 ? "1 session" : `${p.sessionCount} sessions`}
                  </span>
                  {p.branch && <span className="code-landing__card-branch">{p.branch}</span>}
                </button>
              ))}
              <button
                type="button"
                className="code-landing__card code-landing__card--add"
                onClick={onAddProject}
                aria-label="New project"
              >
                <span className="plus" aria-hidden>+</span>
                <span>New project</span>
              </button>
            </div>
          </section>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Run the test to confirm it passes**

```bash
cd desktop-ui && bun run test -- src/features/coding/components/CodeLanding.test.tsx
```

Expected: all 8 populated-state tests pass.

- [ ] **Step 5: Run typecheck and lint**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Expected: zero errors, zero warnings.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/coding/components/CodeLanding.tsx desktop-ui/src/features/coding/components/CodeLanding.test.tsx
git commit -m "$(cat <<'EOF'
feat(coding-mode): add CodeLanding component (populated + empty states)

Pure component: hero "What should we build today?" with a task-input
draft (visual only — no wiring this pass), a 3-col responsive
project grid built from CodeLandingProject[], a trailing "+ New"
dashed card, and an empty-state Create/Import CTA pair when
projects is empty.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 7: `CodeLanding` empty-state tests

**Files:**
- Modify: `desktop-ui/src/features/coding/components/CodeLanding.test.tsx`

(The component already implements both states from Task 6. We add the empty-state tests separately to keep concerns clean and to satisfy the spec's testing matrix.)

- [ ] **Step 1: Append the empty-state describe block to the test file**

Append to the bottom of `desktop-ui/src/features/coding/components/CodeLanding.test.tsx`:

```tsx
describe("CodeLanding (empty)", () => {
  it("renders the Get started heading when there are no projects", () => {
    render(<CodeLanding projects={[]} {...baseHandlers} />);
    expect(screen.getByRole("heading", { level: 1 }).textContent).toBe("Get started");
  });

  it("does not render the Projects section when empty", () => {
    render(<CodeLanding projects={[]} {...baseHandlers} />);
    expect(screen.queryByRole("heading", { level: 2 })).toBeNull();
    expect(document.querySelector(".code-landing__grid")).toBeNull();
  });

  it("renders both empty-state CTAs", () => {
    render(<CodeLanding projects={[]} {...baseHandlers} />);
    expect(screen.getByRole("button", { name: "Create first project" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Import existing repo" })).toBeTruthy();
  });

  it("Create first project calls onAddProject", () => {
    const onAddProject = vi.fn();
    render(<CodeLanding projects={[]} {...baseHandlers} onAddProject={onAddProject} />);
    fireEvent.click(screen.getByRole("button", { name: "Create first project" }));
    expect(onAddProject).toHaveBeenCalled();
  });

  it("Import existing repo calls onImportProject", () => {
    const onImportProject = vi.fn();
    render(
      <CodeLanding projects={[]} {...baseHandlers} onImportProject={onImportProject} />,
    );
    fireEvent.click(screen.getByRole("button", { name: "Import existing repo" }));
    expect(onImportProject).toHaveBeenCalled();
  });

  it("does not render the descriptive subtitle in empty state", () => {
    render(<CodeLanding projects={[]} {...baseHandlers} />);
    expect(
      screen.queryByText(/Pick a project below or describe a task/),
    ).toBeNull();
  });
});
```

- [ ] **Step 2: Run the tests to confirm they pass**

```bash
cd desktop-ui && bun run test -- src/features/coding/components/CodeLanding.test.tsx
```

Expected: 14 tests pass total (8 populated + 6 empty).

- [ ] **Step 3: Run typecheck and lint**

```bash
cd desktop-ui && bun run typecheck && bun run lint
```

Expected: zero errors, zero warnings.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/coding/components/CodeLanding.test.tsx
git commit -m "$(cat <<'EOF'
test(coding-mode): cover CodeLanding empty-state branch

Adds the empty-projects describe block — Get started heading,
absent Projects section, both CTA buttons wire to the correct
handlers, no descriptive subtitle.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 8: Add `codeLandingProps` and `codeLandingNode` to layout types

**Files:**
- Modify: `desktop-ui/src/features/layout/hooks/layoutNodes/types.ts`
- Modify: `desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx`

- [ ] **Step 1: Update `LayoutPrimarySurface` and `LayoutNodesResult`**

In `desktop-ui/src/features/layout/hooks/layoutNodes/types.ts`:

After the existing import block (line 16), add:

```ts
import type { CodeLanding } from "@/features/coding/components/CodeLanding";
```

Inside the `LayoutPrimarySurface` type (currently lines 54–68), add the `codeLandingProps` field. The whole type becomes:

```ts
export type LayoutPrimarySurface = {
  sidebarProps: SidebarChatProps;
  messagesProps: ComponentProps<typeof Messages>;
  composerProps: ComponentProps<typeof Composer> | null;
  approvalToastsProps: ComponentProps<typeof ApprovalToasts>;
  updateToastProps: ComponentProps<typeof UpdateToast>;
  errorToastsProps: ComponentProps<typeof ErrorToasts>;
  homeProps: ComponentProps<typeof Home>;
  codeLandingProps: ComponentProps<typeof CodeLanding>;
  mainHeaderProps: ComponentProps<typeof MainHeader> | null;
  desktopTopbarProps: {
    showBackToChat: boolean;
    onExitDiff: () => void;
  };
  chatViewProps: ChatViewProps;
};
```

Inside `LayoutNodesResult` (currently lines 96–111), add `codeLandingNode`:

```ts
export type LayoutNodesResult = {
  sidebarNode: ReactNode;
  messagesNode: ReactNode;
  composerNode: ReactNode;
  approvalToastsNode: ReactNode;
  updateToastNode: ReactNode;
  errorToastsNode: ReactNode;
  homeNode: ReactNode;
  codeLandingNode: ReactNode;
  mainHeaderNode: ReactNode;
  desktopTopbarLeftNode: ReactNode;
  gitDiffPanelNode: ReactNode;
  gitDiffViewerNode: ReactNode;
  planPanelNode: ReactNode;
  debugPanelNode: ReactNode;
  terminalDockNode: ReactNode;
};
```

- [ ] **Step 2: Update `buildPrimaryNodes.tsx` to produce the node**

Replace `desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx` with:

```tsx
import { ApprovalToasts } from "@app/components/ApprovalToasts";
import { MainHeader } from "@app/components/MainHeader";
import { SidebarChatLayout } from "@app/components/SidebarChatLayout";
import ArrowLeft from "lucide-react/dist/esm/icons/arrow-left";
import { Composer } from "@/features/composer/components/Composer";
import { CodeLanding } from "@/features/coding/components/CodeLanding";
import { Home } from "@/features/home/components/Home";
import { Messages } from "@/features/messages/components/Messages";
import { ErrorToasts } from "@/features/notifications/components/ErrorToasts";
import { UpdateToast } from "@/features/update/components/UpdateToast";
import type { LayoutNodesResult, LayoutPrimarySurface } from "./types";

export type PrimaryLayoutNodesOptions = LayoutPrimarySurface;

type PrimaryLayoutNodes = Pick<
  LayoutNodesResult,
  | "sidebarNode"
  | "messagesNode"
  | "composerNode"
  | "approvalToastsNode"
  | "updateToastNode"
  | "errorToastsNode"
  | "homeNode"
  | "codeLandingNode"
  | "mainHeaderNode"
  | "desktopTopbarLeftNode"
>;

export function buildPrimaryNodes(options: PrimaryLayoutNodesOptions): PrimaryLayoutNodes {
  const sidebarNode = (
    <SidebarChatLayout
      onOpenSettings={options.sidebarProps.onOpenSettings}
      onNewChat={options.sidebarProps.onNewChat}
      onSelectPlugins={options.sidebarProps.onSelectPlugins}
      threads={options.sidebarProps.threads}
      selectedSessionKey={options.sidebarProps.selectedSessionKey}
      onSelectThread={options.sidebarProps.onSelectThread}
    />
  );

  const messagesNode = <Messages {...options.messagesProps} />;

  const composerNode = options.composerProps ? (
    <Composer {...options.composerProps} />
  ) : null;

  const approvalToastsNode = <ApprovalToasts {...options.approvalToastsProps} />;
  const updateToastNode = <UpdateToast {...options.updateToastProps} />;
  const errorToastsNode = <ErrorToasts {...options.errorToastsProps} />;
  const homeNode = <Home {...options.homeProps} />;
  const codeLandingNode = <CodeLanding {...options.codeLandingProps} />;
  const mainHeaderNode = options.mainHeaderProps ? (
    <MainHeader {...options.mainHeaderProps} />
  ) : null;

  const desktopTopbarLeftNode = (
    <>
      {options.desktopTopbarProps.showBackToChat && (
        <button
          className="icon-button back-button"
          onClick={options.desktopTopbarProps.onExitDiff}
          aria-label="Back to chat"
        >
          <ArrowLeft aria-hidden />
        </button>
      )}
      {mainHeaderNode}
    </>
  );

  return {
    sidebarNode,
    messagesNode,
    composerNode,
    approvalToastsNode,
    updateToastNode,
    errorToastsNode,
    homeNode,
    codeLandingNode,
    mainHeaderNode,
    desktopTopbarLeftNode,
  };
}
```

- [ ] **Step 3: Run typecheck**

```bash
cd desktop-ui && bun run typecheck
```

Expected: typecheck **fails** because `MainApp.tsx` does not yet supply `codeLandingProps` to the `primary:` block. That's expected — Task 10 fixes it. Capture the error message:

> Property 'codeLandingProps' is missing in type … but required in type 'LayoutPrimarySurface'.

This confirms the type is wired through correctly. Proceed.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/layout/hooks/layoutNodes/types.ts desktop-ui/src/features/layout/hooks/layoutNodes/buildPrimaryNodes.tsx
git commit -m "$(cat <<'EOF'
feat(coding-mode): thread codeLanding through layoutNodes

Adds codeLandingProps to LayoutPrimarySurface and codeLandingNode to
LayoutNodesResult. buildPrimaryNodes constructs <CodeLanding/> from
the new props. MainApp wiring lands in a follow-up commit so the tree
typechecks end-to-end.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 9: Conditionally render `codeLandingNode` in `DesktopLayout`

**Files:**
- Modify: `desktop-ui/src/features/layout/components/DesktopLayout.tsx`

- [ ] **Step 1: Add the import and prop**

At the top of `desktop-ui/src/features/layout/components/DesktopLayout.tsx`, after the existing imports, add:

```ts
import { useAppMode } from "@app/hooks/useAppMode";
```

In `DesktopLayoutProps` (around line 57), add `codeLandingNode` after `homeNode`:

```ts
type DesktopLayoutProps = {
  sidebarNode: ReactNode;
  updateToastNode: ReactNode;
  approvalToastsNode: ReactNode;
  errorToastsNode: ReactNode;
  homeNode: ReactNode;
  codeLandingNode: ReactNode;
  pluginsNode?: ReactNode;
  dashboardNode?: ReactNode;
  showHome: boolean;
  showWorkspace: boolean;
  topbarLeftNode: ReactNode;
  topbarActionsNode?: ReactNode;
  centerMode: CenterMode;
  preloadGitDiffs: boolean;
  splitChatDiffView: boolean;
  messagesNode: ReactNode;
  gitDiffViewerNode: ReactNode;
  gitDiffPanelNode: ReactNode;
  planPanelNode: ReactNode;
  composerNode: ReactNode;
  terminalDockNode: ReactNode;
  debugPanelNode: ReactNode;
  hasActivePlan: boolean;
  onSidebarResizeStart: (event: MouseEvent<HTMLDivElement>) => void;
  onChatDiffSplitPositionResizeStart: (event: MouseEvent<HTMLDivElement>) => void;
  onRightPanelResizeStart: (event: MouseEvent<HTMLDivElement>) => void;
  onPlanPanelResizeStart: (event: MouseEvent<HTMLDivElement>) => void;
};
```

In the destructuring at the top of the `DesktopLayout` function (around line 86–113), add `codeLandingNode` after `homeNode`:

```ts
export function DesktopLayout({
  sidebarNode,
  updateToastNode,
  approvalToastsNode,
  errorToastsNode,
  homeNode,
  codeLandingNode,
  pluginsNode,
  dashboardNode,
  showHome,
  // …
```

- [ ] **Step 2: Read the current mode and swap the rendered node**

Inside the function body, after the existing `chatPaneNode` line (~116), add:

```ts
const { mode } = useAppMode();
const homeOrCodeNode = mode === "code" ? codeLandingNode : homeNode;
```

Then change the line `{showHome && homeNode}` (currently line 160) to:

```tsx
{showHome && homeOrCodeNode}
```

- [ ] **Step 3: Run typecheck**

```bash
cd desktop-ui && bun run typecheck
```

Expected: typecheck still fails, now with **two** errors — `MainApp.tsx` does not yet supply `codeLandingProps` (from Task 8) and `codeLandingNode` is required by `DesktopLayout` but not passed by its parent. Both are addressed in Task 10. Proceed.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/layout/components/DesktopLayout.tsx
git commit -m "$(cat <<'EOF'
feat(coding-mode): swap home/code node in DesktopLayout based on mode

DesktopLayout now reads useAppMode() and renders codeLandingNode in
place of homeNode whenever showHome is true and mode === "code".
Wire-up of the new prop from MainApp follows in the next commit.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 10: Assemble `codeLandingProps` and pass `codeLandingNode` from `MainApp`

**Files:**
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx` (two regions)

- [ ] **Step 1: Build the `codeLandingProps` value next to where `homeProps` is assembled**

Open `desktop-ui/src/features/app/components/MainApp.tsx`. Find the `primary: { … }` block in the `useMainAppLayoutSurfaces({ … })` call (the section around lines 1750–1810). Inside that block, locate the existing `homeProps` field. Immediately after `homeProps`, add:

```ts
codeLandingProps: {
  projects: workspaces
    .filter((ws) => (ws.kind ?? "main") === "main")
    .map((ws) => ({
      id: ws.id,
      name: ws.name,
      branch: ws.worktree?.branch ?? null,
      sessionCount: (threadsByWorkspace[ws.id] ?? []).length,
    })),
  onSelectProject: selectWorkspace,
  onAddProject: handleAddWorkspace,
  onImportProject: openWorkspaceFromUrlPrompt,
},
```

(All four right-hand-side identifiers — `workspaces`, `threadsByWorkspace`, `selectWorkspace`, `handleAddWorkspace`, `openWorkspaceFromUrlPrompt` — are already in scope at this point in `MainApp.tsx`. Confirm by ⌘F-search if unsure.)

- [ ] **Step 2: Pull `codeLandingNode` out of `displayNodes` and pass it to `appLayout`**

Locate the destructuring of `useMainAppDisplayNodes` (around line 1771):

```ts
const {
  sidebarNode,
  // …
  homeNode,
  // …
} = displayNodes;
```

Add `codeLandingNode` to the destructured set:

```ts
const {
  sidebarNode,
  // …
  homeNode,
  codeLandingNode,
  // …
} = displayNodes;
```

Then in the `appLayout: { … }` block (around line 1820), find the line `homeNode,` (around line 1843) and add `codeLandingNode,` immediately after it:

```ts
appLayout: {
  // …
  homeNode,
  codeLandingNode,
  pluginsNode: appView === "plugins" ? <PluginsView /> : null,
  // …
},
```

- [ ] **Step 3: Run typecheck**

```bash
cd desktop-ui && bun run typecheck
```

Expected: typecheck passes with zero errors.

- [ ] **Step 4: Run lint**

```bash
cd desktop-ui && bun run lint
```

Expected: zero warnings.

- [ ] **Step 5: Run the full test suite**

```bash
cd desktop-ui && bun run test
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```bash
git add desktop-ui/src/features/app/components/MainApp.tsx
git commit -m "$(cat <<'EOF'
feat(coding-mode): assemble CodeLanding props in MainApp

Maps workspaces -> CodeLandingProject[] (main-kind only), wires
selectWorkspace / handleAddWorkspace / openWorkspaceFromUrlPrompt as
the project click + create + import handlers, and threads the new
codeLandingNode through appLayout to DesktopLayout. End-to-end
typecheck now passes.

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 11: Register the new stylesheets in `index.css`

**Files:**
- Modify: `desktop-ui/src/styles/index.css`

- [ ] **Step 1: Inspect the import order**

```bash
grep -n "@import" desktop-ui/src/styles/index.css | tail -20
```

Capture the output so the new imports land alongside other component styles.

- [ ] **Step 2: Add the two new imports**

In `desktop-ui/src/styles/index.css`, after the existing `@import "./sidebar-chat.css";` line, add:

```css
@import "./app-mode-switch.css";
@import "./code-landing.css";
```

If `sidebar-chat.css` is not present in the import list, add the two lines after the last `@import` for a feature-level component stylesheet (e.g. after `@import "./chat.css";`). Maintain alphabetical or near-alphabetical grouping consistent with the file's existing convention.

- [ ] **Step 3: Run lint**

```bash
cd desktop-ui && bun run lint
```

Expected: zero warnings.

- [ ] **Step 4: Build the production bundle to confirm CSS resolves**

```bash
cd desktop-ui && bun run build
```

Expected: build completes successfully (`vite build` reports written assets).

- [ ] **Step 5: Commit**

```bash
git add desktop-ui/src/styles/index.css
git commit -m "$(cat <<'EOF'
style(coding-mode): register app-mode-switch.css and code-landing.css

Co-Authored-By: Claude Opus 4.7 (1M context) <noreply@anthropic.com>
EOF
)"
```

---

### Task 12: Manual verification in the desktop app

**Files:** none — this is a runtime verification pass.

- [ ] **Step 1: Start the Vite dev server**

In one terminal:

```bash
cd desktop-ui && bun run dev:vite
```

Wait for "Local: http://localhost:1420".

- [ ] **Step 2: Start the Tauri dev shell**

In a second terminal at the workspace root:

```bash
cargo tauri dev
```

Wait for the desktop window to appear.

- [ ] **Step 3: Verify the switch position and clickability**

Inspect the sidebar's top row. Confirm:

- The Assistant ⇄ Code segmented pill is visible in the top row of the left panel, right-aligned.
- The macOS traffic-light cluster on the left does not overlap the switch.
- Clicking the **Code** segment fills it (becomes the active segment) and changes the main content area to the Code landing.
- Clicking **Assistant** flips back. Both clicks register without dragging the window (the drag region must not consume the click).

- [ ] **Step 4: Verify keyboard navigation**

Tab into the switch. Press → and ←. Confirm focus moves between segments and the active mode flips on each press.

- [ ] **Step 5: Verify persistence**

Switch to Code, close the app, reopen it. Confirm the app launches in Code mode. Switch back to Assistant, reopen — confirm Assistant mode sticks.

- [ ] **Step 6: Verify the populated Code landing**

With at least one workspace, switch to Code. Confirm:

- Hero reads "What should we build today?"
- Sub-text and a non-functional task input render (the input accepts text but its send button stays disabled until the field is non-empty — clicking it does nothing this pass; this is intentional).
- The Projects header shows the count and the Import / + New buttons.
- Each project card shows name, sessions count, and branch (when present).
- Clicking a project card navigates to that workspace's existing view (verify by switching back to Assistant — the workspace should remain selected).
- Clicking + New opens the existing add-workspace flow.
- Clicking Import opens the existing add-from-URL flow.

- [ ] **Step 7: Verify the empty Code landing**

Temporarily simulate empty state: in DevTools, run `localStorage.clear()` then mount the app with no workspaces (or run against `KLYNTBOT_HOME=~/.klyntbot-empty cargo tauri dev`). Switch to Code mode. Confirm:

- Hero reads "Get started".
- Subtitle ("Pick a project below…") is absent.
- Two large buttons render: a filled white "Create first project" and an outlined "Import existing repo". Each fires its respective handler.

- [ ] **Step 8: Verify the rest of the UI is unchanged**

In Assistant mode, exercise: New chat, Search, Calendar, Plugins, Automations, Project nav items; selecting a workspace; opening a thread; opening Settings. Confirm nothing has regressed.

- [ ] **Step 9: Stop the dev processes**

`Ctrl-C` both the Vite dev server and the Tauri dev shell.

- [ ] **Step 10: Final repo-level verification commands**

```bash
cd desktop-ui && bun run typecheck && bun run lint && bun run test
```

Expected: all pass with zero warnings, all tests green.

- [ ] **Step 11: No commit needed for verification.**

This task does not modify code. If you discovered an issue and patched it during verification, stage and commit that patch as `fix(coding-mode): …` with a short description of what verification revealed.

---

## Self-review

Performed against the spec.

**Spec coverage**

| Spec section | Covered by |
|---|---|
| Top-row segmented pill | Tasks 2, 3, 4 |
| `useAppMode` hook + localStorage persistence | Task 1 |
| Sidebar nav unchanged in both modes | Task 4 (test "renders the same nav items in code mode") |
| `<CodeLanding>` hero + grid | Tasks 5, 6 |
| `<CodeLanding>` empty state | Task 7 |
| Layout pipeline (`LayoutPrimarySurface` / `buildPrimaryNodes` / `DesktopLayout`) | Tasks 8, 9 |
| `MainApp` assembles `codeLandingProps` from existing workspace state | Task 10 |
| CSS registered in `index.css` | Task 11 |
| Drag-region considerations | Task 4 (CSS topbar `z-index: 2`), Tasks 2/3 (`data-tauri-drag-region="false"` on switch + `-webkit-app-region: no-drag` on the wrapper), Task 12 step 3 (manual verify). |
| Tests for `useAppMode`, `AppModeSwitch`, `CodeLanding`, `SidebarChatLayout` | Tasks 1, 2, 4, 6, 7 |
| Manual verification in `cargo tauri dev` | Task 12 |

**Placeholder scan:** none. Every step contains its full payload.

**Type consistency:**
- `AppMode` is defined once in `useAppMode.ts` and imported by `AppModeSwitch.tsx`.
- `CodeLandingProject` is exported from `CodeLanding.tsx` and consumed in `MainApp.tsx` via the inline `codeLandingProps` literal (whose shape matches `ComponentProps<typeof CodeLanding>` because of the type wired in Task 8).
- Handler names: `onSelectProject`, `onAddProject`, `onImportProject` are consistent across the component, its tests, and the `MainApp` assembly site.
- Existing identifiers used in Task 10 — `workspaces`, `threadsByWorkspace`, `selectWorkspace`, `handleAddWorkspace`, `openWorkspaceFromUrlPrompt` — match the names already present in `MainApp.tsx` (verified at the cited line numbers).

No drift found.
