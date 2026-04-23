use ai_core_macros::AiEvent;
use bus::DomainEvent;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, AiEvent, Serialize, Deserialize)]
#[ai(domain = "Productivity")]
pub enum ProductivityEvent {
    #[ai(
        importance = 0.5,
        salience = "accumulate",
        observation_template = "Session ended: {duration_mins}m, quality {quality:.2}",
        metric(
            name = "focus_quality_trend",
            value_from = *quality,
            window = "7d",
            min_samples = 3,
            aggregation = "avg",
        ),
    )]
    SessionEnded {
        session_id: String,
        quality: f64,
        duration_mins: u32,
    },
}

impl From<ProductivityEvent> for DomainEvent {
    fn from(e: ProductivityEvent) -> Self {
        match e {
            ProductivityEvent::SessionEnded {
                session_id,
                quality,
                duration_mins,
            } => DomainEvent::ProductivitySessionEnded {
                session_id,
                quality,
                duration_mins,
            },
        }
    }
}
