//! Maps Claude Code raw line shapes to `SemanticCategory`.

use crate::tracing::types::SemanticCategory;
use serde_json::Value;

/// Decision for a single content block within an assistant/user message,
/// or for a top-level event line.
pub fn categorize_line(top_kind: &str, payload: &Value) -> SemanticCategory {
    match top_kind {
        "system" => categorize_system(payload),
        "pr-link" => SemanticCategory::StatusUpdate,
        "attachment" | "permission-mode" | "ai-title" | "last-prompt"
        | "file-history-snapshot" | "queue-operation" => SemanticCategory::Other,
        _ => SemanticCategory::Other,
    }
}

fn categorize_system(payload: &Value) -> SemanticCategory {
    let sub = payload.get("subtype").and_then(Value::as_str).unwrap_or("");
    match sub {
        "compact_boundary" => SemanticCategory::CompactionBegin,
        "api_error" => SemanticCategory::Error,
        "turn_duration" | "stop_hook_summary" | "away_summary" | "local_command"
        | "scheduled_task_fire" => SemanticCategory::StatusUpdate,
        _ => SemanticCategory::Other,
    }
}

/// Categorize one content block within an `assistant` or `user` message.
pub fn categorize_content_block(role: &str, block: &Value) -> SemanticCategory {
    let block_type = block.get("type").and_then(Value::as_str).unwrap_or("");
    match (role, block_type) {
        ("assistant", "thinking") => SemanticCategory::Thinking,
        ("assistant", "text") => SemanticCategory::AssistantText,
        ("assistant", "tool_use") => SemanticCategory::ToolCall,
        ("user", "text") => SemanticCategory::UserInput,
        ("user", "image") => SemanticCategory::UserInput,
        ("user", "tool_result") => {
            let is_err = block
                .get("is_error")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            if is_err {
                SemanticCategory::Error
            } else {
                SemanticCategory::ToolResult
            }
        }
        _ => SemanticCategory::Other,
    }
}

/// User message whose `content` is a top-level string (slash-command echo).
pub fn user_string_content_category() -> SemanticCategory {
    SemanticCategory::UserInput
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn assistant_thinking() {
        let b = json!({"type":"thinking","thinking":"x"});
        assert_eq!(
            categorize_content_block("assistant", &b),
            SemanticCategory::Thinking
        );
    }

    #[test]
    fn assistant_text() {
        let b = json!({"type":"text","text":"hi"});
        assert_eq!(
            categorize_content_block("assistant", &b),
            SemanticCategory::AssistantText
        );
    }

    #[test]
    fn assistant_tool_use() {
        let b = json!({"type":"tool_use","name":"Bash","input":{"command":"ls"}});
        assert_eq!(
            categorize_content_block("assistant", &b),
            SemanticCategory::ToolCall
        );
    }

    #[test]
    fn user_text() {
        assert_eq!(
            categorize_content_block("user", &json!({"type":"text","text":"hi"})),
            SemanticCategory::UserInput
        );
    }

    #[test]
    fn user_image() {
        assert_eq!(
            categorize_content_block("user", &json!({"type":"image"})),
            SemanticCategory::UserInput
        );
    }

    #[test]
    fn user_tool_result_ok() {
        let b = json!({"type":"tool_result","is_error":false});
        assert_eq!(
            categorize_content_block("user", &b),
            SemanticCategory::ToolResult
        );
    }

    #[test]
    fn user_tool_result_error() {
        let b = json!({"type":"tool_result","is_error":true});
        assert_eq!(
            categorize_content_block("user", &b),
            SemanticCategory::Error
        );
    }

    #[test]
    fn user_string_content() {
        assert_eq!(
            user_string_content_category(),
            SemanticCategory::UserInput
        );
    }

    #[test]
    fn system_compact_boundary() {
        assert_eq!(
            categorize_line("system", &json!({"subtype":"compact_boundary"})),
            SemanticCategory::CompactionBegin
        );
    }

    #[test]
    fn system_api_error() {
        assert_eq!(
            categorize_line("system", &json!({"subtype":"api_error"})),
            SemanticCategory::Error
        );
    }

    #[test]
    fn system_status_subtypes() {
        for sub in [
            "turn_duration",
            "stop_hook_summary",
            "away_summary",
            "local_command",
            "scheduled_task_fire",
        ] {
            assert_eq!(
                categorize_line("system", &json!({"subtype":sub})),
                SemanticCategory::StatusUpdate,
                "subtype: {sub}"
            );
        }
    }

    #[test]
    fn system_unknown_subtype_is_other() {
        assert_eq!(
            categorize_line("system", &json!({"subtype":"bridge_status"})),
            SemanticCategory::Other
        );
    }

    #[test]
    fn pr_link_is_status_update() {
        assert_eq!(
            categorize_line("pr-link", &json!({})),
            SemanticCategory::StatusUpdate
        );
    }

    #[test]
    fn attachment_and_friends_are_other() {
        for kind in [
            "attachment",
            "permission-mode",
            "ai-title",
            "last-prompt",
            "file-history-snapshot",
            "queue-operation",
        ] {
            assert_eq!(
                categorize_line(kind, &json!({})),
                SemanticCategory::Other,
                "kind: {kind}"
            );
        }
    }
}
