//! Streaming relay, chat send/cancel, and interaction response handlers.

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;

use agent::intent_pipeline::engines::debate;
use agent::intent_pipeline::engines::debate_types::{
    DebateConfig, DebateContext, DebateEvent, DebateResult, DefaultInteractionMode,
    SquadInteractionMode,
};
use agent::intent_pipeline::engines::interaction::detect_interaction_mode;
use agent::AgentEvent;
use cognitive::{BlackboardRepo, PersonaAccuracyRepo, ResolvedSquad};
use common::EntityCard;
use desktop_shared::commands::{ChatMessageResponse, SessionContextInput};
use desktop_shared::errors::ApiError;
use desktop_shared::events::{self, *};
use providers::Message;
use storage::{Repos, SessionContextParams};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::errors::map_storage_err;
use crate::state::AppCore;

/// Type aliases for the DashMap types used across both AppCore variants.
pub(super) type ActiveStreams = dashmap::DashMap<String, CancellationToken>;
pub(super) type PendingInteractions =
    dashmap::DashMap<String, (String, tokio::sync::oneshot::Sender<common::FormResponse>)>;

// ── ChatStreamInfo ──────────────────────────────────────────────────────

/// Returned from `chat_send` so the caller can wire up the
/// streaming relay with its own event emitter.
pub struct ChatStreamInfo {
    pub session_key: String,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    pub has_context: bool,
    /// True when the session was just created (not a follow-up in an existing thread).
    pub is_new_session: bool,
}

// ── Helper functions (private) ──────────────────────────────────────────

/// Build a human-readable summary line from a FormResponse.
fn format_interaction_summary(response: &common::FormResponse) -> String {
    match response {
        common::FormResponse::Cancelled => "Cancelled interaction".to_string(),
        common::FormResponse::Completed(answers) => {
            let parts: Vec<String> = answers
                .iter()
                .map(|a| match &a.value {
                    common::AnswerValue::Selected { value } => value.clone(),
                    common::AnswerValue::MultiSelected { values } => values.join(", "),
                    common::AnswerValue::YesNo { answer } => {
                        if *answer { "Yes" } else { "No" }.to_string()
                    }
                    common::AnswerValue::Text { content } => {
                        common::truncate_chars(content, 57, "...")
                    }
                    common::AnswerValue::Skipped => "Skipped".to_string(),
                })
                .collect();
            format!("You answered: {}", parts.join(" · "))
        }
    }
}

/// Map a tool name to its domain category.
fn tool_domain(tool_name: &str) -> Option<&'static str> {
    match tool_name {
        "todo" | "tasks" => Some("task"),
        "project" => Some("project"),
        "area" => Some("area"),
        "okr" => Some("objective"),
        "finance" => Some("finance"),
        _ => None,
    }
}

/// Map a tool name to the EntityKind it modifies, derived from `tool_domain`.
fn entity_kind_for_tool(tool_name: &str) -> Option<desktop_shared::types::EntityKind> {
    tool_domain(tool_name).and_then(desktop_shared::types::EntityKind::parse)
}

/// Returns `true` when the action is a write (create/update/delete/toggle/etc.)
/// rather than a read-only query (list/get/search).
fn is_mutating_action(action: Option<&str>) -> bool {
    !matches!(
        action,
        Some("list" | "get" | "search" | "search-semantic" | "search-hybrid")
    )
}

/// Map an entity_type from EntityCard to a session context entity_kind.
fn entity_kind_for(entity_type: &str) -> Option<&'static str> {
    match entity_type {
        "task" => Some("task"),
        "project" => Some("project"),
        "area" => Some("area"),
        "objective" | "key_result" => Some("objective"),
        s if s.starts_with("finance") => Some("finance"),
        _ => None,
    }
}

/// After streaming completes, infer session context from tool usage and entity
/// creation events. Only sets context when a single domain dominates.
async fn auto_detect_context(
    repos: &Repos,
    session_key: &str,
    tool_names: &[String],
    entity_cards: &[EntityCard],
) -> Result<(), ApiError> {
    // Determine dominant domain from tool names
    let domains: HashSet<&str> = tool_names.iter().filter_map(|n| tool_domain(n)).collect();
    if domains.len() != 1 {
        return Ok(()); // ambiguous or no tools → skip
    }
    let Some(&domain) = domains.iter().next() else {
        return Ok(());
    };

    // Pick the best entity card for this domain (first match)
    let card = entity_cards
        .iter()
        .find(|c| entity_kind_for(&c.entity_type) == Some(domain));

    let entity_id = card.map(|c| c.entity_id.as_str());

    // Resolve PARA ancestry
    let (area_id, project_id) = resolve_ancestry(repos, domain, entity_id).await;

    repos
        .session_context
        .upsert(SessionContextParams {
            session_key,
            context_type: "auto",
            entity_kind: Some(domain),
            entity_id,
            area_id: area_id.as_deref(),
            project_id: project_id.as_deref(),
            is_ephemeral: false, // non-ephemeral — auto-detected sessions are persistent
        })
        .await
        .map_err(map_storage_err)?;

    Ok(())
}

/// Look up area_id and project_id for a given entity.
async fn resolve_ancestry(
    repos: &Repos,
    domain: &str,
    entity_id: Option<&str>,
) -> (Option<String>, Option<String>) {
    let Some(id) = entity_id else {
        return (None, None);
    };

    match domain {
        "task" => {
            if let Ok(Some(task)) = repos.tasks.get(id).await {
                (Some(task.area_id.clone()), task.project_id.clone())
            } else {
                (None, None)
            }
        }
        "project" => {
            if let Ok(Some(proj)) = repos.projects.get(id).await {
                (Some(proj.area_id.clone()), Some(proj.id.clone()))
            } else {
                (None, None)
            }
        }
        "objective" => {
            if let Ok(Some(obj)) = repos.objectives.get(id).await {
                // Objective → project → area
                if let Ok(Some(proj)) = repos.projects.get(&obj.project_id).await {
                    (Some(proj.area_id.clone()), Some(proj.id.clone()))
                } else {
                    (None, Some(obj.project_id.clone()))
                }
            } else {
                (None, None)
            }
        }
        "area" => {
            // The entity IS the area
            (Some(id.to_string()), None)
        }
        // finance — no PARA ancestry
        _ => (None, None),
    }
}

// ── Squad execution ─────────────────────────────────────────────────────

/// Convert session message rows to provider Messages for debate context.
fn session_rows_to_messages(rows: &[storage::SessionMessageRow]) -> Vec<Message> {
    rows.iter()
        .filter_map(|r| match r.role.as_str() {
            "user" => Some(Message::user(&r.content)),
            "assistant" => Some(Message::assistant(&r.content)),
            "system" => Some(Message::system(&r.content)),
            _ => None,
        })
        .collect()
}

