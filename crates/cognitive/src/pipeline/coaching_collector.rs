//! Collects signals from coaching pattern detection.

use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

pub struct CoachingCollector {
    tx: SignalSender,
}

impl CoachingCollector {
    pub fn new(tx: SignalSender) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl SignalConsumer for CoachingCollector {
    fn name(&self) -> &'static str {
        "cognitive.coaching"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "CoachingPatternDetected" {
            return Ok(());
        }
        let Some(bus::DomainEvent::CoachingPatternDetected {
            domain,
            signal_count,
            ..
        }) = signal.raw_event.as_ref()
        else {
            return Ok(());
        };

        let recall_domain = RecallDomain::from_str_or_general(domain.as_str());

        let out = CognitiveSignal {
            source: SignalSource::CoachingPattern,
            content: signal.content.clone(), // rule_text lives here — no match required
            domain: recall_domain,
            confidence: signal.importance,
            context: SignalContext {
                source_count: *signal_count as u32,
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

    fn coaching_dummy() -> AiSignal {
        AiSignal {
            domain: ai_core::RecallDomain::General,
            event_kind: "CoachingPatternDetected",
            importance: 0.85,
            salience: SalienceVerdict::Extract,
            content: String::new(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: None,
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

    #[tokio::test]
    async fn coaching_consumer_uses_declared_rule_text() {
        use ai_core::SignalConsumer;
        let (tx, mut rx) = super::super::signal_queue(8);
        let collector = CoachingCollector::new(tx);
        let sig = AiSignal {
            event_kind: "CoachingPatternDetected",
            content: "Schedule demanding tasks in the morning".into(),
            importance: 0.85,
            raw_event: Some(bus::DomainEvent::CoachingPatternDetected {
                pattern_name: "afternoon_energy_drop".into(),
                confidence: 0.85,
                description: "3/4 after 3pm".into(),
                domain: RecallDomain::Productivity.as_str().into(),
                signal_count: 4,
                rule_text: "Schedule demanding tasks in the morning".into(),
            }),
            ..coaching_dummy()
        };
        collector.consume(&sig).await.unwrap();
        let out = rx.recv().await.unwrap();
        assert_eq!(out.content, "Schedule demanding tasks in the morning");
        assert_eq!(out.source, SignalSource::CoachingPattern);
    }
}
