use bus::domain_events::WakeType;
use bus::{DomainEvent, DomainEventBus};
use config::WakeDeliveryConfig;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

/// Summary of what happened during absence — for UI rendering.
#[derive(Debug, Clone)]
pub struct WakePanel {
    pub greeting: String,
    pub away_secs: u64,
    pub wake_type: WakeType,
    pub focus_suspended: Option<FocusSuspendedInfo>,
    pub immediate_jobs_run: usize,
    pub deferred_jobs_pending: usize,
    pub expired_jobs: usize,
}

#[derive(Debug, Clone)]
pub struct FocusSuspendedInfo {
    pub remaining_secs: u64,
    pub phase_name: String,
}

/// Build the greeting string.
pub fn build_greeting(away_secs: u64, wake_type: WakeType) -> String {
    let hour = jiff::Zoned::now().hour() as u32;
    let period = match hour {
        5..=11 => "Good morning",
        12..=16 => "Good afternoon",
        17..=21 => "Good evening",
        _ => "Welcome back",
    };
    let duration_str = humanize_duration(away_secs);

    match (wake_type, away_secs > 3600) {
        (WakeType::FromSleep, true) => format!("{period}. You were away for {duration_str}."),
        (WakeType::FromSleep, false) => format!("{period}. Quick nap — {duration_str}."),
        (WakeType::FromIdle, true) => format!("{period}. You stepped away for {duration_str}."),
        (WakeType::FromIdle, false) => String::new(),
    }
}

fn humanize_duration(total_secs: u64) -> String {
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    match (hours, minutes) {
        (0, m) => format!("{m}m"),
        (h, 0) => format!("{h}h"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// Compute quiet period seconds based on time of day.
pub fn quiet_period_secs(config: &WakeDeliveryConfig) -> u64 {
    let hour = jiff::Zoned::now().hour() as u32;
    match hour {
        5..=11 => config.quiet_period_morning_secs,
        12..=16 => config.quiet_period_midday_secs,
        20..=23 | 0..=4 => config.quiet_period_evening_secs,
        _ => config.quiet_period_default_secs,
    }
}

/// The WakeOrchestrator — thin subscriber that collects "ready" signals
/// and sequences the user-facing wake experience.
pub struct WakeOrchestrator {
    bus: Arc<DomainEventBus>,
    config: WakeDeliveryConfig,
}

impl WakeOrchestrator {
    pub fn new(bus: Arc<DomainEventBus>, config: WakeDeliveryConfig) -> Self {
        Self { bus, config }
    }

    pub fn start(self) -> tokio::task::JoinHandle<()> {
        let mut rx = self.bus.subscribe();
        tokio::spawn(async move {
            let mut pending_wake: Option<WakeContext> = None;

            loop {
                let event = match rx.recv().await {
                    Ok(e) => e,
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("WakeOrchestrator lagged {n} events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                };

                match event {
                    DomainEvent::SystemDidWake {
                        away_secs,
                        wake_type,
                    } => {
                        if away_secs < self.config.min_absence_for_panel_secs {
                            continue;
                        }
                        pending_wake = Some(WakeContext {
                            away_secs,
                            wake_type,
                            focus_suspended: None,
                            immediate_count: 0,
                            deferred_count: 0,
                            expired_count: 0,
                        });
                    }
                    DomainEvent::FocusSessionSuspended {
                        remaining_secs,
                        phase_name,
                    } => {
                        if let Some(ref mut ctx) = pending_wake {
                            ctx.focus_suspended = Some(FocusSuspendedInfo {
                                remaining_secs,
                                phase_name,
                            });
                        }
                    }
                    DomainEvent::CronCatchUpReady {
                        immediate_count,
                        deferred_count,
                        expired_count,
                    } => {
                        if let Some(ref mut ctx) = pending_wake {
                            ctx.immediate_count = immediate_count;
                            ctx.deferred_count = deferred_count;
                            ctx.expired_count = expired_count;
                        }
                    }
                    DomainEvent::UserReturned { .. } => {
                        if let Some(ctx) = pending_wake.take() {
                            // Spawn the quiet-period wait + panel emit so the event
                            // loop keeps processing other events during the delay.
                            let bus = self.bus.clone();
                            let config = self.config.clone();
                            tokio::spawn(async move {
                                let quiet = quiet_period_secs(&config);
                                tokio::time::sleep(Duration::from_secs(quiet)).await;

                                let greeting = build_greeting(ctx.away_secs, ctx.wake_type);

                                tracing::info!(
                                    "Wake panel: {} | focus={} immediate={} deferred={} expired={}",
                                    greeting,
                                    ctx.focus_suspended.is_some(),
                                    ctx.immediate_count,
                                    ctx.deferred_count,
                                    ctx.expired_count,
                                );

                                bus.publish(DomainEvent::WakePanelReady {
                                    greeting,
                                    away_secs: ctx.away_secs,
                                });
                            });
                        }
                    }
                    _ => {}
                }
            }
        })
    }
}

struct WakeContext {
    away_secs: u64,
    wake_type: WakeType,
    focus_suspended: Option<FocusSuspendedInfo>,
    immediate_count: usize,
    deferred_count: usize,
    expired_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn greeting_long_sleep() {
        let g = build_greeting(4 * 3600 + 12 * 60, WakeType::FromSleep);
        assert!(g.contains("4h 12m"));
        assert!(!g.is_empty());
    }

    #[test]
    fn greeting_short_idle_is_empty() {
        let g = build_greeting(300, WakeType::FromIdle);
        assert!(g.is_empty());
    }

    #[test]
    fn humanize_hours_and_minutes() {
        assert_eq!(humanize_duration(7380), "2h 3m");
        assert_eq!(humanize_duration(3600), "1h");
        assert_eq!(humanize_duration(300), "5m");
    }

    #[test]
    fn quiet_period_returns_positive() {
        let config = WakeDeliveryConfig::default();
        let secs = quiet_period_secs(&config);
        assert!(secs > 0 && secs <= 60);
    }
}
