//! Memory extraction — converts observations into semantic fact candidates.
//!
//! The `ExtractionHandler` trait is implemented in the agent crate with an
//! actual LLM provider. Tests use a mock that returns pre-defined facts.

use crate::types::{Observation, SemanticFact, DEFAULT_MEMORY_TYPE};
use async_trait::async_trait;

/// A candidate fact extracted from an observation, before consolidation.
#[derive(Debug, Clone)]
pub struct ExtractedFact {
    pub domain: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub confidence: f64,
    pub source: String,
    /// Wave 2: who said this. NULL when the speaker IS the subject (i.e.
    /// first-person fact) or unknown.
    pub speaker: Option<String>,
    /// Wave 5 / T1.2: temporal end-bound for the fact. Set when the
    /// observation contains a clear duration or end-date (e.g. "left job
    /// in March 2024"). NULL means open-ended (still valid). Format:
    /// YYYY-MM-DD when day is known, YYYY-MM when only month, YYYY when
    /// only year. Lets retrieval skip facts past their expiry without
    /// losing them as historical truths.
    pub valid_until: Option<String>,
    /// Wave 5b: temporal start-bound. When set (e.g. extraction parsed
    /// "on 7 May 2023" from the source turn), overrides the observation
    /// timestamp during `to_semantic_fact`. Bench-time observations are
    /// stamped with `Timestamp::now()` which is meaningless for replayed
    /// LoCoMo conversations — the model must put the *conversation's*
    /// date into the fact. Format same as `valid_until`.
    pub valid_from: Option<String>,
}

/// Maps extracted facts back to their source observation in a batch.
#[derive(Debug, Clone)]
pub struct BatchExtraction {
    pub observation_index: usize,
    pub facts: Vec<ExtractedFact>,
}

/// Result of batch extraction, including fallback tracking.
#[derive(Debug, Clone)]
pub struct BatchExtractionResult {
    /// Facts grouped by source observation index.
    pub extractions: Vec<BatchExtraction>,
    /// Indices of observations that used heuristic fallback (LLM failed).
    pub fallback_indices: Vec<usize>,
    /// Entities discovered across all observations in the batch.
    pub entities: Vec<ExtractedEntity>,
    /// Relationships between discovered entities.
    pub relationships: Vec<ExtractedRelationship>,
}

/// An entity extracted alongside facts from an observation.
#[derive(Debug, Clone)]
pub struct ExtractedEntity {
    pub name: String,
    pub entity_type: String,
    pub description: Option<String>,
}

/// A relationship between two extracted entities.
#[derive(Debug, Clone)]
pub struct ExtractedRelationship {
    pub source_name: String,
    pub target_name: String,
    pub relationship_type: String,
}

/// Trait for fact extraction from observations.
///
/// Defined here (L3), implemented in the agent crate (L5) with actual LLM
/// providers. This follows the same dependency inversion pattern as
/// `SpawnHandler`.
#[async_trait]
pub trait ExtractionHandler: Send + Sync {
    /// Extract structured semantic facts from a batch of observations.
    /// Returns facts grouped by observation index, plus indices of any
    /// observations that fell back to heuristic extraction.
    async fn extract_facts_batch(
        &self,
        observations: &[Observation],
    ) -> common::Result<BatchExtractionResult>;
}

/// AUDD operation classifier (Mem0 pattern).
///
/// For each candidate fact extracted from an observation, the
/// ingestion consumer can ask a `ConflictResolver` how to apply it
/// against existing memory: ADD a new row, UPDATE an existing one,
/// DELETE a contradicted one, or NOOP if equivalent. Without this,
/// every extraction appends and the memory store accumulates
/// contradictions over time.
///
/// Implementation lives in the agent crate (LLM-backed). Default
/// `NoopConflictResolver` returns `Add` for every candidate, matching
/// pre-AUDD behavior.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConflictDecision {
    Add,
    /// Update the existing fact with the candidate's content.
    /// `existing_id` references the fact to overwrite.
    Update {
        existing_id: String,
    },
    /// Mark `existing_id` as superseded.
    /// `superseded_by` is the candidate (which is also Added fresh).
    Delete {
        existing_id: String,
    },
    Noop,
}

