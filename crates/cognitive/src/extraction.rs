//! Memory extraction — converts observations into semantic fact candidates.
//!
//! The `ExtractionHandler` trait is implemented in the agent crate with an
//! actual LLM provider. Tests use a mock that returns pre-defined facts.

use async_trait::async_trait;
use chrono::Utc;
use tracing::{debug, warn};

use crate::types::{Observation, SemanticFact, DEFAULT_MEMORY_TYPE};

/// A candidate fact extracted from an observation, before consolidation.
#[derive(Debug, Clone)]
pub struct ExtractedFact {
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source: String,
}

/// Trait for LLM-backed fact extraction from observations.
///
/// Defined here (L3), implemented in the agent crate (L5) with actual LLM
/// providers. This follows the same dependency inversion pattern as
/// `EnrichmentHandler` and `SpawnHandler`.
#[async_trait]
pub trait ExtractionHandler: Send + Sync {
    /// Extract structured semantic facts from a free-text observation.
    /// Returns zero or more candidate facts.
    async fn extract_facts(&self, observation: &Observation) -> common::Result<Vec<ExtractedFact>>;
}

/// Classify a fact's memory type based on trigger phrases in the object text.
///
/// Returns one of: `"decision"`, `"milestone"`, `"pattern"`, `"insight"`, or `"fact"` (default).
pub fn classify_memory_type(text: &str) -> &'static str {
    let lower = text.to_lowercase();

    const DECISION_TRIGGERS: &[&str] = &["decided to", "let's go with", "we'll use", "agreed on"];
    const MILESTONE_TRIGGERS: &[&str] =
        &["completed", "shipped", "released", "launched", "finished"];
    const PATTERN_TRIGGERS: &[&str] = &["noticed that", "pattern", "tends to", "usually"];
    const INSIGHT_TRIGGERS: &[&str] = &["realized", "learned", "discovered"];

    for trigger in DECISION_TRIGGERS {
        if lower.contains(trigger) {
            return "decision";
        }
    }
    for trigger in MILESTONE_TRIGGERS {
        if lower.contains(trigger) {
            return "milestone";
        }
    }
    for trigger in PATTERN_TRIGGERS {
        if lower.contains(trigger) {
            return "pattern";
        }
    }
    for trigger in INSIGHT_TRIGGERS {
        if lower.contains(trigger) {
            return "insight";
        }
    }

    DEFAULT_MEMORY_TYPE
}

/// Converts `ExtractedFact` candidates into full `SemanticFact` records
/// ready for consolidation.
pub fn to_semantic_fact(candidate: &ExtractedFact, observation: &Observation) -> SemanticFact {
    let now = Utc::now();
    let combined_text = format!("{} {} {}", candidate.subject, candidate.predicate, candidate.object);
    SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        domain: candidate.domain.clone(),
        subject: candidate.subject.clone(),
        predicate: candidate.predicate.clone(),
        object: candidate.object.clone(),
        confidence: candidate.confidence,
        source: candidate.source.clone(),
        valid_from: observation.timestamp.format("%Y-%m-%d").to_string(),
        valid_until: None,
        recorded_at: now.format("%Y-%m-%dT%H:%M:%S").to_string(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        project_id: None,
        memory_type: classify_memory_type(&combined_text).to_string(),
    }
}

