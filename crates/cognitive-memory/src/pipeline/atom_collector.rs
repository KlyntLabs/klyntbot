//! Collects signals from cross-note atom reinforcement.

use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;
use bus::domain_events::LearningEvent as BusLearningEvent;

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

const MIN_REINFORCEMENT: i64 = 2;

pub struct AtomCollector {
    tx: SignalSender,
}

impl AtomCollector {
    pub fn new(tx: SignalSender) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl SignalConsumer for AtomCollector {
    fn name(&self) -> &'static str {
        "cognitive.atom"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "AtomReinforced" {
            return Ok(());
        }
        let (atom_id, count) = match signal.raw_event.as_ref() {
            Some(bus::DomainEvent::Learning(BusLearningEvent::AtomReinforced {
                atom_id,
                reinforcement_count,
                ..
            })) if *reinforcement_count >= MIN_REINFORCEMENT => {
                (atom_id.clone(), *reinforcement_count)
            }
            _ => return Ok(()),
        };
        let confidence = (0.5 + count as f64 * 0.15).min(0.95);
        let domain = signal
            .metrics
            .category
            .as_deref()
            .map(RecallDomain::from_str_or_general)
            .unwrap_or(RecallDomain::General);
        let out = CognitiveSignal {
            source: SignalSource::AtomReinforcement,
            content: signal.content.clone(),
            domain,
            confidence,
            context: SignalContext {
                related_atom_ids: vec![atom_id],
                source_count: count as u32,
                ..Default::default()
            },
            timestamp: jiff::Timestamp::now(),
        };
        let _ = self.tx.send(out).await;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, SalienceVerdict};

    fn atom_dummy() -> AiSignal {
        AiSignal {
            domain: ai_core::RecallDomain::General,
            event_kind: "AtomReinforced",
            importance: 0.5,
            salience: SalienceVerdict::Accumulate,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
            metric_samples: Vec::new(),
        }
    }

    #[test]
    fn test_confidence_scaling() {
        let conf = |n: i64| (0.5 + n as f64 * 0.15).min(0.95);
        assert!((conf(2) - 0.80).abs() < 0.01);
        assert!((conf(3) - 0.95).abs() < 0.01);
        assert!((conf(10) - 0.95).abs() < 0.01);
    }

    #[tokio::test]
    async fn atom_consumer_forwards() {
        use ai_core::SignalConsumer;
        let (tx, mut rx) = super::super::signal_queue(8);
        let collector = AtomCollector::new(tx);
        let sig = AiSignal {
            event_kind: "AtomReinforced",
            content: "rust errors".into(),
            metrics: AiMetrics {
                category: Some(RecallDomain::Learning.as_str().into()),
                ..Default::default()
            },
            raw_event: Some(bus::DomainEvent::Learning(
                BusLearningEvent::AtomReinforced {
                    atom_id: "a1".into(),
                    referencing_note_id: "n1".into(),
                    new_salience: 0.8,
                    subject: "rust errors".into(),
                    domain: RecallDomain::Learning.as_str().into(),
                    reinforcement_count: 3,
                },
            )),
            ..atom_dummy()
        };
        collector.consume(&sig).await.unwrap();
        let out = rx.recv().await.unwrap();
        assert_eq!(out.source, SignalSource::AtomReinforcement);
        assert_eq!(out.domain, ai_core::RecallDomain::Learning);
    }
}
