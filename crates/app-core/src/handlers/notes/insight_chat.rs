//! Streaming chat handler for Insight Review tabs.
//!
//! Each tab (synthesis, gap_analysis, concept_map, perspectives) exposes an
//! interactive chat interface backed by a persistent session. This module
//! provides the relay function that streams LLM tokens to the frontend via
//! `agent:content_chunk` / `agent:done` events, and the `AppCore` methods
//! that wire it together.
//!

use std::sync::Arc;

use desktop_shared::commands::{InsightChatParams, InsightChatStarted};
use desktop_shared::errors::ApiError;
use desktop_shared::events::{AGENT_CONTENT_CHUNK, AGENT_DONE, AGENT_ERROR};
use futures_util::StreamExt;
use storage::Repos;
use tracing::warn;

use crate::events::AppEventEmitter;
use crate::state::AppCore;

// ── Streaming relay ──────────────────────────────────────────────────────────

/// Relay a single LLM chat stream to the frontend event bus, persisting the
/// completed assistant message into the session store when done.
#[allow(clippy::too_many_arguments)]
pub(super) async fn relay_insight_stream(
    provider: providers::DynProvider,
    messages: Vec<providers::Message>,
    chat_params: providers::ChatParams,
    session_key: String,
    assistant_msg_id: uuid::Uuid,
    repos: Repos,
    emitter: Arc<dyn AppEventEmitter>,
) {
    let mut stream = match provider.chat_stream(&messages, None, &chat_params).await {
        Ok(s) => s,
        Err(e) => {
            emitter.emit_event(
                AGENT_ERROR,
                serde_json::json!({
                    "sessionKey": session_key,
                    "message": e.to_string(),
                }),
            );
            return;
        }
    };

    let mut full_content = String::new();

    while let Some(chunk_result) = StreamExt::next(&mut stream).await {
        match chunk_result {
            Ok(chunk) => {
                if let Some(text) = &chunk.content {
                    full_content.push_str(text);
                    emitter.emit_event(
                        AGENT_CONTENT_CHUNK,
                        serde_json::json!({
                            "sessionKey": session_key,
                            "data": text,
                        }),
                    );
                }
            }
            Err(e) => {
                emitter.emit_event(
                    AGENT_ERROR,
                    serde_json::json!({
                        "sessionKey": session_key,
                        "message": e.to_string(),
                    }),
                );
                return;
            }
        }
    }

    // Persist the completed assistant message.
    if let Err(e) = repos
        .sessions
        .add_message(
            &session_key,
            assistant_msg_id,
            "assistant",
            &full_content,
            None,
            None,
            None,
        )
        .await
    {
        warn!("insight_chat: failed to persist assistant message: {e}");
    }

    emitter.emit_event(
        AGENT_DONE,
        serde_json::json!({
            "sessionKey": session_key,
            "content": full_content,
        }),
    );
}

// ── System prompt ────────────────────────────────────────────────────────────

/// Build the system prompt for a tab chat session.
pub(super) fn tab_chat_system_prompt(
    tab_name: &str,
    note_title: &str,
    tab_content: &str,
    note_body: &str,
) -> String {
    let tab_label = match tab_name {
        "synthesis" => "synthesis",
        "gaps" | "gap_analysis" => "gap analysis",
        "assessment" | "self_assessment" => "self-assessment",
        "concept-map" | "concept_map" => "concept map",
        "perspectives" => "perspectives",
        _ => tab_name,
    };

    let body_preview = common::truncate_at_boundary(note_body, 3000);

    format!(
        "You are an intelligent assistant helping the user explore and discuss their note insights.\n\
         You are discussing the **{tab_label}** tab for the note titled \"{note_title}\".\n\n\
         --- NOTE: {note_title} ---\n\
         {body_preview}\n\
         --- END NOTE ---\n\n\
         --- {tab_label_upper} CONTENT ---\n\
         {tab_content}\n\
         --- END CONTENT ---\n\n\
         Answer the user's questions about this content concisely and accurately.\n\
         Reference specific parts of the content when relevant.\n\
         Keep responses focused (2-5 sentences unless more detail is explicitly requested).",
        tab_label = tab_label,
        note_title = note_title,
        body_preview = body_preview,
        tab_label_upper = tab_label.to_uppercase(),
        tab_content = if tab_content.is_empty() {
            "(Content not yet generated)"
        } else {
            tab_content
        },
    )
}

// ── AppCore methods ──────────────────────────────────────────────────────────

impl AppCore {
    /// Start a streaming chat turn for an Insight Review tab.
    ///
    /// Streams a single AI response.
    #[tracing::instrument(skip(self, emitter), err)]
    pub async fn note_insight_tab_chat(
        &self,
        params: &InsightChatParams,
        emitter: Arc<dyn crate::events::AppEventEmitter>,
    ) -> Result<InsightChatStarted, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?
            .clone();

