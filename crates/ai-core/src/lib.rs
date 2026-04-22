//! Unified AI feature pipeline.
pub mod metrics;
pub mod recall;
pub mod recall_domain;
pub mod router;
pub mod signal;
pub mod traits;

pub use metrics::AiMetrics;
pub use recall::{RecallItem, RecallQuery};
pub use recall_domain::RecallDomain;
pub use router::{SignalRouter, Translator};
pub use signal::{AiSignal, EntityRef, SalienceVerdict};
pub use traits::{AiEntity, AiEventMeta, AiFeature, RecallProvider, SignalConsumer};
