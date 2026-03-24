//! Agent delegation bridge — routes natural language requests through
//! klyntbot's full agent pipeline (intent analysis → tool calls → synthesis).

use std::sync::Arc;

use agent::AgentEvent;
use app_core::AppCore;
use common::FormResponse;
use rmcp::model::{CallToolResult, Content, ProgressNotificationParam, ProgressToken};
use rmcp::service::RoleServer;
use rmcp::{ErrorData as McpError, Peer};
use tokio::sync::mpsc;

/// Optional progress emitter for streaming MCP notifications during agent execution.
pub struct ProgressEmitter {
    pub peer: Peer<RoleServer>,
    pub token: ProgressToken,
}

/// Bridges MCP `agent` tool calls to AppCore's chat pipeline.
pub struct AgentBridge {
    app: Arc<AppCore>,
}

impl AgentBridge {
    pub fn new(app: Arc<AppCore>) -> Self {
        Self { app }
    }

    /// Execute an agent chat request and collect the streamed response.
    ///
    /// When `progress` is provided, emits `notifications/progress` for each
    /// `ContentChunk` event so external MCP clients get real-time feedback.
    pub async fn execute(
        &self,
        params: serde_json::Value,
        progress: Option<ProgressEmitter>,
    ) -> Result<CallToolResult, McpError> {
        let action = params
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("chat");

        match action {
            "chat" => self.handle_chat(params, progress).await,
            "status" => self.handle_status().await,
            _ => Err(McpError::invalid_params(
                format!("Unknown agent action: {action}"),
                None,
            )),
        }
    }

    async fn handle_chat(
        &self,
        params: serde_json::Value,
        progress: Option<ProgressEmitter>,
    ) -> Result<CallToolResult, McpError> {
        let message = params
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| McpError::invalid_params("'message' is required", None))?
            .to_string();

        let session_key = params
            .get("session_key")
            .and_then(|v| v.as_str())
            .map(|s| format!("mcp:{s}"))
            .unwrap_or_else(|| format!("mcp:{}", uuid::Uuid::new_v4()));

        // Call AppCore's chat pipeline
        let (_msg_response, stream_info) = self
            .app
            .chat_send(message, session_key, None)
            .await
            .map_err(|e| McpError::internal_error(e.to_string(), None))?;

        // Collect streamed events into a single response
        let (response, tool_log) =
            collect_agent_stream(stream_info.event_rx, stream_info.interaction_rx, progress).await;

        // Build result
        let mut content = vec![Content::text(response)];
        if !tool_log.is_empty() {
            content.push(Content::text(format!(
                "[Tools used: {}]",
                tool_log.join(", ")
            )));
        }

        Ok(CallToolResult::success(content))
    }

    async fn handle_status(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            serde_json::json!({
                "status": "running",
                "mode": format!("{:?}", self.app.mode),
            })
            .to_string(),
        )]))
    }
}

