use desktop_shared::commands::{
    CreateSquadParams, SquadMemberParams, SquadMemberResponse, SquadResponse, UpdateSquadParams,
};
use desktop_shared::errors::ApiError;

use crate::state::AppCore;

impl AppCore {
    fn squad_repo(&self) -> Result<&cognitive::SquadRepo, ApiError> {
        self.squad_repo
            .as_ref()
            .ok_or_else(|| ApiError::new("NOT_AVAILABLE", "Squad repo not available"))
    }

    pub async fn list_squads(&self) -> Result<Vec<SquadResponse>, ApiError> {
        let repo = self.squad_repo()?;
        let squads = repo
            .list()
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        let mut responses = Vec::new();
        for squad in squads {
            if let Ok(Some(resolved)) = repo.resolve_squad(&squad.id).await {
                responses.push(resolved_to_response(resolved));
            }
        }
        Ok(responses)
    }

    pub async fn get_squad(&self, id: &str) -> Result<SquadResponse, ApiError> {
        let repo = self.squad_repo()?;
        let resolved = repo
            .resolve_squad(id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Squad not found"))?;
        Ok(resolved_to_response(resolved))
    }

    pub async fn create_squad(&self, params: CreateSquadParams) -> Result<SquadResponse, ApiError> {
        let repo = self.squad_repo()?;
        let squad = repo
            .create(&cognitive::NewSquad {
                name: params.name,
                description: params.description,
                icon: params.icon,
                orchestrator_skill: params.orchestrator_skill,
                domains: params.domains,
            })
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;

        // Add members
        for (i, pid) in params.member_persona_ids.iter().enumerate() {
            repo.add_member(&squad.id, pid, "member", i as i64)
                .await
                .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        }

        let resolved = repo
            .resolve_squad(&squad.id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("INTERNAL_ERROR", "Squad not found after creation"))?;
        Ok(resolved_to_response(resolved))
    }

    pub async fn update_squad(&self, params: UpdateSquadParams) -> Result<SquadResponse, ApiError> {
        let repo = self.squad_repo()?;
        let domains_ref: Option<Vec<String>> = params.domains;
        repo.update(
            &params.id,
            params.name.as_deref(),
            params.description.as_deref(),
            params.icon.as_deref(),
            domains_ref.as_deref(),
            params.orchestrator_skill.as_deref(),
            params.default_interaction_mode.as_deref(),
        )
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
        .ok_or_else(|| ApiError::new("NOT_FOUND", "Squad not found"))?;

        let resolved = repo
            .resolve_squad(&params.id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?
            .ok_or_else(|| ApiError::new("NOT_FOUND", "Squad not found after update"))?;
        Ok(resolved_to_response(resolved))
    }

    pub async fn delete_squad(&self, id: &str) -> Result<(), ApiError> {
        let repo = self.squad_repo()?;
        repo.delete(id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    pub async fn add_squad_member(&self, params: SquadMemberParams) -> Result<(), ApiError> {
        let repo = self.squad_repo()?;
        repo.add_member(
            &params.squad_id,
            &params.persona_id,
            params.role_in_squad.as_deref().unwrap_or("member"),
            params.sort_order.unwrap_or(0),
        )
        .await
        .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }

    pub async fn remove_squad_member(
        &self,
        squad_id: &str,
        persona_id: &str,
    ) -> Result<(), ApiError> {
        let repo = self.squad_repo()?;
        repo.remove_member(squad_id, persona_id)
            .await
            .map_err(|e| ApiError::new("INTERNAL_ERROR", e.to_string()))?;
        Ok(())
    }
}

fn resolved_to_response(resolved: cognitive::ResolvedSquad) -> SquadResponse {
    let domains: Vec<String> = serde_json::from_str(&resolved.squad.domains).unwrap_or_default();
    let members: Vec<SquadMemberResponse> = resolved
        .members
        .iter()
        .filter_map(|m| {
            resolved
                .personas
                .iter()
                .find(|p| p.id == m.persona_id)
                .map(|p| SquadMemberResponse {
                    persona_id: m.persona_id.clone(),
                    persona_name: p.name.clone(),
                    persona_icon: p.icon.clone(),
                    persona_role: p.role.clone(),
                    role_in_squad: m.role_in_squad.clone(),
                    sort_order: m.sort_order,
                })
        })
        .collect();
    SquadResponse {
        id: resolved.squad.id,
        name: resolved.squad.name,
        description: resolved.squad.description,
        icon: resolved.squad.icon,
        orchestrator_skill: resolved.squad.orchestrator_skill,
        default_interaction_mode: resolved.squad.default_interaction_mode.clone(),
        source: resolved.squad.source,
        domains,
        is_active: resolved.squad.is_active == 1,
        members,
        created_at: resolved.squad.created_at,
        updated_at: resolved.squad.updated_at,
    }
}
