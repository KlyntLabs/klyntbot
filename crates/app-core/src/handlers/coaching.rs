//! Coaching engine handlers — situation, signals, patterns, feedback, and router status.

use chrono::Timelike;
use desktop_shared::cognitive_commands::*;
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
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
            hour_of_day: chrono::Local::now().hour(),
            recent_context_switches: sit.recent_context_switches,
        })
    }

    pub async fn coaching_signals(&self) -> Result<SignalWindowResponse, ApiError> {
        let acc = self.signal_accumulator()?.lock().await;
        let last_fired = acc.last_fired();
        let signals: Vec<SignalResponse> = acc
            .signals()
            .iter()
            .map(|s| SignalResponse {
                event_type: s.event_type.clone(),
                timestamp: s.timestamp.to_rfc3339(),
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
                        let elapsed = (chrono::Utc::now() - *t).num_seconds();
                        (300 - elapsed).max(0)
                    })
                    .unwrap_or(0);
                TriggerConditionResponse {
                    name: name.to_string(),
                    cooldown_remaining_secs: cooldown_remaining,
                    last_fired: last.map(|t| t.to_rfc3339()),
                }
            })
            .collect();
        Ok(SignalWindowResponse {
            window_size: signals.len(),
            signals,
            triggers,
        })
    }

    pub async fn coaching_patterns(&self) -> Result<Vec<DetectedPatternResponse>, ApiError> {
        let detector = self.pattern_detector()?.lock().await;
        let patterns = detector.detect_patterns();

        Ok(patterns
            .iter()
            .map(|p| DetectedPatternResponse {
                name: p.name.clone(),
                confidence: p.confidence,
                signal_count: p.signal_count,
                description: p.description.clone(),
                domain: p.domain.clone(),
            })
            .collect())
    }

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

    // ── Coaching Mutations ──────────────────────────────────────────────

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

    pub async fn coaching_clear_signals(&self) -> Result<bool, ApiError> {
        let mut acc = self.signal_accumulator()?.lock().await;
        *acc = feature_coaching::SignalAccumulator::new();
        Ok(true)
    }
}
