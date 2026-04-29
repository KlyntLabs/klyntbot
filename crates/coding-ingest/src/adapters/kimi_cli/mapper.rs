//! Map kimi `WireRecord`s to `AgentEventV1`s.
//!
//! Tool calls are buffered per `call_id` because `EventKind::ToolCall`
//! requires both invocation (tool, args_preview) and result (ok,
//! duration_ms, result_preview) at emit time, while kimi emits these in
//! two separate wire messages (`ToolCall` and `ToolResult`).

use crate::adapters::kimi_cli::wire_file::{CollectedEvent, WireRecord};
use crate::event::{AgentEventV1, AgentSource, EventKind, TokenUsage};
use crate::scope_resolver::resolve_scope;
use jiff::Timestamp;
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

const PREVIEW_MAX: usize = 1024;

/// Per-session state carried across consecutive lines.
#[derive(Debug, Default)]
pub struct SessionState {
    /// Buffered `ToolCall { id → (tool, args_preview, started_at_ms) }` waiting
    /// on a matching `ToolResult`.
    pending_tools: HashMap<String, PendingTool>,
    /// `model` extracted from the most recent `StatusUpdate` (kimi reports
    /// usage on `StatusUpdate`, not on the prior assistant text).
    pub last_model: Option<String>,
    /// Whether we've already emitted a `SessionStart` for this session.
    pub session_start_emitted: bool,
}

#[derive(Debug, Clone)]
struct PendingTool {
    tool: String,
    args_preview: String,
    started_at: Timestamp,
}

/// Convert one collected event into zero or more `AgentEventV1`s.
///
/// `state` is mutated. `session_id`, `cwd` are stable for the file.
pub fn map_event(
    state: &mut SessionState,
    collected: &CollectedEvent,
    record: &WireRecord,
    session_id: &str,
    cwd: &std::path::Path,
) -> Vec<AgentEventV1> {
    let occurred_at = unix_seconds_to_ts(record.timestamp);
    let repo = resolve_scope(cwd);
    let turn_id = collected.agent_id.clone();
    let payload = &collected.payload;
    match collected.kind.as_str() {
        "TurnBegin" => map_turn_begin(payload, &repo, turn_id, session_id, cwd, occurred_at),
        "TextPart" => map_text_part(state, payload, &repo, turn_id, session_id, cwd, occurred_at),
        "ToolCall" => {
            buffer_tool_call(state, payload, occurred_at);
            vec![]
        }
        "ToolResult" => {
            map_tool_result(state, payload, &repo, turn_id, session_id, cwd, occurred_at)
        }
        "StatusUpdate" => {
            update_status(state, payload);
            vec![]
        }
        "TurnEnd" | "StepBegin" | "StepInterrupted" | "CompactionBegin" | "CompactionEnd"
        | "MCPLoadingBegin" | "MCPLoadingEnd" | "HookTriggered" | "HookResolved" | "BtwBegin"
        | "BtwEnd" | "PlanDisplay" | "ImageURLPart" | "AudioURLPart" | "VideoURLPart"
        | "ThinkPart" | "ToolCallPart" | "Notification" | "ApprovalRequest"
        | "ApprovalResponse" | "QuestionRequest" | "QuestionResponse" | "HookRequest"
        | "HookResponse" | "ToolCallRequest" | "SteerInput" => vec![],
        other => {
            tracing::debug!(kind = other, "kimi mapper: skipping unknown event type");
            vec![]
        }
    }
}

/// Emit a `SessionStart` event the first time the mapper sees this session.
/// Called by the poller on first record per file.
pub fn maybe_emit_session_start(
    state: &mut SessionState,
    session_id: &str,
    cwd: &std::path::Path,
    occurred_at: Timestamp,
    model: Option<String>,
) -> Option<AgentEventV1> {
    if state.session_start_emitted {
        return None;
    }
    state.session_start_emitted = true;
    Some(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: session_id.to_string(),
        turn_id: None,
        cwd: cwd.to_path_buf(),
        repo: resolve_scope(cwd),
        occurred_at,
        kind: EventKind::SessionStart {
            model,
            source_reason: "kimi-cli".into(),
        },
    })
}

fn map_turn_begin(
    payload: &Value,
    repo: &Option<crate::RepoScope>,
    turn_id: Option<String>,
    session_id: &str,
    cwd: &std::path::Path,
    occurred_at: Timestamp,
) -> Vec<AgentEventV1> {
    let text = extract_user_input_text(payload.get("user_input"));
    if text.is_empty() {
        return vec![];
    }
    vec![AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: session_id.to_string(),
        turn_id,
        cwd: cwd.to_path_buf(),
        repo: repo.clone(),
        occurred_at,
        kind: EventKind::UserPrompt {
            text,
            attachments: vec![],
        },
    }]
}

