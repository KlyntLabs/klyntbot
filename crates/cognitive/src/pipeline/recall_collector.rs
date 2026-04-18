//! Collects signals from conversation recall clusters.
//!
//! Buffers `ChatTurnCompleted` messages and every [`BUFFER_FLUSH_SIZE`] events
//! runs a word-overlap clustering pass. Clusters with [`CLUSTER_THRESHOLD`]+
//! messages from [`SESSION_THRESHOLD`]+ distinct sessions are promoted as
//! [`CognitiveSignal`]s with `source = ConversationRecall`.

use std::collections::HashSet;

use jiff::Timestamp;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::{debug, info, warn};

use super::signal::{CognitiveSignal, SignalContext, SignalSource};
use super::SignalSender;
use crate::repos::procedural_rule::word_overlap_ratio;

/// Minimum messages in a cluster to promote a signal.
const CLUSTER_THRESHOLD: usize = 2;
/// Minimum distinct sessions required across cluster members.
const SESSION_THRESHOLD: usize = 1;
/// Flush (and cluster) the buffer every N buffered messages.
const BUFFER_FLUSH_SIZE: usize = 20;
/// Word-overlap ratio above which two messages are considered similar.
const OVERLAP_THRESHOLD: f64 = 0.5;
/// Ignore very short messages (e.g. "ok", "thanks").
const MIN_MESSAGE_LEN: usize = 20;

struct BufferedMessage {
    content: String,
    session_key: String,
    #[allow(dead_code)]
    timestamp: Timestamp,
}

pub struct RecallCollector;

impl RecallCollector {
    /// Spawn the collector task and return its [`JoinHandle`].
    pub fn start(
        mut event_rx: broadcast::Receiver<bus::DomainEvent>,
        signal_tx: SignalSender,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        tokio::spawn(async move {
            let mut buffer: Vec<BufferedMessage> = Vec::new();

            loop {
                tokio::select! {
                    _ = cancel.cancelled() => break,
                    result = event_rx.recv() => {
                        match result {
                            Ok(bus::DomainEvent::ChatTurnCompleted {
                                session_key,
                                user_message: Some(msg),
                            }) if msg.len() > MIN_MESSAGE_LEN => {
                                buffer.push(BufferedMessage {
                                    content: msg,
                                    session_key,
                                    timestamp: Timestamp::now(),
                                });

                                if buffer.len() >= BUFFER_FLUSH_SIZE {
                                    Self::flush_buffer(&mut buffer, &signal_tx).await;
                                }
                            }
                            Err(broadcast::error::RecvError::Closed) => break,
                            Err(broadcast::error::RecvError::Lagged(n)) => {
                                warn!("RecallCollector lagged {n} events");
                            }
                            _ => {}
                        }
                    }
                }
            }

            // Final flush on shutdown so no buffered data is discarded.
            if !buffer.is_empty() {
                Self::flush_buffer(&mut buffer, &signal_tx).await;
            }
            debug!("RecallCollector stopped");
        })
    }

    async fn flush_buffer(buffer: &mut Vec<BufferedMessage>, signal_tx: &SignalSender) {
        let clusters = cluster_messages(buffer);

        for cluster in clusters {
            let sessions: HashSet<&str> = cluster.iter().map(|m| m.session_key.as_str()).collect();

            if cluster.len() < CLUSTER_THRESHOLD || sessions.len() < SESSION_THRESHOLD {
                continue;
            }

            // Use the longest message as the representative surface form.
            let best = cluster.iter().max_by_key(|m| m.content.len()).unwrap();

            // Confidence scales with cluster size, capped at 0.9.
            let confidence = (0.4 + cluster.len() as f64 * 0.1).min(0.9);

            let signal = CognitiveSignal {
                source: SignalSource::ConversationRecall,
                content: best.content.clone(),
                domain: "general".into(),
                confidence,
                context: SignalContext {
                    source_count: sessions.len() as u32,
                    raw_observations: cluster.iter().map(|m| m.content.clone()).collect(),
                    ..Default::default()
                },
                timestamp: Timestamp::now(),
            };

            info!(
                "RecallCollector: promoting cluster of {} messages across {} sessions (confidence={:.2})",
                cluster.len(),
                sessions.len(),
                confidence,
            );

            let _ = signal_tx.send(signal).await;
        }

        buffer.clear();
    }
}

