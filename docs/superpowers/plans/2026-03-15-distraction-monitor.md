# Distraction Monitor Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Wire the existing DistractionInterceptor into a background monitor that emits distraction alerts during active focus sessions, enabling the already-built frontend overlays.

**Architecture:** A new `DistractionMonitor` subscribes to the `ActivityTick` broadcast, checks for active focus sessions via `FocusManager`, evaluates each tick through `DistractionInterceptor`, and sends `DistractionAlert` values through an `mpsc` channel. The transport layer (Tauri) wires the channel to `distraction:intervention` and `distraction:detected` events that the frontend already listens for.

**Tech Stack:** Rust, tokio (broadcast + mpsc channels), SQLite (in-memory for tests)

**Spec:** `docs/superpowers/specs/2026-03-15-distraction-monitor-design.md`

---

## Chunk 1: Core Monitor + Tests

### Task 1: Fix `DistractionDetectedPayload` field mismatch

The Rust struct has `elapsed_secs: u64` but the frontend TypeScript interface expects `reason: string`. Neither has ever been used (no event is emitted yet). Align Rust to match the frontend.

**Files:**
- Modify: `crates/desktop-shared/src/events.rs:570-576`

- [ ] **Step 1: Fix the payload struct**

Change in `crates/desktop-shared/src/events.rs`:

```rust
// Before (line 575):
pub elapsed_secs: u64,

// After:
pub reason: String,
```

- [ ] **Step 2: Verify it compiles**

Run: `cargo build --workspace 2>&1 | head -30`
Expected: success (no code references `elapsed_secs` since the event was never emitted)

- [ ] **Step 3: Commit**

```bash
git add crates/desktop-shared/src/events.rs
git commit -m "fix(desktop-shared): align DistractionDetectedPayload with frontend

Replace elapsed_secs: u64 with reason: String to match the TypeScript
DistractionDetectedPayload interface in DistractionInterventionBanner.tsx."
```

---

### Task 2: Create `DistractionMonitor` with tests (TDD)

**Files:**
- Create: `crates/feature-productivity/src/distraction/monitor.rs`
- Modify: `crates/feature-productivity/src/distraction/mod.rs`
- Modify: `crates/feature-productivity/src/lib.rs` (re-export)

- [ ] **Step 1: Add module declaration**

In `crates/feature-productivity/src/distraction/mod.rs`, add after line 3:

```rust
pub mod monitor;
```

And add to the re-exports at the bottom:

```rust
pub use monitor::{DistractionAlert, DistractionMonitor};
```

- [ ] **Step 2: Write the `DistractionAlert` struct and `DistractionMonitor` skeleton**

Create `crates/feature-productivity/src/distraction/monitor.rs`:

