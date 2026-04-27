use cognitive::repos::SemanticFactRepo;
use cognitive::types::SemanticFact;
use cognitive::CardType;
use desktop_shared::commands::{
    PracticeCompleteParams, PracticeCompleteResponse, PracticeConfirmParams,
    PracticeConfirmResponse, PracticeEvalResponse, PracticeGetParams, PracticeListParams,
    PracticeSegmentParams, PracticeSegmentResponse, PracticeSessionResponse, PracticeStartParams,
    PracticeSubmitParams,
};
use desktop_shared::errors::ApiError;
use feature_notes::repo::PracticeSessionRow;
use feature_notes::NoteEvent;

use super::practice_prompts;
use crate::errors::map_cognitive_err;
use crate::state::AppCore;

/// Convert a `PracticeSessionRow` into the IPC response type.
fn row_to_response(row: PracticeSessionRow) -> PracticeSessionResponse {
    PracticeSessionResponse {
        id: row.id,
        note_id: row.note_id,
        source_lang: row.source_lang,
        target_lang: row.target_lang,
        status: row.status,
        segments: row.segments,
        current_index: row.current_index as u32,
        results: row.results,
        user_translation_doc: row.user_translation_doc,
        average_score: row.average_score,
        started_at: row.started_at,
        completed_at: row.completed_at,
    }
}

/// Map a letter grade to a numeric score for averaging.
fn grade_to_score(grade: &str) -> f64 {
    match grade {
        "A+" => 4.3,
        "A" => 4.0,
        "A-" => 3.7,
        "B+" => 3.3,
        "B" => 3.0,
        "B-" => 2.7,
        "C+" => 2.3,
        "C" => 2.0,
        "C-" => 1.7,
        "D+" => 1.3,
        "D" => 1.0,
        "F" => 0.0,
        _ => 2.0, // default to C if unrecognised
    }
}

/// Return true if the grade is A+ or A (i.e. "strong" — no flashcard needed).
fn is_strong_grade(grade: &str) -> bool {
    matches!(grade, "A+" | "A")
}

/// Map a grade to FSRS initial stability for spaced-repetition flashcards.
fn grade_to_stability(grade: &str) -> f64 {
    match grade {
        "A-" => 2.0,
        "B+" => 1.5,
        "B" => 1.0,
        "B-" => 0.8,
        "C+" => 0.6,
        "C" => 0.5,
        "C-" => 0.4,
        "D+" => 0.35,
        "D" => 0.3,
        "F" => 0.1,
        _ => 0.5,
    }
}

/// Split note text into practice segments using line boundaries.
///
/// Each non-empty line becomes one segment. Lines without terminal sentence
/// punctuation and shorter than 80 chars are classified as headings; everything
/// else is a sentence. No LLM call — deterministic and instant.
fn split_into_segments(text: &str) -> Vec<desktop_shared::commands::PracticeSegment> {
    let mut segments = Vec::new();
    let mut index = 0u32;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let is_heading = trimmed.len() < 80
            && !trimmed.ends_with('.')
            && !trimmed.ends_with('!')
            && !trimmed.ends_with('?')
            && !trimmed.ends_with('\u{3002}') // 。
            && !trimmed.ends_with('\u{ff01}') // ！
            && !trimmed.ends_with('\u{ff1f}'); // ？

        segments.push(desktop_shared::commands::PracticeSegment {
            index,
            text: trimmed.to_string(),
            segment_type: if is_heading { "heading" } else { "sentence" }.to_string(),
            suggested_focus: "vocabulary".to_string(),
            skipped: false,
        });
        index += 1;
    }

    segments
}

impl AppCore {
    /// Segment a note into practice-sized translation units.
    ///
    /// Splits deterministically by line boundaries — no LLM call needed.
    #[tracing::instrument(skip(self), err)]
    pub async fn practice_segment_note(
        &self,
        params: PracticeSegmentParams,
    ) -> Result<PracticeSegmentResponse, ApiError> {
        let note = self
            .note_repo
            .get_note(&params.note_id)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let segments = split_into_segments(&note.body);
        let estimated_mins = (segments.len() as f64 * 1.2).ceil() as u32;

        Ok(PracticeSegmentResponse {
            segments,
            estimated_mins,
            cached_at: None,
        })
    }