/// Map a `DebateEvent` from the debate engine to an `AgentEvent` for the
/// existing streaming relay. This bridges the gap between the debate engine's
/// event channel and the chat streaming infrastructure.
fn debate_event_to_agent_event(de: DebateEvent) -> Option<AgentEvent> {
    match de {
        DebateEvent::RoundStarted {
            round,
            phase,
            persona_order: _,
            order_reason: _,
        } => Some(AgentEvent::DebateRoundStarted {
            round: u32::from(round),
            total_rounds: 0, // not known at this point, relay ignores it
            phase: phase.as_str().to_string(),
        }),
        DebateEvent::PersonaPerspective {
            persona_id,
            name,
            icon,
            role,
            content,
            round: _,
            challenge,
        } => Some(AgentEvent::PersonaPerspective {
            persona_id,
            persona_name: name,
            persona_icon: icon,
            persona_role: role,
            content,
            challenge,
        }),
        DebateEvent::JudgeDecision {
            round,
            consensus_score,
            speaking_order,
            decision,
        } => Some(AgentEvent::DebateJudgeDecision {
            round: u32::from(round),
            consensus_score,
            decision,
            speaking_order,
            reasoning: String::new(),
        }),
        DebateEvent::RoundCompleted { round, phase: _ } => Some(AgentEvent::DebateRoundCompleted {
            round: u32::from(round),
            consensus_score: 0.0,
        }),
        DebateEvent::ConsensusReached { round, score } => Some(AgentEvent::ConsensusReached {
            round: u32::from(round),
            consensus_score: score,
            summary: String::new(),
        }),
        DebateEvent::SynthesisChunk { content } => Some(AgentEvent::ContentChunk { data: content }),
        // BudgetCheck, SynthesisStarted, DebatePartial, Complete — not mapped to AgentEvent
        _ => None,
    }
}

/// Background task for squad message execution. Takes individual pieces instead of
/// `&AppCore` since this runs in a spawned task.
#[allow(clippy::too_many_arguments)]
async fn execute_squad_message_bg(
    repos: &Repos,
    storage_pool: &storage::StoragePool,
    squad_repo: Option<&cognitive::SquadRepo>,
    cognitive_provider: Option<&providers::DynProvider>,
    domain_event_bus: Option<&Arc<bus::DomainEventBus>>,
    agent: &Arc<agent::AgentLoop>,
    content: &str,
    session_key: &str,
    squad_id: &str,
    event_tx: mpsc::Sender<AgentEvent>,
) -> Result<(), ApiError> {
    // 1. Resolve the squad
    let squad_repo =
        squad_repo.ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Squad repo not available"))?;

    let resolved = squad_repo
        .resolve_squad(squad_id)
        .await
        .map_err(|e| ApiError::new("STORAGE", format!("Failed to resolve squad: {e}")))?
        .ok_or_else(|| ApiError::new("NOT_FOUND", format!("Squad '{squad_id}' not found")))?;

    if resolved.personas.len() < 2 {
        return Err(ApiError::new(
            "NOT_ENOUGH_PERSONAS",
            "Need at least 2 personas for a squad interaction",
        ));
    }

    // 2. Get provider
    let provider = cognitive_provider
        .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

    // 3. Detect interaction mode
    let default_mode = DefaultInteractionMode::from_str(&resolved.squad.default_interaction_mode);
    let mode = detect_interaction_mode(content, &resolved.personas, default_mode);

    // 4. Load orchestrator skill body for domain grounding
    let skill_prompt = {
        let catalog_arc = agent.skill_catalog();
        let catalog = catalog_arc.read().await;
        catalog
            .get(&resolved.squad.orchestrator_skill)
            .map(|pkg| pkg.body.clone())
            .unwrap_or_default()
    };

    // 5. Load conversation history
    let history_rows = repos
        .sessions
        .get_recent_messages(session_key, 20)
        .await
        .unwrap_or_default();
    let conversation_history = session_rows_to_messages(&history_rows);

    // 6. Parse squad domains
    let domains: Vec<String> = serde_json::from_str(&resolved.squad.domains)
        .unwrap_or_else(|_| vec!["general".to_string()]);

    match mode {
        SquadInteractionMode::Debate => {
            execute_squad_debate(
                repos,
                storage_pool,
                domain_event_bus,
                content,
                session_key,
                squad_id,
                &resolved,
                provider,
                &skill_prompt,
                conversation_history,
                domains,
                event_tx,
            )
            .await
        }
        SquadInteractionMode::DirectAddress { persona_id } => {
            execute_direct_address(
                repos,
                domain_event_bus,
                content,
                session_key,
                squad_id,
                &resolved,
                provider,
                &skill_prompt,
                conversation_history,
                &persona_id,
                "direct",
                event_tx,
            )
            .await
        }
        SquadInteractionMode::LeadResponse => {
            // Find the lead persona (role_in_squad == "lead")
            let lead_persona_id = resolved
                .members
                .iter()
                .find(|m| m.role_in_squad == "lead")
                .map(|m| m.persona_id.clone())
                .unwrap_or_else(|| {
                    // Fallback to first persona
                    resolved
                        .personas
                        .first()
                        .map(|p| p.id.clone())
                        .unwrap_or_default()
                });

            execute_direct_address(
                repos,
                domain_event_bus,
                content,
                session_key,
                squad_id,
                &resolved,
                provider,
                &skill_prompt,
                conversation_history,
                &lead_persona_id,
                "lead",
                event_tx,
            )
            .await
        }
    }
}

