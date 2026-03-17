use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

use super::insight_context;

impl AppCore {
    pub async fn note_insight_persona_chat(
        &self,
        params: &PersonaChatParams,
    ) -> Result<PersonaChatResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;

        let note = self
            .note_repo
            .get_note(&params.note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let related_notes = self.fetch_related_notes(&params.note_id).await;
        let ctx = insight_context::assemble_context(&note, &related_notes, None);

        let system_prompt = format!(
            "You are {name}, a {role}. Your communication style is {tone}.\n\
             You are analyzing a note and answering follow-up questions from the user's perspective.\n\
             Stay in character. Be concise (2-4 sentences per response).\n\n\
             --- NOTE CONTEXT ---\n{context}\n--- END CONTEXT ---",
            name = params.persona_name,
            role = params.persona_role,
            tone = params.persona_tone,
            context = ctx.text,
        );

        let mut messages = vec![providers::Message::System {
            content: system_prompt,
        }];

        for msg in &params.history {
            match msg.role.as_str() {
                "user" => messages.push(providers::Message::User {
                    content: providers::UserContent::Text(msg.content.clone()),
                }),
                "assistant" => messages.push(providers::Message::Assistant {
                    content: Some(msg.content.clone()),
                    tool_calls: None,
                    reasoning_content: None,
                }),
                _ => {}
            }
        }

        messages.push(providers::Message::User {
            content: providers::UserContent::Text(params.user_message.clone()),
        });

        let config = self.config.read().await;
        let chat_params = providers::cognitive_chat_params(&config, 512);
        drop(config);

        let response = provider
            .chat(&messages, None, &chat_params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        Ok(PersonaChatResponse {
            reply: response.content.unwrap_or_default(),
        })
    }
}
