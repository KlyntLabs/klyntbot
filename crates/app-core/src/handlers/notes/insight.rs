use std::sync::Arc;

use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;
use futures_util::StreamExt;
use sha2::{Digest, Sha256};

use crate::events::AppEventEmitter;
use crate::state::AppCore;

use super::{insight_context, insight_prompts};

impl AppCore {
    pub async fn note_insight_review(
        &self,
        note_id: &str,
    ) -> Result<InsightReviewStarted, ApiError> {
        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        if note.body.trim().is_empty() {
            return Err(ApiError::new("VALIDATION", "Note has no content"));
        }

        let related_ids = self.get_related_note_ids(note_id).await;
        let hash_input = format!("{}{}{}", note.title, note.body, related_ids.join(","));
        let content_hash = format!("{:x}", Sha256::digest(hash_input.as_bytes()));

        if let Some(ref repo) = self.insight_cache_repo {
            if let Ok(Some(_cached)) = repo.get_if_fresh(note_id, &content_hash).await {
                return Ok(InsightReviewStarted {
                    insight_review_id: format!(
                        "ir-{}",
                        uuid::Uuid::new_v4()
                            .to_string()
                            .split('-')
                            .next()
                            .unwrap_or("0000")
                    ),
                    content_hash,
                    cached: true,
                });
            }
        }

        let insight_review_id = format!(
            "ir-{}",
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("0000")
        );

        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?
            .clone();

        let related_notes = self.fetch_related_notes(note_id).await;
        let ctx = insight_context::assemble_context(&note, &related_notes, None);

        // NoteRow doesn't carry tags — fetch them separately
        let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
        let note_domains = insight_context::extract_note_domains(&tags);

        let selected_personas = if let Some(ref persona_repo) = self.persona_repo {
            persona_repo
                .select_for_note(note_id, &note_domains, 4)
                .await
                .unwrap_or_default()
        } else {
            Vec::new()
        };

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        let emitter = Arc::clone(&self.event_emitter);
        let cache_repo = self.insight_cache_repo.clone();
        let note_id_owned = note_id.to_string();
        let content_hash_clone = content_hash.clone();
        let context_text = ctx.text;
        let note_title = ctx.note_title;

        tokio::spawn(async move {
            run_insight_pipeline(InsightPipelineArgs {
                provider,
                emitter,
                cache_repo,
                note_id: note_id_owned,
                content_hash: content_hash_clone,
                context: context_text,
                note_title,
                params,
                personas: selected_personas,
            })
            .await;
        });

        Ok(InsightReviewStarted {
            insight_review_id,
            content_hash,
            cached: false,
        })
    }

    pub async fn note_insight_cache_get(
        &self,
        note_id: &str,
    ) -> Result<Option<InsightReviewResponse>, ApiError> {
        let repo = match &self.insight_cache_repo {
            Some(r) => r,
            None => return Ok(None),
        };

        let cached = match repo
            .get(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        {
            Some(c) => c,
            None => return Ok(None),
        };

        let self_assessment: Option<Vec<QuizQuestion>> = cached
            .self_assessment
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        let persona_ids: Option<Vec<String>> = cached
            .persona_ids
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok());

        Ok(Some(InsightReviewResponse {
            insight_review_id: cached.id,
            note_id: cached.note_id,
            synthesis: cached.synthesis,
            gap_analysis: cached.gap_analysis,
            self_assessment,
            concept_map: cached.concept_map,
            perspectives: cached.perspectives,
            persona_ids,
        }))
    }

