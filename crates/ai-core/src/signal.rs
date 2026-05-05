use crate::metric::MetricSample;
use crate::metrics::AiMetrics;
use crate::RecallDomain;
use bus::DomainEvent;
use jiff::Timestamp;

/// The unified signal every feature emits and every AI consumer reads.
///
/// Produced from a `DomainEvent` via `#[derive(AiEvent)]`-generated
/// `AiEventMeta::to_signal()`. Broadcast by `SignalRouter` to all
/// registered `SignalConsumer` implementations.
#[derive(Debug, Clone)]
pub struct AiSignal {
    pub domain: RecallDomain,
    pub event_kind: &'static str,
    pub importance: f64,
    pub salience: SalienceVerdict,
    pub content: String,
    pub entity: Option<EntityRef>,
    pub timestamp: Timestamp,
    pub raw_event: Option<DomainEvent>,
    pub metrics: AiMetrics,
    pub coaching_signal: bool,
    pub coaching_rule: Option<String>,
    /// Metric samples emitted by this event, one per `#[ai(metric(...))]` declaration.
    /// Consumed by `MetricHarvestConsumer` and persisted to `ai_metric_samples`.
    pub metric_samples: Vec<MetricSample>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SalienceVerdict {
    Extract,
    Accumulate,
    Discard,
}

impl SalienceVerdict {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Extract => "extract",
            Self::Accumulate => "accumulate",
            Self::Discard => "discard",
        }
    }
}

#[derive(Debug, Clone)]
pub struct EntityRef {
    pub entity_type: &'static str,
    pub id: String,
    pub name: String,
}
