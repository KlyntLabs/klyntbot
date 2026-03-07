//! Heuristic implementations of cognitive handler traits.
//!
//! These provide simple rule-based extraction and consolidation without
//! requiring LLM calls. They serve as reliable fallbacks and can be
//! upgraded to LLM-backed implementations when needed.

use async_trait::async_trait;

use cognitive::extraction::ExtractedFact;
use cognitive::types::{MemoryOp, Observation, SemanticFact};
use cognitive::{ConsolidationHandler, ExtractionHandler};

/// Heuristic fact extraction — parses observations into SPO triples
/// using pattern matching rather than LLM calls.
pub struct HeuristicExtractionHandler;

#[async_trait]
impl ExtractionHandler for HeuristicExtractionHandler {
    async fn extract_facts(&self, observation: &Observation) -> common::Result<Vec<ExtractedFact>> {
        let fact = |domain: &str, predicate: &str, confidence: f64, source: &str| ExtractedFact {
            domain: domain.into(),
            subject: "user".into(),
            predicate: predicate.into(),
            object: observation.content.clone(),
            confidence,
            source: source.into(),
        };
        let od = observation.domain.as_str();

        let facts = match observation.source_event.as_str() {
            "UserStatedFact" => vec![fact(od, "stated", 1.0, "user_stated")],
            "UserCorrectedAI" => vec![fact(od, "corrected", 1.0, "user_stated")],
            "BudgetAlert" => vec![fact("finance", "budget_pressure", 0.9, "observed")],
            "CoachingFeedback" => vec![fact("coaching", "coaching_response", 0.9, "observed")],
            source if source.starts_with("accumulated:") => {
                vec![fact(od, "pattern", 0.7, "inferred")]
            }
            _ if observation.importance >= 0.7 => {
                vec![fact(od, "observation", observation.importance * 0.8, "observed")]
            }
            _ => vec![],
        };

        Ok(facts)
    }
}

/// Heuristic consolidation — decides ADD/UPDATE/DELETE/NOOP using
/// simple text matching on subject+predicate pairs.
pub struct HeuristicConsolidationHandler;

#[async_trait]
impl ConsolidationHandler for HeuristicConsolidationHandler {
    async fn decide(
        &self,
        candidate: &SemanticFact,
        existing: &[SemanticFact],
    ) -> common::Result<MemoryOp> {
        // Single pass: find exact duplicate or predicate-only match
        let mut update_from: Option<&SemanticFact> = None;
        for fact in existing {
            if fact.predicate == candidate.predicate {
                if fact.object == candidate.object {
                    return Ok(MemoryOp::Noop);
                }
                if update_from.is_none() {
                    update_from = Some(fact);
                }
            }
        }

        if let Some(old) = update_from {
            return Ok(MemoryOp::Update {
                id: candidate.id.clone(),
                old_id: old.id.clone(),
            });
        }

        Ok(MemoryOp::Add {
            id: candidate.id.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn test_observation(source: &str, content: &str, importance: f64) -> Observation {
        Observation {
            domain: "test".into(),
            content: content.into(),
            importance,
            source_event: source.into(),
            timestamp: Utc::now(),
        }
    }

    fn test_fact(id: &str, pred: &str, obj: &str) -> SemanticFact {
        SemanticFact {
            id: id.into(),
            domain: "test".into(),
            subject: "user".into(),
            predicate: pred.into(),
            object: obj.into(),
            confidence: 0.8,
            source: "observed".into(),
            valid_from: "2026-03-01".into(),
            valid_until: None,
            recorded_at: "2026-03-06".into(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
        }
    }

    #[tokio::test]
    async fn test_extraction_user_stated_fact() {
        let handler = HeuristicExtractionHandler;
        let obs = test_observation("UserStatedFact", "I prefer dark mode", 1.0);
        let facts = handler.extract_facts(&obs).await.unwrap();
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].source, "user_stated");
        assert_eq!(facts[0].confidence, 1.0);
    }

    #[tokio::test]
    async fn test_extraction_low_importance_skipped() {
        let handler = HeuristicExtractionHandler;
        let obs = test_observation("ProductivityScoreComputed", "Score: 72", 0.5);
        let facts = handler.extract_facts(&obs).await.unwrap();
        assert!(facts.is_empty());
    }

    #[tokio::test]
    async fn test_extraction_high_importance_extracted() {
        let handler = HeuristicExtractionHandler;
        let obs = test_observation("TransactionRecorded", "Over budget!", 0.8);
        let facts = handler.extract_facts(&obs).await.unwrap();
        assert_eq!(facts.len(), 1);
    }

    #[tokio::test]
    async fn test_consolidation_add_when_empty() {
        let handler = HeuristicConsolidationHandler;
        let candidate = test_fact("c1", "peak_hours", "10am-12pm");
        let result = handler.decide(&candidate, &[]).await.unwrap();
        assert!(matches!(result, MemoryOp::Add { .. }));
    }

    #[tokio::test]
    async fn test_consolidation_noop_on_duplicate() {
        let handler = HeuristicConsolidationHandler;
        let candidate = test_fact("c1", "peak_hours", "10am-12pm");
        let existing = vec![test_fact("e1", "peak_hours", "10am-12pm")];
        let result = handler.decide(&candidate, &existing).await.unwrap();
        assert!(matches!(result, MemoryOp::Noop));
    }

    #[tokio::test]
    async fn test_consolidation_update_on_changed_value() {
        let handler = HeuristicConsolidationHandler;
        let candidate = test_fact("c1", "peak_hours", "2pm-4pm");
        let existing = vec![test_fact("e1", "peak_hours", "10am-12pm")];
        let result = handler.decide(&candidate, &existing).await.unwrap();
        assert!(matches!(result, MemoryOp::Update { .. }));
    }

    #[tokio::test]
    async fn test_consolidation_add_different_predicate() {
        let handler = HeuristicConsolidationHandler;
        let candidate = test_fact("c1", "work_style", "deep focus");
        let existing = vec![test_fact("e1", "peak_hours", "10am-12pm")];
        let result = handler.decide(&candidate, &existing).await.unwrap();
        assert!(matches!(result, MemoryOp::Add { .. }));
    }
}
