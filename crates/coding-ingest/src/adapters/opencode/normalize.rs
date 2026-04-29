//! Convert opencode `MessageRow` (joined with its parts) into `AgentEventV1`.

use crate::event::{AgentEventV1, AgentSource, EventKind, TokenUsage};
use crate::scope_resolver::resolve_scope;
use common::Result;
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

use super::schema::{MessageData, MessageRow, PartData, PartRow};

/// Convert one opencode message (envelope + its parts) into a stream of
/// `AgentEventV1`s. A message can produce multiple events: e.g. an assistant
/// message with text + a tool call yields a `ToolCall` event AND an
/// `AssistantMsg` event.
pub fn message_to_events(message: MessageRow, parts: Vec<PartRow>) -> Result<Vec<AgentEventV1>> {
    let envelope: MessageData = match serde_json::from_str(&message.data) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(error = %e, message_id = %message.id, "opencode message JSON parse failed");
            return Ok(vec![]);
        }
    };

    let cwd = envelope
        .path
        .as_ref()
        .map(|p| PathBuf::from(&p.cwd))
        .unwrap_or_else(|| PathBuf::from("/"));
    let repo = resolve_scope(&cwd);
    let occurred_at =
        Timestamp::from_millisecond(message.time_created).unwrap_or_else(|_| Timestamp::now());

    let mut events: Vec<AgentEventV1> = Vec::new();
    for part in parts {
        let body: PartData = match serde_json::from_str(&part.data) {
            Ok(v) => v,
            Err(_) => continue,
        };
        let kind = match (envelope.role.as_str(), body) {
            ("user", PartData::Text { text }) if !text.is_empty() => EventKind::UserPrompt {
                text,
                attachments: vec![],
            },
            ("assistant", PartData::Text { text }) if !text.is_empty() => EventKind::AssistantMsg {
                text,
                truncated: false,
                token_usage: envelope.tokens.as_ref().map(|t| TokenUsage {
                    prompt_tokens: t.input,
                    completion_tokens: t.output,
                    cached_tokens: t.cache.as_ref().map(|c| c.read),
                }),
            },
            (_, PartData::Tool { tool, state }) => {
                let args_preview = serde_json::to_string(&state).unwrap_or_default();
                let args_preview = args_preview.chars().take(512).collect();
                EventKind::ToolCall {
                    tool: if tool.is_empty() {
                        "opencode_tool".into()
                    } else {
                        tool
                    },
                    args_preview,
                    ok: true,
                    duration_ms: 0,
                    result_preview: String::new(),
                }
            }
            ("assistant", PartData::StepFinish { reason: _ }) => EventKind::AssistantMsg {
                text: String::new(),
                truncated: false,
                token_usage: envelope.tokens.as_ref().map(|t| TokenUsage {
                    prompt_tokens: t.input,
                    completion_tokens: t.output,
                    cached_tokens: t.cache.as_ref().map(|c| c.read),
                }),
            },
            _ => continue,
        };
        events.push(AgentEventV1 {
            id: Uuid::new_v4(),
            source: AgentSource::OpenCode,
            session_id: message.session_id.clone(),
            turn_id: Some(message.id.clone()),
            cwd: cwd.clone(),
            repo: repo.clone(),
            occurred_at,
            kind,
        });
    }

    Ok(events)
}