fn map_text_part(
    _state: &mut SessionState,
    payload: &Value,
    repo: &Option<crate::RepoScope>,
    turn_id: Option<String>,
    session_id: &str,
    cwd: &std::path::Path,
    occurred_at: Timestamp,
) -> Vec<AgentEventV1> {
    // Assistant `TextPart` payload shape: `{"text": "..."}`.
    let text = payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    if text.is_empty() {
        return vec![];
    }
    vec![AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: session_id.to_string(),
        turn_id,
        cwd: cwd.to_path_buf(),
        repo: repo.clone(),
        occurred_at,
        kind: EventKind::AssistantMsg {
            text,
            truncated: false,
            token_usage: None,
        },
    }]
}

fn buffer_tool_call(state: &mut SessionState, payload: &Value, occurred_at: Timestamp) {
    let id = match payload.get("id").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return,
    };
    let tool = payload
        .get("function")
        .and_then(|f| f.get("name"))
        .and_then(Value::as_str)
        .or_else(|| payload.get("name").and_then(Value::as_str))
        .unwrap_or("")
        .to_string();
    let args_preview = payload
        .get("function")
        .and_then(|f| f.get("arguments"))
        .and_then(Value::as_str)
        .or_else(|| payload.get("arguments").and_then(Value::as_str))
        .map(|s| truncate_preview(s))
        .unwrap_or_default();
    state.pending_tools.insert(
        id,
        PendingTool {
            tool,
            args_preview,
            started_at: occurred_at,
        },
    );
}

fn map_tool_result(
    state: &mut SessionState,
    payload: &Value,
    repo: &Option<crate::RepoScope>,
    turn_id: Option<String>,
    session_id: &str,
    cwd: &std::path::Path,
    occurred_at: Timestamp,
) -> Vec<AgentEventV1> {
    let id = match payload.get("id").and_then(Value::as_str) {
        Some(s) if !s.is_empty() => s.to_string(),
        _ => return vec![],
    };
    let pending = match state.pending_tools.remove(&id) {
        Some(p) => p,
        None => return vec![],
    };
    let result_preview = payload
        .get("content")
        .map(serde_value_preview)
        .unwrap_or_default();
    let ok = !payload
        .get("is_error")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let duration_ms =
        u32::try_from((occurred_at.as_millisecond() - pending.started_at.as_millisecond()).max(0))
            .unwrap_or(u32::MAX);
    vec![AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::KimiCli,
        session_id: session_id.to_string(),
        turn_id,
        cwd: cwd.to_path_buf(),
        repo: repo.clone(),
        occurred_at,
        kind: EventKind::ToolCall {
            tool: pending.tool,
            args_preview: pending.args_preview,
            ok,
            duration_ms,
            result_preview,
        },
    }]
}

