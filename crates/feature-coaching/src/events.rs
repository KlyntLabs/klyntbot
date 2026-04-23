use ai_core_macros::AiEvent;
use bus::DomainEvent;
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
        }
    }
}
