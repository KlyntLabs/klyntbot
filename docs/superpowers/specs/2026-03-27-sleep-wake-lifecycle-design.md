# Sleep/Wake Lifecycle & Opportunistic Continuity

**Date:** 2026-03-27
**Status:** Approved
**Scope:** Cross-cutting — platform-macos, bus, scheduling, desktop, app-core, feature-productivity, cognitive

## Problem

Klyntbot runs as a local macOS desktop app with 20+ cron jobs (AI reflection, coaching, autotuner, memory decay, proactive suggestions) and real-time features (focus timer, DND, tray countdown). It has zero awareness of macOS sleep/wake cycles or user presence. This breaks the "second brain" promise in several visceral ways:

- **Focus timer destroyed by sleep:** Uses `tokio::time::interval(1s)` with default `Burst` behavior. A 30-minute sleep causes ~1800 ticks to fire instantly on wake, draining the timer to zero. The user returns to find their focus session ended, break started, DND toggled — all before they unlock their screen.
- **DND not restored on crash/force-quit:** DND is toggled on focus start and restored in `FocusTimer::stop()`. If the app crashes mid-session, DND stays on forever.
- **App cannot quit:** `quit_app` calls `app.exit(0)` but `ExitRequested` handler unconditionally prevents exit. `AppCore::shutdown()` is never called from the desktop app.
- **AI jobs fire into the void:** Weekly reflection runs at 9am Monday whether the user is at their desk or commuting. Proactive suggestions run every 4 hours with no presence check.
- **All missed cron jobs fire simultaneously on wake:** No staggering, no priority. Cheap DB cleanup and expensive LLM reflection both fire at the same instant.
- **One-shot `At` jobs silently lost:** If the scheduled time passes while the Mac is asleep, `compute_next_run` returns `None` and the job never fires.
- **App Nap throttles timers:** No `NSProcessInfo beginActivity` assertion, so macOS can throttle `tokio::time` precision when the app is backgrounded.
- **No "user away" detection:** If the user leaves the Mac awake for 6 hours (e.g., goes to dinner), all cron jobs fire into the void — no one sees the results.

## Vision: Opportunistic Continuity

Stop thinking "cron jobs must run at exact wall-clock times." Start thinking "the agent has *intent* and fulfills that intent as soon as the user is present again, without feeling frantic or broken."

The second brain should feel *alive* across the sleep boundary — it greets the user intelligently, preserves intent, and never surprises or disrupts. The wake moment becomes a hero UX moment: "Good morning. You were away for 4h 12m. Here's what I did for you."

## Architecture: Event-Driven with Wake Orchestrator (Approach B)

### Why This Approach

- **Follows existing patterns:** DomainEventBus subscribers (like Mirror, ActivityLog, AtomExtraction) already react to domain events independently. Each feature owns its behavior.
- **No god object:** Unlike a centralized LifecycleCoordinator (Approach A), features subscribe independently. No tight coupling.
- **Incremental delivery:** Unlike an intent-window-first rewrite (Approach C), this ships core resilience first, then layers on intelligence.
- **WakeOrchestrator is a presenter, not a controller:** It collects "ready" signals and sequences the user-facing delivery. It doesn't call into features.

### Signal Flow

```
+----------------------------------------------------------+
|                    platform-macos                         |
|                                                          |
|  +----------------------+   +---------------------------+|
|  |  NSWorkspace          |   |  CGEventSource            ||
|  |  willSleep/didWake    |   |  secondsSinceLastEvent    ||
|  |  (hard boundary)      |   |  (soft presence signal)   ||
|  +----------+------------+   +-------------+-------------+|
|             |                              |              |
|  +----------+------------------------------+------------+ |
|  |              LifecycleMonitor                        | |
|  |  State machine: Active -> Idle -> Sleeping -> Waking | |
|  |  Emits: LifecycleEvent via callback                  | |
|  +-------------------------+----------------------------+ |
+----------------------------+------------------------------+
                             |
                             v
+----------------------------------------------------------+
|                    app-core bridge                        |
|  LifecycleEvent -> DomainEvent (published to bus)        |
+----------------------------+-----------------------------+
                             |
                             v
+----------------------------------------------------------+
|                  DomainEventBus                           |
|                                                          |
|  SystemWillSleep                                         |
|  SystemDidWake { away_duration, wake_type }              |
|  UserBecameIdle { idle_secs }                            |
|  UserReturned { absence_duration, wake_type }            |
+--+----------+------------+---------------+---------------+
   |          |            |               |
   v          v            v               v
+--------+ +------+ +----------+ +--------------------+
| Focus  | | Cron | | Activity | |  WakeOrchestrator  |
| Timer  | | Svc  | | Tracker  | |  (collects "ready" |
|        | |      | | + Nudge  | |   signals, presents|
|Suspend | |Queue | | + Coach  | |   wake greeting)   |
|/Resume | |/Catch| |Mark gap  | |                    |
+--------+ +------+ +----------+ +--------------------+
```