/// Run a full debate with the squad and stream events.
#[allow(clippy::too_many_arguments)]
async fn execute_squad_debate(
    repos: &Repos,
    storage_pool: &storage::StoragePool,
    domain_event_bus: Option<&Arc<bus::DomainEventBus>>,
    content: &str,
    session_key: &str,
    squad_id: &str,
    resolved: &ResolvedSquad,
    provider: &providers::DynProvider,
    skill_prompt: &str,
    conversation_history: Vec<Message>,
    domains: Vec<String>,
    agent_event_tx: mpsc::Sender<AgentEvent>,
) -> Result<(), ApiError> {
    let config = DebateConfig::for_chat();
    let blackboard_repo = BlackboardRepo::new(storage_pool.inner().clone());
    let accuracy_repo = PersonaAccuracyRepo::new(storage_pool.inner().clone());
    let cancel = CancellationToken::new();

    // Load persistent session blackboard (prior direct/lead exchanges in this thread)
    let persistent_key = format!("session:{session_key}:{squad_id}");
    let prior_entries = blackboard_repo
        .list_for_session(&persistent_key)
        .await
        .unwrap_or_default();
    let cognitive_context = if prior_entries.is_empty() {
        None
    } else {
        let prior_context = BlackboardRepo::format_for_prompt(&prior_entries);
        Some(format!(
            "--- Prior squad exchanges in this thread ---\n{prior_context}\n---"
        ))
    };

    let context = DebateContext {
        skill_prompt: skill_prompt.to_string(),
        conversation_history,
        user_message: content.to_string(),
        cognitive_context,
        domains,
    };

    // Create debate event channel
    let (debate_tx, mut debate_rx) = mpsc::channel::<DebateEvent>(100);

    // Spawn relay: DebateEvent → AgentEvent
    let relay_agent_tx = agent_event_tx.clone();
    let relay_handle = tokio::spawn(async move {
        while let Some(de) = debate_rx.recv().await {
            if let Some(ae) = debate_event_to_agent_event(de) {
                if relay_agent_tx.send(ae).await.is_err() {
                    break;
                }
            }
        }
    });

    // Run the debate
    let squad_clone = ResolvedSquad {
        squad: resolved.squad.clone(),
        personas: resolved.personas.clone(),
        members: resolved.members.clone(),
    };
    let result = debate::run_debate(
        config,
        context,
        squad_clone,
        provider,
        &blackboard_repo,
        &accuracy_repo,
        debate_tx,
        None, // no approval flow for chat
        cancel,
    )
    .await
    .map_err(|e| ApiError::new("DEBATE_FAILED", e.to_string()))?;

    // Wait for relay to finish
    let _ = relay_handle.await;

    // Persist each persona's final-round response as a visible message in the thread
    let final_round = result.rounds.last().map(|r| r.round).unwrap_or(0);
    for resp in &result.persona_responses {
        if resp.round == final_round {
            let persona_msg_id = uuid::Uuid::new_v4();
            let persona_meta = serde_json::json!({
                "squad_mode": "debate",
                "squad_id": squad_id,
                "debate_round": resp.round,
                "phase": resp.phase.as_str(),
            });
            let _ = repos
                .sessions
                .add_message(
                    session_key,
                    persona_msg_id,
                    "assistant",
                    &resp.content,
                    None,
                    None,
                    Some(&persona_meta),
                    Some(&resp.persona_id),
                )
                .await;
        }
    }

    // Persist synthesis as assistant message
    let synthesis = &result.synthesis;
    if !synthesis.is_empty() {
        let msg_id = uuid::Uuid::new_v4();
        // Persist the message first (relay will add segments metadata later)
        if let Err(e) = repos
            .sessions
            .add_message(
                session_key,
                msg_id,
                "assistant",
                synthesis,
                None,
                None,
                None,
                None, // persona_id — synthesis is from the whole squad
            )
            .await
        {
            tracing::warn!("failed to persist debate synthesis for {session_key}: {e}");
        }

        // Emit Done — relay_chat_stream will add segments metadata
        let _ = agent_event_tx
            .send(AgentEvent::Done {
                content: synthesis.clone(),
                message_id: Some(msg_id.to_string()),
            })
            .await;

        // Wait briefly for relay to finish persisting segments, then merge debate metadata
        let debate_meta_repos = repos.clone();
        let debate_msg_id = msg_id.to_string();
        let debate_rounds = result.rounds_completed;
        let is_partial = result.partial_reason.is_some();
        let consensus = result.final_consensus_score;
        let transcript_json = serde_json::to_value(&result.rounds).ok();
        let weights_json = serde_json::to_value(&result.learned_weights_applied).ok();
        let debate_squad_id = squad_id.to_string();
        tokio::spawn(async move {
            // Wait for relay_chat_stream to persist segments metadata first
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;

            // Read-modify-write: merge debate fields into existing metadata
            let existing = debate_meta_repos
                .sessions
                .get_message_metadata_by_id(&debate_msg_id)
                .await
                .ok()
                .flatten()
                .unwrap_or_else(|| serde_json::json!({}));

            let mut merged = match existing {
                serde_json::Value::Object(map) => map,
                _ => serde_json::Map::new(),
            };

            merged.insert("squad_mode".into(), serde_json::json!("debate"));
            merged.insert("squad_id".into(), serde_json::json!(debate_squad_id));
            merged.insert("debate_rounds".into(), serde_json::json!(debate_rounds));
            merged.insert("partial".into(), serde_json::json!(is_partial));
            merged.insert("consensus_score".into(), serde_json::json!(consensus));
            if let Some(t) = transcript_json {
                merged.insert("debate_transcript".into(), t);
            }
            if let Some(w) = weights_json {
                merged.insert("learned_weights".into(), w);
            }

            let merged_value = serde_json::Value::Object(merged);
            if let Err(e) = debate_meta_repos
                .sessions
                .update_assistant_metadata_by_id(&debate_msg_id, None, Some(&merged_value))
                .await
            {
                tracing::warn!("failed to persist debate metadata for {debate_msg_id}: {e}");
            }
        });
    } else {
        // Empty synthesis (e.g. cancelled)
        let _ = agent_event_tx
            .send(AgentEvent::Done {
                content: String::new(),
                message_id: None,
            })
            .await;
    }

    // Emit domain event
    emit_debate_completed_event(domain_event_bus, squad_id, session_key, &result);

    // Note: accuracy outcomes are already persisted by `run_debate` (Step 6).
    // The `AccuracyOutcome` structs in the result are receipts, not instructions to write.

    Ok(())
}

/// Execute a direct persona address (or lead response) — single persona LLM call.
#[allow(clippy::too_many_arguments)]
async fn execute_direct_address(
    repos: &Repos,
    domain_event_bus: Option<&Arc<bus::DomainEventBus>>,
    content: &str,
    session_key: &str,
    squad_id: &str,
    resolved: &ResolvedSquad,
    provider: &providers::DynProvider,
    skill_prompt: &str,
    conversation_history: Vec<Message>,
    persona_id: &str,
    mode_label: &str,
    agent_event_tx: mpsc::Sender<AgentEvent>,
) -> Result<(), ApiError> {
    // Find the target persona
    let persona = resolved
        .personas
        .iter()
        .find(|p| p.id == persona_id)
        .ok_or_else(|| {
            ApiError::new(
                "NOT_FOUND",
                format!("Persona '{persona_id}' not found in squad"),
            )
        })?;

    // Build persona system prompt
    let system_prompt = debate::build_persona_system_prompt(skill_prompt, persona, None);

    // Build messages: system + history + user
    let mut messages = vec![Message::system(&system_prompt)];
    messages.extend(conversation_history);
    messages.push(Message::user(content));

    // Call provider
    let params = providers::ChatParams::new(provider.default_model());
    let response = provider
        .chat(&messages, None, &params)
        .await
        .map_err(|e| ApiError::new("LLM_FAILED", e.to_string()))?;

    let response_content = response.content.unwrap_or_default();

    // Emit PersonaPerspective so the UI knows which persona responded
    let _ = agent_event_tx
        .send(AgentEvent::PersonaPerspective {
            persona_id: persona.id.clone(),
            persona_name: persona.name.clone(),
            persona_icon: persona.icon.clone(),
            persona_role: persona.role.clone(),
            content: response_content.clone(),
            challenge: None,
        })
        .await;

    // Persist as assistant message with persona_id
    let msg_id = uuid::Uuid::new_v4();
    let metadata = serde_json::json!({
        "squad_mode": mode_label,
        "squad_id": squad_id,
        "persona_id": persona.id,
        "persona_name": persona.name,
    });
    if let Err(e) = repos
        .sessions
        .add_message(
            session_key,
            msg_id,
            "assistant",
            &response_content,
            None,
            None,
            Some(&metadata),
            Some(&persona.id),
        )
        .await
    {
        tracing::warn!("failed to persist direct address message for {session_key}: {e}");
    }

    // Write to persistent session blackboard so future debates see this exchange
    let blackboard_session_key = format!("session:{session_key}:{squad_id}");
    let blackboard_repo = BlackboardRepo::new(repos.pool().clone());
    let _ = blackboard_repo
        .insert(&cognitive::NewBlackboardEntry {
            session_key: &blackboard_session_key,
            squad_id,
            round: 0,
            persona_id: &persona.id,
            persona_name: &persona.name,
            entry_type: mode_label,
            content: &response_content,
            confidence: 0.8,
            references_entry_id: None,
        })
        .await;

    // Emit Done
    let _ = agent_event_tx
        .send(AgentEvent::Done {
            content: response_content,
            message_id: Some(msg_id.to_string()),
        })
        .await;

    // Emit domain event for interaction pattern tracking
    if let Some(bus) = domain_event_bus {
        bus.publish(bus::DomainEvent::SquadInteractionPattern {
            squad_id: squad_id.to_string(),
            mode: mode_label.to_string(),
            persona_id: Some(persona.id.clone()),
            domain_hint: None,
        });
    }

    Ok(())
}

