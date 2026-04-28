//! Convert opencode `MessageRow` into `AgentEventV1`.

use crate::event::{AgentEventV1, AgentSource, EventKind};
use crate::scope_resolver::resolve_scope;
use common::Result;
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

use super::schema::MessageRow;

/// Convert one opencode `MessageRow` into an `AgentEventV1`.
pub fn row_to_event(row: MessageRow) -> Result<Option<AgentEventV1>> {
    let metadata: Option<serde_json::Value> = row
        .metadata
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok());

    let cwd = metadata
        .as_ref()
        .and_then(|m| m.get("cwd").and_then(|v| v.as_str()))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/"));

    let repo = resolve_scope(&cwd);
    let turn_id = Some(format!("{}-{}", row.session_id, turn_bucket(row.id)));

    let kind = match row.role.as_str() {
        "system" => return Ok(None),
        "user" => EventKind::UserPrompt {
            text: row.content,
            attachments: vec![],
        },
        "assistant" => {
            // Use the structured tool_calls column, NOT a content-prefix heuristic.
            if let Some(tc_json) = row.tool_calls.as_deref() {
                if let Ok(tc_arr) = serde_json::from_str::<Vec<serde_json::Value>>(tc_json) {
                    if let Some(first) = tc_arr.first() {
                        let tool = first
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let args_preview = first
                            .get("arguments")
                            .and_then(|a| serde_json::to_string(a).ok())
                            .unwrap_or_default();
                        return Ok(Some(AgentEventV1 {
                            id: Uuid::new_v4(),
                            source: AgentSource::OpenCode,
                            session_id: row.session_id,
                            turn_id,
                            cwd,
                            repo,
                            occurred_at: parse_timestamp(&row.created_at),
                            kind: EventKind::ToolCall {
                                tool,
                                args_preview: args_preview.chars().take(512).collect(),
                                ok: true,
                                duration_ms: 0,
                                result_preview: String::new(),
                            },
                        }));
                    }
                }
            }
            EventKind::AssistantMsg {
                text: row.content,
                truncated: false,
                token_usage: None,
            }
        }
        "tool" => EventKind::ToolCall {
            tool: row.tool_call_id.unwrap_or_else(|| "opencode_tool".into()),
            args_preview: String::new(),
            ok: true,
            duration_ms: 0,
            result_preview: row.content,
        },
        _ => return Ok(None),
    };

    Ok(Some(AgentEventV1 {
        id: Uuid::new_v4(),
        source: AgentSource::OpenCode,
        session_id: row.session_id,
        turn_id,
        cwd,
        repo,
        occurred_at: parse_timestamp(&row.created_at),
        kind,
    }))
}

/// Group consecutive messages into the same logical turn. Naive: every block
/// of N messages = one turn. Use the row id as the bucket source so that
/// (user, assistant) pairs land together. The 1000-id stride is conservative;
/// turn boundaries are refined post-ingest by the distiller.
fn turn_bucket(id: i64) -> i64 {
    id / 2 // user + assistant pair = bucket
}

fn parse_timestamp(raw: &str) -> Timestamp {
    raw.parse::<i64>()
        .ok()
        .and_then(|s| Timestamp::from_second(s).ok())
        .unwrap_or_else(Timestamp::now)
}
