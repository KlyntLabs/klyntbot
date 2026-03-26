# Focus Mode Redesign: Backend-Owned Lifecycle + DND Integration

**Date:** 2026-03-26
**Status:** Approved
**Scope:** Desktop timer rewrite, DND wiring, frontend simplification

## Problem

Focus Mode works but has structural issues that will compound over time:

1. **Split ownership** — Frontend owns cycle logic (which break is next), backend just runs timers. This creates a hidden contract prone to desync.
2. **Heavy tick loop** — 1-second backend ticks emit events + update tray title every second. Unnecessary overhead for long sessions.
3. **DND is cosmetic** — Toggle exists in UI but does nothing on macOS.
4. **Fragile break_pending** — 5-second auto-transition lives in a React `setTimeout` that can misfire on unmount/remount.

## Design Decisions

| Decision | Choice | Rationale |
|---|---|---|
| Cycle ownership | Backend owns (approach B: decides, frontend confirms) | Single source of truth, eliminates desync |
| Tick frequency | 5-second backend sync + 1-second frontend local countdown (approach A: hybrid) | 80% fewer events, smooth UI via local interpolation |
| DND mechanism | `shortcuts run` for toggle + `defaults read` for state (approach A) | No private APIs, official automation layer |
| DND restore | Capture-and-restore (approach B) | Respects pre-existing DND state |
| Break_pending | Backend-owned phase with 5s countdown (approach A) | Resilient to UI unmount, consistent with backend-owns-lifecycle |
| Auto-continue after break | Backend auto-starts next focus when break ends | Continuous Pomodoro rhythm until user stops |

## Backend Phase State Machine

### FocusSession State

```rust
struct FocusSessionConfig {
    work_secs: u64,
    short_break_secs: u64,
    long_break_secs: u64,
    long_break_after: u32,        // e.g. 4
}

struct FocusSession {
    config: FocusSessionConfig,
    cycle_position: u32,          // 0-based, incremented after each Working completes
    phase: Phase,
    dnd_enabled: bool,
    dnd_was_active_before: bool,  // for capture-and-restore
    sound_enabled: bool,
    notification_enabled: bool,
    action_title: Option<String>,
    action_id: Option<String>,
}

enum Phase {
    Working { remaining: u64, total: u64 },
    BreakPending { remaining: u64 },        // 5-second countdown
    Break { remaining: u64, total: u64 },
    Paused { was_phase: Box<Phase> },
}
```

### Phase Transitions

```
Idle
  → start() → Working(work_secs)
  → DND: if enabled, snapshot current state, toggle on if needed

Working
  → remaining hits 0 → BreakPending(5s), play completion sound/notification
  → stop() → Idle
  → pause() → Paused { was: Working }
  → take_break() → BreakPending(0s, immediate transition)

BreakPending
  → remaining hits 0 → Break(short or long based on cycle_position % long_break_after)
  → start_break() → Break (immediate)
  → extend_work(mins) → Working(new timer)
  → stop() → Idle

Break
  → remaining hits 0 → cycle_position increments, resets to 0 after long break
                      → Working(next cycle, auto-start)
  → skip_break() → Working(next cycle)
  → stop() → Idle
  → pause() → Paused { was: Break }

Paused
  → resume() → restore previous phase with remaining time

Any → Idle:
  → DND: if enabled && !dnd_was_active_before, toggle off (restore)
```

### Cycle Position Logic

- `cycle_position` starts at 0 when session begins.
- Increments when a Working phase completes naturally (not on stop/skip).
- Break type: `if (cycle_position + 1) % long_break_after == 0 → long break, else short break`.
- After a long break completes, `cycle_position` resets to 0.

## Tick Optimization

### Backend: 5-Second Sync

The timer loop runs a 1-second internal tick (for remaining countdown and tray title), but only emits a `focus:sync` event to the frontend every 5 seconds.

