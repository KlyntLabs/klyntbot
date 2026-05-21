use ai_core_macros::AiEvent;
use bus::{DomainEvent, FeedbackResponse};
use serde::{Deserialize, Serialize};

/// Typed events emitted by the coaching subsystem.
///
/// In v2.5 the enum is minimal — just `StrategyApplied` — so `coaching_acceptance_rate`
/// can originate from the pipeline rather than from raw-SQL reads of `coaching_strategies`.
/// v3 expands this enum as part of full `AiFeature` migration of the coaching crate.
#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "Coaching")]
pub enum CoachingEvent {
    #[ai(
        importance = 0.4,
        salience = "accumulate",
        observation_template = "Coaching strategy '{rule_text}' — accepted={accepted}",
        metric(
            name = "coaching_acceptance_rate",
            value_from = if *accepted { 1.0 } else { 0.0 },
            window = "7d",
            min_samples = 5,
            aggregation = "avg",
        ),
    )]
    StrategyApplied {
        strategy_id: String,
        rule_text: String,
        accepted: bool,
    },

    #[ai(
        importance = 0.6,
        salience = "extract",
        observation_template = "Coaching pattern detected: {pattern_name} (severity {severity})"
    )]
    PatternDetected { pattern_name: String, severity: f64 },

    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Coaching feedback: {response} on strategy {strategy_id}",
        metric(
            name = "coaching_feedback_response_rate",
            value_from = if response == "accept" { 1.0_f64 } else { 0.0_f64 },
            window = "30d",
            min_samples = 5,
            aggregation = "avg",
        ),
    )]
    FeedbackReceived {
        strategy_id: String,
        response: String,
    },
}

impl From<CoachingEvent> for DomainEvent {
    fn from(e: CoachingEvent) -> Self {
        match e {
            CoachingEvent::StrategyApplied {
                strategy_id,
                rule_text,
                accepted,
            } => DomainEvent::CoachingStrategyApplied {
                strategy_id,
                rule_text,
                accepted,
            },
            CoachingEvent::PatternDetected {
                pattern_name,
                severity,
            } => DomainEvent::CoachingPatternDetected {
                pattern_name,
                confidence: severity,
                description: String::new(),
                domain: String::new(),
                signal_count: 0,
                rule_text: String::new(),
            },
            CoachingEvent::FeedbackReceived {
                strategy_id,
                response,
            } => DomainEvent::CoachingFeedback {
                intervention_id: strategy_id,
                response: match response.as_str() {
                    "accept" => FeedbackResponse::Helpful,
                    "dismiss" => FeedbackResponse::Dismissed,
                    "stop" => FeedbackResponse::StopSuggesting,
                    _ => FeedbackResponse::Dismissed,
                },
            },
        }
    }
}

/// Translate a bus::DomainEvent into the typed CoachingEvent form, if it's a coaching event.
pub fn try_from_domain_event(e: &bus::DomainEvent) -> Option<CoachingEvent> {
    use bus::DomainEvent;
    match e {
        DomainEvent::CoachingStrategyApplied {
            strategy_id,
            rule_text,
            accepted,
        } => Some(CoachingEvent::StrategyApplied {
            strategy_id: strategy_id.clone(),
            rule_text: rule_text.clone(),
            accepted: *accepted,
        }),
        DomainEvent::CoachingPatternDetected {
            pattern_name,
            confidence,
            ..
        } => Some(CoachingEvent::PatternDetected {
            pattern_name: pattern_name.clone(),
            severity: *confidence,
        }),
        _ => None,
    }
}
