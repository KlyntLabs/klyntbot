# Pomotroid Focus/Pomodoro Port — Design Spec

**Date:** 2026-06-16
**Status:** Approved (pending written-spec review)
**Approach:** Hybrid port — Pomotroid-inspired refinements layered on top of existing tray UI and backend

## 1. Background

Pomotroid is a polished, open-source Pomodoro desktop application (Tauri + SvelteKit) with a complete timer engine, settings system, audio, tray integration, and statistics. Our project (`Klynt/bot`) already has a working focus/Pomodoro subsystem in the tray UI (`desktop-ui/src/features/tray/`) and a mature Rust backend (`feature-focus`, `feature-productivity`, `desktop::focus_timer`). This spec defines how to adapt Pomotroid's proven polish to fill the remaining gaps without throwing away existing work.

## 2. Goals

- Add a dedicated `/focus` main-window view (currently focus UI only exists in the tray popup).
- Sync tray focus settings with the backend `FocusConfig` so defaults persist across devices/reinstalls.
- Make the backend timer drift-correcting (Pomotroid-style absolute `Instant` scheduling).
- Replace hardcoded macOS `afplay` sounds with cross-platform, customizable alert audio.
- Improve notification handling with cross-platform fallback and configurable text.
- Preserve existing Klynt-specific focus features: macOS DND, distraction detection, agent tooling, task-focus deadlines.

## 3. Non-Goals

- Do not replace our DND, distraction monitoring, or agent-tooling layers.
- Do not port Pomotroid's statistics/charts as a first-class feature (existing intelligence/session tables remain primary).
- Do not port the full theme engine; reuse the existing design system.
- Do not add a WebSocket overlay API in this phase.
- Do not add Linux/Windows native DND in this phase (document the macOS limitation).

## 4. Current State

### 4.1 Pomotroid (reference)

- **Stack:** Tauri 2 + Rust backend, SvelteKit/Svelte 5 frontend.
- **Timer engine:** Dedicated OS thread, drift-correcting scheduling via absolute `Instant` targets.
- **Sequence logic:** Work → ShortBreak → Work; LongBreak after configurable work-round count.
- **Settings:** SQLite key/value, single-key save model, side effects applied centrally.
- **UI:** Timer dial, compact mode, per-round colors, settings sidebar, statistics.
- **Audio/Tray/Notifications:** Embedded MP3 alerts, dynamic tray icon, OS notifications.

### 4.2 Our Project

- **Tray UI exists and is functional:**
  - `desktop-ui/src/features/tray/components/FocusControl.tsx` — timer dial, controls, presets, settings panel.
  - `desktop-ui/src/features/tray/hooks/useFocusTimer.ts` — hooks into all `focusSession*` commands/events.
  - `desktop-ui/src/features/tray/components/Tray.tsx` — integrates `FocusControl` into the tray popup.
- **Backend is strong:** `feature-focus` (DND), `feature-productivity` (FocusManager, distraction monitoring), `desktop::focus_timer` (phase state machine), `app-core` handlers, `TemporalScheduler` alarms.
- **Backend gaps:**
  - Timer uses naive `tokio::time::interval(Duration::from_secs(1))` — vulnerable to drift on late wakeups.
  - Completion sounds are hardcoded to macOS `afplay /System/Library/Sounds/*.aiff`.
  - No custom audio file support.
  - Notification fallback for Linux/Windows is not explicit.
- **Frontend gaps:**
  - Focus UI is tray-only; there is no `/focus` main-window route.
  - Focus settings (`FocusSettings`) are stored only in `localStorage`, not synced to backend `FocusConfig`.
  - The UI adds its own `setInterval` local tick on top of 5-second backend syncs, introducing display drift.

## 5. Approach

**Hybrid port (recommended).**

1. **Frontend:** Extract reusable focus components from the tray into a shared `features/focus/` module; add a `/focus` main-window route; sync settings with backend config.
2. **Backend:** Refactor `FocusTimer` tick scheduling to use absolute `Instant` targets; add a cross-platform audio manager with embedded defaults and custom-file support; improve notification dispatch.

## 6. Architecture

