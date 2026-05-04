use std::collections::VecDeque;
use std::convert::Infallible;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use futures_util::stream::Stream;
use serde_json::Value;
use tokio::sync::broadcast;

use super::{err, ok, ApiResult, DevState, SseChannels, SseEmitter};
use crate::app_core::AppCore;
use crate::commands::dev_helpers as dev;
use ::app_core::events::AppEventEmitter;

/// Maximum age for an SSE channel with no active receivers before eviction.
const SSE_CHANNEL_TTL: Duration = Duration::from_secs(300);

/// Evict stale SSE channels that have no active receivers and exceeded TTL.
fn evict_stale_channels(channels: &SseChannels) {
    channels.retain(|_, ch| {
        ch.sender.receiver_count() > 0 || ch.created_at.elapsed() < SSE_CHANNEL_TTL
    });
}

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
    let mode: Option<String> = dev::get(body, "mode");

    evict_stale_channels(sse_channels);

    match core
        .chat_send(content, session_key.clone(), context, mode)
        .await
    {
        Ok((user_msg, stream_info)) => {
            let tx = sse_channels
                .entry(session_key)
                .or_insert_with(|| super::SseChannel {
                    sender: broadcast::channel(256).0,
                    created_at: Instant::now(),
                })
                .sender
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
    evict_stale_channels(&state.sse_channels);

    // Use atomic entry API to avoid TOCTOU race with chat_send:
    // whichever handler runs first creates the channel, the other reuses it.
    let rx = state
        .sse_channels
        .entry(session_key.clone())
        .or_insert_with(|| super::SseChannel {
            sender: broadcast::channel(256).0,
            created_at: Instant::now(),
        })
        .value()
        .sender
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

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

/// Handle `note_insight_tab_chat` separately because it needs SSE channel state to relay
/// streaming LLM events back to the browser via Server-Sent Events.
pub(super) async fn dispatch_insight_tab_chat(
    core: &AppCore,
    body: &Value,
    sse_channels: &SseChannels,
) -> ApiResult {
    let params: desktop_shared::commands::InsightChatParams =
        match serde_json::from_value(body.get("params").cloned().unwrap_or_default()) {
            Ok(p) => p,
            Err(e) => {
                return err(desktop_shared::errors::ApiError::new(
                    "INVALID_PARAMS",
                    e.to_string(),
                ))
            }
        };

    let session_key = params.session_key.clone();
    let tx = sse_channels
        .entry(session_key.clone())
        .or_insert_with(|| super::SseChannel {
            sender: broadcast::channel(256).0,
            created_at: Instant::now(),
        })
        .sender
        .clone();
    let emitter: Arc<dyn AppEventEmitter> = Arc::new(SseEmitter { tx });

    match core.note_insight_tab_chat(&params, emitter).await {
        Ok(started) => ok(started),
        Err(e) => err(e),
    }
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

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

/// SSE endpoint — streams global app events (brain:ambient, provider:degraded, etc.).
///
/// The frontend (`BrainEventBridge.tsx`) connects here in browser dev mode
/// via `new EventSource("/api/brain/events")`. Each SSE frame uses a named event
/// type so the bridge can dispatch matching `CustomEvent`s on `window`, which
/// the existing `useEvent` hook picks up transparently.
pub(super) async fn global_sse_handler(
    State(state): State<DevState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.global_event_tx.subscribe();

    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        loop {
            match rx.recv().await {
                Ok((event_name, payload)) => {
                    let data = serde_json::to_string(&payload).unwrap_or_default();
                    let event = Event::default().event(&event_name).data(data);
                    return Some((Ok(event), rx));
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("global SSE stream lagged by {n} events");
                    continue;
                }
                Err(broadcast::error::RecvError::Closed) => {
                    return None;
                }
            }
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}

/// State for the cognitive SSE stream unfold.
///
/// We carry a `pending` queue so that when a single domain event produces both
/// a `cognitive:domain_event` and a `fabric_graph` SSE frame, we can yield
/// them as two consecutive items without needing separate broadcast channels.
struct CognitiveSseState {
    domain_rx: Option<broadcast::Receiver<bus::DomainEvent>>,
    pipeline_rx: Option<broadcast::Receiver<cognitive::PipelineEvent>>,
    pending: VecDeque<Event>,
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

    let init = CognitiveSseState {
        domain_rx,
        pipeline_rx,
        pending: VecDeque::new(),
    };

    let stream = futures_util::stream::unfold(init, |mut st| async move {
        // Drain any pending events queued from a previous iteration before
        // blocking on the next broadcast message.
        if let Some(event) = st.pending.pop_front() {
            return Some((Ok(event), st));
        }

        loop {
            tokio::select! {
                // Domain events
                result = async {
                    match st.domain_rx.as_mut() {
                        Some(rx) => rx.recv().await.map_err(|e| matches!(e, broadcast::error::RecvError::Closed)),
                        None => std::future::pending::<Result<bus::DomainEvent, bool>>().await,
                    }
                } => {
                    match result {
                        Ok(event) => {
                            // Avoid serializing the full DomainEvent — large payloads
                            // (note content, chat turns) cause GB-scale heap pressure.
                            // Match the production path in app_core.rs: send only the
                            // variant name, not the full payload.
                            let event_type = event.variant_name().to_string();
                            let payload = serde_json::json!({
                                "eventType": &event_type,
                                "salience": "extract",
                                "domain": event.domain(),
                                "timestamp": jiff::Timestamp::now().to_string(),
                                "payload": event_type,
                            });
                            let cog_data = serde_json::to_string(&payload).unwrap_or_default();
                            let cog_event = Event::default().event("cognitive:domain_event").data(cog_data);

                            if let Some(fe) = desktop_shared::commands::fabric::FabricGraphEvent::from_domain_event(&event) {
                                let fabric_data = serde_json::to_string(&fe).unwrap_or_default();
                                let fabric_event = Event::default().event("fabric_graph").data(fabric_data);
                                st.pending.push_back(cog_event);
                                return Some((Ok(fabric_event), st));
                            }

                            return Some((Ok(cog_event), st));
                        }
                        Err(true) => return None, // closed
                        Err(false) => continue, // lagged
                    }
                }

                // Pipeline events
                result = async {
                    match st.pipeline_rx.as_mut() {
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
                            return Some((Ok(event), st));
                        }
                        Err(true) => return None,
                        Err(false) => continue,
                    }
                }
            }
        }
    });

    Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(std::time::Duration::from_secs(15))
            .text("ping"),
    )
}
