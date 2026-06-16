# Pomotroid Focus/Pomodoro Port — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Port Pomotroid's polished focus/Pomodoro logic into Klynt by adding a main-window focus view, syncing settings with backend config, and hardening the backend timer, audio, and notifications.

**Architecture:** Reuse the existing tray focus UI by extracting shared components into `features/focus/`, add a `Focus` app view for the main window, sync defaults through `FocusConfig`, and refactor `desktop::focus_timer` to use drift-correcting scheduling plus a cross-platform audio manager.

**Tech Stack:** Tauri 2, Rust, React/TypeScript, `tokio`, `rodio` (audio), existing `desktop-ui` design system.

---

## File Structure

### Frontend

| File | Responsibility |
|------|----------------|
| `desktop-ui/src/features/focus/types.ts` | Shared focus TypeScript types (moved from tray). |
| `desktop-ui/src/features/focus/hooks/useFocusTimer.ts` | IPC + state hook (moved/refactored from tray). |
| `desktop-ui/src/features/focus/components/FocusDial.tsx` | SVG progress ring. |
| `desktop-ui/src/features/focus/components/FocusDisplay.tsx` | Time + phase label. |
| `desktop-ui/src/features/focus/components/FocusControls.tsx` | Control buttons. |
| `desktop-ui/src/features/focus/components/FocusSettingsPanel.tsx` | Settings panel (moved from tray). |
| `desktop-ui/src/features/focus/components/FocusTimer.tsx` | Composed timer view. |
| `desktop-ui/src/features/focus/pages/FocusPage.tsx` | Main-window focus page. |
| `desktop-ui/src/features/focus/focus.css` | Shared focus component styles (moved from tray.css). |
| `desktop-ui/src/features/tray/components/FocusControl.tsx` | Thin wrapper around shared focus components. |
| `desktop-ui/src/features/tray/tray.css` | Tray-specific styles only. |
| `desktop-ui/src/features/tray/types.ts` | Re-exports from `features/focus/types` (or removed). |
| `desktop-ui/src/features/tray/hooks/useFocusTimer.ts` | Re-exports from `features/focus/hooks/useFocusTimer` (or removed). |
| `desktop-ui/src/features/app/constants/appViews.ts` | Adds `Focus` view. |
| `desktop-ui/src/features/app/components/MainApp.tsx` | Handles `appView === "focus"`. |
| `desktop-ui/src/features/app/components/AppLayout.tsx` | Adds `focusNode` prop. |
| `desktop-ui/src/features/layout/components/DesktopLayout.tsx` | Renders focus node when active. |

### Backend

| File | Responsibility |
|------|----------------|
| `crates/desktop/src/focus_timer.rs` | Drift-correcting timer loop + phase state machine. |
| `crates/desktop/src/focus_audio.rs` | Audio manager: embedded defaults + custom files. |
| `crates/desktop/src/lib.rs` | Registers new audio manager state. |
| `crates/desktop/src/commands/productivity.rs` | New commands: get/set focus defaults, custom sounds. |
| `crates/config/src/schema/productivity.rs` | Adds `PomodoroConfig`. |
| `crates/desktop/src/notify.rs` or new `crates/desktop/src/focus_notify.rs` | Cross-platform notification helper. |

---

## Phase 0: Preparation

### Task 0.1: Verify the project builds and tests pass

**Files:**
- Run in: `/Users/jayden/Projects/Klynt/bot`

- [ ] **Step 1: Run frontend type-check**

```bash
cd desktop-ui && npm run typecheck
```

Expected: passes with no errors (or pre-existing errors noted).

- [ ] **Step 2: Run Rust tests for desktop crate**

```bash
cargo test -p desktop focus_timer
```

Expected: existing tests pass.

- [ ] **Step 3: Commit baseline**

```bash
git add -A
git commit -m "chore: baseline before Pomotroid focus port"
```

---

## Phase 1: Extract Shared Focus Module

### Task 1.1: Create `features/focus/types.ts`

**Files:**
- Create: `desktop-ui/src/features/focus/types.ts`
- Delete later: `desktop-ui/src/features/tray/types.ts`

- [ ] **Step 1: Copy types from tray**

```typescript
export interface TodayTask {
  id: string;
  title: string;
  priority: string | null;
  status: string;
  completed: boolean;
  isOverdue: boolean;
  isDueToday: boolean;
  dueDisplay: string | null;
}

export interface CalendarEvent {
  id: string;
  calendarId: string;
  title: string;
  description: string | null;
  startedAt: string;
  endedAt: string;
  location: string | null;
  attendeesCount: number;
  isRecurring: boolean;
  recurrenceId: string | null;
  source: string;
  externalUid: string;
  sessionId: string | null;
  color: string | null;
  syncedAt: string;
}

export interface FocusSession {
  id: string;
  actionId: string | null;
  projectId: string | null;
  sessionType: string;
  targetMins: number | null;
  startedAt: string;
  endedAt: string | null;
  actualMins: number | null;
  interruptions: number;
  qualityScore: number | null;
  completed: boolean;
  notes: string | null;
}

export interface FocusSyncPayload {
  phase: "working" | "break_pending" | "break" | "paused" | "suspended";
  remainingSecs: number;
  totalSecs: number;
  cyclePosition: number;
  longBreakAfter: number;
  paused: boolean;
  actionTitle: string | null;
  dndActive: boolean;
}

export interface FocusWarningPayload {
  phase: string;
  remainingSecs: number;
}

export interface FocusDndUnavailablePayload {
  message: string;
}

export interface FocusSessionStatus {
  active: boolean;
  sync: FocusSyncPayload | null;
  session: FocusSession | null;
}

export interface FocusSettings {
  focusDuration: number;
  shortBreak: number;
  longBreak: number;
  longBreakAfter: number;
  dndEnabled: boolean;
  soundEnabled: boolean;
  notificationEnabled: boolean;
}

export interface FocusPreset {
  label: string;
  focusDuration: number;
  shortBreak: number;
}

export type FocusPhase = "idle" | "working" | "break_pending" | "break" | "suspended";
```