#[async_trait]
pub trait ConflictResolver: Send + Sync {
    /// Decide what to do with a candidate fact given the closest
    /// existing facts (typically top-k semantically similar). Implementations
    /// MUST be pure-ish — no side effects on storage; the caller applies
    /// the chosen operation.
    async fn classify(
        &self,
        candidate: &ExtractedFact,
        nearest: &[crate::types::SemanticFact],
    ) -> ConflictDecision;
}

/// Default no-op resolver: always ADD. Used when no LLM-backed
/// resolver is wired (e.g., no cognitive provider configured).
/// Preserves pre-AUDD ingestion semantics.
pub struct NoopConflictResolver;

#[async_trait]
impl ConflictResolver for NoopConflictResolver {
    async fn classify(
        &self,
        _candidate: &ExtractedFact,
        _nearest: &[crate::types::SemanticFact],
    ) -> ConflictDecision {
        ConflictDecision::Add
    }
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
    let now = jiff::Timestamp::now();
    let combined_text = format!(
        "{} {} {}",
        candidate.subject, candidate.predicate, candidate.object
    );
    SemanticFact {
        id: uuid::Uuid::new_v4().to_string(),
        domain: candidate.domain.clone(),
        subject: candidate.subject.clone(),
        predicate: candidate.predicate.clone(),
        object: candidate.object.clone(),
        confidence: candidate.confidence,
        source: candidate.source.clone(),
        valid_from: candidate
            .valid_from
            .clone()
            .unwrap_or_else(|| observation.timestamp.strftime("%Y-%m-%d").to_string()),
        valid_until: candidate.valid_until.clone(),
        recorded_at: now.strftime("%Y-%m-%dT%H:%M:%SZ").to_string(),
        superseded_at: None,
        superseded_by: None,
        stability: 1.0,
        last_accessed: None,
        access_count: 0,
        convergence_score: 0.0,
        project_id: None,
        memory_type: classify_memory_type(&combined_text).to_string(),
        scope_type: "system".to_string(),
        scope_id: None,
        scope_repo_id: None,
        metadata: None,
        speaker: candidate.speaker.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Observation;

    fn test_observation() -> Observation {
        Observation {
            domain: "productivity".into(),
            content: "User is most productive between 10am and 12pm".into(),
            importance: 0.8,
            source_event: "ProductivityScoreComputed".into(),
            timestamp: jiff::Timestamp::now(),
        }
    }

    #[test]
    fn test_batch_extraction_result_structure() {
        let result = BatchExtractionResult {
            extractions: vec![
                BatchExtraction {
                    observation_index: 0,
                    facts: vec![ExtractedFact {
                        domain: "productivity".into(),
                        subject: "user".into(),
                        predicate: "peak_hours".into(),
                        object: "10am-12pm".into(),
                        confidence: 0.8,
                        source: "observed".into(),

                        speaker: None,
                        valid_from: None,
                        valid_until: None,
                    }],
                },
                BatchExtraction {
                    observation_index: 1,
                    facts: vec![],
                },
            ],
            fallback_indices: vec![1],
            entities: Vec::new(),
            relationships: Vec::new(),
        };
        assert_eq!(result.extractions.len(), 2);
        assert_eq!(result.extractions[0].facts.len(), 1);
        assert_eq!(result.fallback_indices, vec![1]);
    }

    #[test]
    fn test_classify_decision() {
        assert_eq!(
            classify_memory_type("decided to use PostgreSQL"),
            "decision"
        );
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
        assert_eq!(
            classify_memory_type("learned that caching helps"),
            "insight"
        );
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

            speaker: None,
            valid_from: None,
            valid_until: None,
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
