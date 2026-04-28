//! Tier-1 hook dispatch — kimi-cli emits 13 distinct event kinds via stdin
//! JSON payloads. Each `parse_*` returns a fully-typed `AgentEventV1` tagged
//! with `AgentSource::KimiCli`.

use super::payload;
use crate::event::{AgentEventV1, AgentSource, EventKind, TokenUsage};
use common::{KlyntbotError, Result};
use jiff::Timestamp;
use uuid::Uuid;

/// Dispatch a single hook event by name. Returns `Ok(None)` for unknown events
/// (kimi-cli adds new hooks faster than we can support them — silently skip).
pub fn dispatch(hook_event: &str, raw: &[u8]) -> Result<Option<AgentEventV1>> {
    Ok(Some(match hook_event {
        "SessionStart" => parse_session_start(raw)?,
        "SessionEnd" => parse_session_end(raw)?,
        "UserPrompt" => parse_user_prompt(raw)?,
        "AssistantMsg" => parse_assistant_msg(raw)?,
        "ToolCall" => parse_tool_call(raw)?,
        "FileEdit" => parse_file_edit(raw)?,
        "TestRun" => parse_test_run(raw)?,
        "CompactEvent" => parse_compact_event(raw)?,
        "Error" => parse_error(raw)?,
        "SkillActivated" => parse_skill_activated(raw)?,
        "RecallInjected" => parse_recall_injected(raw)?,
        "ApprovalDecision" => parse_approval_decision(raw)?,
        "ProviderCall" => parse_provider_call(raw)?,
        _ => return Ok(None),
    }))
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

fn parse_session_start(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionStartBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::SessionStart {
            model: b.model,
            source_reason: b.source.unwrap_or_else(|| "unknown".into()),
        },
    ))
}

fn parse_session_end(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SessionEndBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::SessionEnd {
            reason: b.reason.unwrap_or_else(|| "unspecified".into()),
        },
    ))
}

fn parse_user_prompt(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::UserPromptBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::UserPrompt {
            text: b.prompt,
            attachments: b.attachments,
        },
    ))
}

fn parse_assistant_msg(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::AssistantMsgBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::AssistantMsg {
            text: b.text.unwrap_or_default(),
            truncated: b.truncated.unwrap_or(false),
            token_usage: b.token_usage.map(|u| TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                cached_tokens: u.cached_tokens,
            }),
        },
    ))
}

fn parse_tool_call(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ToolCallBody = decode(raw)?;
    let args = serde_json::to_string(&b.args).unwrap_or_default();
    let result = serde_json::to_string(&b.result).unwrap_or_default();
    Ok(base(
        b.common,
        EventKind::ToolCall {
            tool: b.tool,
            args_preview: truncate(&args, 512),
            ok: b.ok,
            duration_ms: b.duration_ms,
            result_preview: truncate(&result, 512),
        },
    ))
}

fn parse_file_edit(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::FileEditBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::FileEdit {
            path: b.path,
            op: b.op,
            bytes: b.bytes,
            diff_preview: b.diff_preview,
        },
    ))
}

fn parse_test_run(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::TestRunBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::TestRun {
            command: b.command,
            framework: b.framework,
            passed: b.passed,
            failed: b.failed,
            duration_ms: b.duration_ms,
        },
    ))
}

fn parse_compact_event(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::CompactEventBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::CompactEvent {
            trigger: b.trigger.unwrap_or_else(|| "unknown".into()),
            token_count: b.token_count.unwrap_or(0),
        },
    ))
}

fn parse_error(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ErrorBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::Error {
            tool: b.tool,
            message: b.message.unwrap_or_else(|| "kimi-cli error".into()),
        },
    ))
}

fn parse_skill_activated(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::SkillActivatedBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::SkillActivated {
            skill_id: b.skill_id,
            source_path: b.source_path,
            trigger: b.trigger.unwrap_or_else(|| "unknown".into()),
        },
    ))
}

fn parse_recall_injected(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::RecallInjectedBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::RecallInjected {
            memory_ids: b.memory_ids,
            coverage_score: b.coverage_score,
            dead_end_warning: b.dead_end_warning,
        },
    ))
}

fn parse_approval_decision(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ApprovalDecisionBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::ApprovalDecision {
            tool: b.tool,
            decision: b.decision,
            layer: b.layer.unwrap_or_else(|| "unknown".into()),
        },
    ))
}

fn parse_provider_call(raw: &[u8]) -> Result<AgentEventV1> {
    let b: payload::ProviderCallBody = decode(raw)?;
    Ok(base(
        b.common,
        EventKind::ProviderCall {
            model: b.model,
            prompt_tokens: b.prompt_tokens,
            completion_tokens: b.completion_tokens,
            cost_usd: b.cost_usd,
            latency_ms: b.latency_ms,
            retries: b.retries,
        },
    ))
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}
