use crate::AppCore;
use common::Result;
use desktop_shared::coding::{MessageDto, ThreadEvent};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use storage::messages::parts::MessagePart;

/// Kind discriminator for the bridge loop's dual text/reasoning buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PartKind {
    Text,
    Reasoning,
}

/// Concatenate Text parts in order. Returns `None` if any non-text/non-
/// reasoning part is present (mixed payload — never dedupe).
fn flatten_assistant_text(parts: &[MessagePart]) -> Option<String> {
    let mut out = String::new();
    for p in parts {
        match p {
            MessagePart::Text { text } => out.push_str(text),
            MessagePart::Reasoning { .. } => {} // ignored for dedupe key
            _ => return None,
        }
    }
    Some(out)
}

/// Insert a new assistant row OR — if its text is a superset/equal to the
/// previously-flushed text row in the same turn — update that row in
/// place. Coalesces streaming-snapshot + final-flush duplicates that the
/// turn handler's two flush sites (ToolStart, Done) would otherwise
/// persist as two rows.
async fn persist_or_coalesce_assistant_text(
    repos: &storage::Repos,
    thread_id: &str,
    turn_id: &str,
    parts: &[MessagePart],
) -> std::result::Result<(), storage::StorageError> {
    let new_text = match flatten_assistant_text(parts) {
        Some(t) => t,
        None => {
            // Mixed payload (tool/file/etc.) — never coalesce.
            let row_id = uuid::Uuid::new_v4();
            return repos
                .sessions
                .add_message_with_parts(thread_id, row_id, "assistant", parts, Some(turn_id), None)
                .await;
        }
    };

    // Query the DB for the latest assistant row in this turn — the source
    // of truth, regardless of how many bridge tasks happen to be running.
    if let Some((prev_id, prev_text)) = repos
        .sessions
        .latest_assistant_text_in_turn(thread_id, turn_id)
        .await?
    {
        let a = prev_text.trim();
        let b = new_text.trim();
        if a == b || a.contains(b) {
            // Existing row already a superset of (or equal to) the new
            // flush — skip the duplicate insert entirely.
            return Ok(());
        }
        if b.contains(a) {
            // New flush is a superset — replace the existing row in place.
            repos.sessions.update_message_parts(prev_id, parts).await?;
            return Ok(());
        }
    }

    // Distinct prose — fresh row.
    let row_id = uuid::Uuid::new_v4();
    repos
        .sessions
        .add_message_with_parts(thread_id, row_id, "assistant", parts, Some(turn_id), None)
        .await?;
    Ok(())
}

/// Response returned synchronously from `coding_message_send`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CodingMessageSendResponse {
    pub turn_id: String,
    pub turn_started_at: i64,
}

