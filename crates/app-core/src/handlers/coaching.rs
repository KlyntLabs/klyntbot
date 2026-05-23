//! Coaching engine handlers — situation, signals, patterns, feedback, and router status.
//!
//! The `coaching_pending_interventions` handler acts as the delivery manager:
//! it gates intervention visibility based on streaming state and consecutive
//! ignore count, so both chat nudge and tray nudge components poll the same
//! endpoint and get consistent delivery decisions.

use std::sync::atomic::Ordering;

use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;
use tracing::debug;

use crate::state::AppCore;

/// Maximum consecutive ignored nudges before delivery is skipped.
const MAX_CONSECUTIVE_IGNORES: i32 = 2;

impl AppCore {
    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_situation(&self) -> Result<UserSituationResponse, ApiError> {
        let sit = self.user_situation()?.lock().await;
        Ok(UserSituationResponse {
            energy_level: sit.energy_level,
            focus_state: sit.focus_state,
            deadline_pressure: sit.deadline_pressure,
            distraction_risk: sit.distraction_risk,
            coaching_receptivity: sit.coaching_receptivity,
            task_avoidance_detected: sit.task_avoidance_detected,
            hours_active_today: sit.hours_active_today,
            mins_since_break: sit.mins_since_break,
            hour_of_day: jiff::Zoned::now().hour() as u32,
            recent_context_switches: sit.recent_context_switches,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_signals(&self) -> Result<SignalWindowResponse, ApiError> {
        let acc = self.signal_accumulator()?;
        let acc = acc.lock().await;
        let last_fired = acc.last_fired();
        let signals: Vec<SignalResponse> = acc
            .signals()
            .iter()
            .map(|s| SignalResponse {
                event_type: s.event_type.clone(),
                timestamp: s.timestamp.to_string(),
                metadata: format!(
                    "app={} task={} cat={}",
                    s.metadata.app.as_deref().unwrap_or("-"),
                    s.metadata.task_id.as_deref().unwrap_or("-"),
                    s.metadata.category.as_deref().unwrap_or("-"),
                ),
            })
            .collect();
        let triggers: Vec<TriggerConditionResponse> = acc
            .condition_names()
            .iter()
            .map(|name| {
                let last = last_fired.get(*name);
                let cooldown_remaining = last
                    .map(|t| {
                        let elapsed = jiff::Timestamp::now().duration_since(*t).as_secs();
                        (300 - elapsed).max(0)
                    })
                    .unwrap_or(0);
                TriggerConditionResponse {
                    name: name.to_string(),
                    cooldown_remaining_secs: cooldown_remaining,
                    last_fired: last.map(|t| t.to_string()),
                }
            })
            .collect();
        Ok(SignalWindowResponse {
            window_size: signals.len(),
            signals,
            triggers,
        })
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_patterns(&self) -> Result<Vec<DetectedPatternResponse>, ApiError> {
        let detector = self.pattern_detector()?;
        let detector = detector.lock().await;
        let patterns = detector.detect_patterns();

        Ok(patterns
            .iter()
            .map(|p| DetectedPatternResponse {
                name: p.name.clone(),
                confidence: p.confidence,
                signal_count: p.signal_count,
                description: p.description.clone(),
                domain: p.domain.as_str().to_string(),
            })
            .collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_feedback_stats(&self) -> Result<Vec<StrategyFeedbackResponse>, ApiError> {
        let tracker = self.feedback_tracker()?.lock().await;

        Ok(tracker
            .all_strategies()
            .iter()
            .map(|s| StrategyFeedbackResponse {
                strategy_type: s.strategy_type.clone(),
                domain: s.domain.clone(),
                times_used: s.times_used,
                acceptance_rate: s.acceptance_rate(),
                effectiveness: s.effectiveness(),
                behavioral_positive: s.behavioral_positive,
                behavioral_negative: s.behavioral_negative,
            })
            .collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_router_status(&self) -> Result<RouterStatusResponse, ApiError> {
        let router = self.intervention_router()?.lock().await;
        let (hourly_limit, daily_limit) = router.limits();

        Ok(RouterStatusResponse {
            hourly_count: router.hourly_count(),
            hourly_limit,
            daily_count: router.daily_count(),
            daily_limit,
        })
    }

    /// Returns pending coaching interventions, gated by delivery rules:
    /// - Returns empty while AI is actively streaming (streaming block)
    /// - Returns empty after [`MAX_CONSECUTIVE_IGNORES`] consecutive ignored nudges
    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_pending_interventions(
        &self,
    ) -> Result<Vec<DeliveredInterventionResponse>, ApiError> {
        // Delivery gate: block while AI is streaming
        if !self.active_streams.is_empty() {
            return Ok(vec![]);
        }

        // Delivery gate: skip after N consecutive ignores
        if self.consecutive_coaching_ignores.load(Ordering::Relaxed) >= MAX_CONSECUTIVE_IGNORES {
            return Ok(vec![]);
        }

        let tracker = self.feedback_tracker()?.lock().await;
        let now = jiff::Timestamp::now();
        Ok(tracker
            .pending_interventions()
            .iter()
            .filter(|p| p.expires_at > now)
            .map(|p| DeliveredInterventionResponse {
                id: p.intervention.id.clone(),
                intervention_type: p.intervention.intervention_type.to_string(),
                message: p.intervention.message.clone(),
                trigger_name: p.intervention.trigger_name.clone(),
                timestamp: p.intervention.delivered_at.to_string(),
            })
            .collect())
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_intervention_log(
        &self,
        limit: Option<i64>,
    ) -> Result<Vec<InterventionLogResponse>, ApiError> {
        let repo = self.coaching_log_repo()?;
        let rows = repo
            .list_recent(limit.unwrap_or(100).min(500))
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|r| InterventionLogResponse {
                id: r.id,
                intervention_type: r.intervention_type,
                message: r.message,
                trigger_name: r.trigger_name,
                feedback: r.feedback,
                delivered_at: r.delivered_at,
                feedback_at: r.feedback_at,
            })
            .collect())
    }

    // ── Coaching Mutations ──────────────────────────────────────────────

    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_reset_dismissals(
        &self,
        trigger_name: Option<String>,
    ) -> Result<bool, ApiError> {
        let mut router = self.intervention_router()?.lock().await;

        if let Some(name) = trigger_name {
            router.reset_dismissals(&name);
        }

        Ok(true)
    }

    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_clear_signals(&self) -> Result<bool, ApiError> {
        let acc = self.signal_accumulator()?;
        let mut acc = acc.lock().await;
        *acc = feature_coaching::SignalAccumulator::new();
        Ok(true)
    }

    /// Seed test trigger data into the pattern detector for debugging.
    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_seed_patterns(&self) -> Result<bool, ApiError> {
        let detector = self.pattern_detector()?;
        let mut detector = detector.lock().await;
        detector.seed_test_triggers("distraction_streak", 5);
        detector.seed_test_triggers("task_avoidance", 4);
        detector.seed_test_triggers("budget_warning", 3);
        Ok(true)
    }

    /// Submit user feedback for a coaching intervention.
    ///
    /// Records explicit feedback in the FeedbackTracker and, if dismissed,
    /// increases the backoff in InterventionRouter. Any explicit feedback
    /// resets the consecutive ignore counter.
    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_submit_feedback(
        &self,
        intervention_id: String,
        response: String,
    ) -> Result<bool, ApiError> {
        let feedback_response = match response.as_str() {
            "helpful" => bus::FeedbackResponse::Helpful,
            "dismissed" => bus::FeedbackResponse::Dismissed,
            "stop" => bus::FeedbackResponse::StopSuggesting,
            _ => {
                return Err(ApiError::new(
                    "INVALID_RESPONSE",
                    "response must be 'helpful', 'dismissed', or 'stop'",
                ));
            }
        };

        // Any explicit interaction resets the consecutive ignore counter
        self.consecutive_coaching_ignores
            .store(0, Ordering::Relaxed);
        debug!("coaching consecutive ignores reset (explicit feedback)");

        // Record in FeedbackTracker (single lock scope — record_explicit removes
        // the pending entry, so we must extract trigger_name + message first)
        let (trigger_name, intervention_message) = {
            let mut tracker = self.feedback_tracker()?.lock().await;
            let pending = tracker
                .pending_interventions()
                .iter()
                .find(|p| p.intervention.id == intervention_id)
                .map(|p| {
                    (
                        p.intervention.trigger_name.clone(),
                        p.intervention.message.clone(),
                    )
                });
            tracker.record_explicit(&intervention_id, &feedback_response);
            pending.map_or((None, None), |(t, m)| (Some(t), Some(m)))
        };

        // Persist feedback to intervention log (works for both live and retroactive feedback)
        if let Ok(repo) = self.coaching_log_repo() {
            let feedback_str = match feedback_response {
                bus::FeedbackResponse::Helpful => "helpful",
                bus::FeedbackResponse::Dismissed => "dismissed",
                bus::FeedbackResponse::StopSuggesting => "stop",
            };
            if let Err(e) = repo.update_feedback(&intervention_id, feedback_str).await {
                debug!("failed to persist coaching feedback: {e}");
            }
        }

        // If dismissed/stop, record dismissal in router for backoff
        if matches!(
            feedback_response,
            bus::FeedbackResponse::Dismissed | bus::FeedbackResponse::StopSuggesting
        ) {
            if let Some(name) = &trigger_name {
                let mut router = self.intervention_router()?.lock().await;
                router.record_dismissal(name);
            }
        }

        // Publish domain event so coaching service can react
        if let Ok(bus) = self.domain_event_bus() {
            let accepted = matches!(feedback_response, bus::FeedbackResponse::Helpful);
            bus.publish(bus::DomainEvent::CoachingFeedback {
                intervention_id: intervention_id.clone(),
                response: feedback_response,
            });

            // Emit strategy-applied event for AI pipeline metrics (coaching_acceptance_rate).
            // strategy_id = trigger_name; rule_text = intervention message.
            // accepted = true only for Helpful; Dismissed and StopSuggesting = false.
            if let (Some(strategy_id), Some(rule_text)) = (trigger_name, intervention_message) {
                bus.publish(bus::DomainEvent::CoachingStrategyApplied {
                    strategy_id,
                    rule_text,
                    accepted,
                });
            }
        }

        Ok(true)
    }

    /// Report that a coaching nudge was ignored (auto-collapsed without interaction).
    /// Increments the consecutive ignore counter; after [`MAX_CONSECUTIVE_IGNORES`]
    /// in a row, delivery is skipped.
    #[tracing::instrument(skip(self), err)]
    pub async fn coaching_report_ignored(&self, intervention_id: String) -> Result<bool, ApiError> {
        let count = self
            .consecutive_coaching_ignores
            .fetch_add(1, Ordering::Relaxed)
            + 1;
        debug!("coaching nudge ignored (id={intervention_id}), consecutive={count}");

        // Persist "ignored" feedback to intervention log
        if let Ok(repo) = self.coaching_log_repo() {
            if let Err(e) = repo.update_feedback(&intervention_id, "ignored").await {
                debug!("failed to persist ignored feedback: {e}");
            }
        }

        Ok(true)
    }
}
