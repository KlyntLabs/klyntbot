//! Rust port of `kimi_cli.wire.file::parse_wire_file_line` from
//! [kimi-cli](https://github.com/MoonshotAI/kimi-cli) (Apache-2.0).
//!
//! Kimi writes per-session JSONL files at
//! `~/.kimi/sessions/<work_dir_hash>/<session_uuid>/wire.jsonl`. The first
//! line is a metadata header; every subsequent line is a `WireRecord` whose
//! `message.type` is the kimi `WireMessage` Python class name (e.g.
//! `"TurnBegin"`, `"TextPart"`, `"ToolCall"`, `"ToolResult"`,
//! `"SubagentEvent"`).

use serde::Deserialize;
use serde_json::Value;

/// First-line header. We only read `protocol_version` for tracing; we don't
/// gate on it today. Captured fixtures show version `"1.9"`.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct WireMetadata {
    /// The discriminator literal `"metadata"`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Wire protocol version string (e.g. `"1.9"`).
    pub protocol_version: String,
}

/// One non-metadata line. `timestamp` is unix epoch seconds (float).
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WireRecord {
    /// Unix epoch seconds; fractional component is microsecond precision.
    pub timestamp: f64,
    /// `WireMessageEnvelope` — `{ type: <ClassName>, payload: <object> }`.
    pub message: WireEnvelope,
}

/// `{type, payload}` envelope as emitted by kimi-cli's `WireMessageEnvelope`.
#[derive(Debug, Clone, Deserialize, PartialEq)]
pub struct WireEnvelope {
    /// Python class name of the `WireMessage` variant.
    #[serde(rename = "type")]
    pub kind: String,
    /// Variant-specific JSON payload.
    pub payload: Value,
}

/// Parsed line — either the metadata header or a record.
#[derive(Debug, Clone, PartialEq)]
pub enum WireLine {
    /// First line of the file.
    Metadata(WireMetadata),
    /// Any subsequent line.
    Record(WireRecord),
}

/// Parse a single line. Tries metadata first; falls back to record.
///
/// Mirrors `parse_wire_file_line` in kimi-cli's `wire/file.py`.
pub fn parse_line(line: &str) -> Result<WireLine, serde_json::Error> {
    if let Ok(meta) = serde_json::from_str::<WireMetadata>(line) {
        if meta.kind == "metadata" {
            return Ok(WireLine::Metadata(meta));
        }
    }
    let record: WireRecord = serde_json::from_str(line)?;
    Ok(WireLine::Record(record))
}

/// One unwrapped event with the closest enclosing subagent id (if any).
#[derive(Debug, Clone, PartialEq)]
pub struct CollectedEvent {
    /// Kimi `WireMessage` type name (e.g. `"TurnBegin"`).
    pub kind: String,
    /// Variant payload — same shape as `WireEnvelope.payload`.
    pub payload: Value,
    /// Innermost subagent `agent_id` if the event was wrapped.
    pub agent_id: Option<String>,
}

/// Flatten a `WireEnvelope`. Non-subagent events return as a single-element
/// vec with `agent_id: None`. `SubagentEvent`s are recursively unwrapped; the
/// innermost `agent_id` is attached to the leaf event.
pub fn collect_events(env: &WireEnvelope) -> Vec<CollectedEvent> {
    let mut out = Vec::new();
    collect_inner(&env.kind, &env.payload, None, &mut out);
    out
}

fn collect_inner(
    kind: &str,
    payload: &Value,
    parent_agent_id: Option<String>,
    out: &mut Vec<CollectedEvent>,
) {
    if kind == "SubagentEvent" {
        let inner_agent_id = payload
            .get("agent_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or(parent_agent_id);
        if let Some(inner) = payload.get("event") {
            let inner_kind = inner.get("type").and_then(Value::as_str).unwrap_or("");
            let inner_payload = inner.get("payload").cloned().unwrap_or(Value::Null);
            if !inner_kind.is_empty() {
                collect_inner(inner_kind, &inner_payload, inner_agent_id, out);
            }
        }
        return;
    }
    out.push(CollectedEvent {
        kind: kind.to_owned(),
        payload: payload.clone(),
        agent_id: parent_agent_id,
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_line_metadata_first_line() {
        let line = r#"{"type":"metadata","protocol_version":"1.9"}"#;
        let parsed = parse_line(line).expect("metadata line should parse");
        match parsed {
            WireLine::Metadata(m) => {
                assert_eq!(m.kind, "metadata");
                assert_eq!(m.protocol_version, "1.9");
            }
            WireLine::Record(_) => panic!("expected metadata"),
        }
    }

    #[test]
    fn parse_line_record_turnbegin() {
        let line = r#"{"timestamp":1777096658.415196,"message":{"type":"TurnBegin","payload":{"user_input":[{"type":"text","text":"hi"}]}}}"#;
        let parsed = parse_line(line).expect("record line should parse");
        match parsed {
            WireLine::Record(r) => {
                assert!((r.timestamp - 1777096658.415196).abs() < 1e-6);
                assert_eq!(r.message.kind, "TurnBegin");
                assert_eq!(
                    r.message.payload["user_input"][0]["text"].as_str().unwrap(),
                    "hi"
                );
            }
            WireLine::Metadata(_) => panic!("expected record"),
        }
    }

    #[test]
    fn parse_line_invalid_json_returns_err() {
        let res = parse_line("{not json");
        assert!(res.is_err(), "invalid JSON must error");
    }

    #[test]
    fn collect_events_passthrough_non_subagent() {
        let env = WireEnvelope {
            kind: "TurnBegin".into(),
            payload: serde_json::json!({"user_input": "hi"}),
        };
        let out = collect_events(&env);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "TurnBegin");
        assert_eq!(out[0].agent_id, None);
    }

    #[test]
    fn collect_events_unwraps_subagent_one_level() {
        let env = WireEnvelope {
            kind: "SubagentEvent".into(),
            payload: serde_json::json!({
                "agent_id": "sub-1",
                "subagent_type": "task",
                "event": {"type": "ToolCall", "payload": {"id": "c1"}}
            }),
        };
        let out = collect_events(&env);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "ToolCall");
        assert_eq!(out[0].agent_id.as_deref(), Some("sub-1"));
    }

    #[test]
    fn collect_events_unwraps_subagent_nested() {
        let env = WireEnvelope {
            kind: "SubagentEvent".into(),
            payload: serde_json::json!({
                "agent_id": "outer",
                "event": {
                    "type": "SubagentEvent",
                    "payload": {
                        "agent_id": "inner",
                        "event": {"type": "TurnBegin", "payload": {"user_input": "hi"}}
                    }
                }
            }),
        };
        let out = collect_events(&env);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].kind, "TurnBegin");
        // Innermost agent_id wins — that's the closest scope to the inner event.
        assert_eq!(out[0].agent_id.as_deref(), Some("inner"));
    }
}