        // Load note for title + context.
        let note = self
            .note_repo
            .get_note(&params.note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        // Resolve tab content from the latest insight (best-effort).
        let tab_content: String = if let Some(ref service) = self.insight_service {
            if let Ok(Some(row)) = service.get_latest(&params.note_id).await {
                let content: feature_insights::InsightContent =
                    serde_json::from_str(&row.content).unwrap_or_default();
                match params.tab_name.as_str() {
                    "synthesis" => content.synthesis.unwrap_or_default(),
                    "gaps" | "gap_analysis" => content.gap_analysis.unwrap_or_default(),
                    "assessment" | "self_assessment" => content.self_assessment.unwrap_or_default(),
                    "concept-map" | "concept_map" => content.concept_map.unwrap_or_default(),
                    "perspectives" => content.perspectives.unwrap_or_default(),
                    _ => String::new(),
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        // Ensure session exists.
        let session_metadata = serde_json::json!({
            "title": format!("{} · {}", note.title, &params.tab_name),
            "type": "insight_tab_chat",
            "noteId": params.note_id,
            "tab": params.tab_name,
        });
        self.repos
            .sessions
            .upsert_session(&params.session_key, &session_metadata)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        // Load prior history BEFORE inserting the new user message.
        let history = self
            .repos
            .sessions
            .get_recent_messages(&params.session_key, 20)
            .await
            .unwrap_or_default();

        // Persist the user message.
        let user_msg_id = uuid::Uuid::new_v4();
        self.repos
            .sessions
            .add_message(
                &params.session_key,
                user_msg_id,
                "user",
                &params.user_message,
                None,
                None,
                None,
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        // Build history messages for LLM context.
        let mut history_messages: Vec<providers::Message> = Vec::with_capacity(history.len());
        for msg in &history {
            match msg.role.as_str() {
                "user" => history_messages.push(providers::Message::User {
                    content: providers::UserContent::Text(msg.content.clone()),
                }),
                "assistant" => history_messages.push(providers::Message::Assistant {
                    content: Some(msg.content.clone()),
                    tool_calls: None,
                    reasoning_content: None,
                }),
                _ => {}
            }
        }

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 1024);
        drop(config);

        let session_key_clone = params.session_key.clone();

        let system_prompt =
            tab_chat_system_prompt(&params.tab_name, &note.title, &tab_content, &note.body);

        let mut llm_messages = Vec::with_capacity(history_messages.len() + 2);
        llm_messages.push(providers::Message::System {
            content: system_prompt,
        });
        llm_messages.extend(history_messages);
        llm_messages.push(providers::Message::User {
            content: providers::UserContent::Text(params.user_message.clone()),
        });

        let assistant_msg_id = uuid::Uuid::new_v4();
        let repos_clone = self.repos.clone();
        let emitter_clone = Arc::clone(&emitter);

        tokio::spawn(relay_insight_stream(
            provider,
            llm_messages,
            chat_params,
            session_key_clone,
            assistant_msg_id,
            repos_clone,
            emitter_clone,
        ));

        Ok(InsightChatStarted {
            session_key: params.session_key.clone(),
            message_id: user_msg_id.to_string(),
        })
    }

    /// Clear all chat sessions for a note's insight tabs.
    #[tracing::instrument(skip(self), err)]
    pub async fn note_insight_clear_tab_chats(&self, note_id: &str) -> Result<(), ApiError> {
        let prefix = format!("insight-chat:{note_id}:");
        self.repos
            .sessions
            .delete_sessions_by_prefix(&prefix)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }
}

// ── Helpers ──────────────────────────────────────────────────────────────────

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::tab_chat_system_prompt;

    #[test]
    fn test_tab_chat_system_prompt_synthesis() {
        let prompt = tab_chat_system_prompt(
            "synthesis",
            "My Test Note",
            "This is the synthesis content.",
            "This is the note body.",
        );
        assert!(prompt.contains("synthesis"));
        assert!(prompt.contains("My Test Note"));
        assert!(prompt.contains("This is the note body."));
        assert!(prompt.contains("This is the synthesis content."));
    }

    #[test]
    fn test_tab_chat_system_prompt_includes_note_body() {
        let long_body: String = "a".repeat(4000);
        let prompt = tab_chat_system_prompt("synthesis", "Title", "content", &long_body);
        let expected_preview = "a".repeat(3000);
        assert!(prompt.contains(&expected_preview));
        assert!(!prompt.contains(&long_body));
    }

    #[test]
    fn test_tab_chat_system_prompt_all_tabs() {
        let tabs = [
            ("synthesis", "SYNTHESIS"),
            ("gap_analysis", "GAP ANALYSIS"),
            ("concept_map", "CONCEPT MAP"),
            ("assessment", "ASSESSMENT"),
            ("perspectives", "PERSPECTIVES"),
        ];
        for (tab_name, expected_upper) in &tabs {
            let prompt =
                tab_chat_system_prompt(tab_name, "Note Title", "Some content.", "Note body.");
            let count = prompt.matches(expected_upper).count();
            assert_eq!(
                count, 1,
                "tab '{tab_name}': expected one '{expected_upper}', found {count}"
            );
            assert!(!prompt.contains("ANALYSIS ANALYSIS"));
            assert!(!prompt.contains("MAP MAP"));
        }
    }

    #[test]
    fn test_tab_chat_system_prompt_empty_content() {
        let prompt = tab_chat_system_prompt("synthesis", "My Note", "", "Note body.");
        assert!(prompt.contains("(Content not yet generated)"));
        assert!(prompt.contains("My Note"));
        assert!(prompt.contains("Note body."));
    }
}
