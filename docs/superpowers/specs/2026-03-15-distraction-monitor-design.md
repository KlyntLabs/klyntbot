# Distraction Monitor — Design Spec

## Problem

All infrastructure for distraction detection during focus mode exists (ActivityTracker, DistractionInterceptor, frontend overlays, Tauri commands) but nothing connects them. The `DistractionInterceptor` is created and stored in `AppCore` but never called during focus sessions. The frontend listens for `distraction:intervention` and `distraction:detected` events that are never emitted.

## Solution

A new `DistractionMonitor` in `feature-productivity` that subscribes to the `ActivityTick` broadcast, checks for active focus sessions, evaluates ticks through the existing `DistractionInterceptor`, and sends alerts via an `mpsc` channel that the transport layer wires to Tauri events.

## Architecture

```
ActivityTracker ──broadcast──→ DistractionMonitor
                                   │
                         FocusManager::get_active()
                                   │
                       DistractionInterceptor::evaluate()
                                   │
                         ┌─────────┴─────────┐
                    ShowOverlay           Allow (skip)
                         │
                    mpsc::send(DistractionAlert)
                         │
              Tauri emits:
                "distraction:intervention" (InterventionPayload)
                "distraction:detected" (DistractionDetectedPayload)
```

Follows the same pattern as `auto_focus_rx` and `nudge_rx` — feature crate produces a channel, `EventChannels` carries it, transport layer consumes it.

## New Types

### `DistractionAlert` (in `feature-productivity::distraction::monitor`)

```rust
pub struct DistractionAlert {
    pub session_id: String,
    pub app_name: String,
    pub window_title: Option<String>,
    pub previous_app: String,
    pub previous_context: String,
    pub needs_llm: bool,
    pub heuristic_verdict: String,
}
```

## DistractionMonitor

### Location

`crates/feature-productivity/src/distraction/monitor.rs`

### Constructor

```rust
impl DistractionMonitor {
    pub fn new(
        tick_rx: broadcast::Receiver<ActivityTick>,
        focus_manager: Arc<FocusManager>,
        interceptor: Arc<Mutex<DistractionInterceptor>>,
        config: FocusConfig,
        cancel: CancellationToken,
    ) -> Self;

    pub fn start(self) -> (mpsc::Receiver<DistractionAlert>, JoinHandle<()>);
}
```

### Tick Processing Logic

On each `ActivityTick`:

1. Skip if idle
2. Skip if `soft_block_enabled` is false (avoids DB lookup)
3. Query `FocusManager::get_active()` for active focus session
4. Skip if no active session or session type is `Break`
5. Check per-app cooldown — skip if same app alerted within `soft_block_cooldown_secs`
6. Call `DistractionInterceptor::evaluate(app_name, window_title)`
7. If `ShowOverlay`: send `DistractionAlert` through channel, record cooldown timestamp
8. If `Allow`: update `previous_app` / `previous_context` tracking (last known productive state)

### Cooldown

- `HashMap<String, Instant>` keyed by app name (lowercased)
- After emitting alert, insert current instant
- On next tick for same app, skip if elapsed < `soft_block_cooldown_secs`
- Lazy prune expired entries

### Previous Context Tracking

- Track `previous_app: String` and `previous_context: String` from the most recent tick that was NOT distracting
- These populate the banner's "You drifted from {previousContext}" message

## Integration Points

### 1. `ProductivityResult` (`init/productivity.rs`)

New field:

```rust
pub distraction_alert_rx: Option<mpsc::Receiver<DistractionAlert>>,
```

Created after the engine and interceptor, using `engine.subscribe()` for a fresh tick receiver.

### 2. `EventChannels` (`init/mod.rs`)

New field:

```rust
pub distraction_alert_rx: Option<mpsc::Receiver<DistractionAlert>>,
```

### 3. `wire_event_channels` (`desktop/src/app_core.rs`)

New forwarder using `spawn_channel_forwarder`:

- Receives `DistractionAlert`
- Emits `DISTRACTION_INTERVENTION` with `InterventionPayload { app_name, window_title, session_id, needs_llm, heuristic_verdict }`
- Emits `DISTRACTION_DETECTED` with `DistractionDetectedPayload { session_id, app_name, previous_app, previous_context, reason }`

Both frontend components (`DistractionOverlay.tsx`, `DistractionInterventionBanner.tsx`) already listen for these events with matching payload shapes.

### 4. `distraction/mod.rs`

Add `pub mod monitor;` to expose the new module.

## What Already Exists (No Changes Needed)

- `DistractionInterceptor` — evaluates app/title against heuristics, learned rules, session whitelist, temp passes
- `heuristics.rs` — classifies Netflix/TikTok/Instagram/Twitch/Discord as always distracting; YouTube/Reddit as ambiguous
- `DistractionOverlay.tsx` — glassmorphic popup with "Back to work" / "Allow briefly" / "This is work" buttons
- `DistractionInterventionBanner.tsx` — in-app banner with "Back to work" / "Not a distraction" / "5 more minutes" / "End focus"
- Tauri commands: `distraction_dismiss`, `distraction_allow_temp`, `distraction_allow_session`
- Event constants: `DISTRACTION_INTERVENTION`, `DISTRACTION_DETECTED`, `DISTRACTION_VERDICT`
- `FocusManager` — manages focus session lifecycle with `get_active()`, `record_distraction()`

## Testing

All tests in `distraction/monitor.rs` using in-memory SQLite:

1. **Emits alert** when focus session active + distracting app detected
2. **No alert** when no focus session active
3. **No alert** during break sessions
4. **No alert** for idle ticks
5. **Cooldown** suppresses repeated alerts for same app within window
6. **Cooldown expires** and re-alerts after configured seconds
7. **Previous context** tracks correctly across tick transitions
8. **Respects cancellation** token for clean shutdown
