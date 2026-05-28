//! Collects knowledge signals from ended sessions by reading session scratchpads.

use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;
use jiff::Timestamp;
use storage::SessionMemoryRepo;

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

const INSIGHT_KEYWORDS: &[&str] = &[
    "learned",
    "decided",
    "error",
    "fixed",
    "important",
    "remember",
    "preference",
    "realized",
    "discovered",
    "pattern",
    "issue",
    "solution",
    "goal",
    "plan",
    "need",
    "want",
    "struggle",
    "improve",
];
const MIN_SCRATCHPAD_LEN: usize = 50;

pub struct SessionCollector {
    tx: SignalSender,
    repo: SessionMemoryRepo,
}

impl SessionCollector {
    pub fn new(tx: SignalSender, repo: SessionMemoryRepo) -> Self {
        Self { tx, repo }
    }

    async fn handle(session_key: &str, repo: &SessionMemoryRepo, tx: &SignalSender) {
        let scratchpad = match repo.get(session_key).await {
            Ok(Some(row)) => row.content,
            _ => return,
        };
        if scratchpad.len() < MIN_SCRATCHPAD_LEN {
            return;
        }
        let insights = extract_insight_sentences(&scratchpad);
        if insights.is_empty() {
            return;
        }
        let confidence = keyword_confidence(&insights);
        let signal = CognitiveSignal {
            source: SignalSource::SessionEnd,
            content: insights.join(" "),
            domain: RecallDomain::General,
            confidence,
            context: SignalContext {
                session_key: Some(session_key.to_string()),
                raw_observations: vec![scratchpad],
                source_count: 1,
                ..Default::default()
            },
            timestamp: Timestamp::now(),
        };
        let _ = tx.send(signal).await;
    }
}

#[async_trait]
impl SignalConsumer for SessionCollector {
    fn name(&self) -> &'static str {
        "cognitive.session"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "SessionEnded" {
            return Ok(());
        }
        let session_id = signal.raw_event.as_ref().and_then(|e| match e {
            bus::DomainEvent::Productivity(bus::ProductivityEvent::SessionEnded {
                session_id,
                ..
            }) => Some(session_id.clone()),
            _ => None,
        });
        let Some(session_id) = session_id else {
            return Ok(());
        };

        Self::handle(&session_id, &self.repo, &self.tx).await;
        Ok(())
    }
}

fn extract_insight_sentences(text: &str) -> Vec<String> {
    text.split(['.', '!', '\n'])
        .filter(|s| {
            let sl = s.to_lowercase();
            INSIGHT_KEYWORDS.iter().any(|kw| sl.contains(*kw))
        })
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() > 10)
        .collect()
}

fn keyword_confidence(insights: &[String]) -> f64 {
    let lowered: Vec<String> = insights.iter().map(|s| s.to_lowercase()).collect();
    let unique_count = INSIGHT_KEYWORDS
        .iter()
        .filter(|kw| lowered.iter().any(|s| s.contains(**kw)))
        .count();
    (0.5 + unique_count as f64 / INSIGHT_KEYWORDS.len() as f64 * 0.3).min(0.8)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ai_core::{AiMetrics, SalienceVerdict};

    fn session_dummy() -> AiSignal {
        AiSignal {
            domain: ai_core::RecallDomain::General,
            event_kind: "SessionEnded",
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
    fn test_extract_insight_sentences() {
        let text = "User asked about Rust. Learned that async requires tokio. Fixed the build error. Had lunch.";
        let insights = extract_insight_sentences(text);
        assert_eq!(insights.len(), 2);
    }

    #[test]
    fn test_no_insights_from_mundane_text() {
        let insights = extract_insight_sentences("The weather is nice today. Goodbye.");
        assert!(insights.is_empty());
    }

    #[test]
    fn test_keyword_confidence_range() {
        let insights = vec!["Learned something new".into(), "Fixed a bug".into()];
        let conf = keyword_confidence(&insights);
        assert!((0.5..=0.8).contains(&conf));
    }

    #[tokio::test]
    async fn session_consumer_ignores_non_session_events() {
        use ai_core::SignalConsumer;
        let (tx, mut rx) = super::super::signal_queue(8);
        let pool = storage::StoragePool::connect_in_memory().await.unwrap();
        let repo = storage::SessionMemoryRepo::new(pool.inner().clone());
        let collector = SessionCollector::new(tx, repo);
        let sig = AiSignal {
            event_kind: "ChatTurnCompleted",
            ..session_dummy()
        };
        collector.consume(&sig).await.unwrap();
        assert!(rx.try_recv().is_err());
    }
}