```rust
//! DistractionMonitor — background task that checks for distracting apps
//! during active focus sessions and sends alerts to the transport layer.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::interceptor::{DistractionInterceptor, InterceptDecision};
use crate::config::FocusConfig;
use crate::focus::FocusManager;
use crate::types::{ActivityTick, SessionType};

/// Alert sent when a distracting app is detected during a focus session.
#[derive(Debug, Clone)]
pub struct DistractionAlert {
    pub session_id: String,
    pub app_name: String,
    pub window_title: Option<String>,
    pub previous_app: String,
    pub previous_context: String,
    pub needs_llm: bool,
}

pub struct DistractionMonitor {
    tick_rx: broadcast::Receiver<ActivityTick>,
    focus_manager: Arc<FocusManager>,
    interceptor: Arc<Mutex<DistractionInterceptor>>,
    config: FocusConfig,
    cancel: CancellationToken,
}

impl DistractionMonitor {
    pub fn new(
        tick_rx: broadcast::Receiver<ActivityTick>,
        focus_manager: Arc<FocusManager>,
        interceptor: Arc<Mutex<DistractionInterceptor>>,
        config: FocusConfig,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            tick_rx,
            focus_manager,
            interceptor,
            config,
            cancel,
        }
    }

    /// Spawn the monitoring task. Returns the alert receiver.
    /// Shutdown is via CancellationToken (JoinHandle intentionally dropped).
    pub fn start(self) -> mpsc::Receiver<DistractionAlert> {
        let (tx, rx) = mpsc::channel(32);
        tokio::spawn(self.run(tx));
        rx
    }

    async fn run(mut self, tx: mpsc::Sender<DistractionAlert>) {
        info!("DistractionMonitor started");
        let mut cooldowns: HashMap<String, Instant> = HashMap::new();
        let mut previous_app = String::new();
        let mut previous_context = String::new();

        loop {
            tokio::select! {
                _ = self.cancel.cancelled() => {
                    info!("DistractionMonitor shutting down");
                    break;
                }
                result = self.tick_rx.recv() => {
                    match result {
                        Ok(tick) => {
                            self.process_tick(
                                &tick,
                                &tx,
                                &mut cooldowns,
                                &mut previous_app,
                                &mut previous_context,
                            ).await;
                        }
                        Err(broadcast::error::RecvError::Lagged(n)) => {
                            warn!("DistractionMonitor lagged {n} ticks");
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            info!("Tick channel closed, stopping DistractionMonitor");
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn process_tick(
        &self,
        tick: &ActivityTick,
        tx: &mpsc::Sender<DistractionAlert>,
        cooldowns: &mut HashMap<String, Instant>,
        previous_app: &mut String,
        previous_context: &mut String,
    ) {
        // 1. Skip idle ticks
        if tick.is_idle {
            return;
        }

        // 2. Skip if soft block disabled
        if !self.config.soft_block_enabled {
            return;
        }

        // 3. Check for active focus session
        let session = match self.focus_manager.get_active().await {
            Ok(Some(s)) => s,
            Ok(None) => {
                // No active session — update previous context
                previous_app.clone_from(&tick.app_name);
                if let Some(ref title) = tick.window_title {
                    previous_context.clone_from(title);
                }
                return;
            }
            Err(e) => {
                debug!("Failed to check active session: {e}");
                return;
            }
        };

        // 4. Skip break sessions
        if session.session_type == SessionType::Break {
            return;
        }

        // 5. Check cooldown
        let (cooldown_key, _) =
            DistractionInterceptor::make_key(&tick.app_name, tick.window_title.as_deref());
        if let Some(last_alert) = cooldowns.get(&cooldown_key) {
            if last_alert.elapsed().as_secs() < self.config.soft_block_cooldown_secs {
                return;
            }
        }

        // 6. Evaluate via interceptor
        let decision = self
            .interceptor
            .lock()
            .await
            .evaluate(&tick.app_name, tick.window_title.as_deref())
            .await;

        match decision {
            InterceptDecision::ShowOverlay { needs_llm } => {
                // 7a. Record distraction on the session
                if let Err(e) = self
                    .focus_manager
                    .record_distraction_for(session, &tick.app_name)
                    .await
                {
                    warn!("Failed to record distraction: {e}");
                }

                // 7b. Send alert
                let alert = DistractionAlert {
                    session_id: session.id.clone(),
                    app_name: tick.app_name.clone(),
                    window_title: tick.window_title.clone(),
                    previous_app: previous_app.clone(),
                    previous_context: previous_context.clone(),
                    needs_llm,
                };

                if tx.send(alert).await.is_err() {
                    debug!("DistractionAlert receiver dropped");
                }

                // 7c. Record cooldown
                cooldowns.insert(cooldown_key, Instant::now());

                // Lazy prune expired cooldowns
                let cooldown_secs = self.config.soft_block_cooldown_secs;
                cooldowns.retain(|_, instant| instant.elapsed().as_secs() < cooldown_secs);
            }
            InterceptDecision::Allow { .. } => {
                // 8. Update previous context from productive ticks
                previous_app.clone_from(&tick.app_name);
                if let Some(ref title) = tick.window_title {
                    previous_context.clone_from(title);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::ProductivityRepos;
    use crate::ProductivityFeature;
    use chrono::Utc;

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(
            &inner,
            &ProductivityFeature::migrations_static(),
        )
        .await
        .unwrap();
        inner
    }

    fn make_tick(app_name: &str, window_title: Option<&str>, is_idle: bool) -> ActivityTick {
        ActivityTick {
            timestamp: Utc::now(),
            app_name: app_name.to_string(),
            bundle_id: None,
            window_title: window_title.map(|s| s.to_string()),
            site_name: None,
            url: None,
            category_id: None,
            category_type: None,
            is_idle,
            idle_secs: 0.0,
            is_context_switch: false,
            project_id: None,
        }
    }

    async fn setup() -> (
        broadcast::Sender<ActivityTick>,
        Arc<FocusManager>,
        Arc<Mutex<DistractionInterceptor>>,
        FocusConfig,
    ) {
        let pool = setup_pool().await;
        let repos = ProductivityRepos::new(pool.clone());
        let config = FocusConfig::default();
        let mgr = Arc::new(FocusManager::new(repos.clone(), config.clone()));
        let interceptor = Arc::new(Mutex::new(DistractionInterceptor::new(
            config.clone(),
            repos.learned_rules.clone(),
        )));
        let (tx, _) = broadcast::channel(128);
        (tx, mgr, interceptor, config)
    }

    #[tokio::test]
    async fn emits_alert_during_focus_session() {
        let (tx, mgr, interceptor, config) = setup().await;
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        // Start a focus session
        mgr.start_session(None, None, Some(25)).await.unwrap();

        // Send a distracting tick (Netflix is always distracting)
        tx.send(make_tick("Netflix", None, false)).unwrap();

        // Should receive an alert
        let alert = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            alert_rx.recv(),
        )
        .await
        .expect("timeout waiting for alert")
        .expect("channel closed");

        assert_eq!(alert.app_name, "Netflix");
        assert!(!alert.needs_llm);

        cancel.cancel();
    }

    #[tokio::test]
    async fn no_alert_without_focus_session() {
        let (tx, _mgr, interceptor, config) = setup().await;
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&_mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        // No focus session started — send distracting tick
        tx.send(make_tick("Netflix", None, false)).unwrap();

        // Should NOT receive an alert
        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            alert_rx.recv(),
        )
        .await;

        assert!(result.is_err(), "Should timeout — no alert expected");
        cancel.cancel();
    }

    #[tokio::test]
    async fn no_alert_during_break_session() {
        let (tx, mgr, interceptor, config) = setup().await;
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        // Start a break session (not focus)
        mgr.start_break_session(5).await.unwrap();

        tx.send(make_tick("Netflix", None, false)).unwrap();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            alert_rx.recv(),
        )
        .await;

        assert!(result.is_err(), "Should timeout — no alert during break");
        cancel.cancel();
    }

    #[tokio::test]
    async fn no_alert_for_idle_ticks() {
        let (tx, mgr, interceptor, config) = setup().await;
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        mgr.start_session(None, None, Some(25)).await.unwrap();

        // Idle tick — should be skipped
        tx.send(make_tick("Netflix", None, true)).unwrap();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            alert_rx.recv(),
        )
        .await;

        assert!(result.is_err(), "Should timeout — idle tick ignored");
        cancel.cancel();
    }

    #[tokio::test]
    async fn cooldown_suppresses_repeated_alerts() {
        let (tx, mgr, interceptor, config) = setup().await;
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        mgr.start_session(None, None, Some(25)).await.unwrap();

        // First tick — should alert
        tx.send(make_tick("Netflix", None, false)).unwrap();
        let alert = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            alert_rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("closed");
        assert_eq!(alert.app_name, "Netflix");

        // Second tick same app — should be suppressed by cooldown
        tx.send(make_tick("Netflix", None, false)).unwrap();

        let result = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            alert_rx.recv(),
        )
        .await;
        assert!(result.is_err(), "Should timeout — cooldown active");

        cancel.cancel();
    }

    #[tokio::test]
    async fn cooldown_expires_allows_realert() {
        let (tx, mgr, interceptor, _) = setup().await;
        // Use 0-second cooldown so it expires immediately
        let config = FocusConfig {
            soft_block_cooldown_secs: 0,
            ..FocusConfig::default()
        };
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        mgr.start_session(None, None, Some(25)).await.unwrap();

        // First alert
        tx.send(make_tick("Netflix", None, false)).unwrap();
        alert_rx.recv().await.expect("first alert");

        // Small delay so the 0-second cooldown expires
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;

        // Second alert — cooldown expired
        tx.send(make_tick("Netflix", None, false)).unwrap();
        let alert = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            alert_rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("closed");
        assert_eq!(alert.app_name, "Netflix");

        cancel.cancel();
    }

    #[tokio::test]
    async fn tracks_previous_context() {
        let (tx, mgr, interceptor, config) = setup().await;
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        mgr.start_session(None, None, Some(25)).await.unwrap();

        // Send productive tick first (Stack Overflow — productive keyword)
        tx.send(make_tick(
            "Google Chrome",
            Some("How to use Rust - Stack Overflow"),
            false,
        ))
        .unwrap();

        // Small yield to let it process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Now send distracting tick
        tx.send(make_tick("Netflix", None, false)).unwrap();

        let alert = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            alert_rx.recv(),
        )
        .await
        .expect("timeout")
        .expect("closed");

        assert_eq!(alert.previous_app, "Google Chrome");
        assert_eq!(
            alert.previous_context,
            "How to use Rust - Stack Overflow"
        );

        cancel.cancel();
    }

    #[tokio::test]
    async fn respects_cancellation() {
        let (tx, mgr, interceptor, config) = setup().await;
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        // Cancel immediately
        cancel.cancel();

        // Channel should close
        let result = tokio::time::timeout(
            std::time::Duration::from_secs(2),
            alert_rx.recv(),
        )
        .await
        .expect("should not timeout");

        assert!(result.is_none(), "Channel should be closed after cancel");
    }

    #[tokio::test]
    async fn records_distraction_on_session() {
        let (tx, mgr, interceptor, config) = setup().await;
        let cancel = CancellationToken::new();
        let rx = tx.subscribe();

        let monitor = DistractionMonitor::new(
            rx,
            Arc::clone(&mgr),
            Arc::clone(&interceptor),
            config,
            cancel.clone(),
        );
        let mut alert_rx = monitor.start();

        mgr.start_session(None, None, Some(25)).await.unwrap();

        tx.send(make_tick("Netflix", None, false)).unwrap();
        alert_rx.recv().await.expect("alert");

        // Small yield for record_distraction_for to complete
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Check session was updated
        let session = mgr.get_active().await.unwrap().unwrap();
        assert_eq!(session.interruptions, 1);
        assert_eq!(session.distraction_events.len(), 1);
        assert_eq!(session.distraction_events[0].app_name, "Netflix");

        cancel.cancel();
    }
}
```