/// Emit `SquadDebateCompleted` domain event after a successful debate.
fn emit_debate_completed_event(
    domain_event_bus: Option<&Arc<bus::DomainEventBus>>,
    squad_id: &str,
    session_key: &str,
    result: &DebateResult,
) {
    if let Some(bus) = domain_event_bus {
        let persona_accuracies: Vec<(String, f64)> = result
            .accuracy_outcomes
            .iter()
            .map(|a| (a.persona_id.clone(), a.consensus_alignment))
            .collect();
        let avg_score = if persona_accuracies.is_empty() {
            0.0
        } else {
            persona_accuracies.iter().map(|(_, s)| s).sum::<f64>() / persona_accuracies.len() as f64
        };
        let top_performer = persona_accuracies
            .iter()
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(id, _)| id.clone());

        bus.publish(bus::DomainEvent::SquadDebateCompleted {
            squad_id: squad_id.to_string(),
            session_key: session_key.to_string(),
            rounds_completed: result.rounds_completed,
            consensus_score: result.final_consensus_score,
            persona_accuracies,
            was_partial: result.partial_reason.is_some(),
            token_cost: result.token_usage.total,
            average_consensus_score: avg_score,
            top_performer_persona_id: top_performer,
        });
    }
}

// ── Public free functions ────────────────────────────────────────────────

pub async fn chat_send(
    repos: &Repos,
    agent: &Arc<agent::AgentLoop>,
    active_streams: &ActiveStreams,
    content: String,
    session_key: String,
    context: Option<SessionContextInput>,
    is_voice: bool,
) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
    // 1. Ensure session exists (title derived from first message, truncated to 60 chars)
    let title: String = content
        .chars()
        .take(60)
        .collect::<String>()
        .trim()
        .to_string();
    let metadata = if is_voice {
        serde_json::json!({ "title": title, "is_voice_session": true })
    } else {
        serde_json::json!({ "title": title })
    };
    let squad_id_ref = context.as_ref().and_then(|c| c.squad_id.as_deref());
    let session_row = repos
        .sessions
        .upsert_session(&session_key, &metadata, squad_id_ref)
        .await
        .map_err(map_storage_err)?;
    let is_new_session = session_row.created_at == session_row.updated_at;

    // 2. Upsert session_context if provided
    let has_context = context.is_some();
    if let Some(ctx) = &context {
        repos
            .session_context
            .upsert(SessionContextParams {
                session_key: &session_key,
                context_type: ctx.context_type.as_deref().unwrap_or("general"),
                entity_kind: ctx.entity_kind.as_deref(),
                entity_id: ctx.entity_id.as_deref(),
                area_id: None,    // area_id resolved later
                project_id: None, // project_id resolved later
                is_ephemeral: ctx.is_ephemeral.unwrap_or(false),
            })
            .await
            .map_err(map_storage_err)?;
    }

    // 3. Call agent with streaming (agent loop stores user + assistant messages)
    let msg_id = uuid::Uuid::new_v4();
    let now = chrono::Utc::now();
    let streaming_handle = agent
        .process_direct_streaming(content.clone(), session_key.clone())
        .await
        .map_err(ApiError::from)?;

    // 4. Track the cancel token
    active_streams.insert(session_key.clone(), streaming_handle.cancel_token);

    // 5. Build the user message response
    let user_msg = ChatMessageResponse {
        id: msg_id.to_string(),
        role: common::MessageRole::User.to_string(),
        content,
        timestamp: now,
        segments: None,
        transparency: None,
        persona_id: None,
        persona_name: None,
    };

    // 6. Build stream info for the caller to wire up
    let stream_info = ChatStreamInfo {
        session_key,
        event_rx: streaming_handle.event_rx,
        interaction_rx: streaming_handle.interaction_rx,
        has_context,
        is_new_session,
    };

    Ok((user_msg, stream_info))
}

pub async fn chat_cancel(
    active_streams: &ActiveStreams,
    pending_interactions: &PendingInteractions,
    session_key: String,
) -> Result<(), ApiError> {
    // Cancel stream
    if let Some((_, token)) = active_streams.remove(&session_key) {
        token.cancel();
    }
    // Cancel any pending interaction
    if let Some((_, (_, tx))) = pending_interactions.remove(&session_key) {
        let _ = tx.send(common::FormResponse::Cancelled);
    }
    Ok(())
}

pub async fn chat_respond_interaction(
    repos: &Repos,
    pending_interactions: &PendingInteractions,
    emitter: &dyn crate::events::AppEventEmitter,
    session_key: String,
    request_id: String,
    response: common::FormResponse,
) -> Result<(), ApiError> {
    // 1. Find and remove the pending oneshot sender, validating request_id
    let (_, (stored_id, sender)) = pending_interactions
        .remove(&session_key)
        .ok_or_else(|| ApiError::new("NOT_FOUND", "no pending interaction for this session"))?;
    if stored_id != request_id {
        return Err(ApiError::new(
            "INVALID_PARAMS",
            format!("request_id mismatch: expected {stored_id}, got {request_id}"),
        ));
    }

    // 2. Build collapsed summary text for the interaction message
    let summary = format_interaction_summary(&response);

    // 3. Send the response through the oneshot to unblock the agent tool immediately
    let _ = sender.send(response);

    // 4. Persist as a synthetic "interaction" message (agent already unblocked)
    let msg_id = uuid::Uuid::new_v4();
    let metadata = serde_json::json!({
        "type": "interaction_response",
        "requestId": request_id,
    });
    if let Err(e) = repos
        .sessions
        .add_message(
            &session_key,
            msg_id,
            "interaction",
            &summary,
            None,
            None,
            Some(&metadata),
            None, // persona_id
        )
        .await
    {
        tracing::warn!("failed to persist interaction message for {session_key}: {e}");
    }

    if let Ok(payload) = serde_json::to_value(events::ChatMessagePayload {
        session_key,
        source: "chat".to_string(),
    }) {
        emitter.emit_event(events::CHAT_MESSAGE_ADDED, payload);
    }

    Ok(())
}

// ── Streaming relay ─────────────────────────────────────────────────────

