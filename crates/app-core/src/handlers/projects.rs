use desktop_shared::commands::{ProjectCreateParams, ProjectResponse, ProjectUpdateParams};
use desktop_shared::errors::ApiError;
use desktop_shared::types::EntityKind;
use storage::{ProjectPatch, ProjectRow};

use crate::errors::map_storage_err;
use crate::state::{AppCore, EntityUpdate, HandlerResult};

pub fn project_to_response(
    row: &ProjectRow,
    task_count: u32,
    completed_count: u32,
    objective_ids: Vec<String>,
) -> ProjectResponse {
    ProjectResponse {
        id: row.id.clone(),
        name: row.name.clone(),
        color: row.color.clone(),
        area_id: row.area_id.clone(),
        task_count,
        completed_count,
        objective_ids: if objective_ids.is_empty() {
            None
        } else {
            Some(objective_ids)
        },
        workflow_id: row.workflow_id.clone(),
        description: row.description.clone(),
        instructions: row
            .instructions
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
        ai_personality: row.ai_personality.clone(),
        user_role: row.user_role.clone(),
        start_date: row.start_date.clone(),
        target_end_date: row.target_end_date.clone(),
        settings: row
            .settings
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok()),
    }
}

pub async fn build_project_response(
    state: &AppCore,
    row: &ProjectRow,
) -> Result<ProjectResponse, ApiError> {
    let (counts, objectives) = tokio::try_join!(
        state.repos.projects.count_tasks_by_status(&row.id),
        state.repos.objectives.list(Some(&row.id), None),
    )
    .map_err(map_storage_err)?;

    let mut task_count: u32 = 0;
    let mut completed_count: u32 = 0;
    for (status, count) in &counts {
        task_count += *count as u32;
        if status == "done" {
            completed_count = *count as u32;
        }
    }

    let objective_ids: Vec<String> = objectives.iter().map(|o| o.id.clone()).collect();

    Ok(project_to_response(
        row,
        task_count,
        completed_count,
        objective_ids,
    ))
}

impl AppCore {
    pub async fn project_create(
        &self,
        params: ProjectCreateParams,
    ) -> HandlerResult<ProjectResponse> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now();

        let row = ProjectRow {
            id: id.clone(),
            area_id: params.area_id,
            name: params.name,
            description: params.description,
            color: params.color.unwrap_or_else(|| "blue".to_string()),
            tags: params.tags.unwrap_or_default(),
            status: "active".to_string(),
            created_at: now,
            updated_at: now,
            workflow_id: None,
            instructions: None,
            ai_personality: None,
            user_role: None,
            start_date: None,
            target_end_date: None,
            settings: None,
        };

        let created = self
            .repos
            .projects
            .create(&row)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Project,
            id: id.clone(),
        }];

        Ok((project_to_response(&created, 0, 0, vec![]), updates))
    }

    pub async fn project_get(&self, id: String) -> Result<ProjectResponse, ApiError> {
        let row = self
            .repos
            .projects
            .get_or_err(&id)
            .await
            .map_err(map_storage_err)?;

        build_project_response(self, &row).await
    }

    pub async fn project_update(
        &self,
        params: ProjectUpdateParams,
    ) -> HandlerResult<ProjectResponse> {
        let patch = ProjectPatch {
            id: params.id.clone(),
            name: params.name,
            area_id: params.area_id,
            color: params.color,
            description: params.description,
            tags: params.tags,
            status: params.status,
            workflow_id: params.workflow_id,
            instructions: params
                .instructions
                .map(|v| Some(serde_json::to_string(&v).unwrap_or_default())),
            ai_personality: params.ai_personality,
            user_role: params.user_role,
            start_date: params.start_date,
            target_end_date: params.target_end_date,
            settings: params
                .settings
                .map(|v| Some(serde_json::to_string(&v).unwrap_or_default())),
        };

        let updated = self
            .repos
            .projects
            .update(&patch)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Project,
            id: params.id,
        }];

        let response = build_project_response(self, &updated).await?;
        Ok((response, updates))
    }

    pub async fn project_delete(&self, id: String) -> HandlerResult<bool> {
        let deleted = self
            .repos
            .projects
            .delete(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = if deleted {
            vec![EntityUpdate {
                kind: EntityKind::Project,
                id,
            }]
        } else {
            vec![]
        };

        Ok((deleted, updates))
    }

    pub async fn project_archive(&self, id: String) -> HandlerResult<ProjectResponse> {
        let archived = self
            .repos
            .projects
            .archive(&id)
            .await
            .map_err(map_storage_err)?;

        let updates = vec![EntityUpdate {
            kind: EntityKind::Project,
            id,
        }];

        let response = build_project_response(self, &archived).await?;
        Ok((response, updates))
    }

    pub async fn project_update_instructions(
        &self,
        id: String,
        instructions: serde_json::Value,
    ) -> HandlerResult<ProjectResponse> {
        let json_str = serde_json::to_string(&instructions)
            .map_err(|e| ApiError::new("SERIALIZATION", e.to_string()))?;
        self.repos
            .projects
            .update_instructions(&id, &json_str)
            .await
            .map_err(map_storage_err)?;

        let row = self
            .repos
            .projects
            .get_or_err(&id)
            .await
            .map_err(map_storage_err)?;
        let response = build_project_response(self, &row).await?;
        let updates = vec![EntityUpdate {
            kind: EntityKind::Project,
            id,
        }];
        Ok((response, updates))
    }

    pub async fn project_update_role(
        &self,
        id: String,
        role: String,
    ) -> HandlerResult<ProjectResponse> {
        self.repos
            .projects
            .update_user_role(&id, &role)
            .await
            .map_err(map_storage_err)?;

        let row = self
            .repos
            .projects
            .get_or_err(&id)
            .await
            .map_err(map_storage_err)?;
        let response = build_project_response(self, &row).await?;
        let updates = vec![EntityUpdate {
            kind: EntityKind::Project,
            id,
        }];
        Ok((response, updates))
    }
}
