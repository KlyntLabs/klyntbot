//! Collects knowledge signals from ended sessions by reading session scratchpads.

use chrono::Utc;
use storage::SessionMemoryRepo;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

const INSIGHT_KEYWORDS: &[&str] = &[
    "learned", "decided", "error", "fixed", "important", "remember",
    "preference", "realized", "discovered", "pattern", "issue", "solution",
    "goal", "plan", "need", "want", "struggle", "improve",
];
const MIN_SCRATCHPAD_LEN: usize = 50;

pub struct SessionCollector;

impl SessionCollector {
    pub fn start(
        mut event_rx: broadcast::Receiver<bus::DomainEvent>,
        signal_tx: SignalSender,
        memory_repo: SessionMemoryRepo,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(bus::DomainEvent::SessionEnded { session_id, .. }) => {
                                Self::handle(&session_id, &memory_repo, &signal_tx).await;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("SessionCollector lagged by {n} events");
                            }
                            _ => {}
                        }
                    }
                }
            }
            debug!("SessionCollector stopped");
        })
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
            domain: "general".into(),
            confidence,
            context: SignalContext {
                session_key: Some(session_key.to_string()),
                raw_observations: vec![scratchpad],
                source_count: 1,
                ..Default::default()
            },
            timestamp: Utc::now(),
        };
        let _ = tx.send(signal).await;
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
    let unique_count = INSIGHT_KEYWORDS
        .iter()
        .filter(|kw| insights.iter().any(|s| s.to_lowercase().contains(**kw)))
        .count();
    (0.5 + unique_count as f64 / INSIGHT_KEYWORDS.len() as f64 * 0.3).min(0.8)
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert!(conf >= 0.5 && conf <= 0.8);
    }
}
