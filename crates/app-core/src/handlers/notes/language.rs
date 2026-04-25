use cognitive::repos::SemanticFactRepo;
use cognitive::types::SemanticFact;
use cognitive::CardType;
use desktop_shared::commands::{
    AnnotationEnrichmentResponse, ConfusableResponse, DetectConfusablesParams,
    EnrichAnnotationParams, EvaluateTranslationParams, QuickTranslateParams,
    QuickTranslateResponse, TranslateBreakdownParams, TranslateBreakdownResponse,
    TranslationEvalResponse, VocabularySaveParams,
};
use desktop_shared::errors::ApiError;

use super::language_prompts;
use feature_notes::NoteEvent;

use crate::errors::map_cognitive_err;
use crate::state::AppCore;

impl AppCore {
    /// Translate text and return sentence breakdown with word-by-word analysis.
    pub async fn language_translate_breakdown(
        &self,
        params: TranslateBreakdownParams,
    ) -> Result<TranslateBreakdownResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        let system =
            language_prompts::translate_breakdown_prompt(&params.source_lang, &params.target_lang);
        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(params.text.clone()),
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
        let mut result: TranslateBreakdownResponse =
            serde_json::from_str(cleaned).map_err(|e| {
                ApiError::new("PARSE_ERROR", format!("Failed to parse LLM response: {e}"))
            })?;

        // Mark words as new/known by checking SemanticFact store
        let sf_repo = SemanticFactRepo::new(self.storage_pool.inner().clone());
        for word in &mut result.words {
            let existing = sf_repo
                .find_vocabulary_by_subject(&word.word)
                .await
                .unwrap_or_default();
            word.is_new = existing.is_empty();
        }

        // Emit TranslationCompleted event
        if let Some(note_id) = &params.note_id {
            if let Some(bus) = &self.domain_event_bus {
                bus.publish(
                    NoteEvent::TranslationCompleted {
                        note_id: note_id.clone(),
                        source_lang: params.source_lang.clone(),
                        target_lang: params.target_lang.clone(),
                        word_count: result.words.len(),
                        is_selection: params.is_selection,
                    }
                    .into(),
                );
            }
        }

