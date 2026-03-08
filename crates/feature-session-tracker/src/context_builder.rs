use common::Result;
use providers::{ChatParams, DynProvider, Message};

use crate::repos::SessionTrackerRepos;
use crate::types::{ChunkSummary, PinnedMessage, SessionContext, SessionMessage};

pub struct ContextBuilder {
    repos: SessionTrackerRepos,
    window_size: usize,
}

impl ContextBuilder {
    pub fn new(repos: SessionTrackerRepos) -> Self {
        Self {
            repos,
            window_size: 30,
        }
    }

    pub fn with_window_size(mut self, size: usize) -> Self {
        self.window_size = size;
        self
    }

    /// Build the full context for a brainstorming session.
    pub async fn build_context(
        &self,
        session_id: &str,
        mut all_messages: Vec<SessionMessage>,
    ) -> Result<SessionContext> {
        let (rolling_summary, pinned_messages) = tokio::try_join!(
            async {
                self.repos
                    .get_latest_summary(session_id)
                    .await
                    .map(|s| s.unwrap_or_default())
            },
            self.repos.list_pins(session_id),
        )?;

        let total_messages = all_messages.len() as i64;
        let recent_messages = if all_messages.len() > self.window_size {
            all_messages.split_off(all_messages.len() - self.window_size)
        } else {
            all_messages
        };

        let estimated_tokens =
            estimate_tokens(&rolling_summary, &pinned_messages, &recent_messages);

        Ok(SessionContext {
            rolling_summary,
            pinned_messages,
            recent_messages,
            total_messages,
            estimated_tokens,
        })
    }

    /// Generate a summary for a chunk of messages using an LLM.
    pub async fn summarize_chunk(
        &self,
        provider: &DynProvider,
        session_id: &str,
        messages: &[SessionMessage],
        chunk_start: i64,
        previous_summary: Option<&str>,
    ) -> Result<ChunkSummary> {
        let conversation_text = format_messages_for_summary(messages);

        let prompt = build_summarization_prompt(&conversation_text, previous_summary);

        let llm_messages = vec![
            Message::system(
                "You are a precise technical summarizer. Output only the summary, no preamble.",
            ),
            Message::user(prompt),
        ];

        let params = ChatParams::new(provider.default_model())
            .with_max_tokens(500)
            .with_temperature(0.2);

        let response = provider.chat(&llm_messages, None, &params).await?;
        let summary_text = response
            .content
            .filter(|s| !s.trim().is_empty())
            .ok_or_else(|| {
                common::KlyntbotError::Provider(common::ProviderError::InvalidResponse(
                    "LLM returned empty summary".into(),
                ))
            })?;

        let chunk = ChunkSummary {
            session_id: session_id.to_string(),
            chunk_start,
            chunk_end: chunk_start + messages.len() as i64,
            summary: summary_text.clone(),
            files_touched: extract_files_from_messages(messages),
            key_decisions: Vec::new(),
            rolling_summary: summary_text,
        };

        self.repos.save_summary(&chunk).await?;

        Ok(chunk)
    }

    /// Format context as a system message for LLM injection.
    pub fn format_as_system_message(context: &SessionContext) -> String {
        let mut parts = Vec::new();

        parts.push("## Active Development Session Context".to_string());

        if !context.rolling_summary.is_empty() {
            parts.push(format!("### Session Summary\n{}", context.rolling_summary));
        }

        if !context.pinned_messages.is_empty() {
            let pins: Vec<String> = context
                .pinned_messages
                .iter()
                .enumerate()
                .map(|(i, p)| {
                    format!(
                        "[Pin {}] ({}): {}",
                        i + 1,
                        p.message_role,
                        p.message_content
                    )
                })
                .collect();
            parts.push(format!("### Pinned Messages\n{}", pins.join("\n")));
        }

        if !context.recent_messages.is_empty() {
            let msgs: Vec<String> = context
                .recent_messages
                .iter()
                .map(format_message_for_context)
                .filter(|s| !s.is_empty())
                .collect();
            parts.push(format!(
                "### Recent Conversation (last {} messages)\n{}",
                msgs.len(),
                msgs.join("\n")
            ));
        }

        parts.join("\n\n")
    }
}

