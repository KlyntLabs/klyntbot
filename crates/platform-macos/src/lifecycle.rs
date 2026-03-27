use std::time::{Duration, Instant};

/// Lifecycle event emitted by the state machine.
#[derive(Debug, Clone, PartialEq)]
pub enum LifecycleEvent {
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
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WakeType {
    FromSleep,
    FromIdle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleState {
    Active,
    Idle,
    Sleeping,
    WakingGrace,
}

pub struct LifecycleConfig {
    pub idle_threshold_secs: u64,
    pub presence_threshold_secs: u64,
    pub wake_grace_period_secs: u64,
}

pub struct LifecycleStateMachine {
    state: LifecycleState,
    config: LifecycleConfig,
    idle_started_at: Option<Instant>,
    sleep_started_at: Option<Instant>,
    last_away_duration: Duration,
}

impl LifecycleStateMachine {
    pub fn new(config: LifecycleConfig) -> Self {
        Self {
            state: LifecycleState::Active,
            config,
            idle_started_at: None,
            sleep_started_at: None,
            last_away_duration: Duration::ZERO,
        }
    }

    pub fn state(&self) -> LifecycleState {
        self.state
    }

    /// Called with CGEventSource idle time. Returns any events that should be emitted.
    pub fn on_idle_reading(&mut self, idle_secs: u64) -> Vec<LifecycleEvent> {
        match self.state {
            LifecycleState::Active => {
                if idle_secs >= self.config.idle_threshold_secs {
                    self.state = LifecycleState::Idle;
                    self.idle_started_at = Some(Instant::now());
                    vec![LifecycleEvent::UserBecameIdle { idle_secs }]
                } else {
                    vec![]
                }
            }
            LifecycleState::Idle => {
                if idle_secs <= self.config.presence_threshold_secs {
                    let absence_duration = self
                        .idle_started_at
                        .map(|t| t.elapsed())
                        .unwrap_or(Duration::ZERO);
                    self.state = LifecycleState::Active;
                    self.idle_started_at = None;
                    vec![LifecycleEvent::UserReturned {
                        absence_duration,
                        wake_type: WakeType::FromIdle,
                    }]
                } else {
                    // Still idle — no duplicate events
                    vec![]
                }
            }
            LifecycleState::Sleeping => {
                // Ignore idle readings while sleeping
                vec![]
            }
            LifecycleState::WakingGrace => {
                if idle_secs <= self.config.presence_threshold_secs {
                    let absence_duration = self.last_away_duration;
                    self.state = LifecycleState::Active;
                    vec![LifecycleEvent::UserReturned {
                        absence_duration,
                        wake_type: WakeType::FromSleep,
                    }]
                } else {
                    vec![]
                }
            }
        }
    }

    /// Called on NSWorkspace willSleep notification.
    pub fn on_will_sleep(&mut self) -> Vec<LifecycleEvent> {
        match self.state {
            LifecycleState::Active | LifecycleState::Idle => {
                self.state = LifecycleState::Sleeping;
                self.sleep_started_at = Some(Instant::now());
                self.idle_started_at = None;
                vec![LifecycleEvent::SystemWillSleep]
            }
            LifecycleState::Sleeping | LifecycleState::WakingGrace => {
                // Already sleeping or waking — ignore
                vec![]
            }
        }
    }

    /// Called on NSWorkspace didWake notification.
    pub fn on_did_wake(&mut self, away_duration: Duration) -> Vec<LifecycleEvent> {
        match self.state {
            LifecycleState::Sleeping => {
                self.last_away_duration = away_duration;
                self.state = LifecycleState::WakingGrace;
                self.sleep_started_at = None;
                vec![LifecycleEvent::SystemDidWake {
                    away_duration,
                    wake_type: WakeType::FromSleep,
                }]
            }
            _ => {
                // Unexpected — ignore
                vec![]
            }
        }
    }

    /// Called when the wake grace timer fires.
    pub fn on_grace_expired(&mut self) -> Vec<LifecycleEvent> {
        match self.state {
            LifecycleState::WakingGrace => {
                let absence_duration = self.last_away_duration;
                self.state = LifecycleState::Active;
                vec![LifecycleEvent::UserReturned {
                    absence_duration,
                    wake_type: WakeType::FromSleep,
                }]
            }
            _ => vec![],
        }
    }
}

/// Configuration for [`LifecycleMonitor`] (mirrors `config::LifecycleConfig` fields so that
/// `platform-macos` does not need to depend on the higher-layer `config` crate).
pub struct MonitorConfig {
    pub idle_threshold_secs: u64,
    pub presence_threshold_secs: u64,
    pub wake_grace_period_secs: u64,
    pub active_poll_interval_secs: u64,
    pub idle_poll_interval_secs: u64,
}

/// High-level monitor that wraps the state machine with real macOS APIs.
///
/// - Polls `CGEventSource` for idle detection via [`crate::input::seconds_since_last_input`]
/// - Manages grace period timer after wake
/// - NSWorkspace observers are stubbed for now (TODO: wire objc2 blocks)
#[cfg(target_os = "macos")]
pub struct LifecycleMonitor {
    shutdown: tokio::sync::watch::Sender<bool>,
}

#[cfg(target_os = "macos")]
impl LifecycleMonitor {
    pub fn start(
        config: MonitorConfig,
        callback: impl Fn(LifecycleEvent) + Send + Sync + 'static,
    ) -> Self {
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
        let callback = std::sync::Arc::new(callback);

        let sm_config = LifecycleConfig {
            idle_threshold_secs: config.idle_threshold_secs,
            presence_threshold_secs: config.presence_threshold_secs,
            wake_grace_period_secs: config.wake_grace_period_secs,
        };

        let active_poll = std::time::Duration::from_secs(config.active_poll_interval_secs);
        let idle_poll = std::time::Duration::from_secs(config.idle_poll_interval_secs);

        let cb = callback.clone();
        tokio::spawn(async move {
            let mut sm = LifecycleStateMachine::new(sm_config);
            let mut poll_interval = tokio::time::interval(active_poll);
            poll_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                tokio::select! {
                    _ = shutdown_rx.changed() => break,
                    _ = poll_interval.tick() => {
                        let idle = Self::get_idle_secs();
                        let events = sm.on_idle_reading(idle);
                        for e in events { cb(e); }

                        // Adapt polling interval based on state
                        let target = match sm.state() {
                            LifecycleState::Active => active_poll,
                            _ => idle_poll,
                        };
                        if poll_interval.period() != target {
                            poll_interval = tokio::time::interval(target);
                            poll_interval.set_missed_tick_behavior(
                                tokio::time::MissedTickBehavior::Skip,
                            );
                        }
                    }
                }
            }
        });

        tracing::info!("LifecycleMonitor started (idle polling active, NSWorkspace stubs)");
        Self {
            shutdown: shutdown_tx,
        }
    }

    pub fn stop(&self) {
        let _ = self.shutdown.send(true);
    }

    fn get_idle_secs() -> u64 {
        let secs = crate::input::seconds_since_last_input();
        secs.max(0.0) as u64
    }
}

/// Stub for non-macOS platforms.
#[cfg(not(target_os = "macos"))]
pub struct LifecycleMonitor;

#[cfg(not(target_os = "macos"))]
impl LifecycleMonitor {
    pub fn start(
        _config: MonitorConfig,
        _callback: impl Fn(LifecycleEvent) + Send + Sync + 'static,
    ) -> Self {
        Self
    }

