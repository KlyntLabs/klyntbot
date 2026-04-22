//! Stage 2: groups signals, computes convergence, decides promotions.

use std::collections::HashSet;

use async_trait::async_trait;
use tracing::info;

use super::signal::{CognitiveSignal, SignalSource};
use crate::repos::procedural_rule::word_overlap_ratio;

const GROUPING_THRESHOLD: f64 = 0.4;

#[derive(Debug, Clone)]
pub struct KnowledgeCluster {
    pub signals: Vec<CognitiveSignal>,
    pub merged_subject: String,
    pub domain: ai_core::RecallDomain,
    pub source_diversity: u32,
    pub convergence_score: f64,
    pub max_confidence: f64,
    pub combined_observations: Vec<String>,
}

#[derive(Debug, Clone)]
pub enum PromotionOp {
    CreateFact {
        subject: String,
        predicate: String,
        object: String,
        domain: ai_core::RecallDomain,
        confidence: f64,
        convergence: f64,
        source: String,
    },
    CreateRule {
        rule_text: String,
        domain: ai_core::RecallDomain,
        confidence: f64,
    },
    CreateEpisode {
        content: String,
        summary: String,
        domain: ai_core::RecallDomain,
        importance: f64,
    },
}

pub fn group_signals(signals: Vec<CognitiveSignal>) -> Vec<KnowledgeCluster> {
    let mut clusters: Vec<KnowledgeCluster> = Vec::new();
    'outer: for signal in signals {
        for cluster in &mut clusters {
            if cluster.domain == signal.domain
                && word_overlap_ratio(&cluster.merged_subject, &signal.content) > GROUPING_THRESHOLD
            {
                if signal.confidence > cluster.max_confidence {
                    cluster.merged_subject = signal.content.clone();
                    cluster.max_confidence = signal.confidence;
                }
                let raw_obs = signal.context.raw_observations.clone();
                cluster.signals.push(signal);
                cluster.combined_observations.extend(raw_obs);
                let sources: HashSet<SignalSource> =
                    cluster.signals.iter().map(|s| s.source).collect();
                cluster.source_diversity = sources.len() as u32;
                cluster.convergence_score = cluster.source_diversity as f64 / 5.0;
                continue 'outer;
            }
        }
        // No existing cluster matched — create a new one.
        clusters.push(KnowledgeCluster {
            merged_subject: signal.content.clone(),
            domain: signal.domain,
            source_diversity: 1,
            convergence_score: 0.2,
            max_confidence: signal.confidence,
            combined_observations: signal.context.raw_observations.clone(),
            signals: vec![signal],
        });
    }
    clusters
}

pub fn heuristic_promote(clusters: &[KnowledgeCluster]) -> Vec<PromotionOp> {
    let mut ops = Vec::new();
    for cluster in clusters {
        let has_coaching = cluster
            .signals
            .iter()
            .any(|s| s.source == SignalSource::CoachingPattern);

        if has_coaching && cluster.max_confidence >= 0.7 {
            ops.push(PromotionOp::CreateRule {
                rule_text: cluster.merged_subject.clone(),
                domain: cluster.domain,
                confidence: cluster.max_confidence,
            });
        } else if cluster.max_confidence >= 0.6 || cluster.convergence_score >= 0.4 {
            let (subject, predicate, object) = extract_spo(&cluster.merged_subject);
            ops.push(PromotionOp::CreateFact {
                subject,
                predicate,
                object,
                domain: cluster.domain,
                confidence: cluster.max_confidence,
                convergence: cluster.convergence_score,
                source: promotion_source(&cluster.signals),
            });
        } else if cluster.max_confidence >= 0.5 {
            let summary = if cluster.merged_subject.len() > 120 {
                format!("{}...", &cluster.merged_subject[..117])
            } else {
                cluster.merged_subject.clone()
            };
            ops.push(PromotionOp::CreateEpisode {
                content: cluster.merged_subject.clone(),
                summary,
                domain: cluster.domain,
                importance: cluster.max_confidence,
            });
        }
    }
    info!(
        "Consolidator: {} ops from {} clusters",
        ops.len(),
        clusters.len()
    );
    ops
}

/// Trait for LLM-backed deep consolidation decisions.
///
/// Defined here (cognitive crate), implemented in the agent crate with an
/// actual LLM provider. This follows the same dependency inversion pattern as
/// `ExtractionHandler` and `ConsolidationHandler`.
#[async_trait]
pub trait DeepConsolidationHandler: Send + Sync {
    /// Given knowledge clusters, use an LLM call to decide promotion operations.
    async fn consolidate_deep(
        &self,
        clusters: &[KnowledgeCluster],
    ) -> common::Result<Vec<PromotionOp>>;
}

/// Use an LLM to decide promotions for the given clusters.
///
/// Falls back to `heuristic_promote` if the LLM call fails.
pub async fn deep_promote(
    clusters: &[KnowledgeCluster],
    handler: &dyn DeepConsolidationHandler,
) -> Vec<PromotionOp> {
    match handler.consolidate_deep(clusters).await {
        Ok(ops) => {
            info!(
                "Deep consolidation: {} ops from {} clusters",
                ops.len(),
                clusters.len()
            );
            ops
        }
        Err(e) => {
            tracing::warn!("Deep consolidation failed, falling back to heuristic: {e}");
            heuristic_promote(clusters)
        }
    }
}

