use crate::{
    recall::{RecallItem, RecallQuery},
    AiSignal, RecallDomain,
};
use async_trait::async_trait;
use bus::DomainEvent;

/// Feature-level declaration. Implemented via `#[derive(AiFeature)]`.
pub trait AiFeature: Send + Sync + 'static {
    const DOMAIN: RecallDomain;
    const SKILL: &'static str;
    type Event: AiEventMeta + Into<DomainEvent>;
}

/// Event-level declaration. Implemented via `#[derive(AiEvent)]`.
pub trait AiEventMeta {
    fn to_signal(&self) -> AiSignal;
    fn event_kind(&self) -> &'static str;
}

/// Entity-level declaration. Implemented via `#[derive(AiEntity)]`.
pub trait AiEntity {
    fn embed_text(&self) -> String;
    fn entity_type() -> &'static str
    where
        Self: Sized;
    fn recall_filter(&self) -> bool {
        true
    }
}

/// Generic subscriber.
#[async_trait]
pub trait SignalConsumer: Send + Sync {
    fn name(&self) -> &'static str;
    async fn consume(&self, signal: &AiSignal) -> common::Result<()>;
}

/// Optional retrieval-side interface for features that want custom recall behaviour.
pub trait RecallProvider: Send + Sync {
    fn domain(&self) -> RecallDomain;
    fn score_query(&self, _query: &RecallQuery) -> f64 {
        0.3
    }
    fn candidates(&self, _query: &RecallQuery) -> Vec<RecallItem> {
        Vec::new()
    }
}
