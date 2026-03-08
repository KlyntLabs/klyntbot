# Session Watcher Wiring Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Wire the existing `SessionWatcher` (file watcher) into app-core so new Claude Code sessions appear in the sidebar in real-time, and session status transitions (Active→Idle→Completed) happen automatically.

**Architecture:** A new `SessionWatcherService` in app-core starts the existing `SessionWatcher`, drains its events, upserts sessions to DB, evaluates status transitions on a 60s tick, and forwards typed events via an mpsc channel. The desktop adapter wires this channel to Tauri events. The frontend already listens for these events.

**Tech Stack:** Rust (tokio, notify crate), Tauri events, existing `feature-session-tracker` crate

---

### Task 1: Add `list_sessions_by_status` to repos

**Files:**
- Modify: `crates/feature-session-tracker/src/repos.rs`

**Step 1: Write the failing test**

Add to the existing `repos.rs` (or create inline test module). We need a query that returns sessions filtered by status — used by the status tick to check only Active/Idle sessions.

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SessionStatus;
    use storage::StoragePool;

    async fn setup_repos() -> SessionTrackerRepos {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &crate::SessionTrackerFeature::migrations_static(),
        )
        .await
        .unwrap();
        SessionTrackerRepos::new(pool.inner().clone())
    }

    fn make_session(id: &str, status: SessionStatus) -> TrackedSession {
        TrackedSession {
            session_id: id.to_string(),
            project_path: "/test/project".to_string(),
            project_name: "project".to_string(),
            jsonl_path: format!("/tmp/{id}.jsonl"),
            status,
            first_message_preview: None,
            message_count: 0,
            git_branch: None,
            last_activity: Some(Utc::now()),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_list_sessions_by_status() {
        let repos = setup_repos().await;
        repos.upsert_session(&make_session("s1", SessionStatus::Active)).await.unwrap();
        repos.upsert_session(&make_session("s2", SessionStatus::Idle)).await.unwrap();
        repos.upsert_session(&make_session("s3", SessionStatus::Completed)).await.unwrap();

        let live = repos.list_sessions_by_status(&[SessionStatus::Active, SessionStatus::Idle]).await.unwrap();
        assert_eq!(live.len(), 2);
        assert!(live.iter().any(|s| s.session_id == "s1"));
        assert!(live.iter().any(|s| s.session_id == "s2"));
    }
}
```

**Step 2: Run test to verify it fails**

Run: `cargo nextest run -p feature-session-tracker -E 'test(test_list_sessions_by_status)'`
Expected: FAIL — `list_sessions_by_status` method doesn't exist

**Step 3: Implement `list_sessions_by_status`**

Add to `SessionTrackerRepos` impl block in `repos.rs`:

```rust
pub async fn list_sessions_by_status(
    &self,
    statuses: &[SessionStatus],
) -> Result<Vec<TrackedSession>, StorageError> {
    if statuses.is_empty() {
        return Ok(Vec::new());
    }
    let placeholders: Vec<&str> = statuses.iter().map(|s| s.as_str()).collect();
    // Build dynamic IN clause — safe because values come from enum, not user input
    let in_clause = placeholders.iter().map(|_| "?").collect::<Vec<_>>().join(",");
    let sql = format!(
        "SELECT * FROM tracked_sessions WHERE status IN ({}) ORDER BY last_activity DESC",
        in_clause
    );

    let mut query = sqlx::query_as::<_, TrackedSessionRow>(&sql);
    for s in &placeholders {
        query = query.bind(*s);
    }

    let rows = query
        .fetch_all(&self.pool)
        .await
        .map_err(StorageError::from)?;

    Ok(rows.into_iter().map(Into::into).collect())
}
```

**Step 4: Run test to verify it passes**

Run: `cargo nextest run -p feature-session-tracker -E 'test(test_list_sessions_by_status)'`
Expected: PASS

**Step 5: Commit**

```bash
git add crates/feature-session-tracker/src/repos.rs
git commit -m "feat(session-tracker): add list_sessions_by_status query"
```

---

### Task 2: Add event payloads to desktop-shared

**Files:**
- Modify: `crates/desktop-shared/src/events.rs`

**Step 1: Add payload structs**

The `SessionStatusPayload` already exists in `feature-session-tracker/src/types.rs` but desktop-shared needs its own since it's the event layer. Add near the existing session event constants (line ~290):

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionNewPayload {
    pub session_id: String,
    pub project_path: String,
    pub project_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionStatusChangedPayload {
    pub session_id: String,
    pub status: String,
}
```

**Step 2: Verify it compiles**

Run: `cargo build -p desktop-shared`
Expected: success

**Step 3: Commit**

```bash
git add crates/desktop-shared/src/events.rs
git commit -m "feat(desktop-shared): add session watcher event payloads"
```

---

### Task 3: Create `SessionWatcherService` in app-core

This is the core new module. It starts the file watcher, processes events, runs the status tick, and forwards domain events.

**Files:**
- Create: `crates/app-core/src/services/mod.rs`
- Create: `crates/app-core/src/services/session_watcher.rs`
- Modify: `crates/app-core/src/lib.rs` — add `pub mod services;`

**Step 1: Create `services/mod.rs`**

```rust
pub mod session_watcher;
```

**Step 2: Create `services/session_watcher.rs`**

```rust
use chrono::Utc;
use feature_session_tracker::discovery;
use feature_session_tracker::repos::SessionTrackerRepos;
use feature_session_tracker::types::{SessionStatus, TrackedSession};
use feature_session_tracker::watcher::{SessionWatcher, WatchEvent};
use std::path::PathBuf;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing::{error, info, warn};

/// Domain events emitted by the session watcher service.
#[derive(Debug, Clone)]
pub enum SessionWatcherEvent {
    NewSession { session: TrackedSession },
    StatusChanged { session_id: String, status: SessionStatus },
}

const STATUS_TICK_SECS: u64 = 60;
const IDLE_THRESHOLD_SECS: i64 = 30 * 60;     // 30 minutes
const COMPLETED_THRESHOLD_SECS: i64 = 60 * 60; // 60 minutes

/// Start the session watcher service.
///
/// Returns `None` if `~/.claude` doesn't exist or the watcher fails to start.
/// The caller receives events via the returned `mpsc::Receiver`.
pub fn start(
    repos: SessionTrackerRepos,
    shutdown: CancellationToken,
) -> Option<mpsc::Receiver<SessionWatcherEvent>> {
    let claude_dir = discovery::default_claude_dir()?;
    let projects_dir = claude_dir.join("projects");

    if !projects_dir.exists() {
        warn!("Claude projects dir does not exist: {}", projects_dir.display());
        return None;
    }

    // Channel from notify watcher → our service loop
    let (watch_tx, watch_rx) = mpsc::unbounded_channel::<WatchEvent>();

    // Start the file watcher
    let watcher = match SessionWatcher::start(&projects_dir, watch_tx) {
        Ok(w) => w,
        Err(e) => {
            error!("Failed to start session watcher: {e}");
            return None;
        }
    };

    // Channel from service → caller (Tauri adapter)
    let (event_tx, event_rx) = mpsc::channel::<SessionWatcherEvent>(64);

    tokio::spawn(run_service(
        watcher,
        watch_rx,
        repos,
        claude_dir,
        event_tx,
        shutdown,
    ));

    Some(event_rx)
}

async fn run_service(
    watcher: SessionWatcher,
    mut watch_rx: mpsc::UnboundedReceiver<WatchEvent>,
    repos: SessionTrackerRepos,
    claude_dir: PathBuf,
    event_tx: mpsc::Sender<SessionWatcherEvent>,
    shutdown: CancellationToken,
) {
    // Initial discovery — populate DB and set watcher offsets
    let discovered = discovery::discover_sessions(&claude_dir).await;
    let jsonl_paths: Vec<PathBuf> = discovered
        .iter()
        .map(|s| PathBuf::from(&s.jsonl_path))
        .collect();
    watcher.init_offsets(&jsonl_paths);

    for session in &discovered {
        if let Err(e) = repos.upsert_session(session).await {
            warn!("Failed to upsert discovered session {}: {e}", session.session_id);
        }
    }
    info!("Session watcher: initial discovery found {} sessions", discovered.len());

    let mut tick = tokio::time::interval(std::time::Duration::from_secs(STATUS_TICK_SECS));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    // Keep watcher alive — it stops when dropped
    let _watcher = watcher;

    loop {
        tokio::select! {
            _ = shutdown.cancelled() => {
                info!("Session watcher service shutting down");
                break;
            }
            event = watch_rx.recv() => {
                let Some(event) = event else { break };
                handle_watch_event(&event, &repos, &event_tx, &claude_dir).await;
            }
            _ = tick.tick() => {
                check_status_transitions(&repos, &event_tx).await;
            }
        }
    }
}

async fn handle_watch_event(
    event: &WatchEvent,
    repos: &SessionTrackerRepos,
    event_tx: &mpsc::Sender<SessionWatcherEvent>,
    claude_dir: &PathBuf,
) {
    match event {
        WatchEvent::NewSession { session_id, jsonl_path } => {
            info!("New session detected: {session_id}");
            // Re-discover to get full metadata (project path, preview, etc.)
            let discovered = discovery::discover_sessions(claude_dir).await;
            if let Some(session) = discovered.into_iter().find(|s| s.session_id == *session_id) {
                if let Err(e) = repos.upsert_session(&session).await {
                    warn!("Failed to upsert new session {session_id}: {e}");
                    return;
                }
                let _ = event_tx.send(SessionWatcherEvent::NewSession { session: session }).await;
            }
        }
        WatchEvent::FileModified { session_id } => {
            // Update last_activity and potentially reactivate
            if let Ok(Some(session)) = repos.get_session(session_id).await {
                if session.status != SessionStatus::Active {
                    if let Err(e) = repos.update_session_status(session_id, &SessionStatus::Active).await {
                        warn!("Failed to reactivate session {session_id}: {e}");
                        return;
                    }
                    let _ = event_tx.send(SessionWatcherEvent::StatusChanged {
                        session_id: session_id.clone(),
                        status: SessionStatus::Active,
                    }).await;
                }
            }
        }
        WatchEvent::NewMessage { session_id, .. } => {
            // Bump message count + last_activity
            let _ = repos.increment_message_count(session_id).await;
        }
    }
}

async fn check_status_transitions(
    repos: &SessionTrackerRepos,
    event_tx: &mpsc::Sender<SessionWatcherEvent>,
) {
    let live_sessions = match repos
        .list_sessions_by_status(&[SessionStatus::Active, SessionStatus::Idle])
        .await
    {
        Ok(s) => s,
        Err(e) => {
            warn!("Status tick: failed to query live sessions: {e}");
            return;
        }
    };

    let now = Utc::now();
    for session in live_sessions {
        let idle_secs = session
            .last_activity
            .map(|t| (now - t).num_seconds())
            .unwrap_or(i64::MAX);

        let new_status = if idle_secs >= COMPLETED_THRESHOLD_SECS {
            Some(SessionStatus::Completed)
        } else if idle_secs >= IDLE_THRESHOLD_SECS && session.status == SessionStatus::Active {
            Some(SessionStatus::Idle)
        } else {
            None
        };

        if let Some(status) = new_status {
            if let Err(e) = repos.update_session_status(&session.session_id, &status).await {
                warn!("Status tick: failed to update {}: {e}", session.session_id);
                continue;
            }
            let _ = event_tx.send(SessionWatcherEvent::StatusChanged {
                session_id: session.session_id,
                status,
            }).await;
        }
    }
}
```

**Step 3: Add `pub mod services;` to `lib.rs`**

Modify `crates/app-core/src/lib.rs` — add `pub mod services;` after the existing modules.

**Step 4: Verify it compiles**

Run: `cargo build -p app-core`
Expected: success

**Step 5: Commit**

```bash
git add crates/app-core/src/services/ crates/app-core/src/lib.rs
git commit -m "feat(app-core): add SessionWatcherService"
```

---

### Task 4: Wire service into `init.rs` and `EventChannels`

**Files:**
- Modify: `crates/app-core/src/init.rs`

**Step 1: Add `session_watcher_rx` to `EventChannels`**

In `EventChannels` struct (around line 23), add:

```rust
pub session_watcher_rx: Option<mpsc::Receiver<crate::services::session_watcher::SessionWatcherEvent>>,
```

**Step 2: Start the service in `AppCore::init`**

After creating `session_tracker_repos` (line 84) and before building the `AppCore` struct (line 308), add:

```rust
// Start session watcher service (optional — graceful if ~/.claude missing).
let session_watcher_rx = crate::services::session_watcher::start(
    session_tracker_repos.clone(),
    shutdown_token.clone(),
);
if session_watcher_rx.is_some() {
    info!("session watcher service started");
}
```

**Step 3: Add to `EventChannels` construction**

In the `EventChannels` construction (around line 340), add `session_watcher_rx`:

```rust
let channels = EventChannels {
    intervention_rx,
    domain_event_bus,
    pipeline_rx,
    auto_focus_rx,
    nudge_rx,
    dashboard_tick_rx,
    dashboard_poll_interval_secs,
    session_watcher_rx,
};
```

**Step 4: Verify it compiles**

Run: `cargo build -p app-core`
Expected: success

**Step 5: Commit**

```bash
git add crates/app-core/src/init.rs
git commit -m "feat(app-core): wire SessionWatcherService into init and EventChannels"
```

---

### Task 5: Wire Tauri event emission in desktop adapter

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

**Step 1: Add session watcher forwarder to `wire_event_channels`**

After the existing pipeline events block (around line 182), before the closing brace of `wire_event_channels`, add:

```rust
// Session watcher → Tauri events (new sessions + status changes)
if let Some(session_watcher_rx) = channels.session_watcher_rx {
    spawn_channel_forwarder(
        session_watcher_rx,
        app_handle,
        shutdown,
        |handle, event| {
            match event {
                app_core::services::session_watcher::SessionWatcherEvent::NewSession { session } => {
                    let payload = events::SessionNewPayload {
                        session_id: session.session_id,
                        project_path: session.project_path,
                        project_name: session.project_name,
                        status: session.status.as_str().to_string(),
                    };
                    if let Err(e) = handle.emit(events::SESSION_NEW, payload) {
                        warn!("failed to emit session:new event: {e}");
                    }
                }
                app_core::services::session_watcher::SessionWatcherEvent::StatusChanged {
                    session_id,
                    status,
                } => {
                    let payload = events::SessionStatusChangedPayload {
                        session_id,
                        status: status.as_str().to_string(),
                    };
                    if let Err(e) = handle.emit(events::SESSION_STATUS, payload) {
                        warn!("failed to emit session:status event: {e}");
                    }
                }
            }
        },
    );
}
```

**Step 2: Verify the full build compiles**

Run: `cargo build --workspace`
Expected: success

**Step 3: Commit**

```bash
git add crates/desktop/src/app_core.rs
git commit -m "feat(desktop): wire session watcher events to Tauri emitter"
```

---

### Task 6: Add unit tests for status transition logic

**Files:**
- Modify: `crates/app-core/src/services/session_watcher.rs`

**Step 1: Write tests for `check_status_transitions`**

Add at the bottom of `session_watcher.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use feature_session_tracker::types::SessionStatus;
    use storage::StoragePool;
    use tokio::sync::mpsc;

    async fn setup_repos() -> SessionTrackerRepos {
        let pool = StoragePool::connect_in_memory().await.unwrap();
        StoragePool::run_feature_migrations(
            pool.inner(),
            &feature_session_tracker::SessionTrackerFeature::migrations_static(),
        )
        .await
        .unwrap();
        SessionTrackerRepos::new(pool.inner().clone())
    }

    fn make_session(id: &str, status: SessionStatus, last_activity: DateTime<Utc>) -> TrackedSession {
        TrackedSession {
            session_id: id.to_string(),
            project_path: "/test".to_string(),
            project_name: "test".to_string(),
            jsonl_path: format!("/tmp/{id}.jsonl"),
            status,
            first_message_preview: None,
            message_count: 0,
            git_branch: None,
            last_activity: Some(last_activity),
            created_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn active_session_becomes_idle_after_30_min() {
        let repos = setup_repos().await;
        let old = Utc::now() - chrono::Duration::minutes(35);
        repos.upsert_session(&make_session("s1", SessionStatus::Active, old)).await.unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        check_status_transitions(&repos, &tx).await;

        let event = rx.try_recv().unwrap();
        match event {
            SessionWatcherEvent::StatusChanged { session_id, status } => {
                assert_eq!(session_id, "s1");
                assert_eq!(status, SessionStatus::Idle);
            }
            _ => panic!("expected StatusChanged"),
        }
    }

    #[tokio::test]
    async fn idle_session_becomes_completed_after_60_min() {
        let repos = setup_repos().await;
        let old = Utc::now() - chrono::Duration::minutes(65);
        repos.upsert_session(&make_session("s1", SessionStatus::Idle, old)).await.unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        check_status_transitions(&repos, &tx).await;

        let event = rx.try_recv().unwrap();
        match event {
            SessionWatcherEvent::StatusChanged { session_id, status } => {
                assert_eq!(session_id, "s1");
                assert_eq!(status, SessionStatus::Completed);
            }
            _ => panic!("expected StatusChanged"),
        }
    }

    #[tokio::test]
    async fn recent_active_session_stays_active() {
        let repos = setup_repos().await;
        let recent = Utc::now() - chrono::Duration::minutes(5);
        repos.upsert_session(&make_session("s1", SessionStatus::Active, recent)).await.unwrap();

        let (tx, mut rx) = mpsc::channel(16);
        check_status_transitions(&repos, &tx).await;

        assert!(rx.try_recv().is_err(), "no transition expected");
    }
}
```

**Step 2: Run tests**

Run: `cargo nextest run -p app-core -E 'test(session_watcher)'`
Expected: all 3 tests PASS

**Step 3: Commit**

```bash
git add crates/app-core/src/services/session_watcher.rs
git commit -m "test(app-core): add session watcher status transition tests"
```

---

### Task 7: Run full workspace checks

**Step 1: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features`
Expected: 0 warnings (fix any that appear)

**Step 2: Run formatter**

Run: `cargo fmt --all --check`
Expected: no formatting issues (fix any that appear)

**Step 3: Run all tests**

Run: `cargo nextest run --workspace`
Expected: all pass

**Step 4: Final commit if any fixes were needed**

```bash
git add -A
git commit -m "chore: fix clippy/fmt issues from session watcher wiring"
```
