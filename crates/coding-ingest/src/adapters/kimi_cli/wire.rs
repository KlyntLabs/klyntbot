//! kimi-cli Tier-2: streaming Wire client.
//!
//! Connects to kimi-cli's local Wire socket (Unix domain socket at
//! `~/.kimi/wire.sock` by default) and translates each frame into an
//! `AgentEvent`. Falls back to Tier-1 hooks if the socket is unavailable.

use crate::event::AgentEvent;
use common::Result;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::net::UnixStream;
use tokio::sync::mpsc;

/// One Wire frame as emitted by kimi-cli (newline-delimited JSON).
#[derive(serde::Deserialize, Debug)]
pub struct WireFrame {
    /// Wire frame type tag (e.g. "user_prompt", "tool_use").
    #[serde(rename = "t")]
    pub frame_type: String,
    /// Session identifier.
    pub session_id: String,
    /// JSON payload specific to the frame type.
    pub payload: serde_json::Value,
}

/// Convert one Wire frame to an `AgentEvent` via the existing dispatch path.
pub fn frame_to_event(frame: &WireFrame) -> Result<Option<AgentEvent>> {
    // Reuse the tier-1 dispatch by mapping frame_type → hook event name.
    let hook_event = match frame.frame_type.as_str() {
        "session_start" => "SessionStart",
        "session_end" => "SessionEnd",
        "user_prompt" => "UserPrompt",
        "assistant_msg" => "AssistantMsg",
        "tool_use" => "ToolCall",
        "skill_activated" => "SkillActivated",
        "recall_injected" => "RecallInjected",
        "approval_decision" => "ApprovalDecision",
        "provider_call" => "ProviderCall",
        _ => return Ok(None),
    };
    let payload_bytes = serde_json::to_vec(&frame.payload)
        .map_err(|e| common::KlyntbotError::Json(e))?;
    super::dispatch::dispatch(hook_event, &payload_bytes).map(|opt| opt.map(AgentEvent::V1))
}

/// Run the streaming loop. Caller owns cancellation by dropping the JoinHandle.
pub async fn run(
    socket_path: std::path::PathBuf,
    tx: mpsc::UnboundedSender<AgentEvent>,
) -> Result<()> {
    let stream = UnixStream::connect(&socket_path)
        .await
        .map_err(|e| common::KlyntbotError::Io(e))?;
    let reader = BufReader::new(stream);
    let mut lines = reader.lines();
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| common::KlyntbotError::Io(e))?
    {
        let frame: WireFrame = match serde_json::from_str(&line) {
            Ok(f) => f,
            Err(e) => {
                tracing::warn!(error = %e, "kimi wire frame parse failed");
                continue;
            }
        };
        match frame_to_event(&frame) {
            Ok(Some(evt)) => {
                if tx.send(evt).is_err() {
                    break;
                }
            }
            Ok(None) => {}
            Err(e) => tracing::warn!(error = %e, "kimi frame dispatch failed"),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_to_event_user_prompt() {
        let frame = WireFrame {
            frame_type: "user_prompt".into(),
            session_id: "s1".into(),
            payload: serde_json::json!({
                "session_id": "s1",
                "cwd": "/tmp",
                "prompt": "hi",
                "attachments": []
            }),
        };
        let evt = frame_to_event(&frame).unwrap().expect("frame produced an event");
        let AgentEvent::V1(v1) = evt;
        assert_eq!(v1.session_id, "s1");
    }

    #[test]
    fn unknown_frame_type_returns_none() {
        let frame = WireFrame {
            frame_type: "exotic_unknown".into(),
            session_id: "s1".into(),
            payload: serde_json::Value::Null,
        };
        assert!(frame_to_event(&frame).unwrap().is_none());
    }
}
