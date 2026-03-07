//! CoachingService — subscribes to DomainEventBus and runs the full coaching
//! pipeline: signal accumulation → trigger evaluation → pattern detection →
//! reasoning → intervention routing → feedback tracking.

use std::sync::Arc;

use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use bus::DomainEvent;
use cognitive::situation::UserSituation;

use crate::feedback::FeedbackTracker;
use crate::pattern_detector::PatternDetector;
use crate::reasoner::{CoachingReasonerHandler, ReasonerInput};
use crate::router::{DeliveredIntervention, InterventionRouter, RoutingResult};
use crate::signal_accumulator::SignalAccumulator;

/// Background service that processes domain events through the coaching pipeline.
pub struct CoachingService {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl CoachingService {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        mut event_rx: broadcast::Receiver<DomainEvent>,
        accumulator: Arc<Mutex<SignalAccumulator>>,
        detector: Arc<Mutex<PatternDetector>>,
        router: Arc<Mutex<InterventionRouter>>,
        feedback: Arc<Mutex<FeedbackTracker>>,
        situation: Arc<Mutex<UserSituation>>,
        reasoner: Arc<dyn CoachingReasonerHandler>,
        intervention_tx: mpsc::Sender<DeliveredIntervention>,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    result = event_rx.recv() => {
                        let event = match result {
                            Ok(e) => e,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("CoachingService lagged, skipped {n} events");
                                continue;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                        };

                        // 1. Push signal into accumulator
                        {
                            let mut acc = accumulator.lock().await;
                            acc.push_event(&event);
                        }

                        // 2. Incrementally update situation from event
                        update_situation_from_event(&situation, &event).await;

                        // 3. Evaluate triggers
                        let sit = situation.lock().await.clone();
                        let fired = {
                            let mut acc = accumulator.lock().await;
                            acc.evaluate(&sit)
                        };

                        if fired.is_empty() {
                            continue;
                        }

                        // 4. Process each fired trigger
                        for trigger in fired {
                            // Record in pattern detector
                            {
                                let mut det = detector.lock().await;
                                det.record_trigger(&trigger);
                            }

                            // Detect patterns
                            let patterns = {
                                let det = detector.lock().await;
                                det.detect_patterns()
                            };

                            // Build reasoner input
                            let input = ReasonerInput {
                                situation: sit.clone(),
                                trigger: trigger.clone(),
                                patterns,
                                relevant_memories: vec![],
                                recent_interventions: vec![],
                            };

                            // Call reasoner
                            let decision = match reasoner.reason(&input).await {
                                Ok(d) => d,
                                Err(e) => {
                                    warn!("Coaching reasoner failed: {e}");
                                    continue;
                                }
                            };

                            // Route intervention
                            let routing = {
                                let mut r = router.lock().await;
                                r.route(&decision, &trigger.condition_name)
                            };

                            match routing {
                                RoutingResult::Delivered(intervention) => {
                                    debug!(
                                        "Coaching intervention delivered: {} via {:?}",
                                        trigger.condition_name, intervention.intervention_type
                                    );
                                    // Record in feedback tracker
                                    {
                                        let mut fb = feedback.lock().await;
                                        fb.record_delivery(&intervention);
                                    }
                                    // Send to consumer
                                    let _ = intervention_tx.send(intervention).await;
                                }
                                RoutingResult::RateLimited { reason } => {
                                    debug!("Coaching intervention rate-limited: {reason}");
                                }
                                RoutingResult::Skipped => {}
                            }
                        }
                    }
                }
            }
        });

        Self {
            cancel_token: cancel,
            task_handle: Some(handle),
        }
    }

    pub async fn stop(&mut self) {
        self.cancel_token.cancel();
        if let Some(handle) = self.task_handle.take() {
            if let Err(e) = handle.await {
                warn!("CoachingService task panicked: {e}");
            }
        }
    }
}

