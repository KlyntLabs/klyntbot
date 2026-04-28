//! Convert opencode `MessageRow` into `AgentEventV1`.

use crate::event::FileOp;
use crate::event::{AgentEventV1, AgentSource, EventKind, TokenUsage};
use common::Result;
use jiff::Timestamp;
use std::path::PathBuf;
use uuid::Uuid;

use super::schema::MessageRow;

pub fn row_to_event(row: MessageRow) -> Result<Option<AgentEventV1>> {
    let kind = match row.role.as_str() {
        "system" => return Ok(None),
        "user" => EventKind::UserPrompt {
            text: row.content,
            attachments: vec![],
        },
        "assistant" => {
            let (text, tool_calls) = parse_assistant_content(&row.content);
            if let Some(tc) = tool_calls {
                EventKind::ToolCall {
                    tool: tc.tool,
                    args_preview: tc.args_preview,
                    ok: true,
                    duration_ms: 0,
                    result_preview: String::new(),
                }
            } else {
                EventKind::AssistantMsg {
                    text,
                    truncated: false,
                    token_usage: None,
                }
            }
        }
        "tool" => EventKind::ToolCall {
            tool: "opencode_tool".into(),
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
        turn_id: None,
        cwd: PathBuf::from("/"),
        repo: None,
        occurred_at: Timestamp::now(),
        kind,
    }))
}

struct ParsedToolCall {
    tool: String,
    args_preview: String,
}

fn parse_assistant_content(content: &str) -> (String, Option<ParsedToolCall>) {
    // Heuristic: if content starts with a JSON object, treat as tool call.
    let trimmed = content.trim();
    if trimmed.starts_with('{') {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Some(name) = v.get("name").and_then(|n| n.as_str()) {
                let args =
                    serde_json::to_string(&v.get("arguments").unwrap_or(&serde_json::Value::Null))
                        .unwrap_or_default();
                return (
                    String::new(),
                    Some(ParsedToolCall {
                        tool: name.into(),
                        args_preview: args,
                    }),
                );
            }
        }
    }
    (content.to_string(), None)
}
