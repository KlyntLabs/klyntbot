//! Types for the per-turn graph linker (KCA Track 2).
//!
//! The linker takes a freshly-committed fact along with its 1-hop neighborhood
//! and cross-entity context, and returns a structured set of operations:
//! entity merges, discovered relationships (typed), and explicit supersessions.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphLinkInput {
    pub new_fact: NewFactRef,
    pub subject_neighborhood: Vec<NeighborRef>,
    pub object_neighborhood: Vec<NeighborRef>,
    pub candidate_facts: Vec<ExistingFactRef>,
    /// Last 1-2 user messages, truncated to ~120 chars each. Provides anchoring context.
    pub recent_user_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewFactRef {
    pub fact_id: String,
    pub subject: String,
    pub subject_entity_id: Option<String>,
    pub predicate: String,
    pub object: String,
    pub object_entity_id: Option<String>,
    pub confidence: f64,
    pub valid_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NeighborRef {
    pub entity_id: String,
    pub name: String,
    pub relationship_type: String,
    pub strength: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExistingFactRef {
    pub fact_id: String,
    pub subject: String,
    pub predicate: String,
    pub object: String,
    pub valid_at: String,
    pub valid_until: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GraphLinkOutput {
    /// Entity merges: "these two entity_ids point to the same real-world thing".
    pub merges: Vec<MergeDecision>,
    /// New typed edges to add to entity_relationships.
    pub discovered_relationships: Vec<DiscoveredRelationship>,
    /// Existing facts to mark as superseded by this fact.
    pub superseded: Vec<SupersedeOp>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergeDecision {
    pub entity_a_id: String,
    pub entity_b_id: String,
    pub canonical_name: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredRelationship {
    pub source_entity_name: String,
    pub target_entity_name: String,
    pub relationship_type: String,
    /// "causal" | "correlational" | "temporal" | "structural"
    pub edge_type: String,
    pub strength: f64,
    pub evidence: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupersedeOp {
    pub old_fact_id: String,
    pub valid_until: String,
    pub reason: String,
}

/// Heuristic gate: skip the LLM call when we have no graph context to work with.
pub fn should_invoke_linker(input: &GraphLinkInput) -> bool {
    let has_neighborhood =
        !input.subject_neighborhood.is_empty() || !input.object_neighborhood.is_empty();
    let has_candidates = !input.candidate_facts.is_empty();
    has_neighborhood || has_candidates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fact() -> NewFactRef {
        NewFactRef {
            fact_id: "f1".into(),
            subject: "Alice".into(),
            subject_entity_id: Some("e1".into()),
            predicate: "prefers".into(),
            object: "Rust".into(),
            object_entity_id: None,
            confidence: 0.8,
            valid_at: "2026-04-29T00:00:00Z".into(),
        }
    }

    #[test]
    fn skip_when_no_neighborhood_and_no_candidates() {
        let i = GraphLinkInput {
            new_fact: fact(),
            subject_neighborhood: vec![],
            object_neighborhood: vec![],
            candidate_facts: vec![],
            recent_user_text: None,
        };
        assert!(!should_invoke_linker(&i));
    }

    #[test]
    fn invoke_when_neighborhood_present() {
        let i = GraphLinkInput {
            new_fact: fact(),
            subject_neighborhood: vec![NeighborRef {
                entity_id: "e2".into(),
                name: "Bob".into(),
                relationship_type: "knows".into(),
                strength: 0.8,
            }],
            object_neighborhood: vec![],
            candidate_facts: vec![],
            recent_user_text: None,
        };
        assert!(should_invoke_linker(&i));
    }

    #[test]
    fn invoke_when_candidate_facts_present() {
        let i = GraphLinkInput {
            new_fact: fact(),
            subject_neighborhood: vec![],
            object_neighborhood: vec![],
            candidate_facts: vec![ExistingFactRef {
                fact_id: "f2".into(),
                subject: "Alice".into(),
                predicate: "knows".into(),
                object: "Java".into(),
                valid_at: "2025-01-01T00:00:00Z".into(),
                valid_until: None,
            }],
            recent_user_text: None,
        };
        assert!(should_invoke_linker(&i));
    }
}