- [ ] **Step 2: Update tray types to re-export**

Modify `desktop-ui/src/features/tray/types.ts`:

```typescript
export * from "../focus/types";
```

- [ ] **Step 3: Type-check**

```bash
cd desktop-ui && npm run typecheck
```

Expected: passes.

- [ ] **Step 4: Commit**

```bash
git add desktop-ui/src/features/focus/types.ts desktop-ui/src/features/tray/types.ts
git commit -m "feat(focus): extract shared focus types"
```

### Task 1.2: Create `features/focus/hooks/useFocusTimer.ts`

**Files:**
- Create: `desktop-ui/src/features/focus/hooks/useFocusTimer.ts`
- Delete later: `desktop-ui/src/features/tray/hooks/useFocusTimer.ts`

- [ ] **Step 1: Copy the hook from tray as starting point**

Copy `desktop-ui/src/features/tray/hooks/useFocusTimer.ts` to `desktop-ui/src/features/focus/hooks/useFocusTimer.ts`.
Change the import of `../types` to `./types` or `../types` depending on new location.

For now keep the local `setInterval` tick; we will remove it in Phase 7.

- [ ] **Step 2: Update tray hook to re-export**

Replace `desktop-ui/src/features/tray/hooks/useFocusTimer.ts` with:

```typescript
export * from "../../focus/hooks/useFocusTimer";
```

- [ ] **Step 3: Update `FocusControl.tsx` imports**

Change `import { FOCUS_PRESETS, type useFocusTimer } from "../hooks/useFocusTimer";` to `import { FOCUS_PRESETS, type useFocusTimer } from "../../focus/hooks/useFocusTimer";`.

- [ ] **Step 4: Type-check and commit**

```bash
cd desktop-ui && npm run typecheck
```

```bash
git add desktop-ui/src/features/focus/hooks/useFocusTimer.ts \
  desktop-ui/src/features/tray/hooks/useFocusTimer.ts \
  desktop-ui/src/features/tray/components/FocusControl.tsx
git commit -m "feat(focus): extract useFocusTimer hook"
```

### Task 1.3: Extract `FocusSettingsPanel` component

**Files:**
- Create: `desktop-ui/src/features/focus/components/FocusSettingsPanel.tsx`
- Modify: `desktop-ui/src/features/tray/components/FocusControl.tsx`

- [ ] **Step 1: Move settings panel code**

Copy the `FocusSettingsPanel` and `SettingRow` functions from `FocusControl.tsx` into `desktop-ui/src/features/focus/components/FocusSettingsPanel.tsx`.

Add exports:

```typescript
export { FocusSettingsPanel };
```

- [ ] **Step 2: Update imports in new file**

```typescript
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import X from "lucide-react/dist/esm/icons/x";
import { useState, useRef } from "react";
import type { FocusSettings } from "../types";
import { Checkbox } from "../../tray/components/Checkbox";
```

- [ ] **Step 3: Remove settings panel from `FocusControl.tsx`**

Delete the `FocusSettingsPanel` and `SettingRow` function definitions from `FocusControl.tsx` and import them:

```typescript
import { FocusSettingsPanel } from "../../focus/components/FocusSettingsPanel";
```

- [ ] **Step 4: Type-check and commit**

```bash
cd desktop-ui && npm run typecheck
```

```bash
git add desktop-ui/src/features/focus/components/FocusSettingsPanel.tsx \
  desktop-ui/src/features/tray/components/FocusControl.tsx
git commit -m "feat(focus): extract FocusSettingsPanel"
```

### Task 1.4: Extract shared focus CSS

**Files:**
- Create: `desktop-ui/src/features/focus/focus.css`
- Modify: `desktop-ui/src/features/tray/tray.css`
- Modify: `desktop-ui/src/features/focus/components/FocusTimer.tsx`
- Modify: `desktop-ui/src/features/focus/components/FocusSettingsPanel.tsx`

- [ ] **Step 1: Move `tc-*` rules from tray.css to focus.css**

Copy all CSS rules starting with `.tc-` from `desktop-ui/src/features/tray/tray.css` into a new file `desktop-ui/src/features/focus/focus.css`.

Leave tray-only rules (`.tray-*`, `.tray-nudge`, `.tray-task`, etc.) in `tray.css`.

- [ ] **Step 2: Import focus.css in shared components**

Add at the top of `FocusTimer.tsx` and `FocusSettingsPanel.tsx`:

```typescript
import "../focus.css";
```

Remove the `import "../tray.css";` from `FocusControl.tsx` if no tray-only classes remain in it.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/focus/focus.css \
  desktop-ui/src/features/tray/tray.css \
  desktop-ui/src/features/focus/components/FocusTimer.tsx \
  desktop-ui/src/features/focus/components/FocusSettingsPanel.tsx \
  desktop-ui/src/features/tray/components/FocusControl.tsx
