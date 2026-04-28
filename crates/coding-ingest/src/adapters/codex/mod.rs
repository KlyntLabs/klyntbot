//! Codex adapter — 5 hook events from OpenAI's Codex CLI.
//!
//! Codex emits JSON via shell hooks configured in its TOML settings.

mod dispatch;
mod payload;

use super::IngestAdapter;
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

/// Adapter for Codex hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct CodexAdapter;

impl IngestAdapter for CodexAdapter {
    fn source_name(&self) -> &'static str {
        "codex"
    }

    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>> {
        match hook_event {
            "SessionStart" => Ok(Some(wrap(parse_session_start(raw)?))),
            "SessionEnd" => Ok(Some(wrap(parse_session_end(raw)?))),
            "UserPromptSubmit" => Ok(Some(wrap(parse_user_prompt(raw)?))),
            "AssistantResponse" => Ok(Some(wrap(parse_assistant_response(raw)?))),
            "ToolUse" => dispatch::parse_tool_use(raw).map(|o| o.map(wrap)),
            _ => Ok(None),
        }
    }
}

fn wrap(v1: AgentEventV1) -> AgentEvent {
    AgentEvent::V1(v1)
}

fn base(common: payload::CommonEnvelope, kind: EventKind) -> AgentEventV1 {
    AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::Codex,
        session_id: common.session_id,
        turn_id: common.turn_id,
        cwd: common.cwd,
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    }
}

fn decode<T: for<'de> serde::Deserialize<'de>>(raw: &[u8]) -> Result<T> {
    serde_json::from_slice(raw)
        .map_err(|e| KlyntbotError::Storage(format!("codex decode: {e}")))
}

fn parse_session_start(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionStartBody = decode(raw)?;
    let kind = EventKind::SessionStart {
        model: b.model,
        source_reason: b.source.unwrap_or_else(|| "unknown".into()),
    };
    Ok(base(b.common, kind))
}

fn parse_session_end(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionEndBody = decode(raw)?;
    let kind = EventKind::SessionEnd {
        reason: b.reason.unwrap_or_else(|| "unspecified".into()),
    };
    Ok(base(b.common, kind))
}

fn parse_user_prompt(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::UserPromptBody = decode(raw)?;
    let kind = EventKind::UserPrompt {
        text: b.prompt,
        attachments: b.attachments,
    };
    Ok(base(b.common, kind))
}

fn parse_assistant_response(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::AssistantResponseBody = decode(raw)?;
    let kind = EventKind::AssistantMsg {
        text: b.text.unwrap_or_default(),
        truncated: b.truncated.unwrap_or(false),
        token_usage: b.token_usage.map(|u| TokenUsage {
            prompt_tokens: u.prompt_tokens,
            completion_tokens: u.completion_tokens,
            cached_tokens: u.cached_tokens,
        }),
    };
    Ok(base(b.common, kind))
}
