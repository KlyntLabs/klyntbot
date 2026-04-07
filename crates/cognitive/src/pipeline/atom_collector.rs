//! Collects signals from cross-note atom reinforcement.

use chrono::Utc;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;

const MIN_REINFORCEMENT: i64 = 2;

pub struct AtomCollector;

impl AtomCollector {
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
                            Ok(bus::DomainEvent::AtomReinforced {
                                atom_id, subject, domain, reinforcement_count, ..
                            }) if reinforcement_count >= MIN_REINFORCEMENT => {
                                let confidence = (0.5 + reinforcement_count as f64 * 0.15).min(0.95);
                                let signal = CognitiveSignal {
                                    source: SignalSource::AtomReinforcement,
                                    content: subject,
                                    domain,
                                    confidence,
                                    context: SignalContext {
                                        related_atom_ids: vec![atom_id],
                                        source_count: reinforcement_count as u32,
                                        ..Default::default()
                                    },
                                    timestamp: Utc::now(),
                                };
                                let _ = signal_tx.send(signal).await;
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("AtomCollector lagged {n}");
                            }
                            _ => {}
                        }
                    }
                }
            }
            debug!("AtomCollector stopped");
        })
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_confidence_scaling() {
        let conf = |n: i64| (0.5 + n as f64 * 0.15).min(0.95);
        assert!((conf(2) - 0.80).abs() < 0.01);
        assert!((conf(3) - 0.95).abs() < 0.01);
        assert!((conf(10) - 0.95).abs() < 0.01);
    }
}