    /// Create a new practice session.
    #[tracing::instrument(skip(self), err)]
    pub async fn practice_start_session(
        &self,
        params: PracticeStartParams,
    ) -> Result<PracticeSessionResponse, ApiError> {
        let now = jiff::Timestamp::now().to_string();
        let id = uuid::Uuid::new_v4().to_string();
        let segments_json =
            serde_json::to_string(&params.segments).unwrap_or_else(|_| "[]".to_string());
        let start_index = params.start_index.unwrap_or(0) as i64;

        let row = PracticeSessionRow {
            id,
            note_id: params.note_id,
            source_lang: params.source_lang,
            target_lang: params.target_lang,
            status: "in_progress".to_string(),
            segments: segments_json,
            current_index: start_index,
            results: "[]".to_string(),
            user_translation_doc: None,
            average_score: None,
            started_at: now.clone(),
            completed_at: None,
            updated_at: now.clone(),
            created_at: now,
        };

        let created = self
            .practice_repo
            .create(&row)
            .await
            .map_err(map_cognitive_err)?;

        Ok(row_to_response(created))
    }

    /// Evaluate a user's translation of a practice unit via LLM.
    #[tracing::instrument(skip(self), err)]
    pub async fn practice_submit_unit(
        &self,
        params: PracticeSubmitParams,
    ) -> Result<PracticeEvalResponse, ApiError> {
        let session = self
            .practice_repo
            .get_by_id(&params.session_id)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Practice session not found"))?;

        // Parse segments to get the source text for this unit
        let segments: Vec<desktop_shared::commands::PracticeSegment> =
            serde_json::from_str(&session.segments).map_err(|e| {
                ApiError::new("PARSE_ERROR", format!("Failed to parse segments: {e}"))
            })?;

        let segment = segments.get(params.index as usize).ok_or_else(|| {
            ApiError::new(
                "VALIDATION",
                format!("Unit index {} out of range", params.index),
            )
        })?;

        // Fetch note body for document-level context
        let note = self
            .note_repo
            .get_note(&session.note_id)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        // Parse existing results for previous context
        let results: Vec<serde_json::Value> =
            serde_json::from_str(&session.results).unwrap_or_default();

        // Build the user message with context
        let mut user_prompt = format!(
            "Source text ({}):\n{}\n\nUser's translation ({}):\n{}",
            session.source_lang, segment.text, session.target_lang, params.user_translation
        );

        // Add full document context
        user_prompt.push_str(&format!("\n\n--- Full document context ---\n{}", note.body));

        // Add previous unit result summary if available
        if let Some(prev) = results.last() {
            if let Some(grade) = prev.get("overall_grade").and_then(|g| g.as_str()) {
                user_prompt.push_str(&format!("\n\n--- Previous unit result ---\nGrade: {grade}"));
            }
        }

        // Call LLM
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let system =
            practice_prompts::evaluation_prompt(&session.source_lang, &session.target_lang);
        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(user_prompt),
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
        let mut eval: PracticeEvalResponse = serde_json::from_str(cleaned).map_err(|e| {
            ApiError::new(
                "PARSE_ERROR",
                format!("Failed to parse evaluation response: {e}"),
            )
        })?;

        // Check for coaching nudge based on recent results
        let coaching_nudge = practice_prompts::coaching_nudge_check(&results);
        eval.coaching_nudge = coaching_nudge;

        Ok(eval)
    }

    /// Confirm a completed unit, record the result, and advance the session.
    #[tracing::instrument(skip(self), err)]
    pub async fn practice_confirm_unit(
        &self,
        params: PracticeConfirmParams,
    ) -> Result<PracticeConfirmResponse, ApiError> {
        let session = self
            .practice_repo
            .get_by_id(&params.session_id)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Practice session not found"))?;

        // Parse current results
        let mut results: Vec<serde_json::Value> =
            serde_json::from_str(&session.results).unwrap_or_default();

        // Parse segments to get count and current unit text
        let segments: Vec<desktop_shared::commands::PracticeSegment> =
            serde_json::from_str(&session.segments).unwrap_or_default();

        // Get the source text for this unit (for KnowledgeAtom creation)
        let source_text = segments
            .get(params.index as usize)
            .map(|s| s.text.clone())
            .unwrap_or_default();

        // Build the result entry and append
        let result_entry = serde_json::json!({
            "index": params.index,
            "final_translation": params.final_translation,
            "confidence_rating": params.confidence_rating,
            "edited": params.edited,
        });
        results.push(result_entry);

        let next_index = params.index + 1;
        let is_complete = next_index as usize >= segments.len();

        let results_json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());

        // Save updated session
        let _ = self
            .practice_repo
            .update_progress(&params.session_id, next_index as i64, &results_json, None)
            .await
            .map_err(map_cognitive_err)?;

