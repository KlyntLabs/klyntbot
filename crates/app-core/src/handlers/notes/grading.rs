use desktop_shared::commands::{
    DiffSegmentResponse, FlashcardExplainParams, FlashcardExplainResponse, FlashcardResponse,
    FlashcardSubmitAnswerParams, GradeResultResponse,
};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

/// Map a 0.0–1.0 score to an FSRS rating label.
pub fn score_to_rating(score: f64) -> &'static str {
    if score >= 0.85 {
        "easy"
    } else if score >= 0.60 {
        "good"
    } else if score >= 0.30 {
        "hard"
    } else {
        "again"
    }
}

/// Try an exact (case-insensitive, trimmed) match. Returns `Some(1.0)` on match, `None` otherwise.
pub fn grade_exact_match(user_answer: &str, expected: &str) -> Option<f64> {
    let u = user_answer.trim();
    let e = expected.trim();
    if u.eq_ignore_ascii_case(e) {
        Some(1.0)
    } else {
        None
    }
}

/// Grade using cosine similarity pre-filter.
///
/// - `>= accept_threshold` → definite pass, score = 0.85 + scaled bonus up to 1.0
/// - `<= fail_threshold`   → definite fail, score = 0.15
/// - in between            → `None` (needs LLM grading)
pub fn grade_semantic(cosine_sim: f64, accept_threshold: f64, fail_threshold: f64) -> Option<f64> {
    if cosine_sim >= accept_threshold {
        // Scale bonus: 0.85 at threshold, 1.0 at cosine_sim=1.0
        let bonus_range = 1.0 - accept_threshold;
        let bonus = if bonus_range > 0.0 {
            ((cosine_sim - accept_threshold) / bonus_range) * 0.15
        } else {
            0.15
        };
        Some((0.85 + bonus).min(1.0))
    } else if cosine_sim <= fail_threshold {
        Some(0.15)
    } else {
        None
    }
}

/// Build (system_prompt, user_prompt) for LLM grading of a flashcard answer.
pub fn build_grading_prompt(
    front: &str,
    back: &str,
    user_answer: &str,
    source_context: Option<&str>,
) -> (String, String) {
    let system = r#"You are an expert flashcard grading assistant. Evaluate the user's answer against the expected answer.

Return ONLY this exact JSON (no extra text):
{
  "score": 0.0-1.0,
  "explanation": "brief explanation of the grade",
  "key_concepts_present": ["concept1", "concept2"],
  "key_concepts_missing": ["concept3"],
  "coaching_nudge": "optional short tip or null",
  "socratic_suggestion": "optional follow-up question or null"
}

Scoring guidelines:
- 1.0: Perfect or essentially equivalent answer
- 0.85-0.99: Correct with minor stylistic differences
- 0.60-0.84: Mostly correct, captures key ideas but misses nuance
- 0.30-0.59: Partially correct, shows some understanding
- 0.00-0.29: Incorrect or fundamentally wrong"#
        .to_string();

    let mut user = format!(
        "Question (front of card):\n{front}\n\nExpected answer (back of card):\n{back}\n\nUser's answer:\n{user_answer}"
    );

    if let Some(ctx) = source_context {
        user.push_str(&format!("\n\nSource context:\n{ctx}"));
    }

    (system, user)
}

/// LLM grading result parsed from JSON.
#[derive(serde::Deserialize)]
struct LlmGradeResult {
    score: f64,
    explanation: Option<String>,
    #[serde(default)]
    key_concepts_present: Vec<String>,
    #[serde(default)]
    key_concepts_missing: Vec<String>,
    coaching_nudge: Option<String>,
    socratic_suggestion: Option<String>,
}