fn estimate_tokens(summary: &str, pins: &[PinnedMessage], recent: &[SessionMessage]) -> usize {
    summary.len() / 4
        + pins
            .iter()
            .map(|p| p.message_content.len() / 4)
            .sum::<usize>()
        + recent
            .iter()
            .map(|m| message_char_len(m) / 4)
            .sum::<usize>()
}

fn message_char_len(msg: &SessionMessage) -> usize {
    match msg {
        SessionMessage::User { text, .. } => text.len(),
        SessionMessage::Assistant { content, .. } => content
            .iter()
            .map(|b| match b {
                crate::types::ContentBlock::Text { text } => text.len(),
                crate::types::ContentBlock::ToolUse { name, input, .. } => {
                    name.len() + input.to_string().len()
                }
                crate::types::ContentBlock::ToolResult { content, .. } => content.to_string().len(),
            })
            .sum(),
        SessionMessage::System { content, .. } => content.len(),
        SessionMessage::Progress { data, .. } => data.to_string().len(),
        SessionMessage::QueueOperation { content, .. } => content.as_deref().map_or(0, |c| c.len()),
    }
}

fn format_message_for_context(msg: &SessionMessage) -> String {
    match msg {
        SessionMessage::User { text, .. } => format!("[user]: {text}"),
        SessionMessage::Assistant { content, .. } => {
            let parts: Vec<String> = content
                .iter()
                .filter_map(|b| match b {
                    crate::types::ContentBlock::Text { text } => Some(text.clone()),
                    crate::types::ContentBlock::ToolUse { name, .. } => {
                        Some(format!("[tool: {name}]"))
                    }
                    crate::types::ContentBlock::ToolResult { .. } => None,
                })
                .collect();
            format!("[assistant]: {}", parts.join(" "))
        }
        SessionMessage::System { content, .. } => format!("[system]: {content}"),
        SessionMessage::Progress { .. } => String::new(),
        SessionMessage::QueueOperation { content, .. } => {
            format!("[injected]: {}", content.as_deref().unwrap_or(""))
        }
    }
}

