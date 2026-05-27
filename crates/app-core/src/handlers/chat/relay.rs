//! Chat relay shell — lifecycle wrapper around the pure `ChatEventTranslator`.

use std::sync::Arc;

use agent::AgentEvent;
use desktop_shared::events::*;
use tokio::sync::mpsc;
use tracing::Instrument;

use super::streaming::PendingInteractions;
use crate::handlers::chat::ActiveStreams;

/// The relay shell extracted from `relay_chat_stream`.
/// Keeps the lifecycle (StreamGuard, two-channel fan-in, heartbeat, select!),
/// emits what the translator returns, and hands terminal outcomes to
/// `TurnFinalizer`.
pub struct ChatRelay {
    pub repos: storage::Repos,
    pub session_key: String,
    pub active_streams: Arc<ActiveStreams>,
    pub pending_interactions: Arc<PendingInteractions>,
    pub event_rx: mpsc::Receiver<AgentEvent>,
    pub interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    pub emitter: Arc<dyn crate::events::AppEventEmitter>,
    pub has_context: bool,
    pub journey_tracker: Option<crate::journey::JourneyTracker>,
    pub domain_event_bus: Option<Arc<bus::DomainEventBus>>,
    pub user_message: Option<String>,
    pub guard_id: u64,
}

impl ChatRelay {
    pub async fn run(self) {
        let Self {
            repos,
            session_key,
            active_streams,
            pending_interactions,
            mut event_rx,
            mut interaction_rx,
            emitter,
            has_context,
            journey_tracker,
            domain_event_bus,
            user_message,
            guard_id,
        } = self;

        // Guard ensures active_streams + pending_interactions cleanup even on panic
        struct StreamGuard {
            key: String,
            guard_id: u64,
            streams: Arc<ActiveStreams>,
            pending: Arc<PendingInteractions>,
        }
        impl Drop for StreamGuard {
            fn drop(&mut self) {
                if let dashmap::Entry::Occupied(e) = self.streams.entry(self.key.clone()) {
                    if e.get().guard_id == self.guard_id {
                        e.remove();
                    }
                }
                self.pending.remove(&self.key);
            }
        }
        let _guard = StreamGuard {
            key: session_key.clone(),
            guard_id,
            streams: Arc::clone(&active_streams),
            pending: Arc::clone(&pending_interactions),
        };

        let sk = &session_key;
        let generation = active_streams.get(sk).map(|e| e.generation).unwrap_or(0);

        let mut translator =
            super::event_translator::ChatEventTranslator::new(sk.clone(), generation);

        // Merge pipeline events and domain-bus agent events into a single stream
        let (merged_tx, mut merged_rx) = mpsc::channel::<AgentEvent>(256);
        let merged_tx2 = merged_tx.clone();

        tokio::spawn(
            async move {
                while let Some(evt) = event_rx.recv().await {
                    if merged_tx.send(evt).await.is_err() {
                        break;
                    }
                }
            }
            .in_current_span(),
        );

        if let Some(ref bus) = domain_event_bus {
            let mut rx = bus.subscribe();
            tokio::spawn(
                async move {
                    while let Ok(evt) = rx.recv().await {
                        if let bus::DomainEvent::Generic { kind, payload } = evt {
                            if kind == "agent_event" {
                                if let Ok(agent_evt) = serde_json::from_value::<AgentEvent>(payload)
                                {
                                    if merged_tx2.send(agent_evt).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                .in_current_span(),
            );
        }

        // Heartbeat: emit every 30s so the frontend knows the turn is still alive.
        let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(30));
        heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        let mut interaction_closed = false;
        loop {
            tokio::select! {
                biased;
                _ = heartbeat.tick() => {
                    let hb = desktop_shared::thread_event_v2::ThreadEvent::Heartbeat {
                        generation,
                        session_key: sk.clone(),
                        server_time: jiff::Timestamp::now().as_millisecond(),
                    };
                    if let Ok(val) = serde_json::to_value(&hb) {
                        emitter.emit_event("thread:event", val);
                    }
                }
                bundle = interaction_rx.recv(), if !interaction_closed => {
                    match bundle {
                        Some(bundle) => {
                            let request_id = uuid::Uuid::new_v4().to_string();
                            if let Ok(val) = serde_json::to_value(&InteractionRequestPayload {
                                session_key: sk.clone(),
                                request_id: request_id.clone(),
                                request: bundle.request,
                            }) {
                                emitter.emit_event(AGENT_INTERACTION_REQUEST, val);
                            }
                            pending_interactions.insert(sk.clone(), (request_id, bundle.response_tx));
                        }
                        None => {
                            interaction_closed = true;
                        }
                    }
                }
                event = merged_rx.recv() => {
                    match event {
                        Some(event) => {
                            let emits = translator.handle(event);

                            if let Some(outcome) = translator.take_terminal() {
                                // (a) persist/publish/journey FIRST (Done only).
                                let finalizer = super::turn_finalizer::TurnFinalizer {
                                    repos: Some(&repos),
                                    domain_event_bus: domain_event_bus.as_ref(),
                                    journey_tracker: journey_tracker.as_ref(),
                                };
                                if let super::event_translator::TurnOutcome::Done {
                                    content: _, message_id, segments, transparency,
                                } = &outcome
                                {
                                    finalizer
                                        .finalize_done(sk, user_message.as_deref(), message_id.as_deref(), segments, transparency)
                                        .await;
                                }
                                // (b) eager active_streams cleanup BEFORE emitting terminal events (race fix).
                                if let dashmap::Entry::Occupied(e) = active_streams.entry(sk.clone()) {
                                    if e.get().guard_id == guard_id {
                                        e.remove();
                                    }
                                }
                                // (c) emit terminal UI events.
                                for e in emits {
                                    emitter.emit_event(e.event, e.payload);
                                }
                                break;
                            } else {
                                for e in emits {
                                    emitter.emit_event(e.event, e.payload);
                                }
                            }
                        }
                        None => {
                            // Event stream closed.
                            if let dashmap::Entry::Occupied(e) = active_streams.entry(sk.clone()) {
                                if e.get().guard_id == guard_id {
                                    e.remove();
                                }
                            }
                            break;
                        }
                    }
                }
                else => break,
            }
        }

        // Auto-detect session context from tool usage (only if not already set)
        if !has_context {
            if let Err(e) = super::streaming::auto_detect_context(
                &repos,
                sk,
                &translator.state().tool_names,
                &translator.state().entity_cards,
            )
            .await
            {
                tracing::debug!("auto-detect context skipped for {sk}: {e}");
            }
        }
    }
}