git commit -m "feat(focus): extract shared focus css"
```

### Task 1.5: Create reusable `FocusTimer` view component

**Files:**
- Create: `desktop-ui/src/features/focus/components/FocusTimer.tsx`
- Modify: `desktop-ui/src/features/tray/components/FocusControl.tsx`

- [ ] **Step 1: Extract `TimerView` into `FocusTimer.tsx`**

Copy the `TimerView`, `CoachingDebrief`, `WarningBanner`, `BreakPendingActions`, `PauseResumeButton`, `SettingsButton`, `QuickDistractionLog`, and `TodayStats` functions from `FocusControl.tsx` into `desktop-ui/src/features/focus/components/FocusTimer.tsx`.

Rename `TimerView` to `FocusTimer` and export it.

Remove tray-specific concerns from the extracted component (learning banner, review prompt) — those stay in the tray wrapper.

- [ ] **Step 2: Update imports in new file**

```typescript
import ChevronRight from "lucide-react/dist/esm/icons/chevron-right";
import Coffee from "lucide-react/dist/esm/icons/coffee";
import Eye from "lucide-react/dist/esm/icons/eye";
import Pause from "lucide-react/dist/esm/icons/pause";
import Play from "lucide-react/dist/esm/icons/play";
import Settings from "lucide-react/dist/esm/icons/settings";
import Sparkles from "lucide-react/dist/esm/icons/sparkles";
import Square from "lucide-react/dist/esm/icons/square";
import X from "lucide-react/dist/esm/icons/x";
import { useEffect, useRef, useState } from "react";
import type { useFocusTimer } from "../hooks/useFocusTimer";
import { formatElapsed, formatHumanDuration } from "../../tray/lib/dates";
import type { FocusSettings } from "../types";
import { Checkbox } from "../../tray/components/Checkbox";
```

- [ ] **Step 3: Refactor `FocusControl.tsx` to use `FocusTimer`**

Keep only:
- `FocusControl` wrapper that toggles between `FocusTimer` and `FocusSettingsPanel`.
- Tray-specific `MicroReviewPrompt` and learning banner logic (move into a tray wrapper if needed).

```typescript
import { FocusTimer } from "../../focus/components/FocusTimer";
```

- [ ] **Step 4: Type-check and commit**

```bash
cd desktop-ui && npm run typecheck
```

```bash
git add desktop-ui/src/features/focus/components/FocusTimer.tsx \
  desktop-ui/src/features/tray/components/FocusControl.tsx
git commit -m "feat(focus): extract reusable FocusTimer view"
```

---

## Phase 2: Add Main-Window Focus View

### Task 2.1: Add `Focus` to `AppView`

**Files:**
- Modify: `desktop-ui/src/features/app/constants/appViews.ts`

- [ ] **Step 1: Add Focus view constant**

```typescript
export const AppView = {
  Home: "home",
  Chat: "chat",
  Plugins: "plugins",
  Calendar: "calendar",
  Focus: "focus",
} as const;
```

- [ ] **Step 2: Commit**

```bash
git add desktop-ui/src/features/app/constants/appViews.ts
git commit -m "feat(focus): add Focus app view constant"
```

### Task 2.2: Render `FocusPage` from `MainApp`

**Files:**
- Create: `desktop-ui/src/features/focus/pages/FocusPage.tsx`
- Modify: `desktop-ui/src/features/app/components/MainApp.tsx`

- [ ] **Step 1: Create `FocusPage.tsx`**

```typescript
import { FocusTimer } from "../components/FocusTimer";
import { FocusSettingsPanel } from "../components/FocusSettingsPanel";
import { useFocusTimer } from "../hooks/useFocusTimer";
import { useState } from "react";

export function FocusPage() {
  const timer = useFocusTimer();
  const [showSettings, setShowSettings] = useState(false);

  return (
    <div className="focus-page">
      {showSettings ? (
        <FocusSettingsPanel
          settings={timer.settings}
          onUpdate={timer.updateSettings}
          onClose={() => setShowSettings(false)}
        />
      ) : (
        <FocusTimer timer={timer} onOpenSettings={() => setShowSettings(true)} />
      )}
    </div>
  );
}
```

- [ ] **Step 2: Wire `FocusPage` into `MainApp`**

Add lazy import near other lazy imports in `MainApp.tsx`:

```typescript
const FocusPage = lazy(() =>
  import("@/features/focus/pages/FocusPage").then((module) => ({
    default: module.FocusPage,
  })),
);
```

Add a setter/selector for focus view. In the section where `onSelectCalendar` is defined, add:

```typescript
const onSelectFocus = useCallback(() => {
  setAppView(AppView.Focus);
}, []);
```

Pass `focusNode` through to `MainAppShell`. Where `MainAppShell` is rendered, add:

```tsx
focusNode={
  <Suspense fallback={null}>
    <FocusPage />
  </Suspense>
}
onSelectFocus={onSelectFocus}
```

- [ ] **Step 3: Update `MainAppShell` props**

Add `focusNode?: ReactNode` and `onSelectFocus?: () => void` to `MainAppShellProps` in `MainAppShell.tsx` and pass through to `AppLayout`.

- [ ] **Step 4: Update `AppLayout` props**

Add `focusNode?: ReactNode` to `AppLayoutProps` in `AppLayout.tsx` and pass through to `DesktopLayout` as `focusNode`.

- [ ] **Step 5: Update `DesktopLayout` to render focus node**

Modify `desktop-ui/src/features/layout/components/DesktopLayout.tsx`:

Add `focusNode?: ReactNode` prop.

Where `centerMode` is used to decide what to render, add focus handling. If `centerMode === "focus"`, render `focusNode`.

- [ ] **Step 6: Add focus button to sidebar/navigation**

Find the sidebar component that renders navigation buttons (likely `desktop-ui/src/features/layout/components/Sidebar.tsx` or similar). Add a Focus button that calls `onSelectFocus`.

- [ ] **Step 7: Type-check and commit**

```bash
cd desktop-ui && npm run typecheck
```

```bash
git add desktop-ui/src/features/focus/pages/FocusPage.tsx \
  desktop-ui/src/features/app/components/MainApp.tsx \
  desktop-ui/src/features/app/components/MainAppShell.tsx \
  desktop-ui/src/features/app/components/AppLayout.tsx \
  desktop-ui/src/features/layout/components/DesktopLayout.tsx
