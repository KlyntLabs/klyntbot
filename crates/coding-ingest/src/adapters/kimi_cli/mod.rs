//! kimi-cli adapter — 13 hook events (tier 1) + Wire streaming (tier 2).
//!
//! Tier-1: shell hooks emit JSON per event.
//! Tier-2: Wire client connects to kimi-cli's local streaming socket.

mod payload;
use super::IngestAdapter;
use crate::event::{AgentEvent, AgentEventV1, AgentSource, EventKind, TokenUsage};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

/// Adapter for kimi-cli hook payloads.
#[derive(Debug, Default, Clone, Copy)]
pub struct KimiAdapter;

impl IngestAdapter for KimiAdapter {
    fn source_name(&self) -> &'static str {
        "kimi-cli"
    }

    fn parse(&self, hook_event: &str, raw: &[u8]) -> Result<Option<AgentEvent>> {
        match hook_event {
            "SessionStart" => Ok(Some(wrap(parse_session_start(raw)?))),
            "SessionEnd" => Ok(Some(wrap(parse_session_end(raw)?))),
            "UserPrompt" => Ok(Some(wrap(parse_user_prompt(raw)?))),
            "AssistantMsg" => Ok(Some(wrap(parse_assistant_msg(raw)?))),
            "ToolCall" => Ok(Some(wrap(parse_tool_call(raw)?))),
            "FileEdit" => Ok(Some(wrap(parse_file_edit(raw)?))),
            "TestRun" => Ok(Some(wrap(parse_test_run(raw)?))),
            "CompactEvent" => Ok(Some(wrap(parse_compact_event(raw)?))),
            "Error" => Ok(Some(wrap(parse_error(raw)?))),
            "SkillActivated" | "RecallInjected" | "ApprovalDecision" | "ProviderCall" => {
                // Rich events parsed as generic AgentEvent.
                decode_generic(raw).map(|o| o.map(wrap))
            }
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
        source: AgentSource::KimiCli,
        session_id: common.session_id,
        turn_id: common.turn_id,
        cwd: common.cwd,
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    }
}

fn decode<T: for<'de> serde::Deserialize<'de>>(raw: &[u8]) -> Result<T> {
    serde_json::from_slice(raw).map_err(|e| KlyntbotError::Storage(format!("kimi-cli decode: {e}")))
}

fn decode_generic(raw: &[u8]) -> Result<Option<AgentEventV1>> {
    serde_json::from_slice(raw)
        .map(Some)
        .map_err(|e| KlyntbotError::Storage(format!("kimi-cli generic decode: {e}")))
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

fn parse_assistant_msg(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::AssistantMsgBody = decode(raw)?;
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

fn parse_tool_call(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ToolCallBody = decode(raw)?;
    let args = serde_json::to_string(&b.args).unwrap_or_default();
    let result = serde_json::to_string(&b.result).unwrap_or_default();
    let kind = EventKind::ToolCall {
        tool: b.tool,
        args_preview: truncate(&args, 512),
        ok: b.ok,
        duration_ms: b.duration_ms,
        result_preview: truncate(&result, 512),
    };
    Ok(base(b.common, kind))
}

fn parse_file_edit(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::FileEditBody = decode(raw)?;
    let kind = EventKind::FileEdit {
        path: b.path,
        op: b.op,
        bytes: b.bytes,
        diff_preview: b.diff_preview,
    };
    Ok(base(b.common, kind))
}

fn parse_test_run(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::TestRunBody = decode(raw)?;
    let kind = EventKind::TestRun {
        command: b.command,
        framework: b.framework,
        passed: b.passed,
        failed: b.failed,
        duration_ms: b.duration_ms,
    };
    Ok(base(b.common, kind))
}

fn parse_compact_event(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::CompactEventBody = decode(raw)?;
    let kind = EventKind::CompactEvent {
        trigger: b.trigger.unwrap_or_else(|| "unknown".into()),
        token_count: b.token_count.unwrap_or(0),
    };
    Ok(base(b.common, kind))
}

fn parse_error(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ErrorBody = decode(raw)?;
    let kind = EventKind::Error {
        tool: b.tool,
        message: b.message.unwrap_or_else(|| "kimi-cli error".into()),
    };
    Ok(base(b.common, kind))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