/// Relay agent streaming events to a transport-agnostic event emitter.
///
/// This contains the entire streaming loop extracted from the desktop
/// `chat_send` command. The caller is responsible for spawning this as a
/// background task and providing the appropriate emitter implementation.
#[allow(clippy::too_many_arguments)]
pub async fn relay_chat_stream(
    repos: Repos,
    session_key: String,
    active_streams: Arc<ActiveStreams>,
    pending_interactions: Arc<PendingInteractions>,
    mut event_rx: mpsc::Receiver<AgentEvent>,
    mut interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    emitter: Arc<dyn crate::events::AppEventEmitter>,
    has_context: bool,
    journey_tracker: Option<crate::journey::JourneyTracker>,
) {
    // Guard ensures active_streams cleanup even on panic
    struct StreamGuard {
        key: String,
        streams: Arc<ActiveStreams>,
    }
    impl Drop for StreamGuard {
        fn drop(&mut self) {
            self.streams.remove(&self.key);
        }
    }
    let _guard = StreamGuard {
        key: session_key.clone(),
        streams: Arc::clone(&active_streams),
    };

    let sk = &session_key;

    // Collect signals for auto-detection
    let mut tool_names: Vec<String> = Vec::with_capacity(4);
    let mut entity_cards: Vec<EntityCard> = Vec::new();

    // Pending action names: ToolStart stashes the action extracted from args,
    // ToolEnd pops it to build the qualified name (e.g. "task:list").
    let mut pending_actions: HashMap<String, VecDeque<String>> = HashMap::new();
    let mut tool_token_sum: u32 = 0;

    // Segment accumulation for structured message persistence
    let mut segments: Vec<events::MessageSegment> = Vec::with_capacity(8);
    let mut current_text = String::new();

    // Transparency data accumulation
    let mut transparency = TransparencyData::default();

    // Flush accumulated text into a finalized text segment.
    let flush_text = |text: &mut String, segs: &mut Vec<events::MessageSegment>| {
        if !text.is_empty() {
            segs.push(events::MessageSegment::Text {
                content: std::mem::take(text),
            });
        }
    };

    // Helper to emit via the emitter (serializes payload to JSON value)
    macro_rules! emit {
        ($event:expr, $payload:expr) => {
            if let Ok(val) = serde_json::to_value(&$payload) {
                emitter.emit_event($event, val);
            }
        };
    }

    loop {
        tokio::select! {
            biased;
            Some(bundle) = interaction_rx.recv() => {
                let request_id = uuid::Uuid::new_v4().to_string();
                emit!(
                    AGENT_INTERACTION_REQUEST,
                    InteractionRequestPayload {
                        session_key: sk.clone(),
                        request_id: request_id.clone(),
                        request: bundle.request,
                    }
                );
                pending_interactions.insert(sk.clone(), (request_id, bundle.response_tx));
            }
            Some(event) = event_rx.recv() => {
                match event {
                    AgentEvent::ContentChunk { data } => {
                        current_text.push_str(&data);
                        emit!(
                            AGENT_CONTENT_CHUNK,
                            ContentChunkPayload {
                                session_key: sk.clone(),
                                data,
                            }
                        );
                    }
                    AgentEvent::ToolStart { name, args, agent } => {
                        flush_text(&mut current_text, &mut segments);
                        tool_names.push(name.clone());
                        let action = args.get("action").and_then(|v| v.as_str()).map(String::from);
                        if let Some(ref a) = action {
                            pending_actions.entry(name.clone()).or_default().push_back(a.clone());
                        }
                        emit!(
                            AGENT_TOOL_START,
                            ToolStartPayload {
                                session_key: sk.clone(),
                                name,
                                action,
                                agent,
                            }
                        );
                    }
                    AgentEvent::ToolEnd { name, success, duration_ms, result, agent } => {
                        // Pop the stashed action from ToolStart (FIFO per tool name)
                        let action = match pending_actions.entry(name.clone()) {
                            std::collections::hash_map::Entry::Occupied(mut e) => {
                                let a = e.get_mut().pop_front();
                                if e.get().is_empty() { e.remove(); }
                                a
                            }
                            std::collections::hash_map::Entry::Vacant(_) => None,
                        };
                        // Estimate tokens from result length (~4 chars per token)
                        let estimated_tokens = result.as_ref().map(|r| (r.len() as u32).saturating_add(3) / 4);
                        if let Some(t) = estimated_tokens { tool_token_sum += t; }
                        segments.push(events::MessageSegment::Tool {
                            name: name.clone(),
                            action: action.clone(),
                            success,
                            duration_ms,
                            result: result.clone(),
                            estimated_tokens,
                            agent: agent.clone(),
                        });
                        transparency.tools.push(TransparencyTool {
                            name: name.clone(),
                            action: action.clone(),
                            success,
                            duration_ms,
                            estimated_tokens,
                            agent: agent.clone(),
                        });
                        // Emit entity:updated so the UI refreshes affected lists
                        if success && is_mutating_action(action.as_deref()) {
                            if let Some(kind) = entity_kind_for_tool(&name) {
                                let payload = serde_json::json!({
                                    "entityKind": kind,
                                    "id": ""
                                });
                                emitter.emit_event(ENTITY_UPDATED, payload);
                            }
                        }
                        emit!(
                            AGENT_TOOL_END,
                            ToolEndPayload {
                                session_key: sk.clone(),
                                name,
                                action,
                                success,
                                duration_ms,
                                result,
                                estimated_tokens,
                                agent,
                            }
                        );
                    }
                    AgentEvent::EntityCreated(card) => {
                        emit!(
                            AGENT_ENTITY_CREATED,
                            EntityCreatedPayload {
                                session_key: sk.clone(),
                                entity_type: card.entity_type.clone(),
                                entity_id: card.entity_id.clone(),
                            }
                        );
                        if let Some(kind) = desktop_shared::types::EntityKind::parse(&card.entity_type)
                        {
                            let payload = serde_json::json!({
                                "entityKind": kind,
                                "id": card.entity_id
                            });
                            emitter.emit_event(ENTITY_UPDATED, payload);
                        }
                        entity_cards.push(card);
                    }
                    AgentEvent::Done { content, message_id } => {
                        flush_text(&mut current_text, &mut segments);
                        if tool_token_sum > 0 {
                            transparency.tool_tokens_total = Some(tool_token_sum);
                        }
                        // Persist segments + transparency to the assistant message metadata.
                        // Use targeted update by message ID when available to avoid
                        // overwriting the wrong message in multi-turn conversations.
                        let mut meta = serde_json::Map::new();
                        if !segments.is_empty() {
                            meta.insert(
                                "segments".to_string(),
                                serde_json::to_value(&segments).unwrap_or_default(),
                            );
                        }
                        meta.insert(
                            "transparency".to_string(),
                            serde_json::to_value(&transparency).unwrap_or_default(),
                        );
                        let meta_value = serde_json::Value::Object(meta);
                        if let Some(ref mid) = message_id {
                            if let Err(e) = repos.sessions.update_assistant_metadata_by_id(
                                mid, None, Some(&meta_value),
                            ).await {
                                tracing::warn!("metadata persist by id failed for {sk}/{mid}: {e}");
                            }
                        } else {
                            // Fallback: legacy path for messages without ID
                            match repos.sessions.update_last_assistant_metadata(
                                sk, None, Some(&meta_value),
                            ).await {
                                Ok(true) => {}
                                Ok(false) => {
                                    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
                                    match repos.sessions.update_last_assistant_metadata(
                                        sk, None, Some(&meta_value),
                                    ).await {
                                        Ok(false) => tracing::warn!("metadata persist: no assistant message found for {sk}"),
                                        Err(e) => tracing::warn!("metadata persist retry failed for {sk}: {e}"),
                                        _ => {}
                                    }
                                }
                                Err(e) => tracing::warn!("metadata persist failed for {sk}: {e}"),
                            }
                        }
                        // Wire: FirstChatResponse journey milestone
                        if let Some(ref tracker) = journey_tracker {
                            if !tracker
                                .is_complete(crate::journey::Milestone::FirstChatResponse)
                                .await
                            {
                                tracker
                                    .mark_complete(
                                        crate::journey::Milestone::FirstChatResponse,
                                    )
                                    .await;
                            }
                        }

                        emit!(
                            AGENT_DONE,
                            DonePayload {
                                session_key: sk.clone(),
                                content,
                            }
                        );
                        emit!(
                            CHAT_MESSAGE_ADDED,
                            ChatMessagePayload {
                                session_key: sk.clone(),
                                source: "chat".to_string(),
                            }
                        );
                        break;
                    }
                    AgentEvent::Error { message } => {
                        emit!(
                            AGENT_ERROR,
                            AgentErrorPayload {
                                session_key: sk.clone(),
                                message,
                            }
                        );
                        break;
                    }
                    AgentEvent::ClassificationComplete { strategy, confidence, source, duration_ms } => {
                        transparency.classification = Some(TransparencyClassification {
                            strategy: strategy.clone(),
                            confidence,
                            source: source.clone(),
                        });
                        transparency.timing.get_or_insert_with(Default::default).classification_ms = Some(duration_ms);
                        emit!(
                            AGENT_CLASSIFICATION_COMPLETE,
                            ClassificationCompletePayload {
                                session_key: sk.clone(),
                                strategy,
                                confidence,
                                source,
                            }
                        );
                    }
                    AgentEvent::ExecutionStarted { engine, max_iterations } => {
                        transparency.execution = Some(TransparencyExecution {
                            engine: engine.clone(),
                            iterations: 0,
                            max_iterations: max_iterations as u32,
                            escalations: 0,
                        });
                        emit!(
                            AGENT_EXECUTION_STARTED,
                            ExecutionStartedPayload {
                                session_key: sk.clone(),
                                engine,
                                max_iterations,
                            }
                        );
                    }
                    AgentEvent::PipelineStarted => {
                        emit!(
                            AGENT_PIPELINE_STARTED,
                            PipelineStartedPayload {
                                session_key: sk.clone(),
                            }
                        );
                    }
                    AgentEvent::ContextAssembled { total_tokens, budget: _, duration_ms } => {
                        transparency.timing.get_or_insert_with(Default::default).context_assembly_ms = Some(duration_ms);
                        emit!(
                            AGENT_CONTEXT_ASSEMBLED,
                            ContextAssembledPayload {
                                session_key: sk.clone(),
                                total_tokens,
                                duration_ms,
                            }
                        );
                    }
                    AgentEvent::IterationStart { iteration, max } => {
                        if let Some(ref mut exec) = transparency.execution {
                            exec.iterations = iteration as u32;
                        }
                        emit!(
                            events::AGENT_ITERATION_START,
                            events::IterationStartPayload {
                                session_key: sk.clone(),
                                iteration,
                                max_iterations: max,
                            }
                        );
                    }
                    AgentEvent::ConfidenceAssessed { score, action } => {
                        emit!(
                            events::AGENT_CONFIDENCE_ASSESSED,
                            events::ConfidenceAssessedPayload {
                                session_key: sk.clone(),
                                score,
                                action,
                            }
                        );
                    }
                    AgentEvent::UsageReport {
                        prompt_tokens, completion_tokens,
                        cache_read_tokens, cache_write_tokens,
                        estimated_cost_usd, model, response_time_ms,
                    } => {
                        transparency.usage = Some(TransparencyUsage {
                            prompt_tokens,
                            completion_tokens,
                            cache_read_tokens,
                            cache_write_tokens,
                        });
                        transparency.cost = Some(TransparencyCost {
                            estimated_usd: estimated_cost_usd,
                            model: model.clone(),
                        });
                        transparency.timing.get_or_insert_with(Default::default).total_ms = response_time_ms;
                        emit!(
                            events::AGENT_USAGE_REPORT,
                            events::UsageReportPayload {
                                session_key: sk.clone(),
                                prompt_tokens,
                                completion_tokens,
                                cache_read_tokens,
                                cache_write_tokens,
                                estimated_cost_usd,
                                model,
                                response_time_ms,
                            }
                        );
                    }
                    AgentEvent::MemoryAccess { action, query, results_count } => {
                        transparency.memory_accesses.push(TransparencyMemoryAccess {
                            action: action.clone(),
                            query: query.clone(),
                            results_count,
                        });
                        emit!(
                            events::AGENT_MEMORY_ACCESS,
                            events::MemoryAccessPayload {
                                session_key: sk.clone(),
                                action,
                                query,
                                results_count,
                            }
                        );
                    }
                    AgentEvent::SkillLoaded { name, trigger, agent } => {
                        transparency.skills.push(TransparencySkill {
                            name: name.clone(),
                            trigger: trigger.clone(),
                            agent: agent.clone(),
                        });
                        emit!(
                            events::AGENT_SKILL_LOADED,
                            events::SkillLoadedPayload {
                                session_key: sk.clone(),
                                name,
                                trigger,
                                agent,
                            }
                        );
                    }
                    AgentEvent::LearningEvent { event_type, detail } => {
                        transparency.learning.push(TransparencyLearning {
                            event_type: event_type.clone(),
                            detail: detail.clone(),
                        });
                        emit!(
                            events::AGENT_LEARNING_EVENT,
                            events::LearningEventPayload {
                                session_key: sk.clone(),
                                event_type,
                                detail,
                            }
                        );
                    }
                    AgentEvent::AgentSelected { name, description } => {
                        transparency.agent_selected = Some(TransparencyAgentSelected {
                            name: name.clone(),
                            description: description.clone(),
                        });
                        emit!(
                            events::AGENT_SELECTED,
                            events::AgentSelectedPayload {
                                session_key: sk.clone(),
                                name,
                                description,
                            }
                        );
                    }
                    AgentEvent::SubagentSpawned { label, profile } => {
                        transparency.subagents.push(TransparencySubagent {
                            label: label.clone(),
                            profile: profile.clone(),
                        });
                        emit!(
                            events::AGENT_SUBAGENT_SPAWNED,
                            events::SubagentSpawnedPayload {
                                session_key: sk.clone(),
                                label,
                                profile,
                            }
                        );
                    }
                    AgentEvent::DelegationStarted { from_agent, to_agent, query, depth } => {
                        transparency.delegations.push(TransparencyDelegation {
                            from_agent: from_agent.clone(),
                            to_agent: to_agent.clone(),
                            query: query.clone(),
                            depth,
                            status: "active".to_string(),
                            duration_ms: None,
                        });
                        emit!(
                            events::AGENT_DELEGATION_STARTED,
                            events::DelegationStartedPayload {
                                session_key: sk.clone(),
                                from_agent,
                                to_agent,
                                query,
                                depth,
                            }
                        );
                    }
                    AgentEvent::DelegationCompleted { from_agent, to_agent, success, duration_ms } => {
                        // Update the matching delegation entry
                        if let Some(d) = transparency.delegations.iter_mut().find(|d| d.to_agent == to_agent && d.status == "active") {
                            d.status = if success { "completed".to_string() } else { "failed".to_string() };
                            d.duration_ms = Some(duration_ms);
                        }
                        emit!(
                            events::AGENT_DELEGATION_COMPLETED,
                            events::DelegationCompletedPayload {
                                session_key: sk.clone(),
                                from_agent,
                                to_agent,
                                success,
                                duration_ms,
                            }
                        );
                    }
                    AgentEvent::McpServerStatus { server_name, status, tool_count, error } => {
                        emit!(
                            events::MCP_SERVER_STATUS,
                            events::McpServerStatusPayload {
                                server_name,
                                status,
                                tool_count,
                                error,
                            }
                        );
                    }
                    AgentEvent::McpStartupComplete { ready, failed, skipped } => {
                        emit!(
                            events::MCP_STARTUP_COMPLETE,
                            events::McpStartupCompletePayload {
                                ready,
                                failed,
                                skipped,
                            }
                        );
                    }
                    AgentEvent::PlanningStarted { .. } => {}
                    AgentEvent::PlanGenerated { steps, raw_plan } => {
                        transparency.plan = Some(events::TransparencyPlan {
                            steps: steps.clone(),
                            completed_steps: Vec::new(),
                        });
                        emit!(
                            events::AGENT_PLAN_GENERATED,
                            events::PlanGeneratedPayload {
                                session_key: sk.to_string(),
                                steps,
                                raw_plan,
                            }
                        );
                    }
                    AgentEvent::PlanStepCompleted { step_index, description, tool_name } => {
                        if let Some(ref mut plan) = transparency.plan {
                            plan.completed_steps.push(step_index);
                        }
                        emit!(
                            events::AGENT_PLAN_STEP_COMPLETED,
                            events::PlanStepCompletedPayload {
                                session_key: sk.to_string(),
                                step_index,
                                description,
                                tool_name,
                            }
                        );
                    }
                    AgentEvent::BudgetWarning { monthly_spend_usd, monthly_budget_usd, usage_percent } => {
                        emit!(
                            events::AGENT_BUDGET_WARNING,
                            events::BudgetWarningPayload {
                                session_key: sk.to_string(),
                                monthly_spend_usd,
                                monthly_budget_usd,
                                usage_percent,
                            }
                        );
                    }
                    AgentEvent::PersonaPerspective { persona_id, persona_name, persona_icon, persona_role, content, challenge } => {
                        emit!(
                            events::AGENT_PERSONA_PERSPECTIVE,
                            events::PersonaPerspectivePayload {
                                session_key: sk.to_string(),
                                persona_id,
                                persona_name,
                                persona_icon,
                                persona_role,
                                content,
                                challenge,
                            }
                        );
                    }
                    AgentEvent::DebateRoundStarted { round, total_rounds, phase } => {
                        emit!(
                            events::AGENT_DEBATE_ROUND_STARTED,
                            events::DebateRoundStartedPayload {
                                session_key: sk.to_string(),
                                round,
                                total_rounds,
                                phase,
                            }
                        );
                    }
                    AgentEvent::DebateJudgeDecision { round, consensus_score, decision, speaking_order, reasoning } => {
                        emit!(
                            events::AGENT_DEBATE_JUDGE_DECISION,
                            events::DebateJudgeDecisionPayload {
                                session_key: sk.to_string(),
                                round,
                                consensus_score,
                                decision,
                                speaking_order,
                                reasoning,
                            }
                        );
                    }
                    AgentEvent::DebateRoundCompleted { round, consensus_score } => {
                        emit!(
                            events::AGENT_DEBATE_ROUND_COMPLETED,
                            events::DebateRoundCompletedPayload {
                                session_key: sk.to_string(),
                                round,
                                consensus_score,
                            }
                        );
                    }
                    AgentEvent::ConsensusReached { round, consensus_score, summary } => {
                        emit!(
                            events::AGENT_CONSENSUS_REACHED,
                            events::ConsensusReachedPayload {
                                session_key: sk.to_string(),
                                round,
                                consensus_score,
                                summary,
                            }
                        );
                    }
                    AgentEvent::MemoryPromoted { fact_id, from_scope, to_scope, subject, predicate } => {
                        emit!(
                            events::AGENT_MEMORY_PROMOTED,
                            events::MemoryPromotedPayload {
                                session_key: sk.to_string(),
                                fact_id,
                                from_scope,
                                to_scope,
                                subject,
                                predicate,
                            }
                        );
                    }
                    // AutoTuner events — forwarded to the UI for toast notifications and panel updates.
                    AgentEvent::AutoTunerReport(report) => {
                        emit!(events::AUTOTUNER_REPORT, report);
                    }
                    AgentEvent::AutoTunerPromotion(promotion) => {
                        emit!(events::AUTOTUNER_PROMOTION, promotion);
                    }
                    AgentEvent::AutoTunerRollback(rollback) => {
                        emit!(events::AUTOTUNER_ROLLBACK, rollback);
                    }
                    AgentEvent::ContextCompressed { before_tokens, after_tokens, iteration } => {
                        tracing::info!(
                            before_tokens,
                            after_tokens,
                            iteration,
                            "mid-loop context compression applied"
                        );
                    }
                    AgentEvent::LoopDetected { iteration, tools_summary, suggestion } => {
                        tracing::info!(
                            iteration,
                            tools_summary = %tools_summary,
                            suggestion = %suggestion,
                            "loop detected: repeating tool pattern"
                        );
                    }
                    AgentEvent::LoopHardStop { iteration, tools_summary } => {
                        tracing::warn!(
                            iteration,
                            tools_summary = %tools_summary,
                            "loop hard-stop: forcing synthesis"
                        );
                    }
                    AgentEvent::ContextReassembled { updates, tokens_added } => {
                        tracing::info!(
                            updates_count = updates.len(),
                            tokens_added,
                            "live context reassembled during execution"
                        );
                    }
                }
            }
            else => break,
        }
    }

    // Auto-detect session context from tool usage (only if not already set)
    if !has_context {
        if let Err(e) = auto_detect_context(&repos, sk, &tool_names, &entity_cards).await {
            tracing::debug!("auto-detect context skipped for {sk}: {e}");
        }
    }
}

