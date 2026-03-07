use crate::types::{ContentBlock, RawContent, RawSessionLine, SessionMessage};
use chrono::{DateTime, Utc};
use tracing::warn;

/// Parse a single JSONL line into a SessionMessage.
/// Returns None for lines that should be filtered out (snapshots, last-prompt, unparseable).
pub fn parse_line(line: &str) -> Option<SessionMessage> {
    let line = line.trim();
    if line.is_empty() {
        return None;
    }

    let raw: RawSessionLine = match serde_json::from_str(line) {
        Ok(r) => r,
        Err(e) => {
            warn!("Failed to parse session line: {e}");
            return None;
        }
    };

    convert_raw(raw)
}

/// Parse all lines from a JSONL string, returning only valid SessionMessages.
pub fn parse_lines(content: &str) -> Vec<SessionMessage> {
    content.lines().filter_map(parse_line).collect()
}

fn convert_raw(raw: RawSessionLine) -> Option<SessionMessage> {
    match raw {
        RawSessionLine::User {
            uuid,
            message,
            is_meta,
            timestamp,
            ..
        } => {
            let ts = parse_timestamp(&timestamp)?;
            let text = extract_text(&message.content);
            Some(SessionMessage::User {
                uuid,
                text,
                timestamp: ts,
                is_meta,
            })
        }
        RawSessionLine::Assistant {
            uuid,
            message,
            timestamp,
            ..
        } => {
            let ts = parse_timestamp(&timestamp)?;
            let content = match message.content {
                RawContent::Text(t) => vec![ContentBlock::Text { text: t }],
                RawContent::Blocks(blocks) => blocks,
            };
            Some(SessionMessage::Assistant {
                uuid,
                content,
                timestamp: ts,
            })
        }
        RawSessionLine::System {
            uuid,
            subtype,
            content,
            timestamp,
            ..
        } => {
            let ts = parse_timestamp(&timestamp)?;
            Some(SessionMessage::System {
                uuid,
                subtype,
                content,
                timestamp: ts,
            })
        }
        RawSessionLine::Progress {
            uuid,
            data,
            timestamp,
            tool_use_id,
        } => {
            let ts = parse_timestamp(&timestamp)?;
            Some(SessionMessage::Progress {
                uuid,
                data,
                tool_use_id,
                timestamp: ts,
            })
        }
        RawSessionLine::QueueOperation {
            operation,
            content,
            timestamp,
            ..
        } => {
            let ts = parse_timestamp(&timestamp)?;
            Some(SessionMessage::QueueOperation {
                operation,
                content,
                timestamp: ts,
            })
        }
        // Filter out non-conversational types
        RawSessionLine::FileHistorySnapshot { .. } | RawSessionLine::LastPrompt { .. } => None,
    }
}

fn extract_text(content: &RawContent) -> String {
    match content {
        RawContent::Text(t) => t.clone(),
        RawContent::Blocks(blocks) => blocks
            .iter()
            .filter_map(|b| match b {
                ContentBlock::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn parse_timestamp(ts: &str) -> Option<DateTime<Utc>> {
    ts.parse::<DateTime<Utc>>()
        .map_err(|e| warn!("Failed to parse timestamp '{ts}': {e}"))
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_user_message() {
        let line = r#"{"parentUuid":null,"isSidechain":false,"userType":"external","cwd":"/home/user/project","sessionId":"abc-123","version":"2.1.71","gitBranch":"main","type":"user","message":{"role":"human","content":[{"type":"text","text":"Hello world"}]},"isMeta":false,"uuid":"msg-001","timestamp":"2026-03-08T10:00:00.000Z"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::User {
                uuid,
                text,
                is_meta,
                ..
            } => {
                assert_eq!(uuid, "msg-001");
                assert_eq!(text, "Hello world");
                assert!(!is_meta);
            }
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn parse_assistant_with_tool_use() {
        let line = r#"{"parentUuid":"msg-001","isSidechain":false,"userType":"external","message":{"role":"assistant","content":[{"type":"text","text":"Let me read that file."},{"type":"tool_use","id":"tool-1","name":"Read","input":{"file_path":"/src/lib.rs"}}]},"type":"assistant","uuid":"msg-002","timestamp":"2026-03-08T10:00:01.000Z"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::Assistant { uuid, content, .. } => {
                assert_eq!(uuid, "msg-002");
                assert_eq!(content.len(), 2);
                assert!(matches!(&content[0], ContentBlock::Text { text } if text == "Let me read that file."));
                assert!(matches!(&content[1], ContentBlock::ToolUse { name, .. } if name == "Read"));
            }
            _ => panic!("Expected Assistant message"),
        }
    }

    #[test]
    fn parse_queue_operation() {
        let line = r#"{"type":"queue-operation","operation":"enqueue","timestamp":"2026-03-08T10:00:02.000Z","sessionId":"abc-123","content":"Please implement this feature"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::QueueOperation {
                operation,
                content,
                ..
            } => {
                assert_eq!(operation, "enqueue");
                assert_eq!(content.unwrap(), "Please implement this feature");
            }
            _ => panic!("Expected QueueOperation"),
        }
    }

    #[test]
    fn parse_file_history_snapshot_returns_none() {
        let line = r#"{"type":"file-history-snapshot","messageId":"snap-001","snapshot":{},"isSnapshotUpdate":false}"#;
        assert!(parse_line(line).is_none());
    }

    #[test]
    fn parse_empty_line_returns_none() {
        assert!(parse_line("").is_none());
        assert!(parse_line("   ").is_none());
    }

    #[test]
    fn parse_invalid_json_returns_none() {
        assert!(parse_line("{not valid json}").is_none());
    }

    #[test]
    fn parse_user_string_content() {
        let line = r#"{"type":"user","uuid":"msg-003","sessionId":"abc","message":{"role":"human","content":"simple string message"},"isMeta":false,"timestamp":"2026-03-08T10:00:00.000Z"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::User { text, .. } => {
                assert_eq!(text, "simple string message");
            }
            _ => panic!("Expected User message"),
        }
    }

    #[test]
    fn parse_progress_message() {
        let line = r#"{"type":"progress","uuid":"prog-001","data":{"type":"hook_progress","hookEvent":"SessionStart","hookName":"clear"},"timestamp":"2026-03-08T10:00:00.000Z","toolUseID":"tool-99"}"#;

        let msg = parse_line(line).unwrap();
        match msg {
            SessionMessage::Progress {
                uuid,
                tool_use_id,
                ..
            } => {
                assert_eq!(uuid, "prog-001");
                assert_eq!(tool_use_id.unwrap(), "tool-99");
            }
            _ => panic!("Expected Progress message"),
        }
    }

    #[test]
    fn parse_lines_filters_correctly() {
        let content = r#"{"type":"file-history-snapshot","messageId":"snap-001","snapshot":{},"isSnapshotUpdate":false}
{"type":"user","uuid":"msg-001","sessionId":"abc","message":{"role":"human","content":"Hello"},"isMeta":false,"timestamp":"2026-03-08T10:00:00.000Z"}
{"type":"assistant","uuid":"msg-002","message":{"role":"assistant","content":[{"type":"text","text":"Hi there"}]},"timestamp":"2026-03-08T10:00:01.000Z"}
invalid json line
{"type":"last-prompt","lastPrompt":"Hello","sessionId":"abc"}"#;

        let messages = parse_lines(content);
        assert_eq!(messages.len(), 2); // Only user + assistant
    }
}