- [ ] **Step 3: Add re-export in `lib.rs`**

In `crates/feature-productivity/src/lib.rs`, the `distraction` module is already `pub mod distraction;`. The `DistractionAlert` and `DistractionMonitor` types are re-exported through `distraction::monitor` which is now `pub mod`. No additional re-export needed — consumers use `feature_productivity::distraction::DistractionMonitor`.

- [ ] **Step 4: Run tests**

Run: `cargo nextest run -p feature-productivity -E 'test(distraction::monitor)'`
Expected: all 8 tests pass

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | head -30`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add crates/feature-productivity/src/distraction/monitor.rs crates/feature-productivity/src/distraction/mod.rs
git commit -m "feat(productivity): add DistractionMonitor background task

Subscribes to ActivityTick broadcast, checks for active focus sessions,
evaluates ticks through DistractionInterceptor, records distractions
on the session, and sends alerts via mpsc channel."
```

---

## Chunk 2: Wiring into Init + Transport

### Task 3: Wire `DistractionMonitor` into productivity init

**Files:**
- Modify: `crates/app-core/src/init/productivity.rs`

- [ ] **Step 1: Add `distraction_alert_rx` to `ProductivityResult`**

In `crates/app-core/src/init/productivity.rs`, add field to struct (after line 24):

