use ai_core_macros::AiEvent;
use bus::DomainEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "General")]
pub enum CoActivationEvent {
    #[ai(
        importance = 0.3,
        salience = "accumulate",
        observation_template = "Co-activation strengthened: {fact_id_a}↔{fact_id_b} → {strength:.2}",
    )]
    Strengthened {
        fact_id_a: String,
        fact_id_b: String,
        strength: f64,
    },
}

impl From<CoActivationEvent> for DomainEvent {
    fn from(e: CoActivationEvent) -> Self {
        match e {
            CoActivationEvent::Strengthened {
                fact_id_a,
                fact_id_b,
                strength,
            } => DomainEvent::CoActivationStrengthened {
                fact_id_a,
                fact_id_b,
                strength,
            },
        }
    }
}
