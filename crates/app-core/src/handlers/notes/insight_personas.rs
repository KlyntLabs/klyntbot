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