/// Group `messages` into clusters by word-overlap similarity.
///
/// Each message is compared against the first (representative) element of every
/// existing cluster. If the overlap exceeds [`OVERLAP_THRESHOLD`] the message
/// joins that cluster; otherwise it starts a new one.
fn cluster_messages(messages: &[BufferedMessage]) -> Vec<Vec<&BufferedMessage>> {
    let mut clusters: Vec<Vec<&BufferedMessage>> = Vec::new();

    'outer: for msg in messages {
        for cluster in &mut clusters {
            if let Some(rep) = cluster.first() {
                if word_overlap_ratio(&rep.content, &msg.content) > OVERLAP_THRESHOLD {
                    cluster.push(msg);
                    continue 'outer;
                }
            }
        }
        clusters.push(vec![msg]);
    }

    clusters
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(content: &str, session: &str) -> BufferedMessage {
        BufferedMessage {
            content: content.into(),
            session_key: session.into(),
            timestamp: Timestamp::now(),
        }
    }

    #[test]
    fn test_cluster_similar_messages() {
        // Use messages that share enough words to exceed the 0.5 Jaccard threshold.
        let messages = vec![
            msg("Rust error handling best practices for Result types", "s1"),
            msg("Rust error handling best practices with Option types", "s2"),
            msg(
                "Rust error handling best practices using unwrap safely",
                "s3",
            ),
            msg("What should I have for dinner tonight at home", "s1"),
        ];
        let clusters = cluster_messages(&messages);
        // Rust error messages should cluster together; dinner is separate.
        assert!(
            clusters.len() >= 2,
            "expected at least 2 clusters, got {}",
            clusters.len()
        );
        let largest = clusters.iter().max_by_key(|c| c.len()).unwrap();
        assert!(
            largest.len() >= 2,
            "expected largest cluster to have ≥2 messages"
        );
    }

    #[test]
    fn test_single_message_forms_own_cluster() {
        let messages = vec![msg("A completely unique query about quantum physics", "s1")];
        let clusters = cluster_messages(&messages);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].len(), 1);
    }

    #[test]
    fn test_confidence_scaling() {
        let conf = |n: usize| (0.4 + n as f64 * 0.1_f64).min(0.9);
        // 3 messages → 0.7
        assert!((conf(3) - 0.7).abs() < 0.01, "conf(3) = {}", conf(3));
        // 5 messages → 0.9 (capped)
        assert!((conf(5) - 0.9).abs() < 0.01, "conf(5) = {}", conf(5));
        // 10 messages → still capped at 0.9
        assert!((conf(10) - 0.9).abs() < 0.01, "conf(10) = {}", conf(10));
    }

    #[test]
    fn test_single_session_now_promotes() {
        // With SESSION_THRESHOLD=1, single-session clusters meeting the
        // message count threshold are now promoted (lowered for faster knowledge promotion).
        // Messages share enough words for Jaccard > 0.5 to cluster together.
        let messages = vec![
            msg("Rust error handling best practices for Result types", "s1"),
            msg("Rust error handling best practices with Option types", "s1"),
            msg(
                "Rust error handling best practices using unwrap safely",
                "s1",
            ),
        ];
        let clusters = cluster_messages(&messages);
        let largest = clusters.iter().max_by_key(|c| c.len()).unwrap();
        let sessions: HashSet<&str> = largest.iter().map(|m| m.session_key.as_str()).collect();
        assert_eq!(sessions.len(), 1);
        assert!(largest.len() >= CLUSTER_THRESHOLD);
        assert!(sessions.len() >= SESSION_THRESHOLD);
    }
}