fn format_messages_for_summary(messages: &[SessionMessage]) -> String {
    messages
        .iter()
        .map(format_message_for_context)
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn build_summarization_prompt(conversation: &str, previous_summary: Option<&str>) -> String {
    let mut prompt = String::from(
        "Summarize this development session chunk concisely. Extract:\n\
         - Current task/goal\n\
         - Key decisions made\n\
         - Files created/modified\n\
         - Problems encountered\n\
         - Current state/blocker (if any)\n\n\
         Keep under 200 words. Be factual.\n\n",
    );

    if let Some(prev) = previous_summary {
        prompt.push_str(&format!("Previous summary to merge with:\n{prev}\n\n"));
    }

    prompt.push_str(&format!("Conversation chunk:\n{conversation}"));
    prompt
}

fn extract_files_from_messages(messages: &[SessionMessage]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut files = Vec::new();
    for msg in messages {
        if let SessionMessage::Assistant { content, .. } = msg {
            for block in content {
                if let crate::types::ContentBlock::ToolUse { name, input, .. } = block {
                    if matches!(name.as_str(), "Read" | "Edit" | "Write") {
                        if let Some(path) = input.get("file_path").and_then(|v| v.as_str()) {
                            if seen.insert(path.to_string()) {
                                files.push(path.to_string());
                            }
                        }
                    }
                }
            }
        }
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn make_user_msg(text: &str) -> SessionMessage {
        SessionMessage::User {
            uuid: "u1".into(),
            text: text.into(),
            timestamp: Utc::now(),
            is_meta: false,
        }
    }

    fn make_assistant_msg(text: &str) -> SessionMessage {
        SessionMessage::Assistant {
            uuid: "a1".into(),
            content: vec![crate::types::ContentBlock::Text { text: text.into() }],
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_format_as_system_message_empty() {
        let ctx = SessionContext {
            rolling_summary: String::new(),
            pinned_messages: vec![],
            recent_messages: vec![],
            total_messages: 0,
            estimated_tokens: 0,
        };
        let result = ContextBuilder::format_as_system_message(&ctx);
        assert_eq!(result, "## Active Development Session Context");
    }

    #[test]
    fn test_format_as_system_message_with_summary_and_pins() {
        let ctx = SessionContext {
            rolling_summary: "Working on auth module".into(),
            pinned_messages: vec![PinnedMessage {
                id: 1,
                session_id: "s1".into(),
                message_uuid: "m1".into(),
                message_content: "Use JWT tokens".into(),
                message_role: "assistant".into(),
                pin_order: 1,
                created_at: Utc::now(),
            }],
            recent_messages: vec![make_user_msg("hello"), make_assistant_msg("hi there")],
            total_messages: 2,
            estimated_tokens: 100,
        };
        let result = ContextBuilder::format_as_system_message(&ctx);
        assert!(result.contains("Working on auth module"));
        assert!(result.contains("[Pin 1] (assistant): Use JWT tokens"));
        assert!(result.contains("[user]: hello"));
        assert!(result.contains("[assistant]: hi there"));
    }

    #[test]
    fn test_format_message_for_context_tool_use() {
        let msg = SessionMessage::Assistant {
            uuid: "a1".into(),
            content: vec![
                crate::types::ContentBlock::Text {
                    text: "Let me read that.".into(),
                },
                crate::types::ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Read".into(),
                    input: serde_json::json!({"file_path": "src/main.rs"}),
                },
            ],
            timestamp: Utc::now(),
        };
        let result = format_message_for_context(&msg);
        assert!(result.contains("[tool: Read]"));
        assert!(result.contains("Let me read that."));
    }

    #[test]
    fn test_extract_files_from_messages() {
        let messages = vec![SessionMessage::Assistant {
            uuid: "a1".into(),
            content: vec![
                crate::types::ContentBlock::ToolUse {
                    id: "t1".into(),
                    name: "Read".into(),
                    input: serde_json::json!({"file_path": "src/main.rs"}),
                },
                crate::types::ContentBlock::ToolUse {
                    id: "t2".into(),
                    name: "Edit".into(),
                    input: serde_json::json!({"file_path": "src/lib.rs"}),
                },
                crate::types::ContentBlock::ToolUse {
                    id: "t3".into(),
                    name: "Read".into(),
                    input: serde_json::json!({"file_path": "src/main.rs"}),
                },
            ],
            timestamp: Utc::now(),
        }];
        let files = extract_files_from_messages(&messages);
        assert_eq!(files, vec!["src/main.rs", "src/lib.rs"]);
    }

    #[test]
    fn test_estimate_tokens() {
        let summary = "a".repeat(400); // 100 tokens
        let pins = vec![PinnedMessage {
            id: 1,
            session_id: "s1".into(),
            message_uuid: "m1".into(),
            message_content: "b".repeat(200),
            message_role: "user".into(),
            pin_order: 1,
            created_at: Utc::now(),
        }];
        let recent = vec![make_user_msg(&"c".repeat(80))];
        let tokens = estimate_tokens(&summary, &pins, &recent);
        assert_eq!(tokens, 100 + 50 + 20); // 400/4 + 200/4 + 80/4
    }

    #[test]
    fn test_build_summarization_prompt_with_previous() {
        let prompt = build_summarization_prompt("some convo", Some("prev summary"));
        assert!(prompt.contains("Previous summary to merge with:"));
        assert!(prompt.contains("prev summary"));
        assert!(prompt.contains("some convo"));
    }

    #[test]
    fn test_build_summarization_prompt_without_previous() {
        let prompt = build_summarization_prompt("some convo", None);
        assert!(!prompt.contains("Previous summary"));
        assert!(prompt.contains("some convo"));
    }
}