Exception: `BreakPending` phase emits every 1 second (it's only 5 seconds total).

```rust
FocusSyncPayload {
    phase: String,           // "working", "break_pending", "break", "paused"
    remaining_secs: u64,
    total_secs: u64,
    cycle_position: u32,
    long_break_after: u32,
    paused: bool,
    action_title: Option<String>,
    dnd_active: bool,
}
```

### Instant Events

| Event | When | Payload |
|---|---|---|
| `focus:phase_changed` | Any phase transition | Full `FocusSyncPayload` |
| `focus:warning` | 30 seconds remaining in Working or Break | `{ phase, remaining_secs }` |
| `focus:dnd_unavailable` | DND toggle failed (shortcut not found) | `{ message }` |

### Frontend: Local Countdown

```typescript
const serverRemaining = payload.remaining_secs;
const receivedAt = Date.now();

// In render loop (1s setInterval):
const elapsed = (Date.now() - receivedAt) / 1000;
const displayRemaining = Math.max(0, serverRemaining - elapsed);
```

5-second syncs correct any drift from system sleep or tab suspension.

### Pause Behavior

Backend stops emitting sync events while paused. Frontend freezes display at last known remaining. On resume, backend sends immediate `phase_changed` with current remaining.

## DND Integration

### New Module: `crates/platform-macos/src/dnd.rs`

```rust
/// Read current DND/Focus state via `defaults read`.
pub fn is_dnd_active() -> bool

/// Toggle DND via `shortcuts run "Toggle Do Not Disturb"`.
/// Returns Ok(()) on success, Err if shortcut not found or timed out.
pub fn toggle_dnd() -> Result<(), String>
```

- `is_dnd_active()`: reads macOS UserDefaults for current Focus/DND state. Fast, no side effects, no permissions.
- `toggle_dnd()`: runs `shortcuts run "Toggle Do Not Disturb"`. Official automation layer, no private APIs.

### Lifecycle Integration

```
Session start (Idle → Working):
  if dnd_enabled:
    dnd_was_active_before = is_dnd_active()
    if !dnd_was_active_before: toggle_dnd()

Session end (any → Idle):
  if dnd_enabled && !dnd_was_active_before:
    toggle_dnd()  // restore previous state
```

DND stays on during breaks — the whole cycle is an uninterrupted rhythm.

### Failure Handling

- If `shortcuts run` fails, log warning and emit `focus:dnd_unavailable`.
- Frontend shows a one-time hint: "Create a Shortcut named 'Toggle Do Not Disturb' to enable this feature."
- Never block session start on DND failure — best-effort enhancement.

### Non-macOS

Both functions are no-ops behind `#[cfg(target_os = "macos")]`. The UI toggle works everywhere (setting persists).

## Frontend Simplification

### Hook State (useFocusTimer)

**Keeps:**
- `serverState: FocusSyncPayload | null` — last backend sync
- `receivedAt: number` — for local countdown interpolation
- `settings: FocusSettings` — localStorage persistence
- `selectedTask: { id, title } | null` — task linking
- `coaching: CoachingIntervention | null` — independent event stream

**Derives (no longer owns):**
- `phase` ← `serverState.phase`
- `paused` ← `serverState.paused`
- `remainingSecs` ← computed from `serverState.remaining_secs - elapsed`
- `totalSecs` ← `serverState.total_secs`
- `cyclePosition` ← `serverState.cycle_position`

**Deletes:**
- `completed` state and `FocusCompletedPayload` type
- `autoBreakTimer` ref and its `useEffect`
- `launchFocus` cycle position calculation
- Phase management logic (idle/focus/break_pending/break transitions)

### IPC Commands

| Command | Payload |
|---|---|
| `focus_session_start` | `{ config: FocusSessionConfig, action_id?, action_title?, dnd_enabled, sound_enabled, notification_enabled }` |
| `focus_session_stop` | `{ notes? }` |
| `focus_session_pause` | `{}` |
| `focus_session_resume` | `{}` |
| `focus_session_extend` | `{ extra_secs }` |
| `focus_session_start_break` | `{}` |
| `focus_session_extend_work` | `{ extra_mins }` |
| `focus_session_skip_break` | `{}` |

### Events

| Event | Hook action |
|---|---|
| `focus:sync` | Update serverState + receivedAt |
| `focus:phase_changed` | Update serverState + receivedAt, refetch today's sessions |
| `focus:warning` | Set showWarning flag |
| `focus:dnd_unavailable` | Show setup hint |

### Reconnect on Mount

Hook calls `focus_session_status` on mount, which returns full `FocusSyncPayload` or null. Handles tray window toggle, tab reopen, etc.

## File Changes

### Modified

| File | Change |
|---|---|
| `crates/desktop/src/focus_timer.rs` | Rewrite: phase state machine, 5s sync tick, break_pending phase, DND hooks, new command set |
| `crates/desktop/src/commands/productivity.rs` | Replace `focus_timer_*` with `focus_session_*` commands, update DEV_COMMANDS and dispatch_dev |
| `crates/desktop-shared/src/events.rs` | Replace FOCUS_TICK/FOCUS_COMPLETED with FOCUS_SYNC/FOCUS_PHASE_CHANGED/FOCUS_WARNING/FOCUS_DND_UNAVAILABLE |
| `crates/desktop-shared/src/commands/productivity.rs` | FocusTimerStatusResponse → FocusSessionStatusResponse |
| `crates/platform-macos/src/lib.rs` | Add `pub mod dnd;` |
| `crates/platform-macos/Cargo.toml` | No new dependencies needed (uses std::process::Command) |
| `desktop-ui/src/shared/hooks/useFocusTimer.ts` | Rewrite: reactive listener, local countdown, delete phase/cycle logic |
| `desktop-ui/src/features/tray/components/FocusControl.tsx` | Adapt to new hook shape, delete auto-break useEffect |
| `desktop-ui/src/shared/types/productivity.ts` | Update payload types |

### New

| File | Purpose |
|---|---|
| `crates/platform-macos/src/dnd.rs` | macOS DND read/toggle |

### Not Touched

- `crates/feature-productivity/src/focus.rs` — FocusManager persistence unchanged
- `crates/app-core/src/handlers/productivity/focus.rs` — AppCore handlers stay, command names update
- `crates/bus/src/domain_events.rs` — FocusSessionStarted/FocusSessionEnded unchanged
- `crates/desktop/src/tray_countdown.rs` — FOCUS_ACTIVE coordination unchanged
- Coaching integration, auto-focus detection, distraction overlay — unchanged

### Test Updates

- `focus_timer.rs` tests rewritten for new state machine (phase transitions, cycle counting, DND hooks)
- Existing `feature-productivity` and integration tests unaffected
