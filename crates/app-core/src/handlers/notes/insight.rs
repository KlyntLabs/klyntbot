use std::sync::Arc;

use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;
use futures_util::StreamExt;

use crate::events::AppEventEmitter;
use crate::state::AppCore;

use super::{insight_context, insight_prompts};

/// Convert an `InsightReviewRow` to the frontend-facing `InsightReviewResponse`.
fn row_to_insight_response(row: feature_insights::InsightReviewRow) -> InsightReviewResponse {
    let content: feature_insights::InsightContent =
        serde_json::from_str(&row.content).unwrap_or_default();

    let self_assessment: Option<Vec<QuizQuestion>> =
        content.self_assessment.as_deref().and_then(|s| {
            // Try parsing as-is first, then try stripping markdown fences
            serde_json::from_str(s)
                .or_else(|_| serde_json::from_str(&strip_markdown_fences(s)))
                .ok()
        });

    let persona_ids: Option<Vec<String>> = serde_json::from_str(&row.persona_ids).ok();

    InsightReviewResponse {
        insight_review_id: row.id,
        note_id: row.note_id,
        version: row.version,
        generated_at: row.generated_at,
        synthesis: content.synthesis,
        gap_analysis: content.gap_analysis,
        self_assessment,
        concept_map: content.concept_map,
        perspectives: content.perspectives,
        persona_ids,
        personas: vec![], // Resolved by caller (note_insight_cache_get) when async context is available
    }
}

