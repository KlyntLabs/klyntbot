//! Map Kimi raw event kinds to provider-agnostic SemanticCategory.

use crate::tracing::types::SemanticCategory;
use serde_json::Value;

pub fn kimi_kind_to_category(raw_kind: &str, payload: &Value) -> SemanticCategory {
    match raw_kind {
        "TurnBegin" => SemanticCategory::TurnBegin,
        "TurnEnd" => SemanticCategory::TurnEnd,
        "StepBegin" => SemanticCategory::StepBegin,
        "StepInterrupted" => SemanticCategory::StepInterrupted,
        "ToolCall" => SemanticCategory::ToolCall,
        "ToolCallPart" => SemanticCategory::ToolCallStream,
        "StatusUpdate" => SemanticCategory::StatusUpdate,
        "SubagentEvent" => SemanticCategory::Subagent,
        "CompactionBegin" => SemanticCategory::CompactionBegin,
        "CompactionEnd" => SemanticCategory::CompactionEnd,
        "ToolResult" => {
            if payload.get("is_error").and_then(Value::as_bool).unwrap_or(false) {
                SemanticCategory::Error
            } else {
                SemanticCategory::ToolResult
            }
        }
        "ContentPart" => match payload.get("type").and_then(Value::as_str) {
            Some("think") => SemanticCategory::Thinking,
            Some("text") => SemanticCategory::AssistantText,
            _ => SemanticCategory::Other,
        },
        _ => SemanticCategory::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn turn_begin() {
        assert_eq!(
            kimi_kind_to_category("TurnBegin", &json!({})),
            SemanticCategory::TurnBegin
        );
    }

    #[test]
    fn turn_end() {
        assert_eq!(
            kimi_kind_to_category("TurnEnd", &json!({})),
            SemanticCategory::TurnEnd
        );
    }

    #[test]
    fn step_begin() {
        assert_eq!(
            kimi_kind_to_category("StepBegin", &json!({"n":1})),
            SemanticCategory::StepBegin
        );
    }

    #[test]
    fn step_interrupted() {
        assert_eq!(
            kimi_kind_to_category("StepInterrupted", &json!({})),
            SemanticCategory::StepInterrupted
        );
    }

    #[test]
    fn content_part_think_is_thinking() {
        assert_eq!(
            kimi_kind_to_category("ContentPart", &json!({"type":"think","think":"..."})),
            SemanticCategory::Thinking
        );
    }

    #[test]
    fn content_part_text_is_assistant_text() {
        assert_eq!(
            kimi_kind_to_category("ContentPart", &json!({"type":"text","text":"hi"})),
            SemanticCategory::AssistantText
        );
    }

    #[test]
    fn tool_call() {
        assert_eq!(
            kimi_kind_to_category("ToolCall", &json!({})),
            SemanticCategory::ToolCall
        );
    }

    #[test]
    fn tool_call_part_is_stream() {
        assert_eq!(
            kimi_kind_to_category("ToolCallPart", &json!({})),
            SemanticCategory::ToolCallStream
        );
    }

    #[test]
    fn tool_result_ok() {
        assert_eq!(
            kimi_kind_to_category("ToolResult", &json!({"is_error":false})),
            SemanticCategory::ToolResult
        );
        // Default-undefined is_error treated as success.
        assert_eq!(
            kimi_kind_to_category("ToolResult", &json!({})),
            SemanticCategory::ToolResult
        );
    }

    #[test]
    fn tool_result_error() {
        assert_eq!(
            kimi_kind_to_category("ToolResult", &json!({"is_error":true})),
            SemanticCategory::Error
        );
    }

    #[test]
    fn status_update() {
        assert_eq!(
            kimi_kind_to_category("StatusUpdate", &json!({})),
            SemanticCategory::StatusUpdate
        );
    }

    #[test]
    fn subagent_event() {
        assert_eq!(
            kimi_kind_to_category("SubagentEvent", &json!({})),
            SemanticCategory::Subagent
        );
    }

    #[test]
    fn compaction_begin_end() {
        assert_eq!(
            kimi_kind_to_category("CompactionBegin", &json!({})),
            SemanticCategory::CompactionBegin
        );
        assert_eq!(
            kimi_kind_to_category("CompactionEnd", &json!({})),
            SemanticCategory::CompactionEnd
        );
    }

    #[test]
    fn unknown_is_other() {
        assert_eq!(
            kimi_kind_to_category("BrandNew2027", &json!({})),
            SemanticCategory::Other
        );
        assert_eq!(
            kimi_kind_to_category("metadata", &json!({})),
            SemanticCategory::Other
        );
    }

    #[test]
    fn content_part_unknown_subtype_is_other() {
        assert_eq!(
            kimi_kind_to_category("ContentPart", &json!({"type":"image"})),
            SemanticCategory::Other
        );
    }
}
