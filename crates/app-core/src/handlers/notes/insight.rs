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
        squad_id: Option<&str>,
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
                        "notebook" => feature_insights::ScopeType::Notebook,
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

        let resolved_squad = self
            .resolve_squad_full(note_id, squad_id, &note_domains)
            .await;

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 4096);
        let changes_params = providers::cognitive_chat_params(&config, 512);
        let response_lang = resolve_response_lang(&config);
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

                    let changes_lang = response_lang.clone();
                    tokio::spawn(async move {
                        let prompt = insight_prompts::changes_summary_prompt(
                            &prev_synthesis,
                            &changes_context,
                            changes_lang.as_deref(),
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

        let pipeline_pool = self.storage_pool.inner().clone();
        let pipeline_bus = self.domain_event_bus.clone();
        let note_body_owned = note.body.clone();
        tokio::spawn(async move {
            run_insight_pipeline(InsightPipelineArgs {
                provider,
                emitter,
                insight_service,
                note_id: note_id_owned,
                content_hash: content_hash_clone,
                context: context_text,
                note_title,
                note_body: note_body_owned,
                params,
                resolved_squad,
                note_domains,
                parent_insight_id,
                scope_note_ids,
                scope_config: scope,
                pool: pipeline_pool,
                domain_event_bus: pipeline_bus,
                response_lang,
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
        let since =
            jiff::Timestamp::now() - jiff::SignedDuration::from_secs(i64::from(days) * 86400);
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
        let response_lang = resolve_response_lang(&config);
        drop(config);

        let prompt = insight_prompts::changes_summary_prompt(
            &prev_synthesis,
            &ctx.text,
            response_lang.as_deref(),
        );
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

        let atom_repo = cognitive::KnowledgeAtomRepo::new(self.storage_pool.inner().clone());
        let active_atoms = atom_repo
            .list_for_note(&params.note_id)
            .await
            .unwrap_or_default()
            .into_iter()
            .filter(|a| a.status == "active")
            .collect::<Vec<_>>();

        let cards: Vec<cognitive::NewFlashcard> = params
            .questions
            .iter()
            .map(|q| {
                let (stability, difficulty) = match q.difficulty.as_str() {
                    "easy" => (4.0, 3.0),
                    "hard" => (0.8, 7.0),
                    _ => (2.0, 5.0),
                };
                cognitive::NewFlashcard {
                    source_note_id: Some(params.note_id.clone()),
                    source_context: if q.source_notes.is_empty() {
                        None
                    } else {
                        Some(format!("From: {}", q.source_notes.join(", ")))
                    },
                    atom_id: active_atoms
                        .iter()
                        .find(|a| {
                            q.question
                                .to_lowercase()
                                .contains(&a.subject.to_lowercase())
                        })
                        .map(|a| a.id.clone()),
                    deck: params.deck_name.clone(),
                    front: q.question.clone(),
                    back: q.correct_answer.clone(),
                    card_type: cognitive::CardType::Basic,
                    cloze_data: None,
                    vocab_data: None,
                    image_data: None,
                    tags: vec![],
                    stability,
                    difficulty,
                    difficulty_estimate: None,
                    prerequisite_concepts: None,
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

        // Emit AtomInteracted for active atoms on this note (quiz engagement signal)
        if let Some(bus) = &self.domain_event_bus {
            let pool = self.storage_pool.inner();
            // Batch-update all active atoms' last_interaction_ts in one query
            let now = jiff::Timestamp::now().to_string();
            let _ = sqlx::query(
                "UPDATE knowledge_atoms SET last_interaction_ts = ?1, updated_at = ?1 \
                 WHERE source_note_id = ?2 AND status = 'active'",
            )
            .bind(&now)
            .bind(&insight.note_id)
            .execute(pool)
            .await;

            // Emit events per atom (in-memory, cheap)
            let atom_repo = cognitive::KnowledgeAtomRepo::new(pool.clone());
            if let Ok(atoms) = atom_repo.list_for_note(&insight.note_id).await {
                for atom in atoms.iter().filter(|a| a.status == "active") {
                    bus.publish(bus::DomainEvent::AtomInteracted {
                        atom_id: atom.id.clone(),
                        interaction_type: "quiz_answer".to_string(),
                        note_id: Some(insight.note_id.clone()),
                    });
                }
            }
        }

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
        let response_lang = resolve_response_lang(&config);
        drop(config);

        let prompt =
            insight_prompts::scenario_challenge_prompt(&ctx_text, response_lang.as_deref());
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
        let response_lang = resolve_response_lang(&config);
        drop(config);

        // Track persona metadata for perspectives tab (returned alongside content)
        let mut tab_personas: Vec<PersonaMetaResponse> = Vec::new();

        let lang = response_lang.as_deref();

        // Perspectives tab uses the debate engine; all other tabs use a single LLM call.
        let content = if tab == "perspectives" {
            use agent::engines::debate::run_debate;
            use agent::engines::debate_types::*;
            use tokio_util::sync::CancellationToken;

            let resolved = self
                .resolve_squad_full(note_id, None, &note_domains)
                .await
                .ok_or_else(|| {
                    ApiError::new(
                        "NOT_ENOUGH_PERSONAS",
                        "No personas available for perspectives",
                    )
                })?;

            tab_personas = resolved
                .personas
                .iter()
                .map(|p| PersonaMetaResponse {
                    id: p.id.clone(),
                    name: p.name.clone(),
                    role: p.role.clone(),
                    icon: p.icon.clone(),
                    tone: p.tone.clone(),
                })
                .collect();

            let debate_config = DebateConfig::for_notes();
            let debate_context = DebateContext {
                skill_prompt: String::new(),
                conversation_history: vec![],
                user_message: format!("{}\n\n{}", note.title, note.body),
                cognitive_context: Some(ctx_text.clone()),
                domains: note_domains.clone(),
            };

            let (debate_event_tx, _debate_event_rx) =
                tokio::sync::mpsc::channel::<DebateEvent>(100);
            let cancel = CancellationToken::new();

            let blackboard_repo = cognitive::BlackboardRepo::new(self.storage_pool.inner().clone());
            let accuracy_repo =
                cognitive::PersonaAccuracyRepo::new(self.storage_pool.inner().clone());

            let result = run_debate(
                debate_config,
                debate_context,
                resolved,
                provider,
                &blackboard_repo,
                &accuracy_repo,
                debate_event_tx,
                None,
                cancel,
            )
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

            let parts: Vec<String> = result
                .persona_responses
                .iter()
                .map(|r| {
                    format!(
                        "## {} — {}\n\n{}",
                        r.persona_name, r.persona_role, r.content
                    )
                })
                .collect();

            if parts.is_empty() {
                String::new()
            } else {
                parts.join("\n\n---\n\n")
            }
        } else {
            let prompt = match tab {
                "synthesis" => insight_prompts::synthesis_prompt(&ctx_text, lang),
                "gaps" => insight_prompts::gap_analysis_prompt(&ctx_text, lang),
                "assessment" => insight_prompts::self_assessment_prompt(&ctx_text, lang),
                "concept-map" => {
                    insight_prompts::concept_map_prompt(&ctx_text, &ctx_note_title, lang)
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
            if tab == "assessment" || tab == "gaps" {
                strip_markdown_fences(&raw_content)
            } else {
                raw_content
            }
        };

        if let Some(ref service) = self.insight_service {
            if let Some(ref latest) = latest_insight {
                // Update existing insight record
                if let Err(e) = service.update_tab(&latest.id, tab, &content).await {
                    tracing::warn!("failed to persist regenerated tab {tab}: {e}");
                }
            } else {
                // No existing insight — create one with just this tab (lazy init).
                // Invalid tabs are already rejected above.
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

    pub async fn note_insight_preview_scope(
        &self,
        params: ScopePreviewParams,
    ) -> Result<ScopePreviewResponse, ApiError> {
        use feature_insights::ScopeType;

        let mut scope_config = feature_insights::ScopeConfig::default();
        if let Some(ref st) = params.scope.scope_type {
            scope_config.scope_type = match st.as_str() {
                "notebook" => ScopeType::Notebook,
                "semantic" => ScopeType::Semantic,
                "project" => ScopeType::Project,
                "manual" => ScopeType::Manual,
                _ => ScopeType::Backlinks,
            };
        }

        let note_ids = if let Some(ref service) = self.insight_service {
            service.resolve_scope(&params.note_id, &scope_config).await
        } else {
            self.get_related_note_ids(&params.note_id).await
        };

        let mut notes = Vec::new();
        let mut total_words: u32 = 0;
        for id in &note_ids {
            if let Ok(Some(note)) = self.note_repo.get_note(id).await {
                let wc = note.body.split_whitespace().count() as u32;
                total_words += wc;
                notes.push(ScopePreviewNote {
                    id: note.id,
                    title: note.title,
                    notebook_id: note.notebook_id,
                    word_count: wc,
                });
            }
        }

        // Add current note's word count
        if let Ok(Some(current)) = self.note_repo.get_note(&params.note_id).await {
            total_words += current.body.split_whitespace().count() as u32;
        }

        // Fetch links between all nodes in scope (current note + scope notes)
        let all_links = self.note_repo.get_all_links().await.unwrap_or_default();
        let mut scope_set = std::collections::HashSet::new();
        scope_set.insert(params.note_id.clone());
        for n in &notes {
            scope_set.insert(n.id.clone());
        }
        let links: Vec<ScopePreviewLink> = all_links
            .into_iter()
            .filter(|l| scope_set.contains(&l.source_id) && scope_set.contains(&l.target_id))
            .map(|l| ScopePreviewLink {
                source_id: l.source_id,
                target_id: l.target_id,
            })
            .collect();

        // Compute atom summary for the current note
        let (strong_atoms, fading_atoms) = if let Ok(repo) = self.knowledge_atom_repo() {
            match repo.list_for_note(&params.note_id).await {
                Ok(atoms) => {
                    let strong = atoms.iter().filter(|a| a.retention_pct >= 0.6).count() as u32;
                    let fading = atoms.iter().filter(|a| a.retention_pct < 0.6).count() as u32;
                    (strong, fading)
                }
                Err(_) => (0, 0),
            }
        } else {
            (0, 0)
        };

        let include_cognitive = params.scope.include_cognitive.unwrap_or(true);
        let deep_dive = params.scope.deep_dive.unwrap_or(false);

        // Compute cognitive context counts when enabled
        let note_title = self
            .note_repo
            .get_note(&params.note_id)
            .await
            .ok()
            .flatten()
            .map(|n| n.title)
            .unwrap_or_default();

        let (facts_count, memories_count) = if include_cognitive {
            if let Some(ref service) = self.insight_service {
                service.cognitive_counts(&note_title, &params.note_id).await
            } else {
                (0, 0)
            }
        } else {
            (0, 0)
        };

        // Compute entity connections when deep dive is enabled
        let entity_count = if deep_dive {
            if let Some(ref service) = self.insight_service {
                service.entity_count(&params.note_id).await
            } else {
                0
            }
        } else {
            0
        };

        let context_summary = ContextSummary {
            total_notes: notes.len() as u32 + 1, // +1 for current note
            total_words,
            strong_atoms,
            fading_atoms,
            facts_count,
            memories_count,
            entity_count,
            include_cognitive,
            deep_dive,
        };

        Ok(ScopePreviewResponse {
            notes,
            links,
            context_summary,
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

    /// Run a multi-round debate on the note content using the squad's personas.
    /// Emits debate events through the insight SSE channel.
    pub async fn note_insight_debate(
        &self,
        note_id: &str,
        squad_id: Option<&str>,
    ) -> Result<(), ApiError> {
        use agent::engines::debate::run_debate;
        use agent::engines::debate_types::*;
        use tokio_util::sync::CancellationToken;

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

        let resolved = self
            .resolve_squad_full(note_id, squad_id, &note_domains)
            .await
            .ok_or_else(|| {
                ApiError::new(
                    "NOT_ENOUGH_PERSONAS",
                    "Need at least 2 personas for a debate",
                )
            })?;
        if resolved.personas.len() < 2 {
            return Err(ApiError::new(
                "NOT_ENOUGH_PERSONAS",
                "Need at least 2 personas for a debate",
            ));
        }

        let blackboard_repo = cognitive::BlackboardRepo::new(self.storage_pool.inner().clone());
        let accuracy_repo = cognitive::PersonaAccuracyRepo::new(self.storage_pool.inner().clone());

        let debate_config = DebateConfig::for_notes();
        let debate_context = DebateContext {
            skill_prompt: String::new(),
            conversation_history: vec![],
            user_message: format!(
                "Analyze this note from your unique perspective. What are the key insights, \
                 weaknesses, and recommendations?\n\n# {}\n\n{}",
                note.title, note.body
            ),
            cognitive_context: None,
            domains: note_domains,
        };

        let (debate_event_tx, mut debate_event_rx) = tokio::sync::mpsc::channel::<DebateEvent>(100);
        let cancel = CancellationToken::new();

        // Clone emitter for the relay task
        let emitter = Arc::clone(&self.event_emitter);

        // Spawn relay task: convert DebateEvents → insight SSE events
        let relay_handle = tokio::spawn(async move {
            while let Some(event) = debate_event_rx.recv().await {
                match event {
                    DebateEvent::RoundStarted {
                        round,
                        phase,
                        persona_order,
                        ..
                    } => {
                        emitter.emit_event(
                            "insight:debate-round-started",
                            serde_json::json!({
                                "round": round,
                                "totalRounds": 0,
                                "phase": phase.as_str(),
                            }),
                        );
                        let _ = persona_order;
                    }
                    DebateEvent::PersonaPerspective {
                        persona_id,
                        name,
                        icon,
                        role,
                        content,
                        challenge,
                        ..
                    } => {
                        emitter.emit_event(
                            "insight:debate-persona",
                            serde_json::json!({
                                "personaId": persona_id,
                                "personaName": name,
                                "personaIcon": icon,
                                "personaRole": role,
                                "content": content,
                                "challenge": challenge,
                            }),
                        );
                    }
                    DebateEvent::JudgeDecision {
                        round,
                        consensus_score,
                        decision,
                        speaking_order,
                    } => {
                        emitter.emit_event(
                            "insight:debate-judge",
                            serde_json::json!({
                                "round": round,
                                "consensusScore": consensus_score,
                                "decision": decision,
                                "speakingOrder": speaking_order,
                            }),
                        );
                    }
                    DebateEvent::RoundCompleted { round, .. } => {
                        emitter.emit_event(
                            "insight:debate-round-completed",
                            serde_json::json!({
                                "round": round,
                            }),
                        );
                    }
                    DebateEvent::ConsensusReached { round, score } => {
                        emitter.emit_event(
                            "insight:debate-consensus",
                            serde_json::json!({
                                "round": round,
                                "consensusScore": score,
                            }),
                        );
                    }
                    DebateEvent::Complete => {}
                    _ => {}
                }
            }
        });

        // Run the debate
        let emitter_done = Arc::clone(&self.event_emitter);
        let provider_clone = provider.clone();
        let note_id_owned = note_id.to_string();
        tokio::spawn(async move {
            let _result = run_debate(
                debate_config,
                debate_context,
                resolved,
                &provider_clone,
                &blackboard_repo,
                &accuracy_repo,
                debate_event_tx,
                None,
                cancel,
            )
            .await;

            let _ = relay_handle.await;

            emitter_done.emit_event(
                "insight:debate-done",
                serde_json::json!({ "noteId": note_id_owned }),
            );
        });

        Ok(())
    }

    /// Resolve squad as a full `ResolvedSquad` (needed by the debate engine).
    ///
    /// Falls back to the builtin general squad, then to domain-based persona
    /// selection wrapped in a synthetic `ResolvedSquad`.
    async fn resolve_squad_full(
        &self,
        note_id: &str,
        squad_id: Option<&str>,
        domains: &[String],
    ) -> Option<cognitive::ResolvedSquad> {
        if let Some(sid) = squad_id {
            if let Some(squad_repo) = &self.squad_repo {
                if let Ok(Some(resolved)) = squad_repo.resolve_squad(sid).await {
                    return Some(resolved);
                }
            }
        }
        if let Some(squad_repo) = &self.squad_repo {
            if let Ok(Some(resolved)) = squad_repo.resolve_squad("builtin-squad-general").await {
                return Some(resolved);
            }
        }
        // Fallback: wrap loose personas in a synthetic ResolvedSquad
        let personas = match &self.persona_repo {
            Some(repo) => repo
                .select_for_note(note_id, domains, 6)
                .await
                .unwrap_or_default(),
            None => return None,
        };
        if personas.is_empty() {
            return None;
        }
        let now = jiff::Timestamp::now().to_string();
        Some(cognitive::ResolvedSquad {
            squad: cognitive::SquadRow {
                id: "synthetic-note-squad".to_string(),
                name: "Note Analysis".to_string(),
                description: "Auto-selected personas for note analysis".to_string(),
                icon: "🔍".to_string(),
                orchestrator_skill: String::new(),
                source: "synthetic".to_string(),
                domains: serde_json::to_string(domains).unwrap_or_else(|_| "[]".to_string()),
                is_active: 1,
                default_interaction_mode: "debate".to_string(),
                last_smart_mode: None,
                last_smart_updated: None,
                created_at: now.clone(),
                updated_at: now,
            },
            members: personas
                .iter()
                .enumerate()
                .map(|(i, p)| cognitive::SquadMemberRow {
                    squad_id: "synthetic-note-squad".to_string(),
                    persona_id: p.id.clone(),
                    role_in_squad: "member".to_string(),
                    sort_order: i as i64,
                })
                .collect(),
            personas,
        })
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
    note_body: String,
    params: providers::ChatParams,
    resolved_squad: Option<cognitive::ResolvedSquad>,
    note_domains: Vec<String>,
    parent_insight_id: Option<String>,
    scope_note_ids: Vec<String>,
    scope_config: feature_insights::ScopeConfig,
    pool: sqlx::SqlitePool,
    domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    response_lang: Option<String>,
}

async fn run_insight_pipeline(args: InsightPipelineArgs) {
    use agent::engines::debate::run_debate;
    use agent::engines::debate_types::*;
    use tokio_util::sync::CancellationToken;

    let InsightPipelineArgs {
        provider,
        emitter,
        insight_service,
        note_id,
        content_hash,
        context,
        note_title,
        note_body,
        params,
        resolved_squad,
        note_domains,
        parent_insight_id,
        scope_note_ids,
        scope_config,
        pool,
        domain_event_bus,
        response_lang,
    } = args;

    let lang = response_lang.as_deref();
    let synthesis = stream_synthesis(&provider, &emitter, &context, &params, lang).await;

    let gaps_prompt = insight_prompts::gap_analysis_prompt(&context, lang);
    let assessment_prompt = insight_prompts::self_assessment_prompt(&context, lang);
    let concept_map_prompt = insight_prompts::concept_map_prompt(&context, &note_title, lang);

    // Extract personas for metadata emission (before moving resolved_squad)
    let personas: Vec<cognitive::PersonaRow> = resolved_squad
        .as_ref()
        .map(|rs| rs.personas.clone())
        .unwrap_or_default();

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

    // Perspectives generation via debate engine (runs concurrently with other tabs)
    let perspectives_future = {
        let provider = provider.clone();
        let emitter = emitter.clone();
        let context = context.clone();
        let note_title = note_title.clone();
        let note_body = note_body.clone();
        let pool = pool.clone();
        async move {
            let squad = match resolved_squad {
                Some(s) if !s.personas.is_empty() => s,
                _ => return None,
            };

            let debate_config = DebateConfig::for_notes();

            let debate_context = DebateContext {
                skill_prompt: String::new(), // Notes don't use orchestrator skills
                conversation_history: vec![],
                user_message: format!("{}\n\n{}", note_title, note_body),
                cognitive_context: Some(context),
                domains: note_domains,
            };

            let (debate_event_tx, mut debate_event_rx) =
                tokio::sync::mpsc::channel::<DebateEvent>(100);
            let cancel = CancellationToken::new();

            let blackboard_repo = cognitive::BlackboardRepo::new(pool.clone());
            let accuracy_repo = cognitive::PersonaAccuracyRepo::new(pool);

            // Spawn relay task: convert DebateEvents → insight SSE events
            let relay_emitter = emitter.clone();
            let relay_handle = tokio::spawn(async move {
                while let Some(event) = debate_event_rx.recv().await {
                    match event {
                        DebateEvent::PersonaPerspective {
                            persona_id,
                            name,
                            content,
                            ..
                        } => {
                            relay_emitter.emit_event(
                                "insight:persona-perspective",
                                serde_json::json!({
                                    "personaId": persona_id,
                                    "personaName": name,
                                    "content": content,
                                }),
                            );
                        }
                        DebateEvent::Complete => {}
                        _ => {} // Other debate events not surfaced in insight pipeline
                    }
                }
            });

            let result = run_debate(
                debate_config,
                debate_context,
                squad,
                &provider,
                &blackboard_repo,
                &accuracy_repo,
                debate_event_tx,
                None,
                cancel,
            )
            .await;

            // Wait for relay task to finish
            let _ = relay_handle.await;

            match result {
                Ok(debate_result) => {
                    let parts: Vec<String> = debate_result
                        .persona_responses
                        .iter()
                        .map(|r| {
                            format!(
                                "## {} — {}\n\n{}",
                                r.persona_name, r.persona_role, r.content
                            )
                        })
                        .collect();
                    if parts.is_empty() {
                        None
                    } else {
                        Some(parts.join("\n\n---\n\n"))
                    }
                }
                Err(e) => {
                    emitter.emit_event(
                        "insight:error",
                        serde_json::json!({ "tab": "perspectives", "error": e.to_string() }),
                    );
                    None
                }
            }
        }
    };

    let ((gaps, assessment, concept_map), perspectives) = tokio::join!(
        async {
            tokio::join!(
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
            )
        },
        perspectives_future,
    );

    emitter.emit_event(
        "insight:tab-done",
        serde_json::json!({ "tab": "perspectives", "content": perspectives }),
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
        if let Ok(row) = service
            .store_insight(
                &note_id,
                &content,
                &content_hash,
                &scope_config,
                &persona_ids,
                parent_insight_id.as_deref(),
                &scope_note_ids,
            )
            .await
        {
            if let Some(gap_content) = &content.gap_analysis {
                let pool = pool.clone();
                let note_id = note_id.clone();
                let insight_id = row.id.clone();
                let gap_content = gap_content.clone();
                let bus = domain_event_bus.clone();
                tokio::spawn(async move {
                    create_atoms_from_gaps(&pool, &note_id, &gap_content, &insight_id, &bus).await;
                });
            }
        }
    }
}

/// Parse gap analysis JSON and create suggested atoms for unmatched gaps.
async fn create_atoms_from_gaps(
    pool: &sqlx::SqlitePool,
    note_id: &str,
    gap_content: &str,
    insight_review_id: &str,
    domain_event_bus: &Option<Arc<bus::DomainEventBus>>,
) {
    // Parse JSON block from gap content (trailing ```json block)
    let Some(json_start) = gap_content.find("```json") else {
        return;
    };
    let json_body = &gap_content[json_start + 7..];
    let Some(json_end) = json_body.find("```") else {
        return;
    };
    let json_str = json_body[..json_end].trim();

    let gaps: Vec<serde_json::Value> = match serde_json::from_str(json_str) {
        Ok(v) => v,
        Err(_) => return,
    };

    let atom_repo = cognitive::KnowledgeAtomRepo::new(pool.clone());
    // Fetch existing atoms ONCE before the loop (not per-gap)
    let existing_atoms = atom_repo.list_for_note(note_id).await.unwrap_or_default();
    let existing_subjects: std::collections::HashSet<String> = existing_atoms
        .iter()
        .map(|a| a.subject.to_lowercase())
        .collect();

    for gap in &gaps {
        let Some(topic) = gap.get("topic").and_then(|v| v.as_str()) else {
            continue;
        };
        let description = gap
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Check if atom already exists for this subject on this note
        if existing_subjects.contains(&topic.to_lowercase()) {
            continue;
        }

        let metadata = serde_json::json!({
            "source": "gap_analysis",
            "insight_review_id": insight_review_id,
        });

        let new_atom = cognitive::NewKnowledgeAtom {
            subject: topic.to_string(),
            atom_type: "concept".to_string(),
            domain: "general".to_string(),
            source_note_id: Some(note_id.to_string()),
            source_context: Some(description.to_string()),
            metadata: Some(metadata.to_string()),
            status: "active".to_string(),
            ..Default::default()
        };

        match atom_repo.create(&new_atom).await {
            Ok(atom) => {
                if let Ok(t) = atom_repo
                    .get_or_create_topic(&atom.domain, &atom.domain)
                    .await
                {
                    let _ = sqlx::query("UPDATE knowledge_atoms SET topic_id = ?1 WHERE id = ?2")
                        .bind(&t.id)
                        .bind(&atom.id)
                        .execute(pool)
                        .await;
                }
                if let Some(bus) = domain_event_bus {
                    bus.publish(bus::DomainEvent::KnowledgeAtomCreated {
                        atom_id: atom.id,
                        atom_type: "concept".to_string(),
                        domain: "general".to_string(),
                        source_note_id: Some(note_id.to_string()),
                        personal_importance: 0.7,
                    });
                }
            }
            Err(e) => {
                tracing::debug!("Failed to create gap atom: {e}");
            }
        }
    }
}

async fn stream_synthesis(
    provider: &providers::DynProvider,
    emitter: &Arc<dyn AppEventEmitter>,
    context: &str,
    params: &providers::ChatParams,
    response_lang: Option<&str>,
) -> Option<String> {
    let messages = vec![
        providers::Message::System {
            content: insight_prompts::synthesis_prompt(context, response_lang),
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

/// Resolve the response language from config.
/// Returns `source_lang`, falling back to `native_lang`, then `None` (English default).
fn resolve_response_lang(config: &config::Config) -> Option<String> {
    config
        .language
        .source_lang
        .clone()
        .or_else(|| config.language.native_lang.clone())
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
