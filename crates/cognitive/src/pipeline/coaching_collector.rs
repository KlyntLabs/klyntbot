//! Collects signals from coaching pattern detection.

use chrono::Utc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

pub struct CoachingCollector;

impl CoachingCollector {
    pub fn start(
        mut event_rx: broadcast::Receiver<bus::DomainEvent>,
        signal_tx: SignalSender,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(bus::DomainEvent::CoachingPatternDetected {
                                pattern_name, confidence, description, domain, signal_count, ..
                            }) => {
                                let rule_text = pattern_to_rule(&pattern_name, &description);
                                let signal = CognitiveSignal {
                                    source: SignalSource::CoachingPattern,
                                    content: rule_text,
                                    domain,
                                    confidence,
                                    context: SignalContext {
                                        source_count: signal_count as u32,
                                        ..Default::default()
                                    },
                                    timestamp: Utc::now(),
                                };
                                let _ = signal_tx.send(signal).await;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("CoachingCollector lagged {n}");
                            }
                            _ => {}
                        }
                    }
                }
            }
            debug!("CoachingCollector stopped");
        })
    }
}

fn pattern_to_rule(name: &str, description: &str) -> String {
    match name {
        "afternoon_energy_drop" => "Schedule demanding tasks in the morning; take breaks in the afternoon when energy drops".into(),
        "chronic_task_avoidance" => "Break avoided tasks into smaller steps to overcome procrastination".into(),
        "habitual_context_switching" => "Batch similar tasks together to reduce context switching overhead".into(),
        "declining_focus_quality" => "Take a break when focus quality starts declining".into(),
        "recurring_budget_pressure" => "Review spending patterns when budget pressure is detected".into(),
        "study_streak_at_risk" => "Complete at least one review session to maintain the study streak".into(),
        "retention_decay_detected" => "Schedule review sessions for domains with declining retention".into(),
        "learning_momentum_create_heavy" => "Balance content creation with review sessions to avoid review backlog".into(),
        _ => description.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_pattern() {
        let text = pattern_to_rule("afternoon_energy_drop", "");
        assert!(text.contains("morning"));
    }

    #[test]
    fn test_unknown_pattern_uses_description() {
        assert_eq!(pattern_to_rule("unknown", "Custom description"), "Custom description");
    }
}