// ── AppCore convenience methods ─────────────────────────────────────────

impl AppCore {
    pub async fn chat_send(
        &self,
        content: String,
        session_key: String,
        context: Option<SessionContextInput>,
    ) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
        // Check if this is a squad session — if so, route to the debate engine
        // instead of the normal agent pipeline.
        let squad_id = context.as_ref().and_then(|c| c.squad_id.clone());

        // Also check if the session already has a squad_id (for follow-up messages
        // that may not re-send the context).
        let effective_squad_id = if squad_id.is_some() {
            squad_id
        } else {
            self.repos
                .sessions
                .get_session(&session_key)
                .await
                .ok()
                .and_then(|s| s.squad_id)
        };

        if let Some(ref sid) = effective_squad_id {
            return self
                .chat_send_squad(content, session_key, context, sid.clone())
                .await;
        }

        let result = chat_send(
            &self.repos,
            &self.agent,
            &self.active_streams,
            content.clone(),
            session_key.clone(),
            context,
            false,
        )
        .await?;

        self.event_emitter
            .emit_chat_thread(result.1.is_new_session, &result.1.session_key);

        // Publish chat turn to cognitive consolidation pipeline
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                user_message: content,
                session_key,
            });
        }

        Ok(result)
    }

    /// Voice-specific chat_send: no session context, `is_voice` flag set to true
    /// so the session metadata includes `"is_voice_session": true`.
    pub async fn chat_send_voice(
        &self,
        content: String,
        session_key: String,
    ) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
        let result = chat_send(
            &self.repos,
            &self.agent,
            &self.active_streams,
            content.clone(),
            session_key.clone(),
            None,
            true,
        )
        .await?;

        self.event_emitter
            .emit_chat_thread(result.1.is_new_session, &result.1.session_key);

        // Publish chat turn to cognitive consolidation pipeline
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                user_message: content,
                session_key,
            });
        }

        Ok(result)
    }

    /// Squad-specific chat_send: handles session setup and runs the squad
    /// execution engine (debate or direct address) instead of the agent pipeline.
    async fn chat_send_squad(
        &self,
        content: String,
        session_key: String,
        context: Option<SessionContextInput>,
        squad_id: String,
    ) -> Result<(ChatMessageResponse, ChatStreamInfo), ApiError> {
        // 1. Ensure session exists
        let title: String = content
            .chars()
            .take(60)
            .collect::<String>()
            .trim()
            .to_string();
        let metadata = serde_json::json!({ "title": title });
        let session_row = self
            .repos
            .sessions
            .upsert_session(&session_key, &metadata, Some(&squad_id))
            .await
            .map_err(map_storage_err)?;

        self.event_emitter.emit_chat_thread(
            session_row.created_at == session_row.updated_at,
            &session_key,
        );

        // 2. Upsert session_context if provided
        let has_context = context.is_some();
        if let Some(ctx) = &context {
            self.repos
                .session_context
                .upsert(SessionContextParams {
                    session_key: &session_key,
                    context_type: ctx.context_type.as_deref().unwrap_or("general"),
                    entity_kind: ctx.entity_kind.as_deref(),
                    entity_id: ctx.entity_id.as_deref(),
                    area_id: None,
                    project_id: None,
                    is_ephemeral: ctx.is_ephemeral.unwrap_or(false),
                })
                .await
                .map_err(map_storage_err)?;
        }

        // 3. Persist the user message
        let msg_id = uuid::Uuid::new_v4();
        let now = chrono::Utc::now();
        self.repos
            .sessions
            .add_message(
                &session_key,
                msg_id,
                "user",
                &content,
                None,
                None,
                None,
                None,
            )
            .await
            .map_err(map_storage_err)?;

        // 4. Create event channels matching the ChatStreamInfo interface
        let (event_tx, event_rx) = mpsc::channel::<AgentEvent>(64);
        let (_interaction_tx, interaction_rx) = mpsc::channel::<tools_core::InteractionBundle>(4);

        // 5. Track cancel token
        let cancel_token = CancellationToken::new();
        self.active_streams
            .insert(session_key.clone(), cancel_token.clone());

        // 6. Spawn squad execution in the background
        let core_repos = self.repos.clone();
        let core_storage = self.storage_pool.clone();
        let core_squad_repo = self.squad_repo.clone();
        let core_cognitive_provider = self.cognitive_provider.clone();
        let core_domain_event_bus = self.domain_event_bus.clone();
        let core_agent = Arc::clone(&self.agent);
        let sk = session_key.clone();
        let sid = squad_id.clone();
        let content_clone = content.clone();

        tokio::spawn(async move {
            // Build a lightweight "core-like" context for the squad execution.
            // We use a temporary AppCoreRef to avoid cloning the entire AppCore.
            let result = execute_squad_message_bg(
                &core_repos,
                &core_storage,
                core_squad_repo.as_ref(),
                core_cognitive_provider.as_ref(),
                core_domain_event_bus.as_ref(),
                &core_agent,
                &content_clone,
                &sk,
                &sid,
                event_tx.clone(),
            )
            .await;

            if let Err(e) = result {
                tracing::error!("squad execution failed for {sk}: {e}");
                let _ = event_tx
                    .send(AgentEvent::Error {
                        message: e.to_string(),
                    })
                    .await;
            }
        });

        // 7. Build user message response
        let user_msg = ChatMessageResponse {
            id: msg_id.to_string(),
            role: common::MessageRole::User.to_string(),
            content: content.clone(),
            timestamp: now,
            segments: None,
            transparency: None,
            persona_id: None,
            persona_name: None,
        };

        // 8. Build stream info
        let is_new_session = session_row.created_at == session_row.updated_at;
        let stream_info = ChatStreamInfo {
            session_key: session_key.clone(),
            event_rx,
            interaction_rx,
            has_context,
            is_new_session,
        };

        // Publish chat turn to cognitive consolidation pipeline
        if let Some(bus) = &self.domain_event_bus {
            bus.publish(bus::DomainEvent::ChatTurnCompleted {
                user_message: content,
                session_key,
            });
        }

        Ok((user_msg, stream_info))
    }

    pub async fn chat_cancel(&self, session_key: String) -> Result<(), ApiError> {
        chat_cancel(
            &self.active_streams,
            &self.pending_interactions,
            session_key,
        )
        .await
    }

    pub async fn chat_respond_interaction(
        &self,
        session_key: String,
        request_id: String,
        response: common::FormResponse,
    ) -> Result<(), ApiError> {
        chat_respond_interaction(
            &self.repos,
            &self.pending_interactions,
            self.event_emitter.as_ref(),
            session_key,
            request_id,
            response,
        )
        .await
    }

    /// Spawn the streaming relay as a background task with the given emitter.
    pub fn spawn_chat_relay(
        &self,
        stream_info: ChatStreamInfo,
        emitter: Arc<dyn crate::events::AppEventEmitter>,
    ) {
        let repos = self.repos.clone();
        let active_streams = Arc::clone(&self.active_streams);
        let pending_interactions = Arc::clone(&self.pending_interactions);
        let journey_tracker = self.journey_tracker.clone();

        tokio::spawn(relay_chat_stream(
            repos,
            stream_info.session_key,
            active_streams,
            pending_interactions,
            stream_info.event_rx,
            stream_info.interaction_rx,
            emitter,
            stream_info.has_context,
            journey_tracker,
        ));
    }
}