### Layered Signals

| Signal | Detects | Purpose |
|--------|---------|---------|
| `NSWorkspace willSleep/didWake` | Lid close, explicit sleep, system idle sleep | Hard boundary: freeze focus timer, snapshot DND, queue cron |
| `CGEventSource` idle time | User actually absent (no keyboard/mouse) | Soft presence: delay expensive jobs until user returns |

Both are needed. `NSWorkspace` alone misses "user left Mac awake for 6 hours." `CGEventSource` alone can't detect actual system sleep.

## Section 1: OS Integration Layer (`platform-macos`)

### New Module: `crates/platform-macos/src/lifecycle.rs`

#### LifecycleMonitor API

```rust
pub struct LifecycleMonitor { /* ... */ }

pub struct LifecycleConfig {
    pub idle_threshold_secs: u64,       // default: 300 (5 min)
    pub presence_threshold_secs: u64,   // default: 2
    pub wake_grace_period_secs: u64,    // default: 60
    pub idle_poll_interval_secs: u64,   // default: 10 (Active), 30 (Idle)
}

pub struct WakeDeliveryConfig {
    pub min_absence_for_panel_secs: u64,        // default: 1800 (30 min)
    pub quiet_period_morning_secs: u64,         // default: 45 (5am-11am)
    pub quiet_period_midday_secs: u64,          // default: 15 (12pm-4pm)
    pub quiet_period_evening_secs: u64,         // default: 60 (after 8pm)
    pub quiet_period_default_secs: u64,         // default: 30 (all other times)
    pub catch_up_tier_stagger_secs: u64,        // default: 120 (2 min between tiers)
    pub idle_resume_prompt_threshold_secs: u64, // default: 600 (10 min — FromIdle focus prompt)
    pub nudge_consolidation_threshold_secs: u64,// default: 1800 (30 min — consolidate nudges)
}

pub enum LifecycleEvent {
    SystemWillSleep,
    SystemDidWake {
        away_duration: Duration,
        wake_type: WakeType,
    },
    UserBecameIdle { idle_secs: u64 },
    UserReturned {
        absence_duration: Duration,
        wake_type: WakeType,
    },
}

pub enum WakeType {
    FromSleep,  // Mac was literally asleep
    FromIdle,   // Mac was awake, user was absent
}

impl LifecycleMonitor {
    pub fn start(
        config: LifecycleConfig,
        callback: impl Fn(LifecycleEvent) + Send + Sync + 'static,
    ) -> Self;
    pub fn stop(&self);
    pub fn state(&self) -> LifecycleState;
    pub fn idle_secs(&self) -> u64;
}
```

#### NSWorkspace Sleep/Wake

Subscribes to `willSleepNotification` and `didWakeNotification` via `objc2-app-kit` (already a dependency). Observer registered on the main thread via `dispatch_async(dispatch_get_main_queue())` since NSWorkspace notifications require the main run loop. Internally posts to a `tokio::sync::mpsc` channel; a spawned task drains it and calls the callback on the tokio runtime.

On `willSleep`: record `sleep_start = Instant::now()`, emit `SystemWillSleep`.
On `didWake`: compute `away_duration`, emit `SystemDidWake`, start grace timer.

#### CGEventSource Idle Detection

Polls `CGEventSourceSecondsSinceLastEventType(.hidSystemState, .any)` via `core-graphics` crate. Adaptive polling: every 10s when Active, every 30s when Idle/Sleeping.

Thresholds:
- `idle_threshold`: 300s (5 min) — Active -> Idle transition
- `presence_threshold`: 2s — Idle -> Active transition

#### Grace Period After `didWake`