```
┌─────────────────────────────────────────────┐
│              desktop-ui (React)             │
│  ┌─────────────┐  ┌──────────────────────┐  │
│  │ /focus      │  │ tray window          │  │
│  │ FocusPage   │  │ FocusControl         │  │
│  │             │  │                      │  │
│  └──────┬──────┘  └──────────┬───────────┘  │
│         │                    │              │
│  ┌──────┴────────────────────┴──────┐       │
│  │     features/focus/*              │       │
│  │  useFocusTimer, FocusDial, etc.   │       │
│  └───────────────┬───────────────────┘       │
└──────────────────┼──────────────────────────┘
                   │ Tauri commands/events
┌──────────────────┼──────────────────────────┐
│              Rust Backend                   │
│  ┌───────────────┴───────────────┐          │
│  │     desktop::FocusTimer       │          │
│  │  + drift-correcting engine    │          │
│  │  + custom audio playback      │          │
│  └───────────────┬───────────────┘          │
│                  │                          │
│  ┌───────────────┼───────────────┐          │
│  │ feature-focus │ feature-prod. │          │
│  │   DNDManager  │ FocusManager  │          │
│  │   FocusBridge │ Distraction   │          │
│  └───────────────┴───────────────┘          │
└─────────────────────────────────────────────┘
```

- `desktop-ui` is the presentation layer only.
- Rust backend remains the single source of truth for timer state, sessions, and DND.
- Timer state is event-driven via `focus:sync` (~1 Hz) and `focus:phase_changed`.

## 7. Frontend Components

### 7.1 New `/focus` Route

- Add `FocusPage` component served from the main window.
- Reuse the same dial, controls, and settings extracted from tray.
- Accessible from the main app navigation.

### 7.2 Shared `features/focus/` Module

Move the following from `features/tray/` to `features/focus/` so both `/focus` and tray can import them:

| File | Responsibility |
|------|----------------|
| `features/focus/types.ts` | Shared focus TypeScript types (`FocusPhase`, `FocusSettings`, etc.). |
| `features/focus/hooks/useFocusTimer.ts` | IPC commands/events + local state (remove local `setInterval` drift). |
| `features/focus/components/FocusDial.tsx` | SVG progress ring with phase-based color. |
| `features/focus/components/FocusDisplay.tsx` | `MM:SS` remaining time + phase label. |
| `features/focus/components/FocusControls.tsx` | Start/pause/resume/skip/restart buttons. |
| `features/focus/components/FocusSettingsPanel.tsx` | Duration/sound/notification/DND settings. |

### 7.3 Tray Refactor

