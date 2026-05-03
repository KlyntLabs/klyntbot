use super::parts::MessagePart;

/// Joins all `Text` parts in a message into a single string.
/// Used by cognitive subsystems that operate on prose.
pub fn extract_text(parts: &[MessagePart]) -> String {
    parts
        .iter()
        .filter_map(|p| match p {
            MessagePart::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns `(call_id, output_text, is_error)` for every `ToolResult` part.
pub fn extract_tool_results(parts: &[MessagePart]) -> Vec<(String, String, bool)> {
    parts
        .iter()
        .filter_map(|p| match p {
            MessagePart::ToolResult {
                call_id,
                output,
                is_error,
            } => Some((call_id.clone(), output.text.clone(), *is_error)),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_text_joins_text_parts() {
        let parts = vec![
            MessagePart::Text {
                text: "hi".into(),
            },
            MessagePart::Reasoning {
                text: "thinking".into(),
                redacted: false,
            },
            MessagePart::Text {
                text: "there".into(),
            },
        ];
        assert_eq!(extract_text(&parts), "hi\nthere");
    }

    #[test]
    fn extract_tool_results_skips_other_kinds() {
        let parts = vec![
            MessagePart::ToolCall {
                call_id: "c1".into(),
                name: "bash".into(),
                args: serde_json::json!({}),
            },
            MessagePart::ToolResult {
                call_id: "c1".into(),
                output: ToolOutput {
                    text: "ok".into(),
                    mime: None,
                    truncated: false,
                },
                is_error: false,
            },
        ];
        let r = extract_tool_results(&parts);
        assert_eq!(r.len(), 1);
        assert_eq!(r[0], ("c1".into(), "ok".into(), false));
    }
}