# include sidebar file if changed
git commit -m "feat(focus): add main-window FocusPage view"
```

---

## Phase 3: Sync Settings with Backend Config

### Task 3.1: Add `PomodoroConfig` to Rust config schema

**Files:**
- Modify: `crates/config/src/schema/productivity.rs`

- [ ] **Step 1: Add `PomodoroConfig` struct**

Insert after `FocusBubbleConfig`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PomodoroConfig {
    #[serde(default = "default_pomodoro_work_mins")]
    pub work_mins: u64,
    #[serde(default = "default_pomodoro_short_break_mins")]
    pub short_break_mins: u64,
    #[serde(default = "default_pomodoro_long_break_mins")]
    pub long_break_mins: u64,
    #[serde(default = "default_pomodoro_long_break_after")]
    pub long_break_after: u64,
    #[serde(default)]
    pub auto_start_work: bool,
    #[serde(default)]
    pub auto_start_break: bool,
}
```

- [ ] **Step 2: Add defaults and embed in `ProductivityConfig`**

Add default functions:

```rust
fn default_pomodoro_work_mins() -> u64 { 25 }
fn default_pomodoro_short_break_mins() -> u64 { 5 }
fn default_pomodoro_long_break_mins() -> u64 { 15 }
fn default_pomodoro_long_break_after() -> u64 { 4 }
```

Add field to `ProductivityConfig`:

```rust
#[serde(default)]
pub pomodoro: PomodoroConfig,
```

- [ ] **Step 3: Add `Default` impl**

```rust
impl Default for PomodoroConfig {
    fn default() -> Self {
        Self {
            work_mins: default_pomodoro_work_mins(),
            short_break_mins: default_pomodoro_short_break_mins(),
            long_break_mins: default_pomodoro_long_break_mins(),
            long_break_after: default_pomodoro_long_break_after(),
            auto_start_work: false,
            auto_start_break: false,
        }
    }
}
```

- [ ] **Step 4: Build config crate**

```bash
cargo check -p config
```

Expected: passes.

- [ ] **Step 5: Commit**

```bash
git add crates/config/src/schema/productivity.rs
git commit -m "feat(config): add PomodoroConfig to productivity schema"
```

### Task 3.2: Add Tauri commands for focus defaults

**Files:**
- Modify: `crates/desktop/src/commands/productivity.rs`
- Modify: `crates/desktop/src/lib.rs` command registration

- [ ] **Step 1: Add command structs/functions**

In `crates/desktop/src/commands/productivity.rs`, add:

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FocusDefaultsResponse {
    pub work_mins: u64,
    pub short_break_mins: u64,
    pub long_break_mins: u64,
    pub long_break_after: u64,
    pub auto_start_work: bool,
    pub auto_start_break: bool,
}