impl AppCore {
    /// Full three-stage grading pipeline: exact match → semantic pre-filter → LLM.
    pub async fn flashcard_submit_answer(
        &self,
        params: FlashcardSubmitAnswerParams,
    ) -> Result<GradeResultResponse, ApiError> {
        // 1. Fetch the card
        let repo = self.flashcard_repo()?;
        let card = repo
            .get_by_id(&params.card_id)
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Flashcard not found"))?;

        let expected = &card.back;

        // 2. Grade via 3-stage pipeline: exact match → semantic → LLM
        // All paths produce `response`; weak-spot atom creation happens after.
        let response = 'grading: {
            // Stage 1: exact match
            if let Some(score) = grade_exact_match(&params.user_answer, expected) {
                let mut resp = build_response(
                    score,
                    "exact_match",
                    expected,
                    Some("Exact match!".to_string()),
                    None,
                    None,
                    vec![],
                    vec![],
                );
                resp.diff_highlights = vec![DiffSegmentResponse {
                    text: params.user_answer.clone(),
                    status: "match".to_string(),
                }];
                break 'grading resp;
            }

            // Stage 2: semantic pre-filter
            let config = self.config.read().await;
            let ar = &config.learning.active_recall;
            let accept_threshold = ar.semantic_auto_accept_threshold;
            let fail_threshold = ar.semantic_auto_fail_threshold;
            drop(config);

            let cosine_sim = self
                .compute_answer_similarity(&params.card_id, &params.user_answer)
                .await;

            if let Some(score) = grade_semantic(cosine_sim, accept_threshold, fail_threshold) {
                let explanation = if score >= 0.85 {
                    format!(
                        "Semantically very close (similarity: {:.2}). Auto-accepted.",
                        cosine_sim
                    )
                } else {
                    format!(
                        "Low semantic similarity ({:.2}). The answer may be off-target.",
                        cosine_sim
                    )
                };

                break 'grading build_response(
                    score,
                    "semantic",
                    expected,
                    Some(explanation),
                    None,
                    None,
                    vec![],
                    vec![],
                );
            }

            // Stage 3: LLM grading (borderline zone)
            let (system_prompt, user_prompt) = build_grading_prompt(
                &card.front,
                expected,
                &params.user_answer,
                card.source_context.as_deref(),
            );

            let llm_result = self.grade_via_llm(&system_prompt, &user_prompt).await;