    pub fn stop(&self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_config() -> LifecycleConfig {
        LifecycleConfig {
            idle_threshold_secs: 300,
            presence_threshold_secs: 2,
            wake_grace_period_secs: 10,
        }
    }

    #[test]
    fn starts_active() {
        let machine = LifecycleStateMachine::new(default_config());
        assert_eq!(machine.state(), LifecycleState::Active);
    }

    #[test]
    fn active_to_idle_on_threshold() {
        let mut machine = LifecycleStateMachine::new(default_config());
        let events = machine.on_idle_reading(300);
        assert_eq!(machine.state(), LifecycleState::Idle);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            LifecycleEvent::UserBecameIdle { idle_secs: 300 }
        ));
    }

    #[test]
    fn idle_to_active_on_input() {
        let mut machine = LifecycleStateMachine::new(default_config());
        // First go idle
        machine.on_idle_reading(300);
        assert_eq!(machine.state(), LifecycleState::Idle);
        // Then return
        let events = machine.on_idle_reading(1);
        assert_eq!(machine.state(), LifecycleState::Active);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            LifecycleEvent::UserReturned {
                wake_type: WakeType::FromIdle,
                ..
            }
        ));
    }

    #[test]
    fn active_to_sleeping_on_will_sleep() {
        let mut machine = LifecycleStateMachine::new(default_config());
        let events = machine.on_will_sleep();
        assert_eq!(machine.state(), LifecycleState::Sleeping);
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], LifecycleEvent::SystemWillSleep);
    }

    #[test]
    fn sleeping_to_waking_grace_on_did_wake() {
        let mut machine = LifecycleStateMachine::new(default_config());
        machine.on_will_sleep();
        let away = Duration::from_secs(3600);
        let events = machine.on_did_wake(away);
        assert_eq!(machine.state(), LifecycleState::WakingGrace);
        assert_eq!(events.len(), 1);
        assert!(
            matches!(events[0], LifecycleEvent::SystemDidWake { away_duration, wake_type: WakeType::FromSleep } if away_duration == away)
        );
    }

    #[test]
    fn waking_grace_to_active_on_user_input() {
        let mut machine = LifecycleStateMachine::new(default_config());
        machine.on_will_sleep();
        machine.on_did_wake(Duration::from_secs(3600));
        assert_eq!(machine.state(), LifecycleState::WakingGrace);
        let events = machine.on_idle_reading(1);
        assert_eq!(machine.state(), LifecycleState::Active);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            LifecycleEvent::UserReturned {
                wake_type: WakeType::FromSleep,
                ..
            }
        ));
    }

    #[test]
    fn waking_grace_expires_to_active() {
        let mut machine = LifecycleStateMachine::new(default_config());
        machine.on_will_sleep();
        machine.on_did_wake(Duration::from_secs(3600));
        assert_eq!(machine.state(), LifecycleState::WakingGrace);
        let events = machine.on_grace_expired();
        assert_eq!(machine.state(), LifecycleState::Active);
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            LifecycleEvent::UserReturned {
                wake_type: WakeType::FromSleep,
                ..
            }
        ));
    }

    #[test]
    fn no_duplicate_idle_events() {
        let mut machine = LifecycleStateMachine::new(default_config());
        // First idle reading
        let events1 = machine.on_idle_reading(300);
        assert_eq!(events1.len(), 1);
        assert_eq!(machine.state(), LifecycleState::Idle);
        // Second idle reading while already idle — no events
        let events2 = machine.on_idle_reading(400);
        assert!(events2.is_empty());
        assert_eq!(machine.state(), LifecycleState::Idle);
    }

    #[test]
    fn below_threshold_stays_active() {
        let mut machine = LifecycleStateMachine::new(default_config());
        let events = machine.on_idle_reading(200);
        assert_eq!(machine.state(), LifecycleState::Active);
        assert!(events.is_empty());
    }
}