#[tauri::command]
pub async fn focus_defaults_get(state: tauri::State<'_, Arc<AppCore>>) -> Result<FocusDefaultsResponse, ApiError> {
    let config = state.config.read().await;
    let pomodoro = &config.productivity.focus.pomodoro;
    Ok(FocusDefaultsResponse {
        work_mins: pomodoro.work_mins,
        short_break_mins: pomodoro.short_break_mins,
        long_break_mins: pomodoro.long_break_mins,
        long_break_after: pomodoro.long_break_after,
        auto_start_work: pomodoro.auto_start_work,
        auto_start_break: pomodoro.auto_start_break,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FocusDefaultsUpdate {
    pub work_mins: Option<u64>,
    pub short_break_mins: Option<u64>,
    pub long_break_mins: Option<u64>,
    pub long_break_after: Option<u64>,
    pub auto_start_work: Option<bool>,
    pub auto_start_break: Option<bool>,
}

#[tauri::command]
pub async fn focus_defaults_set(
    state: tauri::State<'_, Arc<AppCore>>,
    update: FocusDefaultsUpdate,
) -> Result<FocusDefaultsResponse, ApiError> {
    use serde_json::json;

    let patch = json!({
        "focus": {
            "pomodoro": {
                "workMins": update.work_mins,
                "shortBreakMins": update.short_break_mins,
                "longBreakMins": update.long_break_mins,
                "longBreakAfter": update.long_break_after,
                "autoStartWork": update.auto_start_work,
                "autoStartBreak": update.auto_start_break,
            }
        }
    });
    state.config_update_section("productivity".into(), patch).await?;
    focus_defaults_get(state).await
}
```

- [ ] **Step 2: Register commands**

In `crates/desktop/src/lib.rs`, add `focus_defaults_get` and `focus_defaults_set` to the `tauri::generate_handler!` macro list.

- [ ] **Step 3: Regenerate bindings**

```bash
cargo run -p desktop --bin generate-bindings
```

Or the equivalent script in the project. Verify `focusDefaultsGet` and `focusDefaultsSet` appear in `desktop-ui/src/bindings.ts`.

- [ ] **Step 4: Commit**

```bash
git add crates/config/src/schema/productivity.rs \
  crates/desktop/src/commands/productivity.rs \
  crates/desktop/src/lib.rs \
  desktop-ui/src/bindings.ts
git commit -m "feat(focus): add focus defaults commands"
```

### Task 3.3: Load and save settings from backend in `useFocusTimer`

**Files:**
- Modify: `desktop-ui/src/features/focus/hooks/useFocusTimer.ts`
- Modify: `desktop-ui/src/lib/query/queryKeys.ts`

- [ ] **Step 0: Add `focus.defaults` query key**

In `desktop-ui/src/lib/query/queryKeys.ts`, add inside the `focus` object:

```typescript
focus: {
  all: () => ["focus"] as const,
  status: () => ["focus", "status"] as const,
  todaySessions: () => ["focus", "todaySessions"] as const,
  defaults: () => ["focus", "defaults"] as const,
},
```

- [ ] **Step 1: Add defaults query**

Add near the top of `useFocusTimer`:

```typescript
const defaultsQuery = useTauriQuery<{
  workMins: number;
  shortBreakMins: number;
  longBreakMins: number;
  longBreakAfter: number;
}>({
  queryKey: qk.focus.defaults(),
  command: "focus_defaults_get",
  fallback: {
    workMins: 25,
    shortBreakMins: 5,
    longBreakMins: 15,
    longBreakAfter: 4,
  },
});
```

- [ ] **Step 2: Initialize settings from backend defaults**

Update `DEFAULT_SETTINGS` merge logic:

```typescript
const backendDefaults = defaultsQuery.data;
const initialSettings: FocusSettings = {
  ...DEFAULT_SETTINGS,
  ...(backendDefaults && {
    focusDuration: backendDefaults.workMins,
    shortBreak: backendDefaults.shortBreakMins,
    longBreak: backendDefaults.longBreakMins,
    longBreakAfter: backendDefaults.longBreakAfter,
  }),
  ...loadSettings(), // local override takes precedence
};
const [settings, setSettings] = useState(initialSettings);
```

- [ ] **Step 3: Persist overrides to backend**

Add a mutation and update `updateSettings`:

```typescript
const saveDefaultsMut = useTauriMutation<unknown, Partial<FocusDefaultsUpdate>>({
  command: "focus_defaults_set",
});

const updateSettings = useCallback((partial: Partial<FocusSettings>) => {
  setSettings((prev) => {
    const next = { ...prev, ...partial };
    saveSettings(next);
    saveDefaultsMut.mutate({
      workMins: next.focusDuration,
      shortBreakMins: next.shortBreak,
      longBreakMins: next.longBreak,
      longBreakAfter: next.longBreakAfter,
    });
    return next;
  });
}, [saveDefaultsMut]);
```

- [ ] **Step 4: Type-check and commit**

```bash
cd desktop-ui && npm run typecheck
```

```bash
git add desktop-ui/src/features/focus/hooks/useFocusTimer.ts \
  desktop-ui/src/lib/query/queryKeys.ts
git commit -m "feat(focus): sync settings with backend config"
```

---

## Phase 4: Drift-Correcting Timer Engine

### Task 4.1: Refactor `focus_timer.rs` loop to use absolute `Instant` targets

**Files:**
- Modify: `crates/desktop/src/focus_timer.rs`

- [ ] **Step 1: Replace `tokio::time::interval` with `Instant` target sleep**

In `session_loop`, remove:

```rust
let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(1));
```

Add after `let mut sync_counter = 0;`:

```rust
use std::time::{Duration, Instant};
let tick_duration = Duration::from_secs(1);
let mut tick_count: u64 = 0;
let segment_start = Instant::now();
```

Replace `interval.tick().await;` with:

```rust
let next_tick = segment_start + tick_duration * (tick_count + 1);
let sleep_for = next_tick.saturating_duration_since(Instant::now());
tokio::time::sleep(sleep_for).await;
tick_count += 1;
```

- [ ] **Step 2: Reset `tick_count` on phase transitions**

On each phase change (Working → BreakPending, BreakPending → Break, Break → Working, Extend, Suspend resume), reset `tick_count = 0` and recalculate `segment_start = Instant::now()` for the new segment.

For example, after `phase = Phase::BreakPending { ... }`, add:

```rust
segment_start = Instant::now();
tick_count = 0;
```

Do the same for all phase mutations.

- [ ] **Step 3: Add regression test**

Add a test that simulates a late wakeup:

```rust
#[tokio::test]
async fn drift_correction_keeps_timing_accurate() {
    use std::time::{Duration, Instant};
    let start = Instant::now();
    let tick_duration = Duration::from_millis(100);
    let mut tick_count = 0;
    for _ in 0..10 {
        let next = start + tick_duration * (tick_count + 1);
        tokio::time::sleep(next.saturating_duration_since(Instant::now())).await;
        tick_count += 1;
    }
    let elapsed = start.elapsed();
    // Should be close to 1000ms, not 10 * (100ms + overhead)
    assert!(elapsed >= Duration::from_millis(950), "elapsed: {elapsed:?}");
    assert!(elapsed < Duration::from_millis(1150), "elapsed: {elapsed:?}");
}
```

- [ ] **Step 4: Run tests**

```bash
cargo test -p desktop focus_timer
```

Expected: all tests pass.

- [ ] **Step 5: Commit**

```bash
git add crates/desktop/src/focus_timer.rs
git commit -m "feat(focus): drift-correcting timer engine"
```

---

## Phase 5: Cross-Platform Custom Audio

### Task 5.1: Add `rodio` dependency and create audio manager

**Files:**
- Modify: `crates/desktop/Cargo.toml`
- Create: `crates/desktop/src/focus_audio.rs`

- [ ] **Step 1: Add `rodio` to desktop crate**

```toml
[dependencies]
rodio = { version = "0.19", default-features = false, features = ["wav", "mp3"] }
```

- [ ] **Step 2: Create `focus_audio.rs`**

```rust
//! Focus alert audio manager.
//!
//! Plays embedded default MP3s or user-supplied custom files.

use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::sync::Arc;

use rodio::{Decoder, OutputStream, OutputStreamHandle, Sink};
use tauri::AppHandle;
use tracing::warn;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusCue {
    WorkComplete,
    BreakComplete,
}

impl FocusCue {
    fn embedded_bytes(&self) -> &'static [u8] {
        match self {
            FocusCue::WorkComplete => include_bytes!("../../assets/audio/focus-work-complete.mp3"),
            FocusCue::BreakComplete => include_bytes!("../../assets/audio/focus-break-complete.mp3"),
        }
    }

    fn default_filename(&self) -> &'static str {
        match self {
            FocusCue::WorkComplete => "focus-work-complete.mp3",
            FocusCue::BreakComplete => "focus-break-complete.mp3",
        }
    }
}