        // Create KnowledgeAtom + SemanticFact only for weak units (grade ≤ A-)
        let is_weak = !is_strong_grade(&params.overall_grade);
        if is_weak {
            if let Some(atom_repo) = &self.knowledge_atom_repo {
                let domain = format!("language:{}-{}", session.source_lang, session.target_lang);
                let topic = atom_repo
                    .get_or_create_topic(
                        &format!("{}-{}", session.source_lang, session.target_lang),
                        &domain,
                    )
                    .await
                    .ok();

                let atom = cognitive::NewKnowledgeAtom {
                    subject: source_text.clone(),
                    atom_type: "translation_unit".to_string(),
                    domain: domain.clone(),
                    source_note_id: Some(session.note_id.clone()),
                    source_context: Some(params.final_translation.clone()),
                    personal_importance: 0.6,
                    status: "active".to_string(),
                    topic_id: topic.as_ref().map(|t| t.id.clone()),
                    ..Default::default()
                };

                if let Ok(created_atom) = atom_repo.create_batch(vec![atom]).await {
                    // Create SemanticFact: source_text translates_to final_translation
                    let sf_repo = SemanticFactRepo::new(self.storage_pool.inner().clone());
                    let now = jiff::Timestamp::now().to_string();
                    let fact = SemanticFact {
                        id: uuid::Uuid::new_v4().to_string(),
                        domain: domain.clone(),
                        subject: source_text,
                        predicate: "translates_to".to_string(),
                        object: params.final_translation.clone(),
                        confidence: 1.0,
                        source: format!("practice:{}", session.id),
                        valid_from: now.clone(),
                        valid_until: None,
                        recorded_at: now,
                        superseded_at: None,
                        superseded_by: None,
                        stability: 1.0,
                        last_accessed: None,
                        access_count: 0,
                        convergence_score: 0.0,
                        project_id: None,
                        memory_type: "translation_unit".to_string(),
                        scope_type: "system".to_string(),
                        scope_id: None,
                        metadata: None,
                        scope_repo_id: None,
                    };
                    let _ = sf_repo.upsert(&fact).await;

                    // Emit KnowledgeAtomCreated events
                    if let Some(bus) = &self.domain_event_bus {
                        for atom in &created_atom {
                            bus.publish(bus::DomainEvent::KnowledgeAtomCreated {
                                atom_id: atom.id.clone(),
                                atom_type: atom.atom_type.clone(),
                                domain: atom.domain.clone(),
                                source_note_id: atom.source_note_id.clone(),
                                personal_importance: atom.personal_importance,
                            });
                        }
                    }

                    // Update topic aggregates
                    if let Some(topic) = &topic {
                        let _ = atom_repo.update_topic_aggregates(&topic.id).await;
                    }
                }
            }
        } // end if is_weak