After `didWake`, poll CGEventSource every 2s for user input within a 60s grace window. If input detected: emit `UserReturned { FromSleep }`. If 60s expires with no input: emit `UserReturned { FromSleep }` anyway (handles Power Nap, scheduled wake, external display scenarios).

This ensures the wake greeting doesn't fire the instant the lid opens (before password entry), but doesn't wait indefinitely.

#### App Nap Prevention

On `LifecycleMonitor::start()`:
- Base assertion: `beginActivity(.userInitiated, "Klyntbot scheduling")` — prevents timer coalescing, keeps tokio precision.
- During active focus session: upgrade to `beginActivity(.userInitiated | .idleSystemSleepDisabling, "Klyntbot active focus session")` — additionally prevents idle sleep during focus.
- When focus ends: downgrade back to base assertion.

#### State Machine

```
Active --(idle > threshold)--> Idle --(willSleep)--> Sleeping
  ^                             |                       |
  |                             |                       |
  +---(user input detected)-----+                       |
  ^                                                     |
  +-------(didWake + user input)------------------------+
```

- `Active -> Idle`: CGEventSource idle time exceeds threshold. Emits `UserBecameIdle`.
- `Idle -> Active`: User input detected while idle. Emits `UserReturned { FromIdle }`.
- `Active/Idle -> Sleeping`: `willSleep`. Emits `SystemWillSleep`.
- `Sleeping -> WakingGrace -> Active`: `didWake` + user input (or grace expiry). Emits `SystemDidWake` then `UserReturned { FromSleep }`.

Distinction between `FromIdle` and `FromSleep` lets features respond differently — a 5-minute idle return doesn't need a wake greeting, but a 4-hour sleep does.

## Section 2: Domain Events

### New DomainEvent Variants

Four lifecycle events:

```rust
SystemWillSleep,
SystemDidWake {
    away_duration: Duration,
    wake_type: WakeType,
},
UserBecameIdle {
    idle_secs: u64,
},
UserReturned {
    absence_duration: Duration,
    wake_type: WakeType,
},
```

Two "ready" events emitted by features for the WakeOrchestrator:

```rust
FocusSessionSuspended {
    remaining_secs: u64,
    phase: FocusPhase,
},
CronCatchUpReady {
    immediate: Vec<MissedJobSummary>,
    deferred: Vec<MissedJobSummary>,
    expired: Vec<MissedJobSummary>,
},
```

## Section 3: Feature Handlers

Each feature subscribes to lifecycle events independently and owns its own pause/resume logic.

### FocusTimer (highest priority, most user-visible)

