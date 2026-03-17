//! Persona management handlers for Insight Review.

use desktop_shared::commands::*;
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
    /// List all personas (including inactive) for the management UI.
    pub async fn note_insight_list_personas(&self) -> Result<Vec<PersonaResponse>, ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let rows = repo
            .list_all()
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(rows.into_iter().map(persona_row_to_response).collect())
    }

    /// Create a new user-defined persona.
    pub async fn note_insight_create_persona(
        &self,
        params: CreatePersonaParams,
    ) -> Result<PersonaResponse, ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let row = repo
            .create(&cognitive::NewPersona {
                name: params.name,
                role: params.role,
                expertise: params.expertise,
                perspective: params.perspective,
                tone: params.tone,
                icon: params.icon,
                domains: params.domains,
            })
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(persona_row_to_response(row))
    }

    /// Update a non-builtin persona.
    pub async fn note_insight_update_persona(
        &self,
        params: UpdatePersonaParams,
    ) -> Result<PersonaResponse, ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let row = repo
            .update(
                &params.id,
                &cognitive::PersonaUpdate {
                    name: params.name,
                    role: params.role,
                    expertise: params.expertise,
                    perspective: params.perspective,
                    tone: params.tone,
                    icon: params.icon,
                    domains: params.domains,
                },
            )
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Persona not found"))?;

        Ok(persona_row_to_response(row))
    }

    /// Delete a non-builtin persona.
    pub async fn note_insight_delete_persona(&self, id: &str) -> Result<(), ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        repo.delete(id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    /// Toggle a persona's active state.
    pub async fn note_insight_toggle_persona(
        &self,
        id: &str,
        active: bool,
    ) -> Result<(), ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        repo.set_active(id, active)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    /// Set pinned personas for a note (overrides auto-selection).
    pub async fn note_insight_set_pins(
        &self,
        params: SetPersonaPinsParams,
    ) -> Result<(), ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        repo.set_pins(&params.note_id, &params.persona_ids)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    /// Rate a persona (thumbs up/down) — adjusts relevance score.
    pub async fn note_insight_rate_persona(
        &self,
        params: RatePersonaParams,
    ) -> Result<(), ApiError> {
        let repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let delta = if params.helpful { 0.1 } else { -0.1 };
        repo.update_relevance(&params.id, delta)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }
}

impl AppCore {
    /// Auto-generate a persona based on a note's content via LLM.
    pub async fn note_insight_auto_generate_persona(
        &self,
        note_id: &str,
    ) -> Result<PersonaResponse, ApiError> {
        let provider = self
            .cognitive_provider
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "LLM provider not configured"))?;
        let persona_repo = self
            .persona_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Persona repo not available"))?;

        let note = self
            .note_repo
            .get_note(note_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Note not found"))?;

        let existing = persona_repo.list_active().await.unwrap_or_default();
        let existing_names: Vec<&str> = existing.iter().map(|p| p.name.as_str()).collect();

        // NoteRow doesn't carry tags — fetch separately
        let tags = self.note_repo.get_tags(note_id).await.unwrap_or_default();
        let tags_str = tags.join(", ");

        let config = self.config.read().await;
        let params = providers::cognitive_chat_params(&config, 1024);
        drop(config);

        let body_preview = &note.body[..note.body.len().min(500)];
        let prompt = format!(
            r#"Generate a unique expert persona for analyzing content about the following topic.

Note title: {}
Note tags: {}
Note content (first 500 chars): {}

Existing personas (DO NOT duplicate these): {}

Return a JSON object with these exact fields:
{{
  "name": "A distinctive persona name (2-3 words, not a real person)",
  "role": "Their professional role (2-4 words)",
  "expertise": "Domain expertise description (1 sentence)",
  "perspective": "How they approach analysis (1 sentence)",
  "tone": "One of: analytical, curious, pragmatic, skeptical, inquisitive, provocative",
  "icon": "A single emoji that represents this persona",
  "domains": ["2-4 lowercase domain keywords relevant to the note"]
}}

Return ONLY the JSON object, no markdown fences, no explanation."#,
            note.title,
            tags_str,
            body_preview,
            existing_names.join(", "),
        );

        let messages = vec![
            providers::Message::System { content: prompt },
            providers::Message::User {
                content: providers::UserContent::Text("Generate the persona now.".to_string()),
            },
        ];

        let response = provider
            .chat(&messages, None, &params)
            .await
            .map_err(|e| ApiError::new("LLM_ERROR", e.to_string()))?;

        let content = response
            .content
            .ok_or_else(|| ApiError::new("LLM_ERROR", "Empty response from LLM"))?;

        let generated: serde_json::Value = serde_json::from_str(content.trim())
            .map_err(|e| ApiError::new("PARSE_ERROR", format!("Failed to parse persona JSON: {e}")))?;

        let new_persona = cognitive::NewPersona {
            name: generated["name"].as_str().unwrap_or("Expert").to_string(),
            role: generated["role"].as_str().unwrap_or("Domain Analyst").to_string(),
            expertise: generated["expertise"].as_str().unwrap_or("General").to_string(),
            perspective: generated["perspective"].as_str().unwrap_or("Balanced analysis").to_string(),
            tone: generated["tone"].as_str().unwrap_or("analytical").to_string(),
            icon: generated["icon"].as_str().unwrap_or("🧠").to_string(),
            domains: generated["domains"]
                .as_array()
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
        };

        let row = persona_repo
            .create_auto(&new_persona)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        Ok(persona_row_to_response(row))
    }
}

/// Convert a PersonaRow to a PersonaResponse DTO.
fn persona_row_to_response(row: cognitive::PersonaRow) -> PersonaResponse {
    let domains: Vec<String> = serde_json::from_str(&row.domains).unwrap_or_default();
    PersonaResponse {
        id: row.id,
        name: row.name,
        role: row.role,
        expertise: row.expertise,
        perspective: row.perspective,
        tone: row.tone,
        icon: row.icon,
        source: row.source,
        domains,
        is_active: row.is_active == 1,
        relevance_score: row.relevance_score,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}