            match llm_result {
                Ok(result) => {
                    let score = result.score.clamp(0.0, 1.0);
                    build_response(
                        score,
                        "llm",
                        expected,
                        result.explanation,
                        result.coaching_nudge,
                        result.socratic_suggestion,
                        result.key_concepts_present,
                        result.key_concepts_missing,
                    )
                }
                Err(_) => {
                    let fallback_score = 0.85 * cosine_sim;
                    build_response(
                        fallback_score,
                        "semantic_fallback",
                        expected,
                        Some(format!(
                            "AI grading temporarily unavailable. Score based on semantic similarity ({:.2}).",
                            cosine_sim
                        )),
                        None,
                        None,
                        vec![],
                        vec![],
                    )
                }
            }
        };

        // Persist and publish knowledge atom for ALL low-scoring answers (weak spots).
        if let Some(score) = response.score {
            if score < 0.6 {
                let mut atom_id = uuid::Uuid::new_v4().to_string();

                if let Some(atom_repo) = &self.knowledge_atom_repo {
                    let topic_id = atom_repo
                        .get_or_create_topic(&card.deck, &card.deck)
                        .await
                        .ok()
                        .map(|t| t.id);

                    let new_atom = cognitive::NewKnowledgeAtom {
                        subject: format!(
                            "Weak: {}",
                            card.front.chars().take(80).collect::<String>()
                        ),
                        atom_type: "flashcard_weak_spot".to_string(),
                        domain: card.deck.clone(),
                        source_note_id: card.source_note_id.clone(),
                        source_context: Some(format!(
                            "Score: {:.0}% on: {}",
                            score * 100.0,
                            card.front
                        )),
                        personal_importance: 0.7,
                        status: "active".to_string(),
                        topic_id,
                        ..Default::default()
                    };

                    if let Ok(created) = atom_repo.create(&new_atom).await {
                        atom_id = created.id;
                    }
                }

                if let Some(bus) = &self.domain_event_bus {
                    bus.publish(bus::DomainEvent::KnowledgeAtomCreated {
                        atom_id,
                        atom_type: "flashcard_weak_spot".to_string(),
                        domain: card.deck.clone(),
                        source_note_id: card.source_note_id.clone(),
                        personal_importance: 0.7,
                    });
                }
            }
        }

        Ok(response)
    }

    /// Call LLM for grading, returning a parsed `LlmGradeResult`.
    async fn grade_via_llm(
        &self,
        system_prompt: &str,
        user_prompt: &str,
    ) -> Result<LlmGradeResult, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let messages = vec![
            providers::Message::System {
                content: system_prompt.to_string(),
            },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt.to_string()),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let text = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        let cleaned = common::helpers::strip_llm_fences(&text);
        let result: LlmGradeResult = serde_json::from_str(cleaned).map_err(|e| {
            ApiError::new(
                "PARSE_ERROR",
                format!("Failed to parse grading response: {e}"),
            )
        })?;

        Ok(result)
    }

    /// Socratic follow-up: explain a grading result using LLM.
    pub async fn flashcard_explain_answer(
        &self,
        params: FlashcardExplainParams,
    ) -> Result<FlashcardExplainResponse, ApiError> {
        // 1. Fetch card
        let repo = self.flashcard_repo()?;
        let card = repo
            .get_by_id(&params.card_id)
            .await
            .map_err(|e| ApiError::new("STORAGE_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Flashcard not found"))?;

        // 2. Build Socratic tutoring prompt
        let system = r#"You are a Socratic tutor helping a student understand why their answer was graded a certain way.

Your job is NOT to just give the correct answer. Instead:
1. Acknowledge what they got right
2. Ask a guiding question that leads them toward the missing concept
3. Give a brief hint if needed
4. Keep it encouraging and conversational

Be concise — 2-4 sentences max."#
            .to_string();

        let user = format!(
            "Flashcard question:\n{}\n\nExpected answer:\n{}\n\nStudent's answer:\n{}\n\nGrading explanation:\n{}",
            card.front, card.back, params.user_answer, params.grade_explanation
        );

        // 3. Call LLM
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 1024);
        drop(config);

        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(user),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let explanation = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        let mut atom_id_for_event = uuid::Uuid::new_v4().to_string();
        let mut persisted = false;

        if let Some(atom_repo) = &self.knowledge_atom_repo {
            let topic_id = atom_repo
                .get_or_create_topic(&card.deck, &card.deck)
                .await
                .ok()
                .map(|t| t.id);

            let new_atom = cognitive::NewKnowledgeAtom {
                subject: format!(
                    "Socratic: {}",
                    card.front.chars().take(80).collect::<String>()
                ),
                atom_type: "socratic_exchange".to_string(),
                domain: card.deck.clone(),
                source_note_id: card.source_note_id.clone(),
                source_context: Some(explanation.chars().take(200).collect()),
                personal_importance: 0.6,
                status: "active".to_string(),
                topic_id,
                ..Default::default()
            };

            if let Ok(created) = atom_repo.create(&new_atom).await {
                atom_id_for_event = created.id;
                persisted = true;
            }
        }

        // Always publish bus event (preserves behavior for coaching subscribers
        // even when atom_repo is None).
        let saved = if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::KnowledgeAtomCreated {
                atom_id: atom_id_for_event,
                atom_type: "socratic_exchange".to_string(),
                domain: card.deck.clone(),
                source_note_id: card.source_note_id.clone(),
                personal_importance: 0.6,
            });
            persisted
        } else {
            false
        };

        Ok(FlashcardExplainResponse {
            explanation,
            saved_as_memory: saved,
        })
    }

    /// Return prerequisite cards for a given card.
    ///
    /// Delegates to the graph propagation module which finds cards linked via
    /// `note_links` that are due within 7 days.
    pub async fn flashcard_get_prerequisites(
        &self,
        card_id: &str,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        self.flashcard_get_prerequisites_impl(card_id).await
    }
}

/// Build diff highlights from key concepts present/missing.
fn build_diff_highlights(present: &[String], missing: &[String]) -> Vec<DiffSegmentResponse> {
    present
        .iter()
        .map(|c| DiffSegmentResponse {
            text: c.clone(),
            status: "match".to_string(),
        })
        .chain(missing.iter().map(|c| DiffSegmentResponse {
            text: c.clone(),
            status: "missing".to_string(),
        }))
        .collect()
}