/// Run extraction on an observation using the provided handler.
/// Returns full `SemanticFact` records ready for consolidation.
pub async fn extract_from_observation(
    handler: &dyn ExtractionHandler,
    observation: &Observation,
) -> Vec<SemanticFact> {
    match handler.extract_facts(observation).await {
        Ok(candidates) => {
            debug!(
                "Extracted {} fact candidates from observation in domain '{}'",
                candidates.len(),
                observation.domain
            );
            candidates
                .iter()
                .map(|c| to_semantic_fact(c, observation))
                .collect()
        }
        Err(e) => {
            warn!("Extraction failed for observation: {e}");
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Observation;

    struct MockExtractionHandler {
        facts: Vec<ExtractedFact>,
    }

    #[async_trait]
    impl ExtractionHandler for MockExtractionHandler {
        async fn extract_facts(
            &self,
            _observation: &Observation,
        ) -> common::Result<Vec<ExtractedFact>> {
            Ok(self.facts.clone())
        }
    }

    fn test_observation() -> Observation {
        Observation {
            domain: "productivity".into(),
            content: "User is most productive between 10am and 12pm".into(),
            importance: 0.8,
            source_event: "ProductivityScoreComputed".into(),
            timestamp: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_extract_from_observation_returns_facts() {
        let handler = MockExtractionHandler {
            facts: vec![ExtractedFact {
                domain: "productivity".into(),
                subject: "user".into(),
                predicate: "peak_hours".into(),
                object: "10am-12pm".into(),
                confidence: 0.8,
                source: "observed".into(),
            }],
        };

        let facts = extract_from_observation(&handler, &test_observation()).await;
        assert_eq!(facts.len(), 1);
        assert_eq!(facts[0].predicate, "peak_hours");
        assert_eq!(facts[0].object, "10am-12pm");
        assert_eq!(facts[0].stability, 1.0);
    }

    #[tokio::test]
    async fn test_extract_from_observation_handles_empty() {
        let handler = MockExtractionHandler { facts: vec![] };
        let facts = extract_from_observation(&handler, &test_observation()).await;
        assert!(facts.is_empty());
    }

    #[tokio::test]
    async fn test_extract_from_observation_handles_multiple() {
        let handler = MockExtractionHandler {
            facts: vec![
                ExtractedFact {
                    domain: "productivity".into(),
                    subject: "user".into(),
                    predicate: "peak_hours".into(),
                    object: "10am-12pm".into(),
                    confidence: 0.8,
                    source: "observed".into(),
                },
                ExtractedFact {
                    domain: "productivity".into(),
                    subject: "user".into(),
                    predicate: "focus_style".into(),
                    object: "deep_work".into(),
                    confidence: 0.6,
                    source: "inferred".into(),
                },
            ],
        };

        let facts = extract_from_observation(&handler, &test_observation()).await;
        assert_eq!(facts.len(), 2);
        assert_eq!(facts[0].predicate, "peak_hours");
        assert_eq!(facts[1].predicate, "focus_style");
    }

    #[test]
    fn test_classify_decision() {
        assert_eq!(classify_memory_type("decided to use PostgreSQL"), "decision");
        assert_eq!(classify_memory_type("let's go with React"), "decision");
        assert_eq!(classify_memory_type("agreed on the API design"), "decision");
    }

    #[test]
    fn test_classify_milestone() {
        assert_eq!(
            classify_memory_type("completed the auth module"),
            "milestone"
        );
        assert_eq!(classify_memory_type("shipped v2.0"), "milestone");
        assert_eq!(classify_memory_type("launched the beta"), "milestone");
    }

    #[test]
    fn test_classify_pattern() {
        assert_eq!(
            classify_memory_type("noticed that builds are slower"),
            "pattern"
        );
        assert_eq!(classify_memory_type("tends to break on Mondays"), "pattern");
    }

    #[test]
    fn test_classify_insight() {
        assert_eq!(
            classify_memory_type("realized the bottleneck is I/O"),
            "insight"
        );
        assert_eq!(classify_memory_type("learned that caching helps"), "insight");
    }

    #[test]
    fn test_classify_default() {
        assert_eq!(classify_memory_type("the sky is blue"), "fact");
    }

    #[test]
    fn test_to_semantic_fact_sets_defaults() {
        let candidate = ExtractedFact {
            domain: "productivity".into(),
            subject: "user".into(),
            predicate: "peak_hours".into(),
            object: "10am-12pm".into(),
            confidence: 0.8,
            source: "observed".into(),
        };
        let obs = test_observation();
        let fact = to_semantic_fact(&candidate, &obs);

        assert_eq!(fact.domain, "productivity");
        assert_eq!(fact.stability, 1.0);
        assert_eq!(fact.access_count, 0);
        assert!(fact.superseded_at.is_none());
        assert!(fact.valid_until.is_none());
        assert!(!fact.id.is_empty());
    }
}
