use crate::AppCore;
use common::Result;
use desktop_shared::coding::{MessageDto, ThreadEvent};
use storage::messages::parts::MessagePart;

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

        // 1. Append user message with Parts
        let user_msg_id = uuid::Uuid::new_v4();
        let parts = vec![MessagePart::Text {
            text: text.to_string(),
        }];
        self.repos
            .sessions
            .add_message_with_parts(
                thread_id,
                user_msg_id,
                "user",
                &parts,
                Some(&turn_id),
                None,
            )
            .await
            ?;

        // 2. Resolve model
        let config = self.config.read().await;
        let resolved_model = model
            .or_else(|| {
                let m = config.agents.defaults.model.clone();
                if m.is_empty() { None } else { Some(m) }
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

        // 4. Spawn agent task via process_direct_streaming
        let agent = self.agent.clone();
        let active_streams = self.active_streams.clone();
        let thread_events = self.thread_events.clone();
        let cost_events = self.cost_events.clone();
        let thread_id_owned = thread_id.to_string();
        let turn_id_clone = turn_id.clone();
        let text_owned = text.to_string();

        tokio::spawn(async move {
            let result = agent
                .process_direct_streaming(
                    text_owned,
                    thread_id_owned.clone(),
                    Some("coding".into()),
                )
                .await;

            match result {
                Ok(handle) => {
                    // Store cancel token keyed by turn_id so concurrent turns on the
                    // same thread don't overwrite each other.
                    let cancel = handle.cancel_token.clone();
                    active_streams.insert(turn_id_clone.clone(), cancel);

                    // Bridge AgentEvent → ThreadEvent
                    let mut event_rx = handle.event_rx;
                    let broker = thread_events.clone();
                    let cost_broker = cost_events.clone();
                    let tid = thread_id_owned.clone();
                    let tuid_bridge = turn_id_clone.clone();

                    let bridge_handle = tokio::spawn(async move {
                        let mut item_started = false;
                        let mut item_id = String::new();

                        while let Some(evt) = event_rx.recv().await {
                            match evt {
                                agent::AgentEvent::ContentChunk { data, .. } => {
                                    if !item_started {
                                        item_id =
                                            format!("msg-{}", uuid::Uuid::new_v4());
                                        let placeholder = MessageDto {
                                            id: item_id.clone(),
                                            session_id: tid.clone(),
                                            role: "assistant".into(),
                                            parts: vec![],
                                            model: None,
                                            turn_id: Some(tuid_bridge.clone()),
                                            created_at: jiff::Timestamp::now()
                                                .as_millisecond(),
                                            finish_reason: None,
                                        };
                                        broker.publish(ThreadEvent::ItemStarted {
                                            thread_id: tid.clone(),
                                            turn_id: tuid_bridge.clone(),
                                            item: placeholder,
                                        });
                                        item_started = true;
                                    }
                                    broker.publish(ThreadEvent::ItemDelta {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        item_id: item_id.clone(),
                                        part_idx: 0,
                                        delta: desktop_shared::coding::PartDelta::Text {
                                            append: data,
                                        },
                                    });
                                }
                                agent::AgentEvent::ToolStart {
                                    name, args, ..
                                } => {
                                    let call_id = args
                                        .get("call_id")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("unknown")
                                        .to_string();
                                    broker.publish(ThreadEvent::ToolCallStarted {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        item_id: item_id.clone(),
                                        call_id,
                                        tool: name,
                                    });
                                }
                                agent::AgentEvent::ToolEnd {
                                    name,
                                    success,
                                    duration_ms,
                                    ..
                                } => {
                                    broker.publish(ThreadEvent::ToolCallCompleted {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        call_id: "unknown".into(),
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
                                        let completed = MessageDto {
                                            id: item_id.clone(),
                                            session_id: tid.clone(),
                                            role: "assistant".into(),
                                            parts: vec![],
                                            model: None,
                                            turn_id: Some(tuid_bridge.clone()),
                                            created_at: jiff::Timestamp::now()
                                                .as_millisecond(),
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
                                agent::AgentEvent::FileEditWithSymbols {
                                    path, op, ..
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
                                    broker.publish(ThreadEvent::FileChanged {
                                        thread_id: tid.clone(),
                                        turn_id: tuid_bridge.clone(),
                                        path,
                                        change,
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
                    let finish = match join_result {
                        Ok(Ok(_)) => serde_json::json!({"kind": "completed"}),
                        Ok(Err(_)) => {
                            serde_json::json!({"kind": "error", "code": "agent_error"})
                        }
                        Err(_) => serde_json::json!({"kind": "error", "code": "panic"}),
                    };

                    thread_events.publish(ThreadEvent::TurnCompleted {
                        thread_id: thread_id_owned.clone(),
                        turn_id: turn_id_clone.clone(),
                        finish_reason: finish,
                        completed_at,
                        duration_ms,
                    });

                    // Clean up active stream
                    active_streams.remove(&turn_id_clone);
                }
                Err(e) => {
                    tracing::error!("coding turn failed to start: {e}");
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
    pub async fn coding_turn_interrupt(
        &self,
        _thread_id: &str,
        turn_id: &str,
    ) -> Result<()> {
        if let Some((_, token)) = self.active_streams.remove(turn_id) {
            token.cancel();
            Ok(())
        } else {
            Err(common::KlyntbotError::StorageNotFound(format!(
                "active_turn {turn_id}"
            )))
        }
    }

    /// Steer an active coding turn — inject a mid-turn user correction.
    #[tracing::instrument(skip(self), err)]
    pub async fn coding_turn_steer(
        &self,
        _thread_id: &str,
        turn_id: &str,
        text: &str,
    ) -> Result<()> {
        // The SteerQueue is registered per-turn; push text into it.
        // If the turn isn't active or no SteerQueue is registered, return error.
        // For now, we store steer messages as a synthetic user message in the session.
        // Full SteerQueue wiring with LiveContextRefresher is a future enhancement.
        let _ = (turn_id, text);
        Err(common::KlyntbotError::StorageNotFound(
            "steer not yet wired to execution loop".into(),
        ))
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