fn extract_spo(text: &str) -> (String, String, String) {
    for pred in [
        "is a", "is", "has", "prefers", "uses", "works", "likes", "wants", "needs",
    ] {
        if let Some(idx) = text.to_lowercase().find(pred) {
            let subject = text[..idx].trim().to_string();
            let object = text[idx + pred.len()..].trim().to_string();
            if !subject.is_empty() && !object.is_empty() {
                return (subject, pred.to_string(), object);
            }
        }
    }
    ("user".into(), "noted".into(), text.to_string())
}

pub fn promotion_source(signals: &[CognitiveSignal]) -> String {
    let mut sources: Vec<&str> = signals
        .iter()
        .map(|s| match s.source {
            SignalSource::ChatTurn => "chat",
            SignalSource::SessionEnd => "session",
            SignalSource::AtomReinforcement => "notes",
            SignalSource::CoachingPattern => "coaching",
            SignalSource::ConversationRecall => "recall",
            SignalSource::UserStatedFact => "user_stated",
        })
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();
    sources.sort();
    sources.join("+")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pipeline::signal::SignalContext;
    use jiff::Timestamp;

    fn sig(source: SignalSource, content: &str, domain: ai_core::RecallDomain, confidence: f64) -> CognitiveSignal {
        CognitiveSignal {
            source,
            content: content.into(),
            domain,
            confidence,
            context: SignalContext::default(),
            timestamp: Timestamp::now(),
        }
    }

    #[test]
    fn test_group_similar() {
        let signals = vec![
            sig(
                SignalSource::ChatTurn,
                "User is learning Rust programming language",
                ai_core::RecallDomain::Learning,
                0.7,
            ),
            sig(
                SignalSource::AtomReinforcement,
                "Rust programming language concepts",
                ai_core::RecallDomain::Learning,
                0.8,
            ),
        ];
        let clusters = group_signals(signals);
        assert_eq!(clusters.len(), 1);
        assert_eq!(clusters[0].source_diversity, 2);
    }

    #[test]
    fn test_group_dissimilar() {
        let clusters = group_signals(vec![
            sig(
                SignalSource::ChatTurn,
                "User is learning Rust",
                ai_core::RecallDomain::Learning,
                0.7,
            ),
            sig(
                SignalSource::CoachingPattern,
                "Take breaks in the afternoon",
                ai_core::RecallDomain::Productivity,
                0.8,
            ),
        ]);
        assert_eq!(clusters.len(), 2);
    }

    #[test]
    fn test_promote_fact() {
        let clusters = group_signals(vec![sig(
            SignalSource::ChatTurn,
            "Jayden is a software engineer",
            ai_core::RecallDomain::General,
            0.8,
        )]);
        let ops = heuristic_promote(&clusters);
        assert!(matches!(&ops[0], PromotionOp::CreateFact { subject, .. } if subject == "Jayden"));
    }

    #[test]
    fn test_promote_rule() {
        let clusters = group_signals(vec![sig(
            SignalSource::CoachingPattern,
            "Schedule tasks in the morning",
            ai_core::RecallDomain::Productivity,
            0.8,
        )]);
        let ops = heuristic_promote(&clusters);
        assert!(matches!(&ops[0], PromotionOp::CreateRule { .. }));
    }

    #[test]
    fn test_promote_episode() {
        let clusters = group_signals(vec![sig(
            SignalSource::SessionEnd,
            "Fixed a tricky async bug in middleware",
            ai_core::RecallDomain::General,
            0.55,
        )]);
        let ops = heuristic_promote(&clusters);
        assert!(matches!(&ops[0], PromotionOp::CreateEpisode { .. }));
    }

    #[test]
    fn test_extract_spo() {
        let (s, p, o) = extract_spo("Jayden is a software engineer");
        assert_eq!(s, "Jayden");
        assert_eq!(p, "is a");
        assert_eq!(o, "software engineer");
    }

    #[test]
    fn test_convergence_multi_source() {
        // All three signals share enough words to chain-group:
        // s1 vs s2: "Rust language" in common → ratio > 0.4 → merge
        // s2 becomes merged_subject (higher confidence)
        // s2 vs s3: "Rust language learning" in common → ratio > 0.4 → merge
        let clusters = group_signals(vec![
            sig(
                SignalSource::ChatTurn,
                "User is learning Rust language",
                ai_core::RecallDomain::Learning,
                0.6,
            ),
            sig(
                SignalSource::AtomReinforcement,
                "Learning Rust language every day",
                ai_core::RecallDomain::Learning,
                0.7,
            ),
            sig(
                SignalSource::CoachingPattern,
                "Rust language learning is important",
                ai_core::RecallDomain::Learning,
                0.8,
            ),
        ]);
        assert_eq!(clusters.len(), 1);
        assert!((clusters[0].convergence_score - 0.6).abs() < 0.01);
    }
}