        // Emit PracticeUnitCompleted event
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(
                NoteEvent::PracticeUnitCompleted {
                    session_id: session.id.clone(),
                    note_id: session.note_id.clone(),
                    unit_index: params.index,
                    grade: params.overall_grade.clone(),
                    scores: params.scores_json.clone().unwrap_or_default(),
                    confidence_rating: params.confidence_rating,
                    edited: params.edited,
                }
                .into(),
            );
        }

        Ok(PracticeConfirmResponse {
            next_index,
            is_complete,
        })
    }

    /// Retrieve a practice session by ID or find the active one for a note.
    #[tracing::instrument(skip(self), err)]
    pub async fn practice_get_session(
        &self,
        params: PracticeGetParams,
    ) -> Result<Option<PracticeSessionResponse>, ApiError> {
        // Look up by session id first, then by note_id
        // Note: mark_abandoned_stale() is called lazily in list_sessions, not on every get
        if let Some(ref session_id) = params.session_id {
            let row = self
                .practice_repo
                .get_by_id(session_id)
                .await
                .map_err(map_cognitive_err)?;
            return Ok(row.map(row_to_response));
        }

        if let Some(ref note_id) = params.note_id {
            let row = self
                .practice_repo
                .get_active_for_note(note_id)
                .await
                .map_err(map_cognitive_err)?;
            return Ok(row.map(row_to_response));
        }

        Err(ApiError::new(
            "VALIDATION",
            "Either session_id or note_id must be provided",
        ))
    }

    /// Complete a practice session — compute scores, optionally create flashcards.
    #[tracing::instrument(skip(self), err)]
    pub async fn practice_complete_session(
        &self,
        params: PracticeCompleteParams,
    ) -> Result<PracticeCompleteResponse, ApiError> {
        let session = self
            .practice_repo
            .get_by_id(&params.session_id)
            .await
            .map_err(map_cognitive_err)?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Practice session not found"))?;

        let results: Vec<serde_json::Value> =
            serde_json::from_str(&session.results).unwrap_or_default();

        // Compute average score from overall_grade fields in results
        let grades: Vec<f64> = results
            .iter()
            .filter_map(|r| {
                r.get("overall_grade")
                    .and_then(|g| g.as_str())
                    .map(grade_to_score)
            })
            .collect();

        let average_score = if grades.is_empty() {
            0.0
        } else {
            grades.iter().sum::<f64>() / grades.len() as f64
        };

        // Build user_translation_doc by concatenating all final_translation fields
        let user_translation_doc: String = results
            .iter()
            .filter_map(|r| r.get("final_translation").and_then(|t| t.as_str()))
            .collect::<Vec<&str>>()
            .join("\n\n");

        // Identify weak units (grade <= A-, i.e. not A+ or A)
        let weak_units: Vec<&serde_json::Value> = results
            .iter()
            .filter(|r| {
                !r.get("overall_grade")
                    .and_then(|g| g.as_str())
                    .is_some_and(is_strong_grade)
            })
            .collect();
        let weak_unit_count = weak_units.len() as u32;

        // Update results with the user_translation_doc included
        let results_json = serde_json::to_string(&results).unwrap_or_else(|_| "[]".to_string());

        // Save user_translation_doc first, then mark as completed (single logical update)
        let _ = self
            .practice_repo
            .update_progress(
                &params.session_id,
                session.current_index,
                &results_json,
                Some(&user_translation_doc),
            )
            .await;
        let _ = self
            .practice_repo
            .complete(&params.session_id, average_score, &results_json)
            .await
            .map_err(map_cognitive_err)?;

        // Emit PracticeSessionCompleted event
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(
                NoteEvent::PracticeSessionCompleted {
                    session_id: session.id.clone(),
                    note_id: session.note_id.clone(),
                    units_completed: results.len() as u32,
                    average_score,
                    source_lang: session.source_lang.clone(),
                    target_lang: session.target_lang.clone(),
                    weak_unit_count,
                }
                .into(),
            );
        }

        // If save_to_sr: create flashcards for weak units (independent of bus)
        let mut flashcards_created = 0u32;
        if params.save_to_sr {
            if let Some(flashcard_repo) = &self.flashcard_repo {
                let segments_vec: Vec<desktop_shared::commands::PracticeSegment> =
                    serde_json::from_str(&session.segments).unwrap_or_default();

                let mut new_cards = Vec::new();
                for result in &results {
                    let grade = result
                        .get("overall_grade")
                        .and_then(|g| g.as_str())
                        .unwrap_or("C");

                    if is_strong_grade(grade) {
                        continue; // Skip strong units
                    }

                    let idx = result.get("index").and_then(|i| i.as_u64()).unwrap_or(0) as usize;
                    let source_text = segments_vec
                        .get(idx)
                        .map(|s| s.text.clone())
                        .unwrap_or_default();
                    let model_translation = result
                        .get("model_translation")
                        .and_then(|t| t.as_str())
                        .unwrap_or_default();

                    let stability = grade_to_stability(grade);

                    new_cards.push(cognitive::NewFlashcard {
                        source_note_id: Some(session.note_id.clone()),
                        source_context: Some(source_text.clone()),
                        atom_id: None,
                        deck: format!("{}-{}", session.source_lang, session.target_lang),
                        front: source_text,
                        back: model_translation.to_string(),
                        card_type: CardType::Vocabulary,
                        cloze_data: None,
                        vocab_data: Some(serde_json::json!({
                            "type": "translation_unit",
                            "grade": grade,
                            "session_id": session.id,
                        })),
                        image_data: None,
                        tags: vec!["practice".to_string(), "translation".to_string()],
                        stability,
                        difficulty: 0.3,
                        difficulty_estimate: None,
                        prerequisite_concepts: None,
                    });
                }

                if !new_cards.is_empty() {
                    match flashcard_repo.create_batch(new_cards).await {
                        Ok(created) => flashcards_created = created.len() as u32,
                        Err(e) => {
                            tracing::warn!("Failed to create practice flashcards: {e}");
                        }
                    }
                }
            }
        }

        Ok(PracticeCompleteResponse {
            average_score,
            weak_unit_count,
            flashcards_created,
        })
    }

    /// List all practice sessions for a note.
    #[tracing::instrument(skip(self), err)]
    pub async fn practice_list_sessions(
        &self,
        params: PracticeListParams,
    ) -> Result<Vec<PracticeSessionResponse>, ApiError> {
        // Lazily clean up stale sessions when listing (not on every get)
        let _ = self.practice_repo.mark_abandoned_stale().await;

        let rows = self
            .practice_repo
            .list_for_note(&params.note_id)
            .await
            .map_err(map_cognitive_err)?;

        Ok(rows.into_iter().map(row_to_response).collect())
    }
}
