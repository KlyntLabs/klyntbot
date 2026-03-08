# Session Watcher Wiring Design

## Problem

Sessions are only loaded at app startup. New Claude Code sessions created afterward don't appear in the sidebar until the app is restarted or `sync_sessions` is manually called.

## Root Cause

`SessionWatcher` (file watcher using `notify` crate) is implemented but never started. Event constants (`session:new`, `session:status`) are defined but never emitted. The frontend already listens for these events via `useEvent` hooks.

## Solution: Wire the File Watcher + Status Tick

### Architecture

```
SessionWatcher (notify crate, already implemented)
    ↓ WatchEvent via mpsc
SessionWatcherService (new, in app-core)
    ↓ processes events → upserts DB → sends typed payloads
EventChannels.session_watcher_rx
    ↓ mpsc::Receiver<SessionWatcherEvent>
wire_event_channels (desktop/app_core.rs)
    ↓ Tauri emit("session:new") / emit("session:status")
Frontend useEvent hooks (already wired, no changes needed)
```

### New Type

```rust
// in app-core or desktop-shared
pub enum SessionWatcherEvent {
    NewSession { session: TrackedSession },
    StatusChanged { session_id: String, status: SessionStatus },
}
```

### SessionWatcherService

Background task in `app-core` that:

1. Starts `SessionWatcher` on `~/.claude/projects/`
2. Runs initial `discover_sessions()` + `init_offsets()` to skip existing content
3. Drains `WatchEvent`s from the watcher channel:
   - `NewSession`: discovers session metadata, upserts to DB, forwards `SessionWatcherEvent::NewSession`
   - `FileModified`: updates `last_activity` in DB, re-evaluates status (Completed/Idle → Active transition)
4. Runs a **60-second status tick** that checks Active/Idle sessions and emits `SessionWatcherEvent::StatusChanged` on transitions:
   - Active + idle >30min → Idle
   - Idle + idle >60min → Completed
   - FileModified on Idle/Completed → Active

### Event Payloads (desktop-shared)

```rust
pub struct SessionNewPayload {
    pub session_id: String,
    pub project_path: String,
    pub project_name: String,
    pub status: String,
}

pub struct SessionStatusPayload {
    pub session_id: String,
    pub status: String,
}
```

### Tauri Wiring

In `wire_event_channels`, use existing `spawn_channel_forwarder` pattern:
- `NewSession` → `emit("session:new", SessionNewPayload)`
- `StatusChanged` → `emit("session:status", SessionStatusPayload)`

### EventChannels

```rust
pub struct EventChannels {
    // ... existing fields ...
    pub session_watcher_rx: Option<mpsc::Receiver<SessionWatcherEvent>>,
}
```

`Option` because the watcher may fail to start (e.g., `~/.claude` doesn't exist).

## Files Touched

| File | Change |
|------|--------|
| `crates/app-core/src/services/session_watcher.rs` | **New** — `SessionWatcherService` |
| `crates/app-core/src/services/mod.rs` | Add `pub mod session_watcher` |
| `crates/app-core/src/init.rs` | Start service, add rx to `EventChannels` |
| `crates/app-core/src/state.rs` | Store service handle for shutdown |
| `crates/desktop-shared/src/events.rs` | Add payload structs |
| `crates/desktop/src/app_core.rs` | Wire rx → Tauri emit |
| `crates/feature-session-tracker/src/repos.rs` | Add `list_sessions_by_status()` |

No frontend changes needed.

## Edge Cases

- **No `~/.claude` directory:** Service logs warning, returns `None` rx. App works without live tracking.
- **Rapid file modifications:** Only care about existence of modification for `last_activity` update. No per-line processing needed for status tracking.
- **Race between discovery and watcher:** Start watcher first, then discover + `init_offsets()`. Upsert is idempotent.

## Shutdown

Respects `CancellationToken`. On cancel: tick loop stops, event drain stops, `SessionWatcher` dropped (stops `notify`).

## Testing

- Unit test status transition logic with mock timestamps
- DB operations use `connect_in_memory()` pattern
- File watcher itself is `notify` crate's responsibility
