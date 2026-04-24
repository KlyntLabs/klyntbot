//! Claude Code adapter — 7 of Claude Code's hook events → `AgentEvent`.

mod payload;
mod dispatch;

use super::IngestAdapter;
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

/// Adapter for Claude Code hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct ClaudeCodeAdapter;

impl IngestAdapter for ClaudeCodeAdapter {
    fn source_name(&self) -> &'static str { "claude-code" }

    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>> {
        match hook_event {
            "SessionStart" => Ok(Some(wrap(parse_session_start(raw)?))),
            "SessionEnd" => Ok(Some(wrap(parse_session_end(raw)?))),
            "UserPromptSubmit" => Ok(Some(wrap(parse_user_prompt(raw)?))),
            "Stop" => Ok(Some(wrap(parse_stop(raw)?))),
            "PreCompact" => Ok(Some(wrap(parse_pre_compact(raw)?))),
            "PreToolUse" => Ok(None), // not recorded — used for approval layer only
            "PostToolUse" => dispatch::parse_post_tool_use(raw).map(Some).map(|o| o.map(wrap)),
            _ => Ok(None),
        }
    }
}

fn wrap(v1: AgentEventV1) -> AgentEvent { AgentEvent::V1(v1) }

fn base(common: payload::CommonEnvelope, kind: EventKind) -> AgentEventV1 {
    AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::ClaudeCode,
        session_id: common.session_id,
        turn_id: None,
        cwd: common.cwd,
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    }
}

fn decode<T: for<'de> serde::Deserialize<'de>>(raw: &[u8]) -> Result<T> {
    serde_json::from_slice(raw).map_err(|e| KlyntbotError::Storage(format!("claude-code decode: {e}")))
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
    let kind = EventKind::SessionEnd { reason: b.reason.unwrap_or_else(|| "unspecified".into()) };
    Ok(base(b.common, kind))
}

fn parse_user_prompt(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::UserPromptBody = decode(raw)?;
    let kind = EventKind::UserPrompt { text: b.prompt, attachments: b.attachments };
    Ok(base(b.common, kind))
}

fn parse_stop(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::StopBody = decode(raw)?;
    let kind = EventKind::AssistantMsg {
        text: String::new(),
        truncated: false,
        token_usage: None::<TokenUsage>,
    };
    Ok(base(b.common, kind))
}

fn parse_pre_compact(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::PreCompactBody = decode(raw)?;
    let kind = EventKind::CompactEvent {
        trigger: b.trigger.unwrap_or_else(|| "unknown".into()),
        token_count: 0,
    };
    Ok(base(b.common, kind))
}