/// Collect AgentEvent stream into (response_text, tool_names_used).
///
/// Uses `select! biased` to prioritize event_rx over interaction_rx.
/// Auto-declines any ask_user interactions with `FormResponse::Cancelled`.
///
/// When a `ProgressEmitter` is provided, emits MCP `notifications/progress`
/// for each `ContentChunk` and `ToolStart` event so external clients get
/// real-time feedback during long-running agent calls.
async fn collect_agent_stream(
    mut event_rx: mpsc::Receiver<AgentEvent>,
    mut interaction_rx: mpsc::Receiver<tools_core::InteractionBundle>,
    progress: Option<ProgressEmitter>,
) -> (String, Vec<String>) {
    let mut response = String::new();
    let mut tool_log = Vec::new();
    let mut chunks_sent: u64 = 0;

    loop {
        tokio::select! {
            biased;

            event = event_rx.recv() => {
                match event {
                    Some(AgentEvent::ContentChunk { data }) => {
                        response.push_str(&data);
                        // Emit progress notification (best-effort)
                        if let Some(ref emitter) = progress {
                            chunks_sent += 1;
                            let _ = emitter.peer.notify_progress(ProgressNotificationParam {
                                progress_token: emitter.token.clone(),
                                progress: chunks_sent as f64,
                                total: None,
                                message: Some(data),
                            }).await;
                        }
                    }
                    Some(AgentEvent::ToolStart { name, .. }) => {
                        // Emit tool-start as progress notification
                        if let Some(ref emitter) = progress {
                            chunks_sent += 1;
                            let _ = emitter.peer.notify_progress(ProgressNotificationParam {
                                progress_token: emitter.token.clone(),
                                progress: chunks_sent as f64,
                                total: None,
                                message: Some(format!("[calling tool: {name}]")),
                            }).await;
                        }
                        tool_log.push(name);
                    }
                    Some(AgentEvent::ToolEnd { .. }) => {
                        // Already logged on ToolStart
                    }
                    Some(AgentEvent::Done { .. }) => break,
                    Some(AgentEvent::Error { message }) => {
                        if response.is_empty() {
                            response = format!("Agent error: {message}");
                        }
                        break;
                    }
                    None => break, // Channel closed
                    _ => {} // Skip telemetry events
                }
            }

            interaction = interaction_rx.recv() => {
                if let Some(bundle) = interaction {
                    // Auto-decline: MCP has no interactive prompt capability (yet).
                    // Send Cancelled so the agent's ask_user tool unblocks.
                    let _ = bundle.response_tx.send(FormResponse::Cancelled);
                }
            }
        }
    }

    (response, tool_log)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_collect_content_chunks() {
        let (event_tx, event_rx) = mpsc::channel(16);
        let (_interaction_tx, interaction_rx) = mpsc::channel(1);

        // Simulate agent stream
        event_tx
            .send(AgentEvent::ContentChunk {
                data: "Hello ".into(),
            })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::ToolStart {
                name: "task".into(),
                args: serde_json::json!({}),
                agent: None,
            })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::ToolEnd {
                name: "task".into(),
                success: true,
                duration_ms: 50,
                result: None,
                agent: None,
            })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::ContentChunk {
                data: "world".into(),
            })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::Done {
                content: String::new(),
                message_id: None,
            })
            .await
            .unwrap();

        let (response, tools) = collect_agent_stream(event_rx, interaction_rx, None).await;
        assert_eq!(response, "Hello world");
        assert_eq!(tools, vec!["task"]);
    }

    #[tokio::test]
    async fn test_collect_error_event() {
        let (event_tx, event_rx) = mpsc::channel(16);
        let (_interaction_tx, interaction_rx) = mpsc::channel(1);

        event_tx
            .send(AgentEvent::Error {
                message: "something broke".into(),
            })
            .await
            .unwrap();

        let (response, tools) = collect_agent_stream(event_rx, interaction_rx, None).await;
        assert_eq!(response, "Agent error: something broke");
        assert!(tools.is_empty());
    }

    #[tokio::test]
    async fn test_auto_decline_interaction() {
        let (event_tx, event_rx) = mpsc::channel(16);
        let (interaction_tx, interaction_rx) = mpsc::channel(1);

        let (response_tx, response_rx) = tokio::sync::oneshot::channel();

        // Spawn collector so we can interleave sends with yields
        let handle = tokio::spawn(collect_agent_stream(event_rx, interaction_rx, None));

        // Send interaction — yield so the collector can poll it
        interaction_tx
            .send(tools_core::InteractionBundle {
                request: common::InteractionRequest {
                    title: "test".into(),
                    questions: vec![],
                },
                response_tx,
            })
            .await
            .unwrap();
        tokio::task::yield_now().await;

        // Now send content + done
        event_tx
            .send(AgentEvent::ContentChunk { data: "ok".into() })
            .await
            .unwrap();
        event_tx
            .send(AgentEvent::Done {
                content: String::new(),
                message_id: None,
            })
            .await
            .unwrap();

        let (response, _) = handle.await.unwrap();
        assert_eq!(response, "ok");

        // Verify the interaction was auto-declined
        let form_response = response_rx.await.unwrap();
        assert!(matches!(form_response, FormResponse::Cancelled));
    }

    #[tokio::test]
    async fn test_channel_closed_gracefully() {
        let (event_tx, event_rx) = mpsc::channel(16);
        let (_interaction_tx, interaction_rx) = mpsc::channel(1);

        event_tx
            .send(AgentEvent::ContentChunk {
                data: "partial".into(),
            })
            .await
            .unwrap();
        drop(event_tx); // Close channel without Done event

        let (response, tools) = collect_agent_stream(event_rx, interaction_rx, None).await;
        assert_eq!(response, "partial");
        assert!(tools.is_empty());
    }
}