    /// Save quiz questions as flashcards with FSRS init.
    pub async fn insight_save_flashcards(
        &self,
        params: InsightSaveFlashcardsParams,
    ) -> Result<Vec<FlashcardResponse>, ApiError> {
        let repo = self
            .flashcard_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Flashcard repo not available"))?;

        let cards: Vec<cognitive::NewFlashcard> = params
            .questions
            .iter()
            .map(|q| {
                let (stability, difficulty) = match q.difficulty.as_str() {
                    "easy" => (4.0, 0.3),
                    "hard" => (0.8, 0.7),
                    _ => (2.0, 0.5),
                };
                cognitive::NewFlashcard {
                    source_note_id: Some(params.note_id.clone()),
                    insight_review_id: Some(params.insight_review_id.clone()),
                    deck: params.deck_name.clone(),
                    question: q.question.clone(),
                    answer: q.correct_answer.clone(),
                    card_type: if q.question_type == "multiple_choice" {
                        cognitive::CardType::MultipleChoice
                    } else {
                        cognitive::CardType::ShortAnswer
                    },
                    choices: q.choices.as_ref().map(|c| serde_json::json!(c)),
                    stability,
                    difficulty,
                }
            })
            .collect();

        let rows = repo
            .create_batch(cards)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|r| FlashcardResponse {
                id: r.id,
                deck: r.deck,
                question: r.question,
                answer: r.answer,
                card_type: r.card_type,
                choices: r
                    .choices
                    .as_deref()
                    .and_then(|s| serde_json::from_str(s).ok()),
                stability: r.stability,
                difficulty: r.difficulty,
                due_at: r.due_at,
                state: r.state,
                review_count: r.review_count,
                created_at: r.created_at,
            })
            .collect())
    }

    pub async fn note_insight_regenerate_tab(
        &self,
        note_id: &str,
        tab: &str,
    ) -> Result<TabContent, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let related_notes = self.fetch_related_notes(note_id).await;
        let ctx = insight_context::assemble_context(&note, &related_notes, None);

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        let prompt = match tab {
            "synthesis" => insight_prompts::synthesis_prompt(&ctx.text),
            "gaps" => insight_prompts::gap_analysis_prompt(&ctx.text),
            "assessment" => insight_prompts::self_assessment_prompt(&ctx.text),
            "concept-map" => insight_prompts::concept_map_prompt(&ctx.text, &ctx.note_title),
            "perspectives" => {
                let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
                let note_domains = insight_context::extract_note_domains(&tags);
                let personas = if let Some(ref persona_repo) = self.persona_repo {
                    persona_repo
                        .select_for_note(note_id, &note_domains, 4)
                        .await
                        .unwrap_or_default()
                } else {
                    Vec::new()
                };
                let blocks: Vec<(String, String, String, String, String)> = personas
                    .iter()
                    .map(|p| {
                        (
                            p.name.clone(),
                            p.role.clone(),
                            p.expertise.clone(),
                            p.perspective.clone(),
                            p.tone.clone(),
                        )
                    })
                    .collect();
                let persona_blocks = insight_prompts::format_persona_blocks(&blocks);
                insight_prompts::perspectives_prompt(&ctx.text, &persona_blocks)
            }
            _ => return Err(ApiError::new("VALIDATION", "Invalid tab name")),
        };

        let messages = vec![
            providers::Message::System { content: prompt },
            providers::Message::User {
                content: providers::UserContent::Text("Generate the analysis now.".to_string()),
            },
        ];

        let response = provider
            .chat(&messages, None, &params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let content = response.content.unwrap_or_default();

        if let Some(ref repo) = self.insight_cache_repo {
            let related_ids = self.get_related_note_ids(note_id).await;
            let hash_input = format!("{}{}{}", note.title, note.body, related_ids.join(","));
            let content_hash = format!("{:x}", Sha256::digest(hash_input.as_bytes()));
            let _ = repo.update_tab(note_id, &content_hash, tab, &content).await;
        }

        Ok(TabContent {
            tab: tab.to_string(),
            content,
        })
    }

    async fn get_related_note_ids(&self, note_id: &str) -> Vec<String> {
        let backlinks = self
            .note_repo
            .get_backlinks_with_context(note_id)
            .await
            .unwrap_or_default();
        let mut ids: Vec<String> = backlinks.into_iter().map(|(note, _ctx)| note.id).collect();
        ids.sort();
        ids
    }

    async fn fetch_related_notes(&self, note_id: &str) -> Vec<feature_notes::models::NoteRow> {
        let backlinks = self
            .note_repo
            .get_backlinks_with_context(note_id)
            .await
            .unwrap_or_default();
        let mut notes = Vec::new();
        for (backlink_note, _ctx) in &backlinks {
            if let Ok(Some(full_note)) = self.note_repo.get_note(&backlink_note.id).await {
                notes.push(full_note);
            }
        }
        notes
    }
}

struct InsightPipelineArgs {
    provider: providers::DynProvider,
    emitter: Arc<dyn AppEventEmitter>,
    cache_repo: Option<cognitive::InsightCacheRepo>,
    note_id: String,
    content_hash: String,
    context: String,
    note_title: String,
    params: providers::ChatParams,
    personas: Vec<cognitive::PersonaRow>,
}