```rust
pub distraction_alert_rx:
    Option<tokio::sync::mpsc::Receiver<feature_productivity::distraction::monitor::DistractionAlert>>,
```

- [ ] **Step 2: Create and start the monitor in `init_productivity`**

After the line `engine.start();` (line 95) and before the intelligence layer block, add:

```rust
// Start distraction monitor — watches for distracting apps during focus sessions.
let distraction_alert_rx = {
    let monitor_rx = engine.subscribe();
    let monitor = feature_productivity::distraction::DistractionMonitor::new(
        monitor_rx,
        Arc::clone(&mgr),
        Arc::clone(&interceptor),
        prod_config.focus.clone(),
        shutdown_token.child_token(),
    );
    Some(monitor.start())
};
```

- [ ] **Step 3: Add to the return tuple**

In the success tuple (around line 140), add `distraction_alert_rx` after `interceptor`:

```rust
// The tuple currently has 9 elements — add distraction_alert_rx as the 10th.
// Update the None tuple in the error/disabled branches to match (add None).
```

Specifically, change both the success return and the `None` tuples to include the new field. The `ProductivityResult` construction at the bottom also needs the new field.

- [ ] **Step 4: Verify it compiles**

Run: `cargo build -p app-core 2>&1 | head -30`
Expected: success

- [ ] **Step 5: Commit**

```bash
git add crates/app-core/src/init/productivity.rs
git commit -m "feat(app-core): wire DistractionMonitor into productivity init

Creates and starts the monitor after the engine, passing it a tick
broadcast subscription, the FocusManager, and DistractionInterceptor."
```

---

### Task 4: Add `distraction_alert_rx` to `EventChannels`

**Files:**
- Modify: `crates/app-core/src/init/mod.rs`

- [ ] **Step 1: Add field to `EventChannels`**

