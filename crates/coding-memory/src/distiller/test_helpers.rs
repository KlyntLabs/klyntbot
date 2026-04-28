//! Test helpers for distiller integration tests (KCA Track 3 onwards).

use cognitive::repos::entity::EntityRepo;
use cognitive::repos::semantic_fact::SemanticFactRepo;

/// Builder for test fixtures.
pub struct FixtureBuilder {
    user_prompt: Option<String>,
    assistant_msg: Option<String>,
}

impl Default for FixtureBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl FixtureBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            user_prompt: None,
            assistant_msg: None,
        }
    }
    /// Set the user prompt.
    pub fn with_user_prompt(mut self, t: impl Into<String>) -> Self {
        self.user_prompt = Some(t.into());
        self
    }
    /// Set the assistant message.
    pub fn with_assistant_msg(mut self, t: impl Into<String>) -> Self {
        self.assistant_msg = Some(t.into());
        self
    }
    /// Build the fixture.
    pub fn build(self) -> Fixture {
        Fixture {
            user_prompt: self.user_prompt.unwrap_or_default(),
            assistant_msg: self.assistant_msg.unwrap_or_default(),
        }
    }
}

/// Test fixture containing user prompt and assistant message.
pub struct Fixture {
    /// User prompt text.
    pub user_prompt: String,
    /// Assistant message text.
    pub assistant_msg: String,
}

/// Runs the distiller pipeline against an in-memory pool, including extraction (heuristic
/// fallback when no provider) and graph-edge writes.
pub async fn distill_test_turn(
    fixture: &Fixture,
    fact_repo: &SemanticFactRepo,
    entity_repo: &EntityRepo,
) {
    use crate::distiller::phase_c::write_entity_edges_for_distiller_fact;

    // Phase A: extract a single repo_context fact heuristically.
    // For "this repo uses X" patterns, emit (klyntbot, uses, X).
    let combined = format!("{}\n{}", fixture.user_prompt, fixture.assistant_msg);
    if let Some((subject, predicate, object)) = naive_repo_context_extract(&combined) {
        let fact = cognitive::types::SemanticFact {
            id: uuid::Uuid::new_v4().to_string(),
            domain: "coding".into(),
            subject: subject.clone(),
            predicate: predicate.clone(),
            object: object.clone(),
            confidence: 0.85,
            source: "distiller_test".into(),
            valid_from: jiff::Timestamp::now().to_string(),
            valid_until: None,
            recorded_at: jiff::Timestamp::now().to_string(),
            superseded_at: None,
            superseded_by: None,
            stability: 1.0,
            last_accessed: None,
            access_count: 0,
            convergence_score: 0.0,
            project_id: None,
            memory_type: cognitive::types::DEFAULT_MEMORY_TYPE.to_string(),
            scope_type: "system".into(),
            scope_id: None,
            scope_repo_id: None,
            metadata: None,
        };
        let _ = fact_repo.upsert(&fact).await;
        write_entity_edges_for_distiller_fact(&fact, entity_repo).await;
    }
}

fn naive_repo_context_extract(text: &str) -> Option<(String, String, String)> {
    // Patterns: "{repo} uses {tool}" and "this repo uses {tool}".
    let lower = text.to_lowercase();
    let triggers = ["this repo uses ", "the repo uses ", "we use "];
    for t in triggers {
        if let Some(i) = lower.find(t) {
            let after = &text[i + t.len()..];
            let object = after
                .split_whitespace()
                .next()?
                .trim_end_matches('.')
                .to_string();
            return Some(("klyntbot".to_string(), "uses".to_string(), object));
        }
    }
    None
}