pub struct FocusAudioManager {
    app_data_dir: PathBuf,
}

impl FocusAudioManager {
    pub fn new(app: &AppHandle) -> Self {
        let app_data_dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from("."));
        Self { app_data_dir }
    }

    pub fn play(&self, cue: FocusCue, volume: f32) {
        let path = self.app_data_dir.join("audio").join(cue.default_filename());
        let bytes = if path.exists() {
            match fs::read(&path) {
                Ok(b) => b,
                Err(e) => {
                    warn!("Failed to read custom focus audio {:?}: {e}", path);
                    cue.embedded_bytes().to_vec()
                }
            }
        } else {
            cue.embedded_bytes().to_vec()
        };

        // Spawn a thread so the async timer loop is not blocked.
        std::thread::spawn(move || {
            if let Err(e) = play_bytes(bytes, volume) {
                warn!("Failed to play focus audio: {e}");
            }
        });
    }
}

fn play_bytes(bytes: Vec<u8>, volume: f32) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (_stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    let source = Decoder::new(Cursor::new(bytes))?;
    sink.set_volume(volume.clamp(0.0, 1.0));
    sink.append(source);
    sink.sleep_until_end();
    Ok(())
}
```

- [ ] **Step 3: Add default audio assets**

Create placeholder MP3 files:
- `crates/desktop/assets/audio/focus-work-complete.mp3`
- `crates/desktop/assets/audio/focus-break-complete.mp3`

Use short silent or simple beep MP3s. If you don't have audio files, use an empty WAV or generate one with a script.

- [ ] **Step 4: Register audio manager in `lib.rs`**

Add to Tauri setup:

```rust
.manage(Arc::new(focus_audio::FocusAudioManager::new(&app)))
```

- [ ] **Step 5: Replace `afplay` calls in `focus_timer.rs`**

In `on_work_complete` and `on_break_complete`, replace the macOS-only blocks with:

```rust
if sound_enabled {
    if let Some(audio) = app.try_state::<Arc<focus_audio::FocusAudioManager>>() {
        audio.play(focus_audio::FocusCue::WorkComplete, 0.8);
    }
}
```

and for break:

```rust
if sound_enabled {
    if let Some(audio) = app.try_state::<Arc<focus_audio::FocusAudioManager>>() {
        audio.play(focus_audio::FocusCue::BreakComplete, 0.8);
    }
}
```

- [ ] **Step 6: Build desktop crate**

```bash
cargo check -p desktop
```

Expected: passes.

- [ ] **Step 7: Commit**

```bash
git add crates/desktop/Cargo.toml \
  crates/desktop/src/focus_audio.rs \
  crates/desktop/src/lib.rs \
  crates/desktop/src/focus_timer.rs \
  crates/desktop/assets/audio/