- `features/tray/components/FocusControl.tsx` becomes a thin wrapper around `features/focus/*` components.
- Keep tray-specific UI (today's tasks, calendar, footer) in `Tray.tsx`.

### 7.4 Settings Sync

- Replace `localStorage`-only settings in `useFocusTimer` with a hybrid:
  - Load defaults from backend `FocusConfig` on mount.
  - Persist user overrides back to backend config via an existing or new command.
- Keep local edits instant; debounce save to backend.

### 7.5 Remove Local Tick Drift

- Stop using a 1-second `setInterval` to locally decrement `remainingSecs`.
- Drive the display entirely from `focus:sync` / `focus:phase_changed` payloads.
- Use CSS transitions or a lightweight requestAnimationFrame-based smoothing between syncs (no time subtraction).

## 8. Backend Changes

### 8.1 Drift-Correcting Timer Engine

Refactor `crates/desktop/src/focus_timer.rs` to compute absolute `Instant` targets for each tick:

```rust
let next_tick = segment_start + tick_interval * (tick_count + 1);
sleep(next_tick.saturating_duration_since(Instant::now()));
```

This prevents cumulative drift from late wakeups and matches Pomotroid's engine. Keep the existing 5-second sync cadence for UI traffic but ensure internal timing is accurate.

### 8.2 Custom Alert Audio

Replace hardcoded macOS `afplay` calls with a cross-platform audio manager:

- Embed default MP3s for work/break alerts (Pomotroid-style).
- Allow users to set custom MP3/WAV/OGG files per cue.
- Store custom files in the app data directory with fixed stems.
- Use a Rust audio crate compatible with existing stack (e.g., `rodio`).
- Add Tauri commands: `focus_set_custom_sound`, `focus_reset_custom_sound`, `focus_list_sound_cues`.

### 8.3 Persisted Default Pomodoro Config

Extend `crates/config/src/schema/productivity.rs` with Pomodoro-specific defaults:

```rust
pub struct PomodoroConfig {
    pub work_mins: u32,          // default 25
    pub short_break_mins: u32,   // default 5
    pub long_break_mins: u32,    // default 15
    pub long_break_after: u32,   // default 4
    pub auto_start_work: bool,   // default false
    pub auto_start_break: bool,  // default false
}
```

Expose via:
- A new `get_focus_defaults` Tauri command (or reuse config read path).
- A new `set_focus_defaults` command to update `FocusConfig`.
- `useFocusTimer` loads these on mount and falls back to tray local defaults only if the backend returns none.

### 8.4 Cross-Platform Notifications

- Keep Tauri native notification for macOS/Windows.
- Add Linux `notify-send` fallback if Tauri notification fails.
- Make notification title/body configurable and localizable (can be hardcoded English first, i18n later).

## 9. Data Flow

1. **App start:** UI fetches focus defaults from backend config + loads any tray local override.
2. **Start:** User clicks Start → `focusSessionStart(params)` → Rust creates `SessionState`, starts drift-corrected loop, emits `focus:phase_changed`.
3. **Ticking:** Backend emits `focus:sync` every 5 seconds (and every 1s during `BreakPending`). UI updates display directly from payload.
4. **Phase Transition:** Backend detects work complete → emits final work sync + `focus:phase_changed` for break. UI updates + shows toast/notification.
5. **Pause/Resume:** User clicks Pause → `focusSessionPause` → backend pauses engine, emits paused `focus:phase_changed`.
6. **Completion:** Final phase complete → backend emits completion sync + notification + opens tray window.
7. **Settings Change:** UI saves config → backend updates `FocusConfig` → next session uses new values (idle-only reconfiguration).

## 10. Error Handling

| Scenario | Handling |
|----------|----------|
| `focus:dnd_unavailable` | Show non-blocking helper card with manual DND steps. |
| `focus:warning` (30 s left) | Show subtle toast + tray title pulse. |
| IPC command failure | Log, show error toast, enable `focusSessionStatus` polling fallback. |
| Event stream lost | `useFocusTimer` falls back to `focusSessionStatus` polling every 5 s. |
| Custom audio decode failure | Revert to embedded default, surface warning in settings. |
| Backend config load failure | Use tray local defaults, do not block UI. |

## 11. Testing

### 11.1 Rust

- Unit tests for drift-correcting timer engine (simulate late wakeups).
- Unit tests for Pomodoro sequence advancement (existing tests extended).
- Unit tests for custom audio file probing and fallback.
- Unit tests for notification fallback dispatch.

### 11.2 Frontend

- Component tests for `FocusDial` progress calculation.
- Component tests for `FocusDisplay` formatting.
- Hook tests for event-to-state mapping.
- Route test for `/focus` rendering.

### 11.3 E2E

- Start a focus session, wait for a tick, pause, resume, skip break, complete.
- Verify `/focus` and tray routes render correctly.
- Verify settings changes persist across app restart.

## 12. Risks & Mitigations

| Risk | Mitigation |
|------|------------|
| Backend refactor breaks DND/distraction integration | Keep `FocusManager`/`DndManager` calls unchanged; only refactor tick scheduling. |
| New audio crate adds heavy dependencies | Evaluate `rodio` vs lighter alternatives; gate behind feature flag if needed. |
| Frontend state gets out of sync with backend | Remove local tick drift; use event-driven sync + polling fallback. |
| Shared `features/focus/` module breaks existing tray | Refactor incrementally; keep tray `FocusControl` as wrapper during transition. |

## 13. Success Criteria

- User can open `/focus` in the main window and start/pause/resume/skip a focus session.
- Tray focus UI continues to work and shares code with `/focus`.
- Focus settings persist in backend config and survive app restart.
- Timer remains accurate after OS sleep/wake.
- Custom audio files can be selected and played on all platforms.
- Existing Klynt features (DND, distraction detection, agent tools) continue to work.

## 14. Out of Scope (Future Work)

- Pomotroid-style statistics dashboard (reuse existing intelligence tables).
- Full theme engine port.
- WebSocket overlay API.
- Linux/Windows native DND.
- Per-language i18n for notification text.
