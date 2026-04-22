//! Feeds chat turn content into the unified pipeline for cross-source convergence.
//!
//! Note: this runs alongside the existing extraction pipeline in
//! BackgroundConsolidationService, which handles deep LLM-based fact
//! extraction. This collector's purpose is convergence — grouping chat
//! signals with atom, coaching, and session signals.

use jiff::Timestamp;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

/// Minimum message length to bother sending to the pipeline.
const MIN_MESSAGE_LEN: usize = 20;

pub struct ChatTurnCollector;

impl ChatTurnCollector {
    pub fn start(
        mut event_rx: broadcast::Receiver<bus::DomainEvent>,
        signal_tx: SignalSender,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(bus::DomainEvent::ChatTurnCompleted { session_key, user_message: Some(msg) }) => {
                                if msg.len() >= MIN_MESSAGE_LEN {
                                    let signal = CognitiveSignal {
                                        source: SignalSource::ChatTurn,
                                        content: msg,
                                        domain: ai_core::RecallDomain::General,
                                        confidence: 0.6,
                                        context: SignalContext {
                                            session_key: Some(session_key),
                                            source_count: 1,
                                            ..Default::default()
                                        },
                                        timestamp: Timestamp::now(),
                                    };
                                    let _ = signal_tx.send(signal).await;
                                }
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("ChatTurnCollector lagged {n}");
                            }
                            _ => {}
                        }
                    }
                }
            }
            debug!("ChatTurnCollector stopped");
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_min_message_length() {
        assert_eq!(MIN_MESSAGE_LEN, 20);
    }
}