In `crates/app-core/src/init/mod.rs`, add after line 31 (`dashboard_poll_interval_secs`):

```rust
pub distraction_alert_rx:
    Option<tokio::sync::mpsc::Receiver<feature_productivity::distraction::monitor::DistractionAlert>>,
```

- [ ] **Step 2: Populate the field in `init_with_sender`**

In the `EventChannels` construction (around line 245), add:

```rust
distraction_alert_rx: productivity_result.distraction_alert_rx,
```

(Where `productivity_result` is the destructured `ProductivityResult` — match the existing destructuring pattern.)

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p app-core 2>&1 | head -30`
Expected: success

- [ ] **Step 4: Commit**

```bash
git add crates/app-core/src/init/mod.rs
git commit -m "feat(app-core): add distraction_alert_rx to EventChannels

Transport layer can now receive DistractionAlert values and wire
them to platform-specific events (Tauri, SSE, etc.)."
```

---

### Task 5: Wire Tauri event emission in `desktop/src/app_core.rs`

**Files:**
- Modify: `crates/desktop/src/app_core.rs`

- [ ] **Step 1: Add the distraction alert forwarder**

In `wire_event_channels`, after the nudge forwarder block (around line 153), add:

```rust
// Distraction alerts → Tauri events (intervention overlay + detected banner)
if let Some(distraction_rx) = channels.distraction_alert_rx {
    spawn_channel_forwarder(distraction_rx, app_handle, shutdown, |handle, alert| {
        // Emit intervention event (for DistractionOverlay.tsx)
        let intervention = desktop_shared::events::InterventionPayload {
            app_name: alert.app_name.clone(),
            window_title: alert.window_title.clone(),
            session_id: alert.session_id.clone(),
            needs_llm: alert.needs_llm,
            heuristic_verdict: if alert.needs_llm {
                "ambiguous".to_string()
            } else {
                "confident_distracting".to_string()
            },
        };
        if let Err(e) = handle.emit(
            desktop_shared::events::DISTRACTION_INTERVENTION,
            intervention,
        ) {
            warn!("failed to emit distraction intervention: {e}");
        }

        // Emit detected event (for DistractionInterventionBanner.tsx)
        let detected = desktop_shared::events::DistractionDetectedPayload {
            app_name: alert.app_name,
            session_id: alert.session_id,
            previous_app: alert.previous_app,
            previous_context: alert.previous_context,
            reason: "Distracting app detected during focus session".to_string(),
        };
        if let Err(e) = handle.emit(
            desktop_shared::events::DISTRACTION_DETECTED,
            detected,
        ) {
            warn!("failed to emit distraction detected: {e}");
        }
    });
}
```

- [ ] **Step 2: Add import for `DistractionAlert`**

The `spawn_channel_forwarder` closure receives the alert by value, so the type must be in scope. The existing imports pull from `feature_productivity` and `desktop_shared` — no new import needed since the type is inferred by the closure. Verify with a build.

- [ ] **Step 3: Verify it compiles**

Run: `cargo build -p desktop 2>&1 | head -30`
Expected: success

- [ ] **Step 4: Run all workspace tests**

Run: `cargo nextest run --workspace 2>&1 | tail -20`
Expected: all pass

- [ ] **Step 5: Run clippy**

Run: `cargo clippy --workspace --all-targets --all-features 2>&1 | head -30`
Expected: 0 warnings

- [ ] **Step 6: Commit**

```bash
git add crates/desktop/src/app_core.rs
git commit -m "feat(desktop): wire distraction alerts to Tauri events

Forwards DistractionAlert to both distraction:intervention (overlay)
and distraction:detected (banner) events, completing the pipeline
from ActivityTracker to frontend overlays."
```

---

### Task 6: End-to-end verification

- [ ] **Step 1: Run the full test suite**

Run: `cargo nextest run --workspace && cargo test --workspace --doc`
Expected: all pass

- [ ] **Step 2: Run clippy and fmt**

Run: `cargo clippy --workspace --all-targets --all-features && cargo fmt --all --check`
Expected: 0 warnings, formatting OK

- [ ] **Step 3: Build the desktop app**

Run: `cargo build -p desktop`
Expected: success

- [ ] **Step 4: Manual smoke test (optional)**

1. Run `cargo tauri dev`
2. Start a focus session from the UI
3. Switch to a browser and open YouTube or Netflix
4. Verify the distraction overlay/banner appears
5. Test "Allow briefly" and "This is work" buttons
6. Verify cooldown — switching back to the same app within 60s should not re-alert
