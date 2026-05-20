//! CoachingService — receives AiSignals via mpsc channel and runs the full coaching
//! pipeline: signal accumulation → trigger evaluation → pattern detection →
//! reasoning → intervention routing → feedback tracking.

use std::sync::Arc;

use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use ai_core::AiSignal;
use bus::DomainEvent;
use cognitive::situation::UserSituation;

use crate::feedback::FeedbackTracker;
use crate::pattern_detector::PatternDetector;
use crate::reasoner::{CoachingReasonerHandler, InterventionType, ReasonerInput};
use crate::router::{DeliveredIntervention, InterventionRouter, RoutingResult};
use crate::signal_accumulator::{SignalAccumulator, TriggerFired};

/// Background service that processes AiSignals through the coaching pipeline.
pub struct CoachingService {
    cancel_token: CancellationToken,
    task_handle: Option<JoinHandle<()>>,
}

impl CoachingService {
    #[allow(clippy::too_many_arguments)]
    pub fn start(
        mut signal_rx: mpsc::Receiver<AiSignal>,
        accumulator: Arc<Mutex<SignalAccumulator>>,
        detector: Arc<Mutex<PatternDetector>>,
        router: Arc<Mutex<InterventionRouter>>,
        feedback: Arc<Mutex<FeedbackTracker>>,
        situation: Arc<Mutex<UserSituation>>,
        reasoner: Arc<dyn CoachingReasonerHandler>,
        intervention_tx: mpsc::Sender<DeliveredIntervention>,
        intervention_log: Option<storage::CoachingInterventionLogRepo>,
        cancel: CancellationToken,
    ) -> Self {
        let cancel_clone = cancel.clone();
        let handle = tokio::spawn(async move {
            // Focus mode state: when active, queue triggers instead of delivering
            let mut focus_active = false;
            let mut queued_triggers: Vec<TriggerFired> = Vec::new();

            loop {
                tokio::select! {
                    _ = cancel_clone.cancelled() => break,
                    Some(signal) = signal_rx.recv() => {
                        // Track focus mode state from raw_event
                        if let Some(ref event) = signal.raw_event {
                            match event {
                                DomainEvent::FocusSessionStarted { .. } => {
                                    debug!("Focus session started — coaching delivery paused");
                                    focus_active = true;
                                }
                                DomainEvent::FocusSessionEnded { quality, interruptions, duration_secs } => {
                                    debug!("Focus session ended — draining queued triggers");
                                    let q = *quality;
                                    let i = *interruptions;
                                    let d = *duration_secs;
                                    focus_active = false;

                                    // Deliver post-session debrief if there were queued triggers
                                    if !queued_triggers.is_empty() {
                                        let debrief = build_focus_debrief(
                                            &queued_triggers, q, i, d,
                                            &reasoner, &situation,
                                        ).await;

                                        if let Some(intervention) = debrief {
                                            {
                                                let mut fb = feedback.lock().await;
                                                fb.record_delivery(&intervention);
                                            }
                                            persist_intervention(intervention_log.as_ref(), &intervention).await;
                                            let _ = intervention_tx.send(intervention).await;
                                        }

                                        queued_triggers.clear();
                                    }
                                }
                                _ => {}
                            }
                        }

                        // 1. Push signal into accumulator (always, even during focus)
                        {
                            let mut acc = accumulator.lock().await;
                            acc.push_event(&signal);
                        }

                        // 2. Incrementally update situation from signal
                        update_situation_from_signal(&situation, &signal).await;

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
                            // Record in pattern detector (always)
                            {
                                let mut det = detector.lock().await;
                                det.record_trigger(&trigger);
                            }

                            // If focus is active, queue the trigger for post-session debrief
                            if focus_active {
                                debug!("Focus active — queuing trigger: {}", trigger.condition_name);
                                queued_triggers.push(trigger);
                                continue;
                            }

                            // Detect patterns (learning patterns no longer
                            // trigger coaching — they work under the hood)
                            let patterns = {
                                let det = detector.lock().await;
                                det.detect_patterns()
                            };

                            // Pass through LLM reasoner
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
                                    // Persist to intervention log
                                    persist_intervention(intervention_log.as_ref(), &intervention).await;
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

/// Build a consolidated post-session debrief from queued triggers.
async fn build_focus_debrief(
    queued: &[TriggerFired],
    quality: f64,
    interruptions: i32,
    duration_secs: i64,
    reasoner: &Arc<dyn CoachingReasonerHandler>,
    situation: &Arc<Mutex<UserSituation>>,
) -> Option<DeliveredIntervention> {
    let trigger_names: Vec<&str> = queued.iter().map(|t| t.condition_name.as_str()).collect();
    let duration_mins = duration_secs / 60;
    let quality_pct = (quality * 100.0).round() as i32;

    // Build a synthetic trigger for the debrief
    let debrief_trigger = TriggerFired {
        condition_name: "focus_session_debrief".into(),
        confidence: 0.9,
        context: format!(
            "Focus session ended: {}min, quality {}%, {} interruptions, queued triggers: {}",
            duration_mins,
            quality_pct,
            interruptions,
            trigger_names.join(", ")
        ),
    };

    let sit = situation.lock().await.clone();
    let input = ReasonerInput {
        situation: sit,
        trigger: debrief_trigger,
        patterns: vec![],
        relevant_memories: vec![],
        recent_interventions: vec![],
    };

    match reasoner.reason(&input).await {
        Ok(decision) if decision.should_intervene => {
            let message = decision.message.unwrap_or_else(|| {
                format!(
                    "Focus session complete: {}min, quality {}%. {} interruption(s) detected.",
                    duration_mins, quality_pct, interruptions
                )
            });
            Some(DeliveredIntervention {
                id: uuid::Uuid::new_v4().to_string(),
                intervention_type: InterventionType::ChatMessage,
                message,
                delivered_at: jiff::Timestamp::now(),
                trigger_name: "focus_session_debrief".into(),
                action_url: None,
            })
        }
        Ok(_) => {
            debug!("Reasoner decided no debrief needed for this focus session");
            None
        }
        Err(e) => {
            warn!("Debrief reasoner failed: {e}");
            // Fallback: deliver a heuristic debrief if there were significant issues
            if interruptions >= 2 || quality < 0.5 {
                Some(DeliveredIntervention {
                    id: uuid::Uuid::new_v4().to_string(),
                    intervention_type: InterventionType::ChatMessage,
                    message: format!(
                        "Focus session complete: {}min, quality {}%. {} interruption(s) — consider blocking distracting apps during your next session.",
                        duration_mins, quality_pct, interruptions
                    ),
                    delivered_at: jiff::Timestamp::now(),
                    trigger_name: "focus_session_debrief".into(),
                    action_url: None,
                })
            } else {
                None
            }
        }
    }
}

/// Incrementally update UserSituation from an AiSignal.
async fn update_situation_from_signal(situation: &Arc<Mutex<UserSituation>>, signal: &AiSignal) {
    let mut sit = situation.lock().await;
    match signal.event_kind {
        "DistractionDetected" => {
            sit.distraction_risk = (sit.distraction_risk + 0.15).min(1.0);
        }
        "FocusSessionStarted" => {
            sit.focus_state = 0.9;
            sit.distraction_risk = (sit.distraction_risk - 0.2).max(0.0);
        }
        "FocusSessionEnded" => {
            if let Some(amt) = signal.metrics.amount {
                sit.focus_state = amt;
            }
        }
        "TaskDeferred" => {
            sit.task_avoidance_detected = true;
        }
        "BudgetAlert" => {
            sit.deadline_pressure = (sit.deadline_pressure + 0.2).min(1.0);
        }
        "ActivitySessionCompleted" => {
            sit.hours_active_today += 0.5;
        }
        _ => {}
    }
}

/// Persist a delivered intervention to the coaching_intervention_log table.
async fn persist_intervention(
    log_repo: Option<&storage::CoachingInterventionLogRepo>,
    intervention: &DeliveredIntervention,
) {
    if let Some(repo) = log_repo {
        if let Err(e) = repo
            .insert(
                &intervention.id,
                intervention.intervention_type.as_str(),
                &intervention.message,
                &intervention.trigger_name,
                &intervention.delivered_at.to_string(),
                intervention.action_url.as_deref(),
            )
            .await
        {
            warn!("failed to persist intervention to log: {e}");
        }
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
    async fn test_coaching_service_processes_budget_signals() {
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
                message: Some("Check your budget!".into()),
                intervention_type: InterventionType::ChatMessage,
                reasoning: "test".into(),
                observations: vec![],
            },
        });

        let (intervention_tx, mut intervention_rx) = tokio::sync::mpsc::channel(64);
        let (signal_tx, signal_rx) = tokio::sync::mpsc::channel(64);
        let cancel = CancellationToken::new();

        let _service = CoachingService::start(
            signal_rx,
            accumulator.clone(),
            detector,
            router,
            feedback,
            situation.clone(),
            reasoner,
            intervention_tx,
            None,
            cancel.clone(),
        );

        // Send a budget alert AiSignal through the channel
        let budget_signal = ai_core::AiSignal {
            domain: ai_core::RecallDomain::Productivity,
            event_kind: "FocusAlert",
            importance: 0.9,
            salience: ai_core::SalienceVerdict::Extract,
            content: "food budget at 90%".into(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
            metrics: ai_core::AiMetrics {
                category: Some("food".into()),
                amount: Some(450.0),
                ..Default::default()
            },
            coaching_signal: true,
            coaching_rule: Some("Review spending patterns when budget pressure is detected".into()),
            metric_samples: Vec::new(),
        };
        let _ = signal_tx.send(budget_signal).await;

        // Wait briefly for processing
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        // Should have received an intervention
        let intervention = intervention_rx.try_recv();
        assert!(
            intervention.is_ok(),
            "Expected an intervention to be delivered"
        );
        assert_eq!(intervention.unwrap().message, "Check your budget!");

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
        let (_signal_tx, signal_rx) = tokio::sync::mpsc::channel(64);
        let cancel = CancellationToken::new();

        let mut service = CoachingService::start(
            signal_rx,
            accumulator,
            detector,
            router,
            feedback,
            situation,
            reasoner,
            tx,
            None,
            cancel.clone(),
        );

        service.stop().await;
        // Should not panic or hang
    }
}