/// Construct a `GradeResultResponse` from common fields.
#[allow(clippy::too_many_arguments)]
fn build_response(
    score: f64,
    method: &str,
    expected_answer: &str,
    explanation: Option<String>,
    coaching_nudge: Option<String>,
    socratic_suggestion: Option<String>,
    key_concepts_present: Vec<String>,
    key_concepts_missing: Vec<String>,
) -> GradeResultResponse {
    GradeResultResponse {
        score: Some(score),
        suggested_rating: score_to_rating(score).to_string(),
        grading_method: method.to_string(),
        explanation,
        diff_highlights: build_diff_highlights(&key_concepts_present, &key_concepts_missing),
        expected_answer: expected_answer.to_string(),
        coaching_nudge,
        socratic_suggestion,
        key_concepts_present,
        key_concepts_missing,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_score_to_rating() {
        // Boundary: 0.85 is "easy"
        assert_eq!(score_to_rating(1.0), "easy");
        assert_eq!(score_to_rating(0.85), "easy");
        assert_eq!(score_to_rating(0.99), "easy");

        // Boundary: 0.60–0.84 is "good"
        assert_eq!(score_to_rating(0.84), "good");
        assert_eq!(score_to_rating(0.60), "good");
        assert_eq!(score_to_rating(0.72), "good");

        // Boundary: 0.30–0.59 is "hard"
        assert_eq!(score_to_rating(0.59), "hard");
        assert_eq!(score_to_rating(0.30), "hard");
        assert_eq!(score_to_rating(0.45), "hard");

        // Boundary: 0.00–0.29 is "again"
        assert_eq!(score_to_rating(0.29), "again");
        assert_eq!(score_to_rating(0.0), "again");
        assert_eq!(score_to_rating(0.15), "again");
    }

    #[test]
    fn test_exact_match_grading() {
        // Exact match (case insensitive)
        assert_eq!(grade_exact_match("Hello World", "hello world"), Some(1.0));

        // Exact match with leading/trailing whitespace
        assert_eq!(grade_exact_match("  Hello  ", "Hello"), Some(1.0));

        // Exact match, identical
        assert_eq!(grade_exact_match("answer", "answer"), Some(1.0));

        // Miss — different content
        assert_eq!(grade_exact_match("wrong answer", "right answer"), None);

        // Miss — partial match
        assert_eq!(grade_exact_match("Hello", "Hello World"), None);

        // Empty strings match
        assert_eq!(grade_exact_match("", ""), Some(1.0));
        assert_eq!(grade_exact_match("  ", "  "), Some(1.0));
    }

    #[test]
    fn test_semantic_grading() {
        let accept = 0.78;
        let fail = 0.45;

        // Above accept threshold → pass with scaled score
        let result = grade_semantic(0.90, accept, fail);
        assert!(result.is_some());
        let score = result.unwrap();
        assert!((0.85..=1.0).contains(&score), "score was {score}");

        // At exactly accept threshold → pass
        let result = grade_semantic(0.78, accept, fail);
        assert!(result.is_some());
        assert!(result.unwrap() >= 0.85);

        // At max similarity → 1.0
        let result = grade_semantic(1.0, accept, fail);
        assert_eq!(result, Some(1.0));

        // Below fail threshold → fail
        let result = grade_semantic(0.30, accept, fail);
        assert_eq!(result, Some(0.15));

        // At exactly fail threshold → fail
        let result = grade_semantic(0.45, accept, fail);
        assert_eq!(result, Some(0.15));

        // Between thresholds → None (needs LLM)
        let result = grade_semantic(0.60, accept, fail);
        assert!(result.is_none());

        // Just above fail threshold → None
        let result = grade_semantic(0.46, accept, fail);
        assert!(result.is_none());

        // Just below accept threshold → None
        let result = grade_semantic(0.77, accept, fail);
        assert!(result.is_none());
    }

    #[test]
    fn test_build_grading_prompt() {
        let (system, user) = build_grading_prompt(
            "What is Rust?",
            "A systems programming language",
            "A fast language",
            None,
        );
        assert!(system.contains("flashcard grading"));
        assert!(user.contains("What is Rust?"));
        assert!(user.contains("A systems programming language"));
        assert!(user.contains("A fast language"));
        assert!(!user.contains("Source context"));

        // With source context
        let (_, user_with_ctx) =
            build_grading_prompt("Q", "A", "my answer", Some("from chapter 3"));
        assert!(user_with_ctx.contains("Source context"));
        assert!(user_with_ctx.contains("from chapter 3"));
    }

    #[test]
    fn test_build_diff_highlights() {
        let present = vec!["ownership".to_string(), "borrowing".to_string()];
        let missing = vec!["lifetimes".to_string()];

        let highlights = build_diff_highlights(&present, &missing);
        assert_eq!(highlights.len(), 3);
        assert_eq!(highlights[0].text, "ownership");
        assert_eq!(highlights[0].status, "match");
        assert_eq!(highlights[1].text, "borrowing");
        assert_eq!(highlights[1].status, "match");
        assert_eq!(highlights[2].text, "lifetimes");
        assert_eq!(highlights[2].status, "missing");
    }
}
