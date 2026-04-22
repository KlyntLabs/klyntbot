//! Feeds chat turn content into the unified pipeline for cross-source convergence.
//!
//! Note: this runs alongside the existing extraction pipeline in
//! BackgroundConsolidationService, which handles deep LLM-based fact
//! extraction. This collector's purpose is convergence — grouping chat
//! signals with atom, coaching, and session signals.

use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;
use jiff::Timestamp;

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

/// Minimum message length to bother sending to the pipeline.
const MIN_MESSAGE_LEN: usize = 20;

pub struct ChatTurnCollector {
    tx: SignalSender,
}

impl ChatTurnCollector {
    pub fn new(tx: SignalSender) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl SignalConsumer for ChatTurnCollector {
    fn name(&self) -> &'static str {
        "cognitive.chat_turn"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "ChatTurnCompleted" {
            return Ok(());
        }
        if signal.content.len() < MIN_MESSAGE_LEN {
            return Ok(());
        }

        let session_key = signal.raw_event.as_ref().and_then(|e| match e {
            bus::DomainEvent::ChatTurnCompleted { session_key, .. } => Some(session_key.clone()),
            _ => None,
        });

        let out = CognitiveSignal {
            source: SignalSource::ChatTurn,
            content: signal.content.clone(),
            domain: RecallDomain::General,
            confidence: 0.6,
            context: SignalContext {
                session_key,
                source_count: 1,
                ..Default::default()
            },
            timestamp: Timestamp::now(),
        };
        let _ = self.tx.send(out).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, SalienceVerdict};

    fn dummy_ai_signal() -> AiSignal {
        AiSignal {
            domain: ai_core::RecallDomain::General,
            event_kind: "ChatTurnCompleted",
            importance: 0.3,
            salience: SalienceVerdict::Accumulate,
            content: "A long enough message to pass the min length filter".into(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

    #[test]
    fn test_min_message_length() {
        assert_eq!(MIN_MESSAGE_LEN, 20);
    }

    #[tokio::test]
    async fn consumer_forwards_chat_turn_signals() {
        use ai_core::SignalConsumer;
        let (tx, mut rx) = super::super::signal_queue(8);
        let collector = ChatTurnCollector::new(tx);

        let sig = dummy_ai_signal();
        collector.consume(&sig).await.unwrap();
        let out = rx.recv().await.unwrap();
        assert_eq!(out.source, SignalSource::ChatTurn);
    }

    #[tokio::test]
    async fn consumer_ignores_non_chat_events() {
        use ai_core::SignalConsumer;
        let (tx, mut rx) = super::super::signal_queue(8);
        let collector = ChatTurnCollector::new(tx);

        let sig = AiSignal {
            event_kind: "SessionEnded",
            ..dummy_ai_signal()
        };
        collector.consume(&sig).await.unwrap();
        assert!(rx.try_recv().is_err());
    }
}