fn update_status(_state: &mut SessionState, payload: &Value) {
    if let Some(usage) = payload.get("token_usage") {
        let _ = TokenUsage {
            prompt_tokens: usage
                .get("prompt_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            completion_tokens: usage
                .get("completion_tokens")
                .and_then(Value::as_u64)
                .unwrap_or(0) as u32,
            cached_tokens: usage
                .get("cached_tokens")
                .and_then(Value::as_u64)
                .map(|v| v as u32),
        };
        // Token-usage attachment to the prior AssistantMsg is post-emission
        // (the row is already in ingest_event_log). Distiller-side enrichment
        // is parked — recording the parse here keeps the door open without
        // breaking anything if usage shape drifts.
    }
}

fn extract_user_input_text(input: Option<&Value>) -> String {
    let Some(value) = input else {
        return String::new();
    };
    if let Some(s) = value.as_str() {
        return s.to_string();
    }
    if let Some(arr) = value.as_array() {
        return arr
            .iter()
            .filter_map(|part| {
                let kind = part.get("type").and_then(Value::as_str)?;
                if kind == "text" {
                    Some(part.get("text").and_then(Value::as_str)?.to_string())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("");
    }
    String::new()
}

fn truncate_preview(s: &str) -> String {
    if s.len() <= PREVIEW_MAX {
        s.to_string()
    } else {
        format!("{}…[{} bytes]", &s[..PREVIEW_MAX], s.len())
    }
}

fn serde_value_preview(v: &Value) -> String {
    let s = match v {
        Value::String(s) => s.clone(),
        _ => v.to_string(),
    };
    truncate_preview(&s)
}

fn unix_seconds_to_ts(s: f64) -> Timestamp {
    let secs = s.trunc() as i64;
    let nanos = ((s.fract() * 1_000_000_000.0) as i64).clamp(0, 999_999_999) as i32;
    Timestamp::new(secs, nanos).unwrap_or_else(|_| Timestamp::now())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapters::kimi_cli::wire_file::WireEnvelope;
    use std::path::Path;

    fn record(env: WireEnvelope, ts: f64) -> WireRecord {
        WireRecord {
            timestamp: ts,
            message: env,
        }
    }

    fn collected(kind: &str, payload: Value) -> CollectedEvent {
        CollectedEvent {
            kind: kind.into(),
            payload,
            agent_id: None,
        }
    }

    #[test]
    fn turn_begin_emits_user_prompt() {
        let mut state = SessionState::default();
        let c = collected(
            "TurnBegin",
            serde_json::json!({"user_input": [{"type":"text","text":"hi"}]}),
        );
        let r = record(
            WireEnvelope {
                kind: "TurnBegin".into(),
                payload: c.payload.clone(),
            },
            100.0,
        );
        let out = map_event(&mut state, &c, &r, "sess1", Path::new("/tmp"));
        assert_eq!(out.len(), 1);
        match &out[0].kind {
            EventKind::UserPrompt { text, .. } => assert_eq!(text, "hi"),
            other => panic!("expected UserPrompt, got {other:?}"),
        }
    }

    #[test]
    fn turn_begin_string_input_works() {
        let mut state = SessionState::default();
        let c = collected("TurnBegin", serde_json::json!({"user_input": "plain"}));
        let r = record(
            WireEnvelope {
                kind: "TurnBegin".into(),
                payload: c.payload.clone(),
            },
            1.0,
        );
        let out = map_event(&mut state, &c, &r, "s", Path::new("/tmp"));
        assert_eq!(out.len(), 1);
    }

    #[test]
    fn text_part_emits_assistant_msg() {
        let mut state = SessionState::default();
        let c = collected("TextPart", serde_json::json!({"text":"hello back"}));
        let r = record(
            WireEnvelope {
                kind: "TextPart".into(),
                payload: c.payload.clone(),
            },
            1.0,
        );
        let out = map_event(&mut state, &c, &r, "s", Path::new("/tmp"));
        assert_eq!(out.len(), 1);
        assert!(matches!(out[0].kind, EventKind::AssistantMsg { .. }));
    }

    #[test]
    fn tool_call_then_result_emits_one_toolcall() {
        let mut state = SessionState::default();
        // ToolCall — buffered, no emission.
        let c1 = collected(
            "ToolCall",
            serde_json::json!({"id":"c1","function":{"name":"Read","arguments":"{\"path\":\"/x\"}"}}),
        );
        let r1 = record(
            WireEnvelope {
                kind: "ToolCall".into(),
                payload: c1.payload.clone(),
            },
            100.0,
        );
        let out1 = map_event(&mut state, &c1, &r1, "s", Path::new("/tmp"));
        assert!(out1.is_empty(), "ToolCall must buffer, not emit");

        // ToolResult — pairs with buffered ToolCall.
        let c2 = collected(
            "ToolResult",
            serde_json::json!({"id":"c1","content":"ok","is_error":false}),
        );
        let r2 = record(
            WireEnvelope {
                kind: "ToolResult".into(),
                payload: c2.payload.clone(),
            },
            100.5,
        );
        let out2 = map_event(&mut state, &c2, &r2, "s", Path::new("/tmp"));
        assert_eq!(out2.len(), 1);
        match &out2[0].kind {
            EventKind::ToolCall {
                tool,
                ok,
                duration_ms,
                ..
            } => {
                assert_eq!(tool, "Read");
                assert!(*ok);
                assert!(*duration_ms <= 1000, "should be ~500ms, got {duration_ms}");
            }
            other => panic!("expected ToolCall, got {other:?}"),
        }
    }

    #[test]
    fn unknown_kind_is_skipped() {
        let mut state = SessionState::default();
        let c = collected("BrandNewType_2027", serde_json::json!({}));
        let r = record(
            WireEnvelope {
                kind: c.kind.clone(),
                payload: c.payload.clone(),
            },
            1.0,
        );
        let out = map_event(&mut state, &c, &r, "s", Path::new("/tmp"));
        assert!(out.is_empty());
    }

    #[test]
    fn session_start_emitted_once() {
        let mut state = SessionState::default();
        let ts = unix_seconds_to_ts(100.0);
        let first =
            maybe_emit_session_start(&mut state, "s", Path::new("/tmp"), ts, Some("k2".into()));
        let second =
            maybe_emit_session_start(&mut state, "s", Path::new("/tmp"), ts, Some("k2".into()));
        assert!(first.is_some());
        assert!(second.is_none());
    }
}