| Event | Reaction |
|-------|----------|
| `SystemWillSleep` | Transition to `Suspended` state. Record `suspended_at = Utc::now()`. Preserve DND state (don't toggle). Stop decrementing. Emit `FocusSessionSuspended`. |
| `SystemDidWake` | No action — wait for user. |
| `UserReturned { FromSleep }` | If suspended: show resume prompt — "Continue your focus session? 18:32 remaining". Resume / End (shows quality summary) / Restart. |
| `UserReturned { FromIdle }` | If absence < 10 min: silently resume. If >= 10 min: same prompt as FromSleep. Threshold configurable. |

**Architectural fix — wall-clock anchoring:** Replace counter-based `remaining -= 1` with `remaining = target_end.signed_duration_since(Utc::now())`. This makes the timer inherently sleep-safe. The `Suspended` state stops updating `target_end`; on resume it recomputes from `Utc::now() + remaining`.

**DND safety net:** On `SystemWillSleep`, persist `{ dnd_was_on, dnd_original_state }` to a `dnd_override` SQLite table (singleton row). On app startup, check for orphaned override and restore. Handles crash/force-quit.

### CronService

| Event | Reaction |
|-------|----------|
| `SystemWillSleep` | Record `sleep_start_ms`. Set `is_sleeping = true`. |
| `SystemDidWake` | Scan all enabled jobs. Classify missed jobs. Emit `CronCatchUpReady`. Run `immediate` jobs. Recompute `next_run_at_ms` for all recurring jobs from `now_ms()`. |
| `UserReturned` | WakeOrchestrator handles deferred job execution. |

**Missed job classification:**

```rust
fn classify_missed_job(job: &CronJob, sleep_start: i64, now: i64) -> MissedJobClass {
    let missed = job.next_run_at_ms.map_or(false, |next| next >= sleep_start && next <= now);
    if !missed { return NotMissed; }

    match (&job.schedule, &job.intent_window) {
        (CronSchedule::At { .. }, _) if !job.catch_up => Expired,
        (CronSchedule::At { .. }, _) => Immediate,
        (_, Some(window)) if window.priority == WhenPresent => Deferred,
        _ if is_cheap_job(&job.name) => Immediate,
        _ => Deferred,
    }
}
```

Cheap jobs (static set): `JOB_ATOM_DECAY`, `JOB_SESSION_CLEANUP`, `JOB_MEMORY_MAINTENANCE`, `JOB_ANALYTICS_CLEANUP`, `JOB_BLACKBOARD_CLEANUP`, `JOB_MIRROR_CLEANUP`, `JOB_OVERDUE_CHECK`, `JOB_RECURRING_TASKS`, `JOB_REMINDER_CHECK`, `JOB_FOCUS_CHECK`.

### ActivityTracker / Productivity

| Event | Reaction |
|-------|----------|
| `SystemWillSleep` | Record `sleep_start`. |
| `SystemDidWake` | Insert `ActivityGap { start, end, reason: Sleep }` into activity log. Daily summary shows "away for 4h 12m." |
| `UserReturned` | Resume normal polling. |

### NudgeService / Coaching

| Event | Reaction |
|-------|----------|
| `SystemWillSleep` | Queue pending nudges. |
| `UserReturned` | If absence > 30 min: consolidate into single "while you were away" debrief. If < 30 min: deliver individually. |

### Cognitive (Background Consolidation, Atom Extraction)

| Event | Reaction |
|-------|----------|
| `SystemDidWake` | If away > 30 min: trigger consolidation pass for pending items. |
| `UserReturned` | Normal operation resumes (event-driven services resume on new domain events). |

### ContextInferenceLoop

Standalone fix: set `MissedTickBehavior::Skip` on both intervals (inference + archival) to prevent burst on wake.

### Tray Countdown

No changes needed. Already wall-clock based.

## Section 4: WakeOrchestrator & Wake Greeting

### WakeOrchestrator (new, in `app-core`)

A thin subscriber that collects "ready" signals from features and sequences the user-facing wake experience. It does not control feature behavior — it only presents results.

#### Lifecycle

```
SystemDidWake received
       |
  [Collect "ready" signals — 5s window]
       |  <- FocusSessionSuspended?
       |  <- CronCatchUpReady?
       |  <- NudgesQueued?
       |  <- ActivityGapRecorded?
       |
  (wait for UserReturned)
       |
  [Adaptive quiet period]
       |  Morning wake: 45s
       |  Mid-day idle return: 15s
       |  After 8pm: 60s + optional "tomorrow" deferral
       |
  [Assemble Wake Panel]
       |
  +----+----+----+
  |         |         |
  Focus   Wake      Staggered
  Resume  Summary   Catch-up
```

#### Wake Panel

```rust
pub struct WakePanel {
    pub greeting: String,
    pub absence_summary: AbsenceSummary,
    pub focus_resume: Option<FocusResume>,
    pub completed_items: Vec<CompletedItem>,
    pub pending_items: Vec<PendingItem>,
    pub expired_items: Vec<ExpiredItem>,
    pub coaching_debrief: Option<String>,
    pub cognitive_snippet: Option<String>,
    pub dismiss_action: DismissAction,
}

pub struct FocusResume {
    pub remaining_secs: u64,
    pub phase: FocusPhase,
    // User actions: Resume | End (shows quality summary) | Restart | Abandon
}

pub struct ExpiredItem {
    pub job_name: String,
    pub scheduled_at: DateTime<Utc>,
    pub description: String,
    pub can_reschedule: bool,
    pub smart_reschedule: Option<String>,  // "Tomorrow 9am (your energy peak)"
}
```

#### Greeting Logic

Adapts to time of day, absence type, and duration. Minimum 30-minute absence for wake panel (shorter absences are silent). Personalized with a lightweight cognitive memory snippet for absences > 1 hour:

> "Good morning. You were away for 4h 12m. Last night you were reflecting on your Q2 portfolio — here's what I prepared."

#### Delivery Sequence

1. **Focus resume prompt** (if applicable) — first thing the user sees. Blocking: deferred jobs don't start until user responds.
2. **Wake panel** — unified surface (first-class desktop UI panel, not scattered notifications). Contains greeting, completed items, pending items with live progress, expired items with smart reschedule options, coaching debrief.
3. **Staggered catch-up** — deferred jobs in priority tiers with 2-min stagger:
   - Tier 1 (Reflection): weekly reflection, weekly narrative
   - Tier 2 (Planning): proactive scan, daily planning
   - Tier 3 (Maintenance): price refresh, insight refresh
   - Within a tier: sequential (not parallel) to avoid LLM cost spikes
   - Wake panel updates live as jobs complete

#### User Actions on Wake Panel

- **"Catch up now"** — runs all deferred jobs immediately (no stagger).
- **"Stay quiet"** — dismisses panel, skips deferred jobs for this wake cycle.
- **Reschedule expired items** — smart options from task/calendar layer ("Tomorrow 9am, your energy peak").

#### Edge Cases

- **Multiple short sleep/wake cycles:** Debounce — if `SystemDidWake` fires within 2 min of a previous `UserReturned`, merge into existing wake cycle.
- **Brief microsleep (< 30s):** Focus timer's wall-clock anchoring handles it. No suspension, no panel.
- **No missed jobs:** No panel shown. Silent return.
- **Deep Sleep mode:** Panel shows "Here's what I would have done" with explicit "Run now" buttons. No auto-execution.
- **Recovery after crash:** Wake greeting includes reassuring line: "I recovered cleanly from an unexpected shutdown — your focus session and DND state are safe."

#### Notification Fatigue Guard

Total notifications during a wake cycle: exactly 1 (the unified panel). Never separate tray notification + panel. The single surface is the contract.

## Section 5: Intent Windows — Opportunistic Scheduling Overlay

### Concept

Intent windows sit between the cron schedule and job execution. A cron schedule says *when* a job ideally runs. An intent window says *under what conditions* it should actually fire, with a deadline after which the cron fallback takes over.

### Data Model

```rust
pub struct IntentWindow {
    pub trigger: IntentTrigger,
    pub tolerance: Duration,
    pub catch_up: CatchUpPriority,
}

pub enum IntentTrigger {
    UserPresent,
    FirstActivityAfter { after_local: NaiveTime },
    MinActiveMinutes { minutes: u32 },
    UserIdle { min_idle_secs: u64 },
    // Future: CognitiveContext { salience_above: f32 }
}

pub enum CatchUpPriority {
    Immediate,    // cheap, no LLM — run now
    WhenPresent,  // expensive — wait for user
    WhenIdle,     // very expensive, low urgency — wait for idle
}
```

### Storage

New columns on `cron_jobs` table:

```sql
ALTER TABLE cron_jobs ADD COLUMN intent_window TEXT;           -- JSON, nullable
ALTER TABLE cron_jobs ADD COLUMN intent_pending_since_ms INTEGER;  -- nullable
```

Pre-release: direct schema change, no migration script.

### Default Intent Windows for AI Jobs

| Job | Cron Schedule | Intent Trigger | Tolerance | Catch-up |
|-----|--------------|----------------|-----------|----------|
| `JOB_WEEKLY_REFLECTION` | Mon 9am | `FirstActivityAfter { 8:00 }` | 2h | WhenPresent |
| `JOB_MIRROR_WEEKLY_NARRATIVE` | Sun 10am | `FirstActivityAfter { 9:00 }` | 3h | WhenPresent |
| `JOB_AUTOTUNER_NIGHTLY` | 2am daily | `UserIdle { 300 }` | 4h | WhenIdle |
| `JOB_PROACTIVE_SCAN` | Every 4h | `MinActiveMinutes { 5 }` | 1h | WhenPresent |
| `JOB_INSIGHT_REFRESH` | Every 24h | `UserIdle { 600 }` | 6h | WhenIdle |
| `JOB_ATOM_EXTRACTION_CATCHALL` | 2am daily | `UserIdle { 300 }` | 4h | WhenIdle |
| `JOB_DAILY_PLANNING` | Configured | `FirstActivityAfter { configured }` | 1h | WhenPresent |
| `JOB_WEEKLY_REPORT` | Sun 6pm | `UserPresent` | 2h | WhenPresent |
| All maintenance jobs | Various | *(none)* | — | Immediate |

Pattern: **LLM-heavy jobs get intent windows. Pure-DB jobs don't.**

### Timer Loop Integration

When a cron job fires and has an intent window:

1. Check `evaluate_trigger(&window.trigger, &lifecycle_state)`.
2. If `TriggerMet`: execute immediately.
3. If `TriggerNotMet`: set `intent_pending_since = now_ms()`, skip execution. The pending-intent evaluator runs on each tick and on lifecycle events.
4. If tolerance expires: execute anyway (fallback).

`lifecycle_state` is a lightweight snapshot from `LifecycleMonitor::state()` + `idle_secs()`, passed at init. CronService queries state — it doesn't subscribe to events.

### User-Created Jobs

Agent (`CronTool`) gains optional `intent_window` parameter. If omitted, traditional cron. Agent can suggest intent windows:

> "I've scheduled your weekly portfolio review for Monday 9am. Since this uses AI analysis, I'll wait until you're actually at your desk before running it."

### Global "Smart Scheduling" Toggle

Settings -> Automations. Default: On. When off, all intent windows are ignored (pure cron). First-time discovery in wake panel: "I waited for you to be present (Smart Scheduling is on). Want to turn it off?"

### Automations Dashboard

Each job shows a badge: "Smart (waits for you)" or "Strict (runs at 9am)". One-click edit to change intent window. "Adjust timing" link also appears in the wake panel's pending section.

## Section 6: Graceful Shutdown, DND Recovery & App Nap

### Bug Fix 1: Desktop App Cannot Quit

Add shared `AtomicBool` quit flag. `quit_app` command sets it, runs graceful shutdown sequence (stop focus timer -> persist lifecycle state -> shutdown app core), then calls `app.exit(0)`. `ExitRequested` handler only prevents exit when flag is false.

```rust
static QUIT_REQUESTED: AtomicBool = AtomicBool::new(false);

// quit_app: set flag, stop focus timer (restores DND), persist state, shutdown core, exit
// ExitRequested: only prevent_exit() if !QUIT_REQUESTED
```

### Bug Fix 2: DND Not Restored on Crash

Persist DND override state to SQLite singleton table:

```sql
CREATE TABLE IF NOT EXISTS dnd_override (
    id INTEGER PRIMARY KEY DEFAULT 1,
    original_state TEXT NOT NULL,
    overridden_at TEXT NOT NULL,
    session_id TEXT
);
```

- On focus start (DND toggle): INSERT OR REPLACE.
- On focus end (DND restore): DELETE.
- On app startup: if row exists, restore original DND state and delete row.

### Bug Fix 3: App Nap (Already Covered)

`LifecycleMonitor::start()` calls `beginActivity(.userInitiated)`. During active focus: upgrade to include `.idleSystemSleepDisabling`. On focus end: downgrade.

### Startup Recovery Sequence

Runs in `AppCore::init` before cron and lifecycle start:

1. **DND recovery:** Check `dnd_override` table. If orphaned row, restore and delete.
2. **Focus session recovery:** Check for active session. If found, mark as "interrupted" for WakeOrchestrator to present resume prompt on next launch.
3. **Pending intent recovery:** Handled by `CronService::recompute_next_runs()` — pending intents with expired tolerance get executed, others continue waiting.

### Quit-to-Launch Continuity

Persist `FocusSessionSuspended` state so on next launch the WakeOrchestrator shows the resume prompt without needing a sleep/wake cycle. The second brain feels continuous across app restarts.

### Deep Sleep Mode + Shutdown

When Deep Sleep is enabled, treat shutdown as an extension of sleep: queue all deferred/pending jobs for next wake greeting. No automatic tolerance fallback until user says "Catch up now."

## Section 7: Testing Strategy

### Layer 1: Unit Tests (per crate, no OS dependencies)

**LifecycleMonitor state machine:** Pure `LifecycleStateMachine` struct tested with mock clock. Tests: state transitions, grace period expiry, debounce, `FromIdle` vs `FromSleep` distinction.

**Intent window evaluation:** `evaluate_trigger` against mock `LifecycleState` snapshots. Tolerance expiry. Pending intent persistence across restart.

**Missed job classification:** Static tests for cheap/expensive/one-shot classification across all combinations.

**FocusTimer wall-clock anchoring:** `remaining` computed from `target_end - now`, not decremented. Suspended state freezes remaining. Phase transitions at correct times.

**WakeOrchestrator:** Collection window, greeting logic (time-of-day, absence thresholds, wake types), minimum absence threshold, debounce, Deep Sleep mode.

**Startup recovery:** DND orphan recovery, interrupted focus session detection, clean-state no-op.

### Layer 1.5: Continuity Journey Suite (`tests/continuity/`)

End-to-end user stories that validate the *feeling*, not just correctness:

- **"Coffee-run idle"** — 5 min away, silent resume, no panel.
- **"Full sleep wake"** — 4h sleep, warm greeting with cognitive snippet, focus resume prompt first, live-updating panel.
- **"Crash mid-focus"** — Force-quit, next launch shows recovery line + resume option.
- **"Deep Sleep mode + manual quit"** — Informational-only panel with "Run now" buttons.
- **"Left Mac awake 6h"** — Intent windows waited, no jobs fired into void.

Uses bus + in-memory DB. If any journey produces notification spam or jarring prompt, the test fails.

**Notification fatigue guard:** Every wake-orchestrator test asserts total notifications <= 1 (the unified panel).

**Cognitive snippet test:** Inject fake high-salience episodic memory before long sleep, assert greeting includes it.

### Layer 2: Integration Tests (cross-crate, via facade)

In `tests/integration/`, using `StoragePool::connect_in_memory()`.

- **Sleep/wake -> cron catch-up:** Simulate missed jobs during sleep, verify `CronCatchUpReady` classification.
- **Focus suspend -> resume:** Start session, publish `SystemWillSleep`, verify suspension, publish `UserReturned`, verify resume prompt.
- **WakeOrchestrator end-to-end:** Full event sequence -> panel assembly verification. Uses `tokio::time::pause()`.

### Layer 3: Platform Tests (macOS-specific, CI-excluded)

Gated behind `#[cfg(target_os = "macos")]` + `--features macos-integration`. Manual pre-release.

- NSWorkspace observer registration/cleanup.
- CGEventSource polling returns reasonable values.
- App Nap assertion lifecycle.
- DND query/restore subprocess.

### Manual UX Sign-off Checklist (Pre-release)

- Lid close -> open after 30 min -> verify greeting tone.
- Repeated lid open/close (debounce in reality).
- Leave Mac awake 6h -> return -> verify intent windows.
- Deep Sleep toggle -> quit -> relaunch -> verify paused state.

## Crates Touched

| Crate | Layer | Changes |
|-------|-------|---------|
| `platform-macos` | L0 | New `lifecycle.rs`: LifecycleMonitor, NSWorkspace observer, CGEventSource polling, App Nap assertion |
| `bus` | L1 | New DomainEvent variants (4 lifecycle + 2 ready events) |
| `scheduling` | L3 | IntentWindow on CronJob, intent evaluation in timer loop, missed-job classification, pending-intent persistence |
| `desktop` | L7 | Graceful quit fix (`QUIT_REQUESTED` flag), FocusTimer wall-clock rewrite + Suspended state, DND persistence |
| `app-core` | L7 | New WakeOrchestrator subscriber, lifecycle bridge, startup recovery, intent window defaults in `ensure_cron_jobs` |
| `feature-productivity` | L4 | ActivityGap recording, NudgeService queuing/consolidation |
| `cognitive` | L5 | ContextInferenceLoop `MissedTickBehavior::Skip`, consolidation-on-wake |
| `storage` | L2 | `dnd_override` table, `intent_window` + `intent_pending_since_ms` columns on `cron_jobs` |
| `config` | L1 | `LifecycleConfig`, `SleepBehaviorConfig` (thresholds, quiet periods, Smart Scheduling toggle) |
| `activity-log` | L4 | ActivityGap model + repo |

## Non-Goals

- **Replacing cron entirely with intent windows.** Intent windows are an overlay. Users can still set exact cron times.
- **Preventing system sleep.** We respect macOS power management. Only prevent idle sleep during active focus sessions.
- **Running jobs while the Mac is asleep.** No wake assertions, no background refresh. Jobs queue and catch up on wake.
- **Cross-device sync.** This is a single-machine solution. No cloud coordination.

## Open Questions (Resolved During Design)

- **Signal source?** Resolved: layered (NSWorkspace + CGEventSource).
- **Intent windows: overlay or rewrite?** Resolved: overlay on existing cron.
- **Architecture pattern?** Resolved: event-driven with thin WakeOrchestrator (Approach B).
- **Wake greeting: notification or UI panel?** Resolved: first-class desktop UI surface.