git commit -m "feat(focus): cross-platform custom alert audio"
```

### Task 5.2: Add commands to set custom sounds

**Files:**
- Modify: `crates/desktop/src/commands/productivity.rs`
- Modify: `crates/desktop/src/lib.rs`

- [ ] **Step 1: Add sound file commands**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FocusSoundCuesResponse {
    pub cues: Vec<FocusSoundCue>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FocusSoundCue {
    pub id: String,
    pub label: String,
    pub has_custom: bool,
}

#[tauri::command]
pub async fn focus_sound_cues(
    app: AppHandle,
) -> Result<FocusSoundCuesResponse, ApiError> {
    let mgr = FocusAudioManager::new(&app);
    let cues = vec![
        FocusSoundCue { id: "work_complete".into(), label: "Work complete".into(), has_custom: has_custom_sound(&app, FocusCue::WorkComplete) },
        FocusSoundCue { id: "break_complete".into(), label: "Break complete".into(), has_custom: has_custom_sound(&app, FocusCue::BreakComplete) },
    ];
    Ok(FocusSoundCuesResponse { cues })
}

fn has_custom_sound(app: &AppHandle, cue: FocusCue) -> bool {
    let dir = app.path().app_data_dir().unwrap_or_else(|_| PathBuf::from(".")).join("audio");
    dir.join(cue.default_filename()).exists()
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FocusSetCustomSoundPayload {
    pub cue: String,
    pub source_path: String,
}

#[tauri::command]
pub async fn focus_set_custom_sound(
    app: AppHandle,
    payload: FocusSetCustomSoundPayload,
) -> Result<(), ApiError> {
    use std::io::Write;
    let cue = match payload.cue.as_str() {
        "work_complete" => FocusCue::WorkComplete,
        "break_complete" => FocusCue::BreakComplete,
        _ => return Err(ApiError::BadRequest(format!("Unknown cue: {}", payload.cue))),
    };
    let ext = std::path::Path::new(&payload.source_path)
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .unwrap_or_default();
    if !matches!(ext.as_str(), "mp3" | "wav" | "ogg") {
        return Err(ApiError::BadRequest("Only MP3, WAV, OGG are supported".into()));
    }

    let data = std::fs::read(&payload.source_path)
        .map_err(|e| ApiError::Internal(format!("Failed to read sound file: {e}")))?;

    // Probe with rodio before committing.
    if let Err(e) = rodio::Decoder::new(std::io::Cursor::new(&data)) {
        return Err(ApiError::BadRequest(format!("Invalid audio file: {e}")));
    }

    let dest_dir = app.path().app_data_dir()
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .join("audio");
    std::fs::create_dir_all(&dest_dir)
        .map_err(|e| ApiError::Internal(format!("Failed to create audio dir: {e}")))?;
    let dest = dest_dir.join(cue.default_filename());
    let mut file = std::fs::File::create(&dest)
        .map_err(|e| ApiError::Internal(format!("Failed to create sound file: {e}")))?;
    file.write_all(&data)
        .map_err(|e| ApiError::Internal(format!("Failed to write sound file: {e}")))?;
    Ok(())
}

#[tauri::command]
pub async fn focus_reset_custom_sound(
    app: AppHandle,
    cue: String,
) -> Result<(), ApiError> {
    let cue = match cue.as_str() {
        "work_complete" => FocusCue::WorkComplete,
        "break_complete" => FocusCue::BreakComplete,
        _ => return Err(ApiError::BadRequest(format!("Unknown cue: {cue}"))),
    };
    let dest = app.path().app_data_dir()
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .join("audio")
        .join(cue.default_filename());
    if dest.exists() {
        let _ = std::fs::remove_file(&dest);
    }
    Ok(())
}
```

- [ ] **Step 2: Register commands and regenerate bindings**

Add commands to `lib.rs` `generate_handler!` and run bindings generation.

- [ ] **Step 3: Commit**

```bash
git add crates/desktop/src/commands/productivity.rs crates/desktop/src/lib.rs desktop-ui/src/bindings.ts
git commit -m "feat(focus): custom sound file commands"
```

---

## Phase 6: Cross-Platform Notifications

### Task 6.1: Improve notification fallback

**Files:**
- Modify: `crates/desktop/src/notify.rs` (or create `crates/desktop/src/focus_notify.rs`)
- Modify: `crates/desktop/src/focus_timer.rs`

- [ ] **Step 1: Create notification helper**

If `crates/desktop/src/notify.rs` exists, extend it. Otherwise create `focus_notify.rs`:

```rust
use tauri::AppHandle;
use tracing::warn;

pub fn send_focus_notification(app: &AppHandle, title: &str, body: &str) {
    let sender = crate::notify::TauriNotificationSender::new(app.clone());
    if let Err(e) = sender.send_sync(title, body) {
        warn!("Tauri notification failed: {e}");
        #[cfg(target_os = "linux")]
        {
            let _ = std::process::Command::new("notify-send")
                .arg(title)
                .arg(body)
                .spawn();
        }
    }
}
```

- [ ] **Step 2: Use helper in `focus_timer.rs`**

Replace `TauriNotificationSender::new(...).send_sync(...)` calls with:

```rust
crate::focus_notify::send_focus_notification(app, "Focus Session Complete", &body);
```

and break:

```rust
crate::focus_notify::send_focus_notification(app, "Break Over", "Ready for the next focus session!");
```

- [ ] **Step 3: Build and commit**

```bash
cargo check -p desktop
```

```bash
git add crates/desktop/src/focus_notify.rs crates/desktop/src/focus_timer.rs
git commit -m "feat(focus): cross-platform notification fallback"
```

---

## Phase 7: Remove Local Tick Drift

### Task 7.1: Drive display from sync events only

**Files:**
- Modify: `desktop-ui/src/features/focus/hooks/useFocusTimer.ts`

- [ ] **Step 1: Remove local `setInterval` tick**

Delete:

