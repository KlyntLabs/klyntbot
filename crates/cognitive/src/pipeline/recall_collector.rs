//! Collects signals from conversation recall clusters.
//!
//! Buffers `ChatTurnCompleted` messages and every [`BUFFER_FLUSH_SIZE`] events
//! runs a word-overlap clustering pass. Clusters with [`CLUSTER_THRESHOLD`]+
//! messages from [`SESSION_THRESHOLD`]+ distinct sessions are promoted as
//! [`CognitiveSignal`]s with `source = ConversationRecall`.

use std::collections::HashSet;
use std::sync::Arc;

use ai_core::{AiSignal, RecallDomain, SignalConsumer};
use async_trait::async_trait;
use jiff::Timestamp;
use tokio::sync::Mutex;
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

pub struct RecallCollector {
    tx: SignalSender,
    buffer: Arc<Mutex<Vec<BufferedMessage>>>,
}

impl RecallCollector {
    pub fn new(tx: SignalSender) -> Self {
        Self {
            tx,
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    async fn flush_if_needed(&self) {
        let mut buf = self.buffer.lock().await;
        if buf.len() >= BUFFER_FLUSH_SIZE {
            Self::flush_buffer(&mut buf, &self.tx).await;
        }
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
                domain: RecallDomain::General,
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

#[async_trait]
impl SignalConsumer for RecallCollector {
    fn name(&self) -> &'static str {
        "cognitive.recall"
    }

    async fn consume(&self, signal: &AiSignal) -> common::Result<()> {
        if signal.event_kind != "ChatTurnCompleted" {
            return Ok(());
        }
        if signal.content.len() <= MIN_MESSAGE_LEN {
            return Ok(());
        }

        let session_key = signal.raw_event.as_ref().and_then(|e| match e {
            bus::DomainEvent::ChatTurnCompleted { session_key, .. } => Some(session_key.clone()),
            _ => None,
        }).unwrap_or_default();

        {
            let mut buf = self.buffer.lock().await;
            buf.push(BufferedMessage {
                content: signal.content.clone(),
                session_key,
                timestamp: jiff::Timestamp::now(),
            });
        }
        self.flush_if_needed().await;
        Ok(())
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
    use ai_core::{AiMetrics, SalienceVerdict};

    fn make_chat_signal(msg: &str, session: &str) -> AiSignal {
        AiSignal {
            domain: ai_core::RecallDomain::General,
            event_kind: "ChatTurnCompleted",
            importance: 0.3,
            salience: SalienceVerdict::Accumulate,
            content: msg.to_string(),
            entity: None,
            timestamp: jiff::Timestamp::now(),
            raw_event: Some(bus::DomainEvent::ChatTurnCompleted {
                session_key: session.to_string(),
                user_message: Some(msg.to_string()),
            }),
            metrics: AiMetrics::default(),
            coaching_signal: false,
            coaching_rule: None,
        }
    }

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

    #[tokio::test]
    async fn recall_consumer_buffers_and_flushes() {
        use ai_core::SignalConsumer;
        let (tx, mut rx) = super::super::signal_queue(32);
        let collector = RecallCollector::new(tx);

        for i in 0..BUFFER_FLUSH_SIZE {
            let sig = make_chat_signal(
                &format!("rust error handling best practices msg {i}"),
                &format!("s{}", i % 3),
            );
            collector.consume(&sig).await.unwrap();
        }
        // after BUFFER_FLUSH_SIZE messages flush happens; at least one cluster promoted
        let promoted = rx.try_recv();
        assert!(promoted.is_ok());
    }
}
