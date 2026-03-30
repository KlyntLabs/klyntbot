use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use serde_json::Value;
use tokio::sync::broadcast;

use super::{err, ok, ApiResult, DevState, SseChannels, SseEmitter};
use crate::app_core::AppCore;
use crate::commands::dev_helpers as dev;
use ::app_core::events::AppEventEmitter;

/// Handle `chat_send` separately because it needs SSE channel state to relay
/// streaming agent events back to the browser via Server-Sent Events.
pub(super) async fn dispatch_chat_send(
    core: &AppCore,
    body: &Value,
    sse_channels: &SseChannels,
) -> ApiResult {
    let content = match dev::get_str(body, "content") {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let session_key = match dev::get_str(body, "sessionKey") {
        Ok(v) => v,
        Err(e) => return err(e),
    };
    let context: Option<desktop_shared::commands::SessionContextInput> = dev::get(body, "context");

    match core.chat_send(content, session_key.clone(), context).await {
        Ok((user_msg, stream_info)) => {
            let tx = sse_channels
                .entry(session_key)
                .or_insert_with(|| broadcast::channel(256).0)
                .clone();
            let emitter: Arc<dyn AppEventEmitter> = Arc::new(SseEmitter { tx });
            core.spawn_chat_relay(stream_info, emitter);
            ok(user_msg)
        }
        Err(e) => err(e),
    }
}

/// SSE endpoint — streams agent events for a chat session.
///
/// The frontend (`useAgentStream.ts`) connects here in browser dev mode
/// via `new EventSource("/api/events/{sessionKey}")`.
pub(super) async fn sse_handler(
    State(state): State<DevState>,
    Path(session_key): Path<String>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Use atomic entry API to avoid TOCTOU race with chat_send:
    // whichever handler runs first creates the channel, the other reuses it.
    let rx = state
        .sse_channels
        .entry(session_key.clone())
        .or_insert_with(|| broadcast::channel(256).0)
        .value()
        .subscribe();

    let sse_channels = Arc::clone(&state.sse_channels);
    let sk = session_key.clone();

    let stream = futures_util::stream::unfold(
        (rx, sse_channels, sk),
        |(mut rx, channels, sk)| async move {
            loop {
                match rx.recv().await {
                    Ok((event_name, payload)) => {
                        let data = serde_json::to_string(&payload).unwrap_or_default();
                        let event = Event::default().event(&event_name).data(data);
                        if event_name == "agent:done" || event_name == "agent:error" {
                            channels.remove(&sk);
                        }
                        return Some((Ok(event), (rx, channels, sk)));
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!("SSE stream for {sk} lagged by {n} events");
                        continue;
                    }
                    Err(broadcast::error::RecvError::Closed) => {
                        channels.remove(&sk);
                        return None;
                    }
                }
            }
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// SSE endpoint — streams insight review events (synthesis chunks, tab-done, etc.).
///
/// The frontend (`useInsightSSE.ts`) connects here in browser dev mode
/// via `new EventSource("/api/insight/events")`.
pub(super) async fn insight_sse_handler(
    State(state): State<DevState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.insight_tx.subscribe();

    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok((event_name, payload)) => {
                    let data = serde_json::to_string(&payload).unwrap_or_default();
                    let event = Event::default().event(&event_name).data(data);
                    return Some((Ok(event), rx));
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("insight SSE stream lagged by {n} events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return None;
                }
            }
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// SSE endpoint — streams cognitive debug events (domain events + pipeline).
///
/// The frontend (`DebugDashboard.tsx`) connects here in browser dev mode
/// via `new EventSource("/api/cognitive/stream")`.
pub(super) async fn cognitive_sse_handler(
    State(state): State<DevState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    // Subscribe to domain events
    let domain_rx = state
        .core
        .domain_event_bus
        .as_ref()
        .map(|bus| bus.subscribe());

    // Subscribe to pipeline events
    let pipeline_rx = state
        .core
        .pipeline_broadcast
        .as_ref()
        .map(|tx| tx.subscribe());

    let stream = futures_util::stream::unfold(
        (domain_rx, pipeline_rx),
        |(mut domain_rx, mut pipeline_rx)| async move {
            loop {
                tokio::select! {
                    // Domain events
                    result = async {
                        match domain_rx.as_mut() {
                            Some(rx) => rx.recv().await.map_err(|e| matches!(e, broadcast::error::RecvError::Closed)),
                            None => std::future::pending::<Result<bus::DomainEvent, bool>>().await,
                        }
                    } => {
                        match result {
                            Ok(event) => {
                                let salience = cognitive::salience::evaluate_salience(&event);
                                let domain = domain_for_event(&event);
                                let salience_str = match salience {
                                    cognitive::types::SalienceVerdict::Extract => "extract",
                                    cognitive::types::SalienceVerdict::Accumulate => "accumulate",
                                    cognitive::types::SalienceVerdict::Discard => "discard",
                                };
                                let payload = serde_json::json!({
                                    "eventType": format!("{:?}", event).split('{').next().unwrap_or("Unknown").trim(),
                                    "salience": salience_str,
                                    "domain": domain,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                    "payload": serde_json::to_value(&event).unwrap_or_default(),
                                });
                                let data = serde_json::to_string(&payload).unwrap_or_default();
                                let event = Event::default().event("cognitive:domain_event").data(data);
                                return Some((Ok(event), (domain_rx, pipeline_rx)));
                            }
                            Err(true) => return None, // closed
                            Err(false) => continue, // lagged
                        }
                    }

                    // Pipeline events
                    result = async {
                        match pipeline_rx.as_mut() {
                            Some(rx) => rx.recv().await.map_err(|e| matches!(e, broadcast::error::RecvError::Closed)),
                            None => std::future::pending::<Result<cognitive::PipelineEvent, bool>>().await,
                        }
                    } => {
                        match result {
                            Ok(pe) => {
                                let event_name = match &pe {
                                    cognitive::PipelineEvent::Extraction { .. } => "cognitive:extraction",
                                    cognitive::PipelineEvent::Consolidation { .. } => "cognitive:consolidation",
                                    _ => "cognitive:other",
                                };
                                let data = serde_json::to_string(&pe).unwrap_or_default();
                                let event = Event::default().event(event_name).data(data);
                                return Some((Ok(event), (domain_rx, pipeline_rx)));
                            }
                            Err(true) => return None,
                            Err(false) => continue,
                        }
                    }
                }
            }
        },
    );

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Map a DomainEvent to its domain string.
fn domain_for_event(event: &bus::DomainEvent) -> &'static str {
    match event {
        bus::DomainEvent::TaskCreated { .. }
        | bus::DomainEvent::TaskCompleted { .. }
        | bus::DomainEvent::TaskDeferred { .. }
        | bus::DomainEvent::GoalProgress { .. }
        | bus::DomainEvent::TaskDecomposed { .. }
        | bus::DomainEvent::TaskExecutionStarted { .. }
        | bus::DomainEvent::TaskExecutionCompleted { .. }
        | bus::DomainEvent::TaskExecutionFailed { .. }
        | bus::DomainEvent::TaskBlocked { .. }
        | bus::DomainEvent::TaskUnblocked { .. }
        | bus::DomainEvent::DayPlanGenerated { .. }
        | bus::DomainEvent::ProactiveSuggestionCreated { .. }
        | bus::DomainEvent::TaskFocusStarted { .. }
        | bus::DomainEvent::TaskFocusEnded { .. }
        | bus::DomainEvent::EstimationRecorded { .. }
        | bus::DomainEvent::TaskExecutionProgress { .. }
        | bus::DomainEvent::TaskStatusChanged { .. }
        | bus::DomainEvent::TaskPriorityChanged { .. }
        | bus::DomainEvent::TaskFieldUpdated { .. } => "work",
        bus::DomainEvent::ActivitySessionCompleted { .. }
        | bus::DomainEvent::FocusSessionStarted { .. }
        | bus::DomainEvent::FocusSessionEnded { .. }
        | bus::DomainEvent::DistractionDetected { .. }
        | bus::DomainEvent::ProductivityScoreComputed { .. }
        | bus::DomainEvent::SessionCreated { .. }
        | bus::DomainEvent::SessionEnded { .. }
        | bus::DomainEvent::QualityScored { .. }
        | bus::DomainEvent::PredictiveAlert { .. }
        | bus::DomainEvent::NarrativeGenerated { .. }
        | bus::DomainEvent::RuleEvolved { .. }
        | bus::DomainEvent::VoiceJournalProcessed { .. } => "energy",
        bus::DomainEvent::TransactionRecorded { .. } | bus::DomainEvent::BudgetAlert { .. } => {
            "finance"
        }
        bus::DomainEvent::UserStatedFact { .. } => "general",
        bus::DomainEvent::UserCorrectedAI { .. } => "learning",
        bus::DomainEvent::CoachingFeedback { .. } => "coaching",
        bus::DomainEvent::ChatTurnCompleted { .. } => "general",
        bus::DomainEvent::NoteCreated { .. }
        | bus::DomainEvent::NoteUpdated { .. }
        | bus::DomainEvent::NoteContentChanged { .. }
        | bus::DomainEvent::NoteDeleted { .. } => "notes",
        bus::DomainEvent::TaskHierarchyChanged { .. } => "work",
        bus::DomainEvent::ToolCallExecuted { .. } => "general",
        bus::DomainEvent::BehavioralPatternDetected { .. } => "learning",
        bus::DomainEvent::ContradictionDetected { .. } => "learning",
        bus::DomainEvent::AutotunerDecision { .. } => "learning",
        bus::DomainEvent::KnowledgeAtomCreated { .. }
        | bus::DomainEvent::KnowledgeAtomAccepted { .. }
        | bus::DomainEvent::KnowledgeAtomArchived { .. }
        | bus::DomainEvent::AtomFlashcardReviewed { .. }
        | bus::DomainEvent::AtomReinforced { .. }
        | bus::DomainEvent::AtomInteracted { .. }
        | bus::DomainEvent::RetentionMilestoneReached { .. }
        | bus::DomainEvent::TranslationCompleted { .. }
        | bus::DomainEvent::NoteStudied { .. }
        | bus::DomainEvent::PracticeUnitCompleted { .. }
        | bus::DomainEvent::PracticeSessionCompleted { .. }
        | bus::DomainEvent::KnowledgeTransferDetected { .. }
        | bus::DomainEvent::CoachingLearningDigest { .. }
        | bus::DomainEvent::FlashcardSessionCompleted { .. } => "learning",
        bus::DomainEvent::InterventionTriggered { .. } => "productivity",
        bus::DomainEvent::MemoryPendingConfirmation { .. } => "memory",
        bus::DomainEvent::SkillRouted { .. } => "agent",
        bus::DomainEvent::TrialActivated { .. } => "autotuner",
        bus::DomainEvent::MirrorTrialKilled { .. } => "mirror",
        bus::DomainEvent::MirrorSnippetCreated { .. } => "mirror",
        _ => "general",
    }
}