async fn run_insight_pipeline(args: InsightPipelineArgs) {
    let InsightPipelineArgs {
        provider,
        emitter,
        cache_repo,
        note_id,
        content_hash,
        context,
        note_title,
        params,
        personas,
    } = args;

    let synthesis = stream_synthesis(&provider, &emitter, &context, &params).await;

    let gaps_prompt = insight_prompts::gap_analysis_prompt(&context);
    let assessment_prompt = insight_prompts::self_assessment_prompt(&context);
    let concept_map_prompt = insight_prompts::concept_map_prompt(&context, &note_title);

    let persona_blocks: Vec<(String, String, String, String, String)> = personas
        .iter()
        .map(|p| {
            (
                p.name.clone(),
                p.role.clone(),
                p.expertise.clone(),
                p.perspective.clone(),
                p.tone.clone(),
            )
        })
        .collect();
    let formatted_blocks = insight_prompts::format_persona_blocks(&persona_blocks);
    let perspectives_prompt = insight_prompts::perspectives_prompt(&context, &formatted_blocks);

    let personas_meta: Vec<serde_json::Value> = personas
        .iter()
        .map(|p| {
            serde_json::json!({
                "id": p.id,
                "name": p.name,
                "role": p.role,
                "icon": p.icon,
                "tone": p.tone,
            })
        })
        .collect();
    emitter.emit_event(
        "insight:perspectives-meta",
        serde_json::json!({ "personas": personas_meta }),
    );

    let (gaps, assessment, concept_map, perspectives) = tokio::join!(
        generate_tab(&provider, &emitter, "gaps", &gaps_prompt, &params),
        generate_tab(
            &provider,
            &emitter,
            "assessment",
            &assessment_prompt,
            &params,
        ),
        generate_tab(
            &provider,
            &emitter,
            "concept-map",
            &concept_map_prompt,
            &params,
        ),
        generate_tab(
            &provider,
            &emitter,
            "perspectives",
            &perspectives_prompt,
            &params,
        ),
    );

    if let Some(ref repo) = cache_repo {
        let persona_ids_json = serde_json::to_string(
            &personas.iter().map(|p| &p.id).collect::<Vec<_>>(),
        )
        .ok();
        let _ = repo
            .upsert(
                &note_id,
                &content_hash,
                synthesis.as_deref(),
                gaps.as_deref(),
                assessment.as_deref(),
                concept_map.as_deref(),
                perspectives.as_deref(),
                persona_ids_json.as_deref(),
            )
            .await;
    }
}

async fn stream_synthesis(
    provider: &providers::DynProvider,
    emitter: &Arc<dyn AppEventEmitter>,
    context: &str,
    params: &providers::ChatParams,
) -> Option<String> {
    let messages = vec![
        providers::Message::System {
            content: insight_prompts::synthesis_prompt(context),
        },
        providers::Message::User {
            content: providers::UserContent::Text("Generate the synthesis now.".to_string()),
        },
    ];

    match provider.chat_stream(&messages, None, params).await {
        Ok(mut stream) => {
            let mut full_content = String::new();
            while let Some(chunk_result) = StreamExt::next(&mut stream).await {
                match chunk_result {
                    Ok(chunk) => {
                        if let Some(text) = &chunk.content {
                            full_content.push_str(text);
                            emitter.emit_event(
                                "insight:synthesis-chunk",
                                serde_json::json!({ "content": text }),
                            );
                        }
                    }
                    Err(e) => {
                        emitter.emit_event(
                            "insight:error",
                            serde_json::json!({ "tab": "synthesis", "error": e.to_string() }),
                        );
                        return None;
                    }
                }
            }
            emitter.emit_event("insight:synthesis-done", serde_json::json!({}));
            Some(full_content)
        }
        Err(e) => {
            emitter.emit_event(
                "insight:error",
                serde_json::json!({ "tab": "synthesis", "error": e.to_string() }),
            );
            None
        }
    }
}

async fn generate_tab(
    provider: &providers::DynProvider,
    emitter: &Arc<dyn AppEventEmitter>,
    tab_name: &str,
    prompt: &str,
    params: &providers::ChatParams,
) -> Option<String> {
    let messages = vec![
        providers::Message::System {
            content: prompt.to_string(),
        },
        providers::Message::User {
            content: providers::UserContent::Text("Generate the analysis now.".to_string()),
        },
    ];

    match provider.chat(&messages, None, params).await {
        Ok(response) => {
            let content = response.content.unwrap_or_default();
            emitter.emit_event(
                "insight:tab-done",
                serde_json::json!({ "tab": tab_name, "content": content }),
            );
            Some(content)
        }
        Err(e) => {
            emitter.emit_event(
                "insight:error",
                serde_json::json!({ "tab": tab_name, "error": e.to_string() }),
            );
            None
        }
    }
}