impl AppCore {
    /// Send a user message to a coding thread, starting a new turn.
    ///
    /// Returns synchronously with the `turn_id` — the agent runs in a
    /// background task and emits `ThreadEvent`s via the broker.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_message_send(
        &self,
        thread_id: &str,
        text: &str,
        model: Option<String>,
    ) -> Result<CodingMessageSendResponse> {
        let turn_id = format!("turn-{}", uuid::Uuid::new_v4());
        let started_at = jiff::Timestamp::now().as_millisecond();

        // 1. Persist the user message to the DB and echo it to the FE so the
        // chat shows the prompt immediately. `add_message_with_parts` mirrors
        // text into both the legacy `content` column and the structured
        // `parts` column, so Anthropic-spec providers don't 400 on empty
        // content. Without this DB write, the prompt only ever lives in the
        // in-memory session that downstream `setup_session` populates — and
        // that in-memory session is never flushed to disk on the coding-mode
        // happy path (the bridge writes assistant rows, not user ones), so
        // the prompt would silently disappear on refresh.
        let user_msg_id = uuid::Uuid::new_v4();
        let user_part = MessagePart::Text {
            text: text.to_string(),
        };
        self.repos
            .sessions
            .add_message_with_parts(
                thread_id,
                user_msg_id,
                "user",
                std::slice::from_ref(&user_part),
                Some(&turn_id),
                None,
            )
            .await?;
        self.thread_events.publish(ThreadEvent::ItemStarted {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.clone(),
            item: MessageDto {
                id: user_msg_id.to_string(),
                session_id: thread_id.to_string(),
                role: "user".into(),
                parts: vec![serde_json::to_value(&user_part).unwrap()],
                model: None,
                turn_id: Some(turn_id.clone()),
                created_at: started_at,
                finish_reason: None,
            },
        });

        // 2. Resolve model
        let config = self.config.read().await;
        let resolved_model = model
            .or_else(|| {
                let m = config.agents.defaults.model.clone();
                if m.is_empty() {
                    None
                } else {
                    Some(m)
                }
            })
            .unwrap_or_else(|| "default".into());
        drop(config);

        // 3. Emit TurnStarted
        self.thread_events.publish(ThreadEvent::TurnStarted {
            thread_id: thread_id.to_string(),
            turn_id: turn_id.clone(),
            model: resolved_model.clone(),
            started_at,
        });

        // ── Autogenerate title on first user message of an unnamed session ──
        // Cheap fast-path checks before spawning: skip if cognitive provider
        // unavailable, skip if session already titled, skip if not first message.
        if let Some(provider) = self.cognitive_provider.clone() {
            let title_repo = self.repos.sessions.clone();
            let emitter = self.event_emitter.clone();
            let session_key = thread_id.to_string();
            let first_msg = text.to_string();
            let title_model = resolved_model.clone();
            tokio::spawn(async move {
                // Re-check title and message count from inside the task to avoid
                // races with concurrent first messages on the same session.
                let (session_res, msg_count_res) = tokio::join!(
                    title_repo.get_session(&session_key),
                    title_repo.count_user_messages(&session_key),
                );
                let session = match session_res {
                    Ok(s) => s,
                    Err(_) => return,
                };
                let already_titled = session
                    .metadata
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false);
                if already_titled {
                    return;
                }
                let user_msg_count = msg_count_res.unwrap_or(i64::MAX);
                if user_msg_count > 1 {
                    return;
                }
                let _ = crate::coding::title_service::autogenerate_title(
                    title_repo,
                    provider,
                    emitter,
                    session_key,
                    first_msg,
                    title_model,
                )
                .await;
            });
        }

        // 4. Register a steer receiver for this turn and spawn a drain task
        // that persists each pushed steer as a synthetic user message in the
        // session — the next iteration's prompt assembly will pick it up.
        let steer_rx = self.steer_queue.register_turn(&turn_id);
        {
            let repos_drain = self.repos.clone();
            let thread_id_drain = thread_id.to_string();
            let turn_id_drain = turn_id.clone();
            tokio::spawn(async move {
                let mut rx = steer_rx;
                while let Some(text) = rx.recv().await {
                    let msg_id = uuid::Uuid::new_v4();
                    let parts = vec![MessagePart::Text { text }];
                    if let Err(e) = repos_drain
                        .sessions
                        .add_message_with_parts(
                            &thread_id_drain,
                            msg_id,
                            "user",
                            &parts,
                            Some(&turn_id_drain),
                            None,
                        )
                        .await
                    {
                        tracing::error!("steer persist failed for turn {turn_id_drain}: {e}");
                    }
                }
            });
        }

        // 5. Re-bind the coding tools to this thread's workspace cwd before
        // spawning the agent. Failure here is a hard error — we used to
        // tracing::warn! and run the turn anyway with the boot-time default
        // cwd, which silently scaffolded files inside the running binary's
        // launch directory. The cost of failing here is one rejected turn
        // with a clear error message; the cost of falling through is
        // polluting the user's filesystem.
        if let Err(e) = self.rebind_tool_kit_for_thread(thread_id).await {
            tracing::error!(thread = %thread_id, error = %e, "cwd-rebind failed; aborting turn");
            self.thread_events.publish(ThreadEvent::TurnCompleted {
                thread_id: thread_id.to_string(),
                turn_id: turn_id.clone(),
                finish_reason: serde_json::json!({ "error": e.to_string() }),
                completed_at: jiff::Timestamp::now().as_millisecond(),
                duration_ms: 0,
            });
            return Err(e);
        }

        // 6. Spawn agent task via process_direct_streaming
        let agent = self.agent.clone();
        let repos_for_bridge = self.repos.clone();
        let active_streams = self.active_streams.clone();
        let thread_events = self.thread_events.clone();
        let cost_events = self.cost_events.clone();
        let steer_queue = self.steer_queue.clone();
        let thread_id_owned = thread_id.to_string();
        let turn_id_clone = turn_id.clone();
        let text_owned = text.to_string();

        tokio::spawn(async move {
            tracing::info!(turn = %turn_id_clone, "coding turn spawn: entering process_direct_streaming");
            let result = agent
                .process_direct_streaming(
                    text_owned,
                    thread_id_owned.clone(),
                    Some("coding".into()),
                )
                .await;
            tracing::info!(
                turn = %turn_id_clone,
                ok = result.is_ok(),
                "coding turn spawn: process_direct_streaming returned"
            );

            match result {
                Ok(handle) => {
                    // Store cancel token keyed by turn_id so concurrent turns on the
                    // same thread don't overwrite each other.
                    let cancel = handle.cancel_token.clone();
                    active_streams.insert(
                        turn_id_clone.clone(),
                        crate::handlers::chat::ActiveStreamEntry {
                            guard_id: 0,
                            generation: 0,
                            cancel,
                        },
                    );

                    // Bridge AgentEvent → ThreadEvent
                    let mut event_rx = handle.event_rx;
                    let broker = thread_events.clone();
                    let cost_broker = cost_events.clone();
                    let tid = thread_id_owned.clone();
                    let tuid_bridge = turn_id_clone.clone();
                    // Repos handle for persisting tool/file/command parts so they
                    // survive a thread reload (without this, only user prompts and
                    // the final assistant text persist; transparency steps vanish).
                    let repos_bridge = repos_for_bridge.clone();

                    // Shared flag set by the bridge when it sees Cancelled so
                    // the post-join finish_reason computation can emit the right
                    // kind instead of defaulting to "completed".
                    let turn_cancelled = Arc::new(AtomicBool::new(false));
                    let turn_cancelled_bridge = turn_cancelled.clone();

                    let bridge_handle = tokio::spawn(async move {
                        // Heartbeat task — keeps the FE watchdog alive during long turns.
                        let hb_broker = broker.clone();
                        let hb_tid = tid.clone();
                        let _hb_handle = tokio::spawn(async move {
                            let mut interval = tokio::time::interval(std::time::Duration::from_secs(30));
                            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
                            loop {
                                interval.tick().await;
                                hb_broker.publish(desktop_shared::coding::events::ThreadEvent::Heartbeat {
                                    subscription_id: hb_tid.clone(),
                                    server_time: jiff::Timestamp::now().as_millisecond(),
                                });
                            }
                        });

                        let mut item_started = false;
                        let mut item_id = String::new();
                        // Two parallel buffers — one per stream channel — and a
                        // chronologically ordered `pending_parts` list. The model can
                        // alternate between text and reasoning within a single
                        // iteration ("answer prose" then "let me think more" then
                        // more answer); we capture each kind-burst as its own
                        // MessagePart and persist them in arrival order. Mirrors
                        // kimi-cli's TextPart vs ThinkPart distinction so reasoning
                        // shows in its natural place rather than being collapsed
                        // into text or dropped entirely.
                        let mut text_buffer = String::new();
                        let mut reasoning_buffer = String::new();
                        let mut last_kind: Option<PartKind> = None;
                        let mut pending_parts: Vec<MessagePart> = Vec::new();
                        // Tracks which `part_idx` the next live ItemDelta should
                        // target. Increments at every kind transition so the FE
                        // reducer renders each burst as its own part instead of
                        // merging them into one accumulating row.
                        let mut current_part_idx: u32 = 0;

                        while let Some(evt) = event_rx.recv().await {
                            match evt {
                                agent::AgentEvent::ContentChunk { data, .. } => {
                                    if !item_started {
                                        item_id = format!("msg-{}", uuid::Uuid::new_v4());
                                        let placeholder = MessageDto {
                                            id: item_id.clone(),
                                            session_id: tid.clone(),
                                            role: "assistant".into(),
                                            parts: vec![],
                                            model: None,
                                            turn_id: Some(tuid_bridge.clone()),
                                            created_at: jiff::Timestamp::now().as_millisecond(),
                                            finish_reason: None,
                                        };
                                        broker.publish(ThreadEvent::ItemStarted {
                                            thread_id: tid.clone(),
                                            turn_id: tuid_bridge.clone(),
                                            item: placeholder,
                                        });
                                        item_started = true;
                                        current_part_idx = 0;
                                    }
                                    // Kind transition: reasoning → text. Commit the
                                    // reasoning burst as its own MessagePart and bump
                                    // part_idx so the FE renders this text as a
                                    // distinct sibling part, not appended to the
                                    // reasoning block.
                                    if last_kind == Some(PartKind::Reasoning)
                                        && !reasoning_buffer.is_empty()
                                    {
                                        pending_parts.push(MessagePart::Reasoning {
                                            text: std::mem::take(&mut reasoning_buffer),
                                            redacted: false,
                                        });
                                        current_part_idx += 1;
                                    }
                                    last_kind = Some(PartKind::Text);
                                    text_buffer.push_str(&data);
                                    broker.publish(ThreadEvent::ItemDelta {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        item_id: item_id.clone(),
                                        part_idx: current_part_idx,
                                        delta: desktop_shared::coding::PartDelta::Text {
                                            append: data,
                                        },
                                    });
                                }
                                agent::AgentEvent::ReasoningChunk { data, .. } => {
                                    if !item_started {
                                        item_id = format!("msg-{}", uuid::Uuid::new_v4());
                                        let placeholder = MessageDto {
                                            id: item_id.clone(),
                                            session_id: tid.clone(),
                                            role: "assistant".into(),
                                            parts: vec![],
                                            model: None,
                                            turn_id: Some(tuid_bridge.clone()),
                                            created_at: jiff::Timestamp::now().as_millisecond(),
                                            finish_reason: None,
                                        };
                                        broker.publish(ThreadEvent::ItemStarted {
                                            thread_id: tid.clone(),
                                            turn_id: tuid_bridge.clone(),
                                            item: placeholder,
                                        });
                                        item_started = true;
                                        current_part_idx = 0;
                                    }
                                    // Kind transition: text → reasoning. Commit the
                                    // text burst before starting the reasoning part.
                                    if last_kind == Some(PartKind::Text) && !text_buffer.is_empty()
                                    {
                                        pending_parts.push(MessagePart::Text {
                                            text: std::mem::take(&mut text_buffer),
                                        });
                                        current_part_idx += 1;
                                    }
                                    last_kind = Some(PartKind::Reasoning);
                                    reasoning_buffer.push_str(&data);
                                    broker.publish(ThreadEvent::ItemDelta {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        item_id: item_id.clone(),
                                        part_idx: current_part_idx,
                                        delta: desktop_shared::coding::PartDelta::Reasoning {
                                            append: data,
                                            redacted: false,
                                        },
                                    });
                                }
                                agent::AgentEvent::ToolStart {
                                    name,
                                    args,
                                    call_id: ev_call_id,
                                    ..
                                } => {
                                    // Iteration boundary: commit any in-flight
                                    // text/reasoning bursts in arrival order, then
                                    // persist all pending parts as one assistant row.
                                    // Each iteration's prose becomes its own
                                    // MessageDto so the FE renders narration
                                    // interleaved with tool calls instead of one
                                    // oversized blob at the top.
                                    if !text_buffer.is_empty() {
                                        pending_parts.push(MessagePart::Text {
                                            text: std::mem::take(&mut text_buffer),
                                        });
                                    }
                                    if !reasoning_buffer.is_empty() {
                                        pending_parts.push(MessagePart::Reasoning {
                                            text: std::mem::take(&mut reasoning_buffer),
                                            redacted: false,
                                        });
                                    }
                                    if !pending_parts.is_empty() {
                                        let parts = std::mem::take(&mut pending_parts);
                                        if let Err(e) = persist_or_coalesce_assistant_text(
                                            &repos_bridge,
                                            &tid,
                                            &tuid_bridge,
                                            &parts,
                                        )
                                        .await
                                        {
                                            tracing::warn!("iteration parts persist failed: {e}");
                                        }
                                        item_started = false;
                                        last_kind = None;
                                        current_part_idx = 0;
                                    }
                                    // Prefer the call_id carried on the AgentEvent (provider-
                                    // assigned, stable across ToolStart/ToolEnd). The legacy
                                    // `args.call_id` lookup almost never matched any real arg
                                    // schema, which is why every tool ended up persisted with
                                    // `"unknown"` and the FE merge collapsed them into a single
                                    // row. Fall back to a fresh UUID only if neither source has
                                    // an id — that still lets the FE distinguish tools, just
                                    // without provider-traceable correlation.
                                    let call_id = ev_call_id
                                        .or_else(|| {
                                            args.get("call_id")
                                                .and_then(|v| v.as_str())
                                                .map(String::from)
                                        })
                                        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
                                    // Persist so the tool call shows up on thread reload
                                    // (without this, tool transparency only exists during
                                    // the live turn and disappears on refresh).
                                    let tool_call_part = MessagePart::ToolCall {
                                        call_id: call_id.clone(),
                                        name: name.clone(),
                                        args: args.clone(),
                                    };
                                    let row_id = uuid::Uuid::new_v4();
                                    if let Err(e) = repos_bridge
                                        .sessions
                                        .add_message_with_parts(
                                            &tid,
                                            row_id,
                                            "assistant",
                                            &[tool_call_part],
                                            Some(&tuid_bridge),
                                            None,
                                        )
                                        .await
                                    {
                                        tracing::warn!("tool_call persist failed: {e}");
                                    }
                                    // Surface the tool call as its own item so the
                                    // FE renders it inline (transparency for the user).
                                    let tool_item_id = format!("tool-{}", uuid::Uuid::new_v4());
                                    broker.publish(ThreadEvent::ItemStarted {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        item: MessageDto {
                                            id: tool_item_id.clone(),
                                            session_id: tid.clone(),
                                            role: "assistant".into(),
                                            parts: vec![serde_json::to_value(
                                                MessagePart::ToolCall {
                                                    call_id: call_id.clone(),
                                                    name: name.clone(),
                                                    args: args.clone(),
                                                },
                                            )
                                            .unwrap()],
                                            model: None,
                                            turn_id: Some(tuid_bridge.clone()),
                                            created_at: jiff::Timestamp::now().as_millisecond(),
                                            finish_reason: None,
                                        },
                                    });
                                    broker.publish(ThreadEvent::ToolCallStarted {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        item_id: tool_item_id,
                                        call_id,
                                        tool: name,
                                    });
                                }
                                agent::AgentEvent::ToolEnd {
                                    name,
                                    success,
                                    duration_ms,
                                    result,
                                    call_id: ev_call_id,
                                    ..
                                } => {
                                    // Render the tool's output as a user-visible item
                                    // (prefix with bash/file/etc. via the kind tag).
                                    let result_text = result.unwrap_or_default();
                                    // Use the call_id from the event so the FE can pair this
                                    // result back to its originating ToolCall row. Hardcoding
                                    // `"unknown"` here was the cause of every tool except the
                                    // last one in a turn appearing stuck on the loading
                                    // spinner — every result ended up with the same id and
                                    // the FE map only kept the last association.
                                    let result_call_id =
                                        ev_call_id.unwrap_or_else(|| "unknown".into());
                                    let result_part = MessagePart::ToolResult {
                                        call_id: result_call_id.clone(),
                                        output: storage::messages::parts::ToolOutput {
                                            text: result_text.clone(),
                                            mime: None,
                                            truncated: false,
                                        },
                                        is_error: !success,
                                    };
                                    let row_id = uuid::Uuid::new_v4();
                                    if let Err(e) = repos_bridge
                                        .sessions
                                        .add_message_with_parts(
                                            &tid,
                                            row_id,
                                            "assistant",
                                            &[result_part],
                                            Some(&tuid_bridge),
                                            None,
                                        )
                                        .await
                                    {
                                        tracing::warn!("tool_result persist failed: {e}");
                                    }
                                    broker.publish(ThreadEvent::ItemStarted {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        item: MessageDto {
                                            id: format!("tres-{}", uuid::Uuid::new_v4()),
                                            session_id: tid.clone(),
                                            role: "assistant".into(),
                                            parts: vec![serde_json::to_value(
                                                MessagePart::ToolResult {
                                                    call_id: result_call_id.clone(),
                                                    output: storage::messages::parts::ToolOutput {
                                                        text: result_text.clone(),
                                                        mime: None,
                                                        truncated: false,
                                                    },
                                                    is_error: !success,
                                                },
                                            )
                                            .unwrap()],
                                            model: None,
                                            turn_id: Some(tuid_bridge.clone()),
                                            created_at: jiff::Timestamp::now().as_millisecond(),
                                            finish_reason: None,
                                        },
                                    });
                                    broker.publish(ThreadEvent::ToolCallCompleted {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        call_id: result_call_id,
                                        success,
                                        duration_ms,
                                    });
                                    // For bash tools, also emit CommandExecuted
                                    if name == "bash" {
                                        broker.publish(ThreadEvent::CommandExecuted {
                                            thread_id: tid.clone(),
                                            turn_id: tuid_bridge.clone(),
                                            command: vec![name.clone()],
                                            exit_code: if success { Some(0) } else { Some(1) },
                                        });
                                    }
                                }
                                agent::AgentEvent::ContextCompressed {
                                    before_tokens,
                                    after_tokens,
                                    ..
                                } => {
                                    broker.publish(ThreadEvent::ContextCompressed {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        before_tokens: before_tokens as u64,
                                        after_tokens: after_tokens as u64,
                                    });
                                }
                                agent::AgentEvent::Done { .. } => {
                                    if item_started {
                                        // Final flush — commit any trailing
                                        // text/reasoning bursts and persist the
                                        // chronologically-ordered parts as one row.
                                        if !text_buffer.is_empty() {
                                            pending_parts.push(MessagePart::Text {
                                                text: std::mem::take(&mut text_buffer),
                                            });
                                        }
                                        if !reasoning_buffer.is_empty() {
                                            pending_parts.push(MessagePart::Reasoning {
                                                text: std::mem::take(&mut reasoning_buffer),
                                                redacted: false,
                                            });
                                        }
                                        if !pending_parts.is_empty() {
                                            let parts = std::mem::take(&mut pending_parts);
                                            if let Err(e) = persist_or_coalesce_assistant_text(
                                                &repos_bridge,
                                                &tid,
                                                &tuid_bridge,
                                                &parts,
                                            )
                                            .await
                                            {
                                                tracing::warn!("final parts persist failed: {e}");
                                            }
                                        }
                                        let completed = MessageDto {
                                            id: item_id.clone(),
                                            session_id: tid.clone(),
                                            role: "assistant".into(),
                                            parts: vec![],
                                            model: None,
                                            turn_id: Some(tuid_bridge.clone()),
                                            created_at: jiff::Timestamp::now().as_millisecond(),
                                            finish_reason: None,
                                        };
                                        broker.publish(ThreadEvent::ItemCompleted {
                                            thread_id: tid.clone(),
                                            turn_id: tuid_bridge.clone(),
                                            item: completed,
                                        });
                                    }
                                    break;
                                }
                                agent::AgentEvent::TurnComplete { .. } => {
                                    // Will be followed by Done — handled there
                                }
                                agent::AgentEvent::Cancelled {
                                    partial_content,
                                    partial_reasoning,
                                } => {
                                    tracing::info!(
                                        thread = %tid,
                                        turn = %tuid_bridge,
                                        content_len = partial_content.len(),
                                        reasoning_len = partial_reasoning.len(),
                                        "coding turn: cancelled mid-stream"
                                    );
                                    turn_cancelled_bridge.store(true, Ordering::Relaxed);
                                    // partial_content already arrived via ContentChunk →
                                    // ItemDelta path; no extra publish needed. The terminal
                                    // TurnCompleted below will carry finish_reason = cancelled.
                                }
                                agent::AgentEvent::FileEditWithSymbols {
                                    path,
                                    op,
                                    diff_full,
                                    ..
                                } => {
                                    let change = match op.as_str() {
                                        "write" => {
                                            desktop_shared::coding::FileChangeKindDto::Created
                                        }
                                        "edit" | "apply_patch" => {
                                            desktop_shared::coding::FileChangeKindDto::Modified
                                        }
                                        _ => desktop_shared::coding::FileChangeKindDto::Modified,
                                    };
                                    // Persist file_change part so reload shows the file
                                    // mutation history, not just text replies. Tools
                                    // already computed the unified diff via diffy; we
                                    // just stop dropping it on the floor here.
                                    let fc_part = MessagePart::FileChange(Box::new(
                                        storage::messages::parts::FileChangeData {
                                            path: std::path::PathBuf::from(&path),
                                            before: None,
                                            after: String::new(),
                                            diff_unified: diff_full.clone(),
                                            applied: true,
                                        },
                                    ));
                                    let row_id = uuid::Uuid::new_v4();
                                    if let Err(e) = repos_bridge
                                        .sessions
                                        .add_message_with_parts(
                                            &tid,
                                            row_id,
                                            "assistant",
                                            &[fc_part],
                                            Some(&tuid_bridge),
                                            None,
                                        )
                                        .await
                                    {
                                        tracing::warn!("file_change persist failed: {e}");
                                    }
                                    broker.publish(ThreadEvent::FileChanged {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        path,
                                        change,
                                        diff_unified: diff_full,
                                    });
                                }
                                agent::AgentEvent::UsageReport {
                                    model,
                                    prompt_tokens,
                                    completion_tokens,
                                    estimated_cost_usd,
                                    ..
                                } => {
                                    cost_broker.publish(desktop_shared::coding::CostUpdate {
                                        thread_id: Some(tid.clone()),
                                        provider: model,
                                        prompt_tokens_delta: prompt_tokens as u64,
                                        completion_tokens_delta: completion_tokens as u64,
                                        usd_delta: estimated_cost_usd,
                                        thread_total_usd: None,
                                        ceiling_breached: false,
                                    });
                                }
                                agent::AgentEvent::Error { message } => {
                                    // Surface errors in the chat so silent hangs become visible.
                                    broker.publish(ThreadEvent::ItemStarted {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        item: MessageDto {
                                            id: format!("err-{}", uuid::Uuid::new_v4()),
                                            session_id: tid.clone(),
                                            role: "assistant".into(),
                                            parts: vec![serde_json::to_value(MessagePart::Text {
                                                text: format!("⚠️ {message}"),
                                            })
                                            .unwrap()],
                                            model: None,
                                            turn_id: Some(tuid_bridge.clone()),
                                            created_at: jiff::Timestamp::now().as_millisecond(),
                                            finish_reason: None,
                                        },
                                    });
                                }
                                _ => {
                                    // Other AgentEvent variants — ignore for now
                                }
                            }
                        }
                    });

                    // Wait for pipeline and bridge to finish concurrently
                    let (join_result, _) = tokio::join!(handle.handle, bridge_handle);

                    let completed_at = jiff::Timestamp::now().as_millisecond();
                    let duration_ms = (completed_at - started_at) as u64;
                    let finish = if turn_cancelled.load(Ordering::Relaxed) {
                        serde_json::json!({"kind": "cancelled"})
                    } else {
                        match join_result {
                            Ok(Ok(_)) => serde_json::json!({"kind": "completed"}),
                            Ok(Err(_)) => {
                                serde_json::json!({"kind": "error", "code": "agent_error"})
                            }
                            Err(_) => serde_json::json!({"kind": "error", "code": "panic"}),
                        }
                    };

                    thread_events.publish(ThreadEvent::TurnCompleted {
                        thread_id: thread_id_owned.clone(),
                        turn_id: turn_id_clone.clone(),
                        finish_reason: finish,
                        completed_at,
                        duration_ms,
                    });

                    // Clean up active stream + steer queue (closes drain task)
                    active_streams.remove(&turn_id_clone);
                    steer_queue.unregister_turn(&turn_id_clone);
                }
                Err(e) => {
                    tracing::error!("coding turn failed to start: {e}");
                    steer_queue.unregister_turn(&turn_id_clone);
                    let completed_at = jiff::Timestamp::now().as_millisecond();
                    thread_events.publish(ThreadEvent::TurnCompleted {
                        thread_id: thread_id_owned,
                        turn_id: turn_id_clone,
                        finish_reason: serde_json::json!({
                            "kind": "error",
                            "code": "spawn_failed",
                            "message": e.to_string()
                        }),
                        completed_at,
                        duration_ms: (completed_at - started_at) as u64,
                    });
                }
            }
        });

        Ok(CodingMessageSendResponse {
            turn_id,
            turn_started_at: started_at,
        })
    }

    /// Interrupt an active coding turn.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_turn_interrupt(&self, _thread_id: &str, turn_id: &str) -> Result<()> {
        if let Some((_, entry)) = self.active_streams.remove(turn_id) {
            entry.cancel.cancel();
            Ok(())
        } else {
            Err(common::KlyntbotError::StorageNotFound(format!(
                "active_turn {turn_id}"
            )))
        }
    }

    /// Steer an active coding turn — inject a mid-turn user correction.
    ///
    /// Pushes the text onto the per-turn `SteerQueue`. The turn handler's
    /// drain task picks it up between iterations and persists it as a
    /// synthetic user message in the session — the next iteration's prompt
    /// assembly then includes it. Errors if the turn isn't active.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_turn_steer(
        &self,
        _thread_id: &str,
        turn_id: &str,
        text: &str,
    ) -> Result<()> {
        self.steer_queue.push(turn_id, text.to_string())
    }

    /// Resolve the workspace path for `thread_id` and re-register every tool
    /// in the shared registry with that path as its cwd. Returns Err on any
    /// failure — caller must abort the turn rather than fall through with a
    /// stale cwd. Failure modes (all hard errors, never warnings):
    ///   - the session row is missing
    ///   - the session row has no workspace_id
    ///   - the workspace row is missing
    ///   - the agent has no tool kit registered
    async fn rebind_tool_kit_for_thread(&self, thread_id: &str) -> Result<()> {
        let session = self
            .repos
            .sessions
            .get_session(thread_id)
            .await
            .map_err(|e| {
                common::KlyntbotError::Storage(format!(
                    "cwd-rebind: get_session({thread_id}) failed: {e}"
                ))
            })?;
        let ws_id = session.workspace_id.as_deref().ok_or_else(|| {
            common::KlyntbotError::Storage(format!(
                "cwd-rebind: coding session {thread_id} has no workspace_id"
            ))
        })?;
        let ws = self.repos.workspaces.get(ws_id).await.map_err(|e| {
            common::KlyntbotError::Storage(format!(
                "cwd-rebind: workspaces.get({ws_id}) failed: {e}"
            ))
        })?;
        let kit = self.tool_kit.lock().unwrap().clone().ok_or_else(|| {
            common::KlyntbotError::Storage("cwd-rebind: tool_kit is None".to_string())
        })?;
        let ws_path = std::path::PathBuf::from(&ws.path);
        let rebound = (*kit).clone().with_cwd(ws_path.clone());
        let reg = self.agent.tool_registry();
        let mut registry = reg.write().await;
        rebound.register_all(&mut registry);
        tracing::info!(
            thread = %thread_id,
            workspace = %ws_path.display(),
            "cwd-rebind: tool kit rebound"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn coding_message_send_response_serializes() {
        let resp = CodingMessageSendResponse {
            turn_id: "turn-123".into(),
            turn_started_at: 1000,
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["turnId"], "turn-123");
        assert_eq!(json["turnStartedAt"], 1000);
    }
}
