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
use crate::types::{ActivityTick, CategoryType, SessionType};

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
        // Track when user first dwelled on a distracting app (key, start_time)
        let mut dwell_start: Option<(String, Instant)> = None;

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
                                &mut dwell_start,
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
        dwell_start: &mut Option<(String, Instant)>,
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
                // No active session — update previous context and reset dwell
                *dwell_start = None;
                update_context(previous_app, previous_context, tick);
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

        // 4b. Skip apps already categorized as productive or neutral
        if tick
            .category_type
            .is_some_and(|ct| ct != CategoryType::Distracting)
        {
            *dwell_start = None;
            previous_app.clone_from(&tick.app_name);
            if let Some(ref title) = tick.window_title {
                previous_context.clone_from(title);
            }
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
                // 7. Grace period — only alert after sustained dwelling on the SAME app
                let grace_secs = self.config.cooldown_grace_secs;
                if grace_secs > 0 {
                    match dwell_start {
                        Some((ref key, start)) if *key == cooldown_key => {
                            if start.elapsed().as_secs() < grace_secs {
                                return;
                            }
                        }
                        Some(_) => {
                            // Switched to a different distracting app — restart timer
                            // (prevents indefinite suppression by alternating apps)
                            *dwell_start = Some((cooldown_key.clone(), Instant::now()));
                            return;
                        }
                        None => {
                            *dwell_start = Some((cooldown_key.clone(), Instant::now()));
                            return;
                        }
                    }
                }

                // Grace period elapsed — clear dwell tracker
                *dwell_start = None;

                // Clone session_id before record_distraction_for consumes session by value
                let session_id = session.id.clone();

                // 8a. Record distraction on the session (consumes session)
                if let Err(e) = self
                    .focus_manager
                    .record_distraction_for(session, &tick.app_name)
                    .await
                {
                    warn!("Failed to record distraction: {e}");
                }

                // 8b. Send alert
                let alert = DistractionAlert {
                    session_id,
                    app_name: tick.app_name.clone(),
                    window_title: tick.window_title.clone(),
                    previous_app: previous_app.clone(),
                    previous_context: previous_context.clone(),
                    needs_llm,
                };

                if tx.send(alert).await.is_err() {
                    debug!("DistractionAlert receiver dropped");
                }

                // 8c. Record cooldown
                cooldowns.insert(cooldown_key, Instant::now());

                // Lazy prune expired cooldowns
                let cooldown_secs = self.config.soft_block_cooldown_secs;
                cooldowns.retain(|_, instant| instant.elapsed().as_secs() < cooldown_secs);
            }
            InterceptDecision::Allow { .. } => {
                // 9. User on productive content — clear dwell tracker and update context
                *dwell_start = None;
                update_context(previous_app, previous_context, tick);
            }
        }
    }
}

#[inline]
fn update_context(previous_app: &mut String, previous_context: &mut String, tick: &ActivityTick) {
    previous_app.clone_from(&tick.app_name);
    if let Some(ref title) = tick.window_title {
        previous_context.clone_from(title);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::ProductivityRepos;
    

    async fn setup_pool() -> sqlx::SqlitePool {
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let inner = pool.inner().clone();
        storage::StoragePool::run_feature_migrations(&inner, &crate::productivity_migrations())
            .await
            .unwrap();
        inner
    }

    fn make_tick(app_name: &str, window_title: Option<&str>, is_idle: bool) -> ActivityTick {
        ActivityTick {
            timestamp: jiff::Timestamp::now(),
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
        let config = FocusConfig {
            cooldown_grace_secs: 0,
            ..FocusConfig::default()
        };
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
        tokio::task::yield_now().await;

        // Should receive an alert
        let alert = tokio::time::timeout(std::time::Duration::from_secs(10), alert_rx.recv())
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
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), alert_rx.recv()).await;

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

        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), alert_rx.recv()).await;

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

        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), alert_rx.recv()).await;

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
        tokio::task::yield_now().await;
        let alert = tokio::time::timeout(std::time::Duration::from_secs(10), alert_rx.recv())
            .await
            .expect("timeout")
            .expect("closed");
        assert_eq!(alert.app_name, "Netflix");

        // Second tick same app — should be suppressed by cooldown
        tx.send(make_tick("Netflix", None, false)).unwrap();

        let result =
            tokio::time::timeout(std::time::Duration::from_millis(100), alert_rx.recv()).await;
        assert!(result.is_err(), "Should timeout — cooldown active");

        cancel.cancel();
    }

    #[tokio::test]
    async fn cooldown_expires_allows_realert() {
        let (tx, mgr, interceptor, _) = setup().await;
        // Use 0-second cooldown so it expires immediately
        let config = FocusConfig {
            soft_block_cooldown_secs: 0,
            cooldown_grace_secs: 0,
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
        let alert = tokio::time::timeout(std::time::Duration::from_secs(10), alert_rx.recv())
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
        tokio::task::yield_now().await;

        let alert = tokio::time::timeout(std::time::Duration::from_secs(10), alert_rx.recv())
            .await
            .expect("timeout")
            .expect("closed");

        assert_eq!(alert.previous_app, "Google Chrome");
        assert_eq!(alert.previous_context, "How to use Rust - Stack Overflow");

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
        let result = tokio::time::timeout(std::time::Duration::from_secs(10), alert_rx.recv())
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
        // recv() acts as synchronization — alert is sent after record_distraction_for completes
        alert_rx.recv().await.expect("alert");

        // Check session was updated
        let session = mgr.get_active().await.unwrap().unwrap();
        assert_eq!(session.interruptions, 1);
        assert_eq!(session.distraction_events.len(), 1);
        assert_eq!(session.distraction_events[0].app_name, "Netflix");

        cancel.cancel();
    }
}