impl AppCore {
    pub async fn note_insight_review(
        &self,
        note_id: &str,
        scope_params: Option<&InsightScopeConfigParams>,
        emitter_override: Option<Arc<dyn crate::events::AppEventEmitter>>,
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

        let scope = match scope_params {
            Some(params) => {
                let mut s = feature_insights::ScopeConfig::default();
                if let Some(ref st) = params.scope_type {
                    s.scope_type = match st.as_str() {
                        "semantic" => feature_insights::ScopeType::Semantic,
                        "project" => feature_insights::ScopeType::Project,
                        "manual" => feature_insights::ScopeType::Manual,
                        _ => feature_insights::ScopeType::Backlinks,
                    };
                }
                if let Some(r) = params.radius {
                    s.radius = r;
                }
                if let Some(ref ids) = params.node_ids {
                    s.node_ids = ids.clone();
                }
                if let Some(c) = params.include_cognitive {
                    s.include_cognitive = c;
                }
                if let Some(d) = params.deep_dive {
                    s.deep_dive = d;
                }
                if let Some(m) = params.merge_threshold {
                    s.merge_threshold = m;
                }
                s
            }
            None => feature_insights::ScopeConfig::default(),
        };

        // Resolve scope once — used for both hash computation and context building
        let scope_note_ids = if let Some(ref service) = self.insight_service {
            service.resolve_scope(note_id, &scope).await
        } else {
            self.get_related_note_ids(note_id).await
        };

        let content_hash = feature_insights::InsightService::compute_input_hash(
            &note.title,
            &note.body,
            &scope_note_ids,
        );

        if let Some(ref service) = self.insight_service {
            if let Ok(Some(cached)) = service.check_cache(note_id, &content_hash).await {
                return Ok(InsightReviewStarted {
                    insight_review_id: cached.id,
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

        // NoteRow doesn't carry tags — fetch them separately
        let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
        let entity_repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let note_domains = insight_context::extract_note_domains(&tags, Some(&entity_repo)).await;

        // Build context via InsightService pipeline (merge check + cognitive injection)
        let (context_text, note_title, parent_insight_id) =
            if let Some(ref service) = self.insight_service {
                // Fetch related notes for the resolved scope
                let mut related_notes = Vec::new();
                for id in &scope_note_ids {
                    if let Ok(Some(n)) = self.note_repo.get_note(id).await {
                        related_notes.push(n);
                    }
                }

                match service
                    .prepare_context(
                        &note,
                        &related_notes,
                        &scope_note_ids,
                        &scope,
                        &note_domains,
                    )
                    .await
                {
                    Ok(prepared) => (
                        prepared.context.text,
                        prepared.context.note_title,
                        prepared.parent_insight_id,
                    ),
                    Err(e) => {
                        tracing::warn!("prepare_context failed, falling back: {e}");
                        let related_notes = self.fetch_related_notes(note_id).await;
                        let ctx = insight_context::assemble_context(&note, &related_notes, None);
                        (ctx.text, ctx.note_title, None)
                    }
                }
            } else {
                let related_notes = self.fetch_related_notes(note_id).await;
                let ctx = insight_context::assemble_context(&note, &related_notes, None);
                (ctx.text, ctx.note_title, None)
            };

        let selected_personas = self.select_personas(note_id, &note_domains).await;

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        let emitter = emitter_override.unwrap_or_else(|| Arc::clone(&self.event_emitter));

        // Fire "What's Changed" summary if a previous version exists
        if let Some(ref service) = self.insight_service {
            if let Ok(Some(prev)) = service.get_latest(note_id).await {
                let prev_content: feature_insights::InsightContent =
                    serde_json::from_str(&prev.content).unwrap_or_default();
                if let Some(prev_synthesis) = prev_content.synthesis {
                    let changes_emitter = emitter.clone();
                    let changes_context = context_text.clone();
                    let changes_provider = provider.clone();
                    let changes_config = self.config.read().await;
                    let changes_params = providers::cognitive_chat_params(&changes_config, 512);
                    drop(changes_config);

                    tokio::spawn(async move {
                        let prompt = insight_prompts::changes_summary_prompt(
                            &prev_synthesis,
                            &changes_context,
                        );
                        let messages = vec![
                            providers::Message::System { content: prompt },
                            providers::Message::User {
                                content: providers::UserContent::Text(
                                    "Generate the changes summary now.".to_string(),
                                ),
                            },
                        ];
                        if let Ok(response) = changes_provider
                            .chat(&messages, None, &changes_params)
                            .await
                        {
                            if let Some(summary) = response.content {
                                changes_emitter.emit_event(
                                    "insight:changes-summary",
                                    serde_json::json!({ "summary": summary }),
                                );
                            }
                        }
                    });
                }
            }
        }

        let insight_service = self.insight_service.clone();
        let note_id_owned = note_id.to_string();
        let content_hash_clone = content_hash.clone();

        tokio::spawn(async move {
            run_insight_pipeline(InsightPipelineArgs {
                provider,
                emitter,
                insight_service,
                note_id: note_id_owned,
                content_hash: content_hash_clone,
                context: context_text,
                note_title,
                params,
                personas: selected_personas,
                parent_insight_id,
                scope_note_ids,
                scope_config: scope,
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
        let service = match &self.insight_service {
            Some(s) => s,
            None => return Ok(None),
        };

        let row = match service
            .get_latest(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        {
            Some(r) => r,
            None => return Ok(None),
        };

        let mut response = row_to_insight_response(row);

        // Resolve persona IDs → full metadata for PersonaCard rendering
        if let Some(ref ids) = response.persona_ids {
            if !ids.is_empty() {
                if let Some(ref repo) = self.persona_repo {
                    let mut personas = Vec::new();
                    for id in ids {
                        if let Ok(Some(p)) = repo.get(id).await {
                            personas.push(PersonaMetaResponse {
                                id: p.id,
                                name: p.name,
                                role: p.role,
                                icon: p.icon,
                                tone: p.tone,
                            });
                        }
                    }
                    response.personas = personas;
                }
            }
        }

        Ok(Some(response))
    }

    /// Knowledge growth metrics from the cognitive fact store.
    pub async fn note_insight_knowledge_growth(
        &self,
        days: u32,
    ) -> Result<KnowledgeGrowthResponse, ApiError> {
        let fact_repo = cognitive::repos::SemanticFactRepo::new(self.storage_pool.inner().clone());
        let temporal = cognitive::TemporalService::new(fact_repo);
        let since = chrono::Utc::now() - chrono::Duration::days(i64::from(days));
        let summary = temporal
            .change_summary(since, None)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        let by_domain = summary
            .by_domain
            .into_iter()
            .map(|(domain, count)| DomainCount { domain, count })
            .collect();

        Ok(KnowledgeGrowthResponse {
            new_facts_count: summary.new_facts.len(),
            updated_facts_count: summary.updated_facts.len(),
            superseded_facts_count: summary.superseded_facts.len(),
            by_domain,
            period_days: days,
        })
    }

    /// Generate a "What's Changed" summary comparing current note context to the last insight.
    pub async fn note_insight_changes_summary(
        &self,
        note_id: &str,
    ) -> Result<Option<ChangesSummaryResponse>, ApiError> {
        let service = match &self.insight_service {
            Some(s) => s,
            None => return Ok(None),
        };

        let latest = match service.get_latest(note_id).await.ok().flatten() {
            Some(r) => r,
            None => return Ok(None),
        };

        if latest.version < 2 {
            return Ok(None);
        }

        let prev_content: feature_insights::InsightContent =
            serde_json::from_str(&latest.content).unwrap_or_default();
        let prev_synthesis = match prev_content.synthesis {
            Some(s) => s,
            None => return Ok(None),
        };

        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let related_notes = self.fetch_related_notes(note_id).await;
        let ctx = insight_context::assemble_context(&note, &related_notes, None);

        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 512);
        drop(config);

        let prompt = insight_prompts::changes_summary_prompt(&prev_synthesis, &ctx.text);
        let messages = vec![
            providers::Message::System { content: prompt },
            providers::Message::User {
                content: providers::UserContent::Text(
                    "Generate the changes summary now.".to_string(),
                ),
            },
        ];

        let response = provider
            .chat(&messages, None, &params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        match response.content {
            Some(summary) if !summary.contains("No significant changes") => {
                Ok(Some(ChangesSummaryResponse { summary }))
            }
            _ => Ok(None),
        }
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
            .map(super::flashcard::flashcard_to_response)
            .collect())
    }

    pub async fn note_insight_submit_quiz(
        &self,
        params: &InsightQuizSubmitParams,
    ) -> Result<(), ApiError> {
        let service = self
            .insight_service
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Insight service not available"))?;

        let insight = service
            .get_version(&params.insight_review_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Insight not found"))?;

        let note = self
            .note_repo
            .get_note(&insight.note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        service
            .compute_progress(&params.insight_review_id, &note.body, Some(params.score))
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(())
    }

    pub async fn note_insight_generate_scenario(
        &self,
        note_id: &str,
    ) -> Result<ScenarioChallengeResponse, ApiError> {
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

        let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
        let entity_repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let note_domains = insight_context::extract_note_domains(&tags, Some(&entity_repo)).await;

        let ctx_text = if let Some(ref service) = self.insight_service {
            let scope: feature_insights::ScopeConfig = service
                .get_latest(note_id)
                .await
                .ok()
                .flatten()
                .and_then(|l| serde_json::from_str(&l.scope_config).ok())
                .unwrap_or_default();

            let scope_ids = service.resolve_scope(note_id, &scope).await;
            let mut related_notes = Vec::new();
            for id in &scope_ids {
                if let Ok(Some(n)) = self.note_repo.get_note(id).await {
                    related_notes.push(n);
                }
            }

            match service
                .prepare_context(&note, &related_notes, &scope_ids, &scope, &note_domains)
                .await
            {
                Ok(prepared) => prepared.context.text,
                Err(_) => {
                    let related = self.fetch_related_notes(note_id).await;
                    insight_context::assemble_context(&note, &related, None).text
                }
            }
        } else {
            let related = self.fetch_related_notes(note_id).await;
            insight_context::assemble_context(&note, &related, None).text
        };

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        let prompt = insight_prompts::scenario_challenge_prompt(&ctx_text);
        let messages = vec![
            providers::Message::System { content: prompt },
            providers::Message::User {
                content: providers::UserContent::Text(
                    "Generate the scenario challenge now.".to_string(),
                ),
            },
        ];

        let response = provider
            .chat(&messages, None, &params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let content = response.content.unwrap_or_default();
        let cleaned = strip_markdown_fences(&content);

        serde_json::from_str::<ScenarioChallengeResponse>(&cleaned).map_err(|e| {
            ApiError::new(
                "PARSE_ERROR",
                format!("Failed to parse scenario response: {e}"),
            )
        })
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

        // Use InsightService pipeline for consistent context quality.
        // Fetch tags + domains once (reused for both context building and persona selection).
        let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
        let entity_repo = cognitive::repos::EntityRepo::new(self.storage_pool.inner().clone());
        let note_domains = insight_context::extract_note_domains(&tags, Some(&entity_repo)).await;

        // Cache latest insight for both scope config and tab update
        let latest_insight = if let Some(ref service) = self.insight_service {
            service.get_latest(note_id).await.ok().flatten()
        } else {
            None
        };

        let (ctx_text, ctx_note_title) = if let Some(ref service) = self.insight_service {
            let scope: feature_insights::ScopeConfig = latest_insight
                .as_ref()
                .and_then(|l| serde_json::from_str(&l.scope_config).ok())
                .unwrap_or_default();

            let scope_ids = service.resolve_scope(note_id, &scope).await;
            let mut related_notes = Vec::new();
            for id in &scope_ids {
                if let Ok(Some(n)) = self.note_repo.get_note(id).await {
                    related_notes.push(n);
                }
            }

            match service
                .prepare_context(&note, &related_notes, &scope_ids, &scope, &note_domains)
                .await
            {
                Ok(prepared) => (prepared.context.text, prepared.context.note_title),
                Err(e) => {
                    tracing::warn!("prepare_context failed in regenerate_tab, falling back: {e}");
                    let related = self.fetch_related_notes(note_id).await;
                    let ctx = insight_context::assemble_context(&note, &related, None);
                    (ctx.text, ctx.note_title)
                }
            }
        } else {
            let related = self.fetch_related_notes(note_id).await;
            let ctx = insight_context::assemble_context(&note, &related, None);
            (ctx.text, ctx.note_title)
        };

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 4096);
        drop(config);

        // Track persona metadata for perspectives tab (returned alongside content)
        let mut tab_personas: Vec<PersonaMetaResponse> = Vec::new();

        let prompt = match tab {
            "synthesis" => insight_prompts::synthesis_prompt(&ctx_text),
            "gaps" => insight_prompts::gap_analysis_prompt(&ctx_text),
            "assessment" => insight_prompts::self_assessment_prompt(&ctx_text),
            "concept-map" => insight_prompts::concept_map_prompt(&ctx_text, &ctx_note_title),
            "perspectives" => {
                let personas = self.select_personas(note_id, &note_domains).await;
                tab_personas = personas
                    .iter()
                    .map(|p| PersonaMetaResponse {
                        id: p.id.clone(),
                        name: p.name.clone(),
                        role: p.role.clone(),
                        icon: p.icon.clone(),
                        tone: p.tone.clone(),
                    })
                    .collect();
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
                insight_prompts::perspectives_prompt(&ctx_text, &persona_blocks)
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

        let raw_content = response.content.unwrap_or_default();
        // Only strip fences for JSON-producing tabs (matching generate_tab pipeline behavior)
        let content = if tab == "assessment" || tab == "gaps" {
            strip_markdown_fences(&raw_content)
        } else {
            raw_content
        };

        if let Some(ref service) = self.insight_service {
            if let Some(ref latest) = latest_insight {
                // Update existing insight record
                if let Err(e) = service.update_tab(&latest.id, tab, &content).await {
                    tracing::warn!("failed to persist regenerated tab {tab}: {e}");
                }
            } else {
                // No existing insight — create one with just this tab (lazy init).
                // Invalid tabs are already rejected by the prompt match above.
                let mut insight_content = feature_insights::InsightContent::default();
                match tab {
                    "synthesis" => insight_content.synthesis = Some(content.clone()),
                    "gaps" => insight_content.gap_analysis = Some(content.clone()),
                    "assessment" => insight_content.self_assessment = Some(content.clone()),
                    "concept-map" => insight_content.concept_map = Some(content.clone()),
                    "perspectives" => insight_content.perspectives = Some(content.clone()),
                    _ => unreachable!("invalid tab already rejected"),
                }
                let input_hash = feature_insights::InsightService::compute_input_hash(
                    &note.title,
                    &note.body,
                    &[],
                );
                if let Err(e) = service
                    .store_insight(
                        note_id,
                        &insight_content,
                        &input_hash,
                        &feature_insights::ScopeConfig::default(),
                        &[],
                        None,
                        &[],
                    )
                    .await
                {
                    tracing::warn!("failed to create insight for tab {tab}: {e}");
                }
            }
        }

        Ok(TabContent {
            tab: tab.to_string(),
            content,
            personas: tab_personas,
        })
    }

    pub async fn note_insight_get_evolution(
        &self,
        note_id: &str,
    ) -> Result<InsightEvolutionResponse, ApiError> {
        let service = self
            .insight_service
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Insight service not available"))?;

        let note_title = self
            .note_repo
            .get_note(note_id)
            .await
            .ok()
            .flatten()
            .map(|n| n.title)
            .unwrap_or_default();

        let evolution = service
            .get_evolution(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        let mut points = Vec::new();
        let mut prev_snapshot: Option<feature_insights::ProgressSnapshotRow> = None;

        // Iterate oldest-first for change note generation
        for (version, snapshot) in evolution.iter().rev() {
            let change_note = match snapshot {
                Some(snap) => feature_insights::generate_change_note(snap, prev_snapshot.as_ref()),
                None => "No progress data".to_string(),
            };

            points.push(InsightEvolutionPoint {
                version: version.version,
                generated_at: version.generated_at.clone(),
                flashcard_success: snapshot.as_ref().map_or(0.0, |s| s.flashcard_success),
                semantic_drift: snapshot.as_ref().map_or(0.0, |s| s.semantic_drift),
                gap_closure: snapshot.as_ref().map_or(0.0, |s| s.gap_closure),
                quiz_score: snapshot.as_ref().map_or(0.0, |s| s.quiz_score),
                overall_progress: snapshot.as_ref().map_or(0.0, |s| s.overall_progress),
                change_note,
            });

            if let Some(snap) = snapshot {
                prev_snapshot = Some(snap.clone());
            }
        }

        Ok(InsightEvolutionResponse {
            note_id: note_id.to_string(),
            note_title,
            versions: points,
        })
    }

    pub async fn note_insight_get_version(
        &self,
        insight_id: &str,
    ) -> Result<InsightReviewResponse, ApiError> {
        let service = self
            .insight_service
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Insight service not available"))?;

        let row = service
            .get_version(insight_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Insight version not found"))?;

        Ok(row_to_insight_response(row))
    }

    pub async fn note_insight_list_versions(
        &self,
        note_id: &str,
    ) -> Result<Vec<InsightVersionResponse>, ApiError> {
        let service = self
            .insight_service
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Insight service not available"))?;
        let versions = service
            .list_versions(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(versions
            .into_iter()
            .map(|v| InsightVersionResponse {
                id: v.id,
                version: v.version,
                generated_at: v.generated_at,
                input_hash: v.input_hash,
                has_parent: v.parent_insight_id.is_some(),
            })
            .collect())
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

    pub(crate) async fn fetch_related_notes(
        &self,
        note_id: &str,
    ) -> Vec<feature_notes::models::NoteRow> {
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

    async fn select_personas(
        &self,
        note_id: &str,
        domains: &[String],
    ) -> Vec<cognitive::PersonaRow> {
        match &self.persona_repo {
            Some(repo) => repo
                .select_for_note(note_id, domains, 4)
                .await
                .unwrap_or_default(),
            None => Vec::new(),
        }
    }
}

struct InsightPipelineArgs {
    provider: providers::DynProvider,
    emitter: Arc<dyn AppEventEmitter>,
    insight_service: Option<Arc<feature_insights::InsightService>>,
    note_id: String,
    content_hash: String,
    context: String,
    note_title: String,
    params: providers::ChatParams,
    personas: Vec<cognitive::PersonaRow>,
    parent_insight_id: Option<String>,
    scope_note_ids: Vec<String>,
    scope_config: feature_insights::ScopeConfig,
}

async fn run_insight_pipeline(args: InsightPipelineArgs) {
    let InsightPipelineArgs {
        provider,
        emitter,
        insight_service,
        note_id,
        content_hash,
        context,
        note_title,
        params,
        personas,
        parent_insight_id,
        scope_note_ids,
        scope_config,
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

    if let Some(ref service) = insight_service {
        let content = feature_insights::InsightContent {
            synthesis,
            gap_analysis: gaps,
            self_assessment: assessment,
            concept_map,
            perspectives,
        };
        let persona_ids: Vec<String> = personas.iter().map(|p| p.id.clone()).collect();
        let _ = service
            .store_insight(
                &note_id,
                &content,
                &content_hash,
                &scope_config,
                &persona_ids,
                parent_insight_id.as_deref(),
                &scope_note_ids,
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

/// Strip markdown code fences (```json...``` or ```...```) from LLM output.
/// LLMs frequently wrap JSON responses in fences despite being told not to.
fn strip_markdown_fences(content: &str) -> String {
    let trimmed = content.trim();
    if let Some(rest) = trimmed.strip_prefix("```") {
        // Remove optional language tag (e.g. "json", "mermaid") on the first line
        let rest = rest
            .strip_prefix("json")
            .or_else(|| rest.strip_prefix("mermaid"))
            .unwrap_or(rest);
        let rest = rest.trim_start_matches('\n');
        if let Some(body) = rest.strip_suffix("```") {
            return body.trim().to_string();
        }
    }
    trimmed.to_string()
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
            let raw = response.content.unwrap_or_default();
            // Strip markdown fences for JSON-producing tabs
            let content = if tab_name == "assessment" || tab_name == "gaps" {
                strip_markdown_fences(&raw)
            } else {
                raw
            };
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