/// Incrementally update UserSituation from a domain event.
async fn update_situation_from_event(situation: &Arc<Mutex<UserSituation>>, event: &DomainEvent) {
    let mut sit = situation.lock().await;
    match event {
        DomainEvent::DistractionDetected { .. } => {
            sit.distraction_risk = (sit.distraction_risk + 0.15).min(1.0);
        }
        DomainEvent::FocusSessionEnded { quality, .. } => {
            sit.focus_state = *quality;
        }
        DomainEvent::TaskDeferred { .. } => {
            sit.task_avoidance_detected = true;
        }
        DomainEvent::BudgetAlert { .. } => {
            sit.deadline_pressure = (sit.deadline_pressure + 0.2).min(1.0);
        }
        DomainEvent::ActivitySessionCompleted { .. } => {
            sit.hours_active_today += 0.5;
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasoner::{CoachingDecision, InterventionType};

    struct MockReasoner {
        decision: CoachingDecision,
    }

    #[async_trait::async_trait]
    impl CoachingReasonerHandler for MockReasoner {
        async fn reason(&self, _input: &ReasonerInput) -> common::Result<CoachingDecision> {
            Ok(self.decision.clone())
        }
    }

    #[tokio::test]
    async fn test_coaching_service_processes_distraction_events() {
        let accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
        let detector = Arc::new(Mutex::new(PatternDetector::new()));
        let router = Arc::new(Mutex::new(InterventionRouter::default()));
        let feedback = Arc::new(Mutex::new(FeedbackTracker::new()));
        let situation = Arc::new(Mutex::new(UserSituation {
            coaching_receptivity: 0.7,
            ..Default::default()
        }));

        let reasoner: Arc<dyn CoachingReasonerHandler> = Arc::new(MockReasoner {
            decision: CoachingDecision {
                should_intervene: true,
                confidence: 0.8,
                message: Some("Take a break!".into()),
                intervention_type: InterventionType::ChatMessage,
                reasoning: "test".into(),
                observations: vec![],
            },
        });

        let (intervention_tx, mut intervention_rx) = tokio::sync::mpsc::channel(64);
        let cancel = CancellationToken::new();
        let bus = bus::DomainEventBus::new(16);
        let event_rx = bus.subscribe();

        let _service = CoachingService::start(
            event_rx,
            accumulator,
            detector,
            router,
            feedback,
            situation,
            reasoner,
            intervention_tx,
            cancel.clone(),
        );

        // Push 3 distraction events to trigger distraction_streak
        for _ in 0..3 {
            bus.publish(DomainEvent::DistractionDetected {
                app: "reddit".into(),
                duration_secs: None,
                context: "test".into(),
            });
        }

        // Wait briefly for processing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Should have received an intervention
        let intervention = intervention_rx.try_recv();
        assert!(
            intervention.is_ok(),
            "Expected an intervention to be delivered"
        );
        assert_eq!(intervention.unwrap().message, "Take a break!");

        cancel.cancel();
    }

    #[tokio::test]
    async fn test_coaching_service_stops_gracefully() {
        let accumulator = Arc::new(Mutex::new(SignalAccumulator::new()));
        let detector = Arc::new(Mutex::new(PatternDetector::new()));
        let router = Arc::new(Mutex::new(InterventionRouter::default()));
        let feedback = Arc::new(Mutex::new(FeedbackTracker::new()));
        let situation = Arc::new(Mutex::new(UserSituation::default()));
        let reasoner: Arc<dyn CoachingReasonerHandler> = Arc::new(MockReasoner {
            decision: CoachingDecision {
                should_intervene: false,
                confidence: 0.0,
                message: None,
                intervention_type: InterventionType::None,
                reasoning: "test".into(),
                observations: vec![],
            },
        });

        let (tx, _rx) = tokio::sync::mpsc::channel(64);
        let cancel = CancellationToken::new();
        let bus = bus::DomainEventBus::new(16);

        let mut service = CoachingService::start(
            bus.subscribe(),
            accumulator,
            detector,
            router,
            feedback,
            situation,
            reasoner,
            tx,
            cancel.clone(),
        );

        service.stop().await;
        // Should not panic or hang
    }
}
