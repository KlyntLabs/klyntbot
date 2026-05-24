use std::sync::Arc;

use bus::DomainEventBus;
use tokio::sync::mpsc;

/// Bundle of receiver channels that callers wire to their transport (Tauri, SSE, etc.).
pub struct EventChannels {
    pub intervention_rx: mpsc::Receiver<feature_coaching::router::DeliveredIntervention>,
    pub domain_event_bus: Arc<DomainEventBus>,
    pub pipeline_rx: tokio::sync::broadcast::Receiver<::cognitive::PipelineEvent>,
    pub nudge_rx: Option<mpsc::Receiver<feature_productivity::types::NudgeRecord>>,
    pub dashboard_tick_rx:
        Option<tokio::sync::broadcast::Receiver<feature_productivity::ActivityTick>>,
    pub dashboard_poll_interval_secs: u64,
    pub distraction_alert_rx:
        Option<tokio::sync::mpsc::Receiver<feature_productivity::distraction::DistractionAlert>>,
}

/// Build EventChannels by extracting registered receivers from the FeatureHost.
pub fn build(
    host: &crate::plugin::host::FeatureHost,
    domain_event_bus: Arc<DomainEventBus>,
    pipeline_rx: tokio::sync::broadcast::Receiver<::cognitive::PipelineEvent>,
) -> EventChannels {
    let prod_bundle = host.get::<crate::plugins::productivity::ProductivityInitResult>();

    EventChannels {
        intervention_rx: host
            .get::<crate::plugins::coaching::CoachingInitResult>()
            .and_then(|b| b.intervention_rx.lock().unwrap().take())
            .expect("coaching plugin always provides intervention_rx"),
        domain_event_bus,
        pipeline_rx,
        nudge_rx: prod_bundle
            .as_ref()
            .and_then(|b| b.nudge_rx.lock().unwrap().take()),
        dashboard_tick_rx: prod_bundle
            .as_ref()
            .and_then(|b| b.dashboard_tick_rx.lock().unwrap().take()),
        dashboard_poll_interval_secs: prod_bundle
            .as_ref()
            .map(|b| b.dashboard_poll_interval_secs)
            .unwrap_or(60),
        distraction_alert_rx: prod_bundle
            .as_ref()
            .and_then(|b| b.distraction_alert_rx.lock().unwrap().take()),
    }
}