```typescript
const [localTick, setLocalTick] = useState(0);
const isRunning = !!serverState && !serverState.paused;
useEffect(() => {
  if (!isRunning) return;
  const id = setInterval(() => setLocalTick((t) => t + 1), 1000);
  return () => clearInterval(id);
}, [isRunning]);

useEffect(() => {
  setLocalTick(0);
}, []);
```

- [ ] **Step 2: Use server remaining directly**

Change:

```typescript
const remainingSecs = useMemo(() => {
  if (!serverState || !isActive) return null;
  const elapsed = localTick;
  return Math.max(0, serverState.remainingSecs - elapsed);
}, [serverState, isActive, localTick]);
```

to:

```typescript
const remainingSecs = useMemo(() => {
  if (!serverState || !isActive) return null;
  return Math.max(0, serverState.remainingSecs);
}, [serverState, isActive]);
```

- [ ] **Step 3: Type-check and commit**

```bash
cd desktop-ui && npm run typecheck
```

```bash
git add desktop-ui/src/features/focus/hooks/useFocusTimer.ts
git commit -m "feat(focus): remove local tick drift"
```

---

## Phase 8: Integration & Tests

### Task 8.1: Add frontend route/view tests

**Files:**
- Create: `desktop-ui/src/features/focus/components/__tests__/FocusTimer.test.tsx`
- Create: `desktop-ui/src/features/focus/hooks/__tests__/useFocusTimer.test.ts` (or `.tsx`)

- [ ] **Step 1: Add FocusTimer display test**

```typescript
import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FocusTimer } from "../FocusTimer";

const makeTimer = (overrides = {}) => ({
  phase: "idle" as const,
  paused: false,
  active: false,
  remainingSecs: null,
  totalSecs: null,
  actionTitle: null,
  showWarning: false,
  dndHint: null,
  coaching: null,
  settings: {
    focusDuration: 25,
    shortBreak: 5,
    longBreak: 15,
    longBreakAfter: 4,
    dndEnabled: false,
    soundEnabled: true,
    notificationEnabled: true,
  },
  completedSessions: 0,
  cyclePosition: 0,
  longBreakAfter: 4,
  todayStats: { sessions: 0, totalMins: 0, avgQuality: null },
  activePreset: "Custom",
  isLoading: false,
  start: vi.fn(),
  stop: vi.fn(),
  pause: vi.fn(),
  resume: vi.fn(),
  extend: vi.fn(),
  startBreak: vi.fn(),
  extendWork: vi.fn(),
  skipBreak: vi.fn(),
  takeBreak: vi.fn(),
  logDistraction: vi.fn(),
  updateSettings: vi.fn(),
  applyPreset: vi.fn(),
  dismissCoaching: vi.fn(),
  dismissDndHint: vi.fn(),
  selectTask: vi.fn(),
  selectedTaskId: null,
  selectedTaskTitle: null,
  ...overrides,
});

describe("FocusTimer", () => {
  it("renders idle state with start button", () => {
    render(<FocusTimer timer={makeTimer() as any} onOpenSettings={vi.fn()} />);
    expect(screen.getByRole("button", { name: /start/i })).toBeInTheDocument();
  });
});
```

- [ ] **Step 2: Run tests**

```bash
cd desktop-ui && npm test -- features/focus/components/__tests__/FocusTimer.test.tsx
```

Expected: passes.

- [ ] **Step 3: Commit**

```bash
git add desktop-ui/src/features/focus/components/__tests__/
git commit -m "test(focus): add FocusTimer view tests"
```

### Task 8.2: Run full integration checks

**Files:**
- Run in: `/Users/jayden/Projects/Klynt/bot`

- [ ] **Step 1: Type-check frontend**

```bash
cd desktop-ui && npm run typecheck
```

- [ ] **Step 2: Run Rust tests**

```bash
cargo test -p desktop focus
```

- [ ] **Step 3: Build desktop app**

```bash
cargo build -p desktop
```

Expected: builds successfully.

- [ ] **Step 4: Commit final state**

```bash
git add -A
git commit -m "feat(focus): complete Pomotroid port integration"
```

---

## Self-Review

### Spec Coverage

| Spec Requirement | Task(s) |
|------------------|---------|
| Add `/focus` main-window view | Phase 2 |
| Extract shared focus module | Phase 1 |
| Sync settings with backend `FocusConfig` | Phase 3 |
| Drift-correcting timer engine | Phase 4 |
| Cross-platform custom audio | Phase 5 |
| Cross-platform notifications | Phase 6 |
| Remove local tick drift | Phase 7 |
| Tests | Phase 8 |

### Placeholder Scan

- No "TBD", "TODO", or "implement later" remain.
- Each task has concrete file paths, code snippets, and commands.
- Each test task includes actual test code.

### Type Consistency

- `FocusSyncPayload.phase` now includes `"suspended"` in TypeScript to match Rust.
- `FocusSettings` shape is consistent across hook, settings panel, and backend defaults.
- Command names (`focus_defaults_get`, `focus_defaults_set`) match between Rust and frontend `useTauriQuery`/`useTauriMutation` usage.

---

## Execution Handoff

**Plan complete and saved to `docs/superpowers/plans/2026-06-16-pomotroid-focus-port.md`.**

**Two execution options:**

**1. Subagent-Driven (recommended)** — Dispatch a fresh subagent per task, review between tasks, fast iteration.

**2. Inline Execution** — Execute tasks in this session using `executing-plans`, batch execution with checkpoints.

**Which approach?**