        Ok(result)
    }

    /// Evaluate a user's translation attempt across 4 dimensions.
    pub async fn language_evaluate_translation(
        &self,
        params: EvaluateTranslationParams,
    ) -> Result<TranslationEvalResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let system =
            language_prompts::evaluate_translation_prompt(&params.source_lang, &params.target_lang);
        let user_prompt = format!(
            "Source text ({}):\n{}\n\nStudent's translation ({}):\n{}",
            params.source_lang, params.source_text, params.target_lang, params.user_translation
        );
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
        serde_json::from_str(cleaned)
            .map_err(|e| ApiError::new("PARSE_ERROR", format!("Failed to parse: {e}")))
    }

    /// Save vocabulary words as flashcards + semantic facts.
    pub async fn language_save_vocabulary(
        &self,
        params: VocabularySaveParams,
    ) -> Result<Vec<desktop_shared::commands::FlashcardResponse>, ApiError> {
        let flashcard_repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;
        let sf_repo = SemanticFactRepo::new(self.storage_pool.inner().clone());

        let now = jiff::Timestamp::now().to_string();

        // Dedup: skip words that already have flashcards in this deck
        let fronts: Vec<String> = params.words.iter().map(|w| w.word.clone()).collect();
        let existing_fronts = flashcard_repo
            .find_existing_fronts(&params.deck, &fronts)
            .await
            .unwrap_or_default();

        let mut new_cards = Vec::new();

        for item in &params.words {
            if existing_fronts.contains(&item.word) {
                continue; // Skip duplicate
            }
            let vocab_data = serde_json::json!({
                "word": item.word,
                "reading": item.reading,
                "meaning": item.meaning,
                "example_sentence": item.example_sentence,
                "part_of_speech": item.part_of_speech,
            });

            new_cards.push(cognitive::NewFlashcard {
                source_note_id: params.note_id.clone(),
                source_context: item.example_sentence.clone(),
                atom_id: None,
                deck: params.deck.clone(),
                front: item.word.clone(),
                back: item.meaning.clone(),
                card_type: CardType::Vocabulary,
                cloze_data: None,
                vocab_data: Some(vocab_data),
                image_data: None,
                tags: vec!["vocabulary".to_string(), "language-learning".to_string()],
                stability: 1.0,
                difficulty: 0.3,
                difficulty_estimate: None,
                prerequisite_concepts: None,
            });

            // Also save as SemanticFact for CJK-safe vocabulary lookup
            let fact_id = uuid::Uuid::new_v4().to_string();
            let fact = SemanticFact {
                id: fact_id,
                domain: "learning".to_string(),
                subject: item.word.clone(),
                predicate: "meaning".to_string(),
                object: item.meaning.clone(),
                confidence: 1.0,
                source: format!("note:{}", params.note_id.as_deref().unwrap_or("manual")),
                valid_from: now.clone(),
                valid_until: None,
                recorded_at: now.clone(),
                superseded_at: None,
                superseded_by: None,
                stability: 1.0,
                last_accessed: None,
                access_count: 0,
                convergence_score: 0.0,
                project_id: None,
                memory_type: "vocabulary".to_string(),
                scope_type: "system".to_string(),
                scope_id: None,
                metadata: None,
                scope_repo_id: None,
            };
            sf_repo.upsert(&fact).await.map_err(map_cognitive_err)?;
        }

        let created = flashcard_repo
            .create_batch(new_cards)
            .await
            .map_err(map_cognitive_err)?;

        // Create Knowledge Atoms alongside flashcards
        if let Some(atom_repo) = &self.knowledge_atom_repo {
            let target_lang = params.deck.clone();
            let domain = format!("language:{target_lang}");
            let topic = atom_repo
                .get_or_create_topic(&target_lang, &domain)
                .await
                .ok();

            // Dedup: skip words that already have atoms in this domain
            let existing_atom_subjects = atom_repo
                .find_existing_subjects(&domain, &fronts)
                .await
                .unwrap_or_default();

            let mut new_atoms = Vec::new();
            for item in &params.words {
                if existing_atom_subjects.contains(&item.word) {
                    continue; // Skip duplicate atom
                }
                let vocab_metadata = serde_json::json!({
                    "word": item.word,
                    "reading": item.reading,
                    "meaning": item.meaning,
                    "partOfSpeech": item.part_of_speech,
                    "exampleSentence": item.example_sentence,
                });

                new_atoms.push(cognitive::NewKnowledgeAtom {
                    subject: item.word.clone(),
                    atom_type: "vocabulary".to_string(),
                    domain: domain.clone(),
                    source_note_id: params.note_id.clone(),
                    source_context: item.example_sentence.clone(),
                    personal_importance: 0.7,
                    status: "active".to_string(),
                    metadata: Some(serde_json::to_string(&vocab_metadata).unwrap_or_default()),
                    topic_id: topic.as_ref().map(|t| t.id.clone()),
                    ..Default::default()
                });
            }

            if let Ok(created_atoms) = atom_repo.create_batch(new_atoms).await {
                // Link flashcards to atoms by matching word/front
                for card in &created {
                    if let Some(atom) = created_atoms.iter().find(|a| a.subject == card.front) {
                        let _ = sqlx::query("UPDATE flashcards SET atom_id = ?1 WHERE id = ?2")
                            .bind(&atom.id)
                            .bind(&card.id)
                            .execute(flashcard_repo.pool())
                            .await;
                    }
                }

                // Emit events (atoms are created as "active" — no separate Accepted event needed)
                if let Some(bus) = &self.domain_event_bus {
                    for atom in &created_atoms {
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

        Ok(created
            .into_iter()
            .map(super::flashcard::flashcard_to_response)
            .collect())
    }

    /// Detect confusable words by checking existing vocabulary.
    pub async fn language_detect_confusables(
        &self,
        params: DetectConfusablesParams,
    ) -> Result<ConfusableResponse, ApiError> {
        let sf_repo = SemanticFactRepo::new(self.storage_pool.inner().clone());
        let similar = sf_repo
            .find_similar_vocabulary(&params.word, 5)
            .await
            .map_err(map_cognitive_err)?;

        if similar.is_empty() {
            return Ok(ConfusableResponse {
                has_confusable: false,
                confusable_word: None,
                confusable_meaning: None,
                explanation: None,
            });
        }

        let confusable = &similar[0];

        // Use LLM to explain the difference
        let provider = match self.cognitive_provider.as_ref() {
            Some(p) => p,
            None => {
                return Ok(ConfusableResponse {
                    has_confusable: true,
                    confusable_word: Some(confusable.subject.clone()),
                    confusable_meaning: Some(confusable.object.clone()),
                    explanation: None,
                });
            }
        };

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 1024);
        drop(config);

        let system = language_prompts::detect_confusables_prompt(&params.source_lang);
        let user_prompt = format!(
            "Word 1: {} (new word)\nWord 2: {} ({})",
            params.word, confusable.subject, confusable.object
        );
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

        let text = response.content.unwrap_or_default();
        let cleaned = common::helpers::strip_llm_fences(&text);

        let explanation: Option<String> = serde_json::from_str::<serde_json::Value>(cleaned)
            .ok()
            .and_then(|v| {
                v.get("explanation")
                    .and_then(|e| e.as_str().map(String::from))
            });

        Ok(ConfusableResponse {
            has_confusable: true,
            confusable_word: Some(confusable.subject.clone()),
            confusable_meaning: Some(confusable.object.clone()),
            explanation,
        })
    }

    /// Enrich an annotation with language data (translation + word breakdown).
    pub async fn language_enrich_annotation(
        &self,
        params: EnrichAnnotationParams,
    ) -> Result<AnnotationEnrichmentResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 2048);
        drop(config);

        let system =
            language_prompts::enrich_annotation_prompt(&params.source_lang, &params.target_lang);
        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(params.quoted_text),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let text = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response"))?;

        let cleaned = common::helpers::strip_llm_fences(&text);
        let mut result: AnnotationEnrichmentResponse = serde_json::from_str(cleaned)
            .map_err(|e| ApiError::new("PARSE_ERROR", format!("Failed to parse: {e}")))?;

        // Mark words as new/known (same as translate_breakdown)
        let sf_repo = SemanticFactRepo::new(self.storage_pool.inner().clone());
        for word in &mut result.words {
            let existing = sf_repo
                .find_vocabulary_by_subject(&word.word)
                .await
                .unwrap_or_default();
            word.is_new = existing.is_empty();
        }

        Ok(result)
    }

    /// Quick-translate a short text selection (translation only, no vocabulary).
    pub async fn language_quick_translate(
        &self,
        params: QuickTranslateParams,
    ) -> Result<QuickTranslateResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 512);
        drop(config);

        let system =
            language_prompts::quick_translate_prompt(&params.source_lang, &params.target_lang);
        let messages = vec![
            providers::Message::System { content: system },
            providers::Message::User {
                content: providers::UserContent::Text(params.text.clone()),
            },
        ];

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let translation = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?
            .trim()
            .to_string();

        Ok(QuickTranslateResponse {
            translation,
            words: vec![],
        })
    }
}
