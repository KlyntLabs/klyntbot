use desktop_shared::commands::{
    ProjectCreateParams, ProjectHealthMetricsResponse, ProjectResponse, ProjectUpdateParams,
};
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
        let now: storage::SqlTs = jiff::Timestamp::now().into();

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
        let patch = ProjectPatch {
            id: id.clone(),
            instructions: Some(Some(json_str)),
            ..Default::default()
        };
        let updated = self
            .repos
            .projects
            .update(&patch)
            .await
            .map_err(map_storage_err)?;

        let response = build_project_response(self, &updated).await?;
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
        let patch = ProjectPatch {
            id: id.clone(),
            user_role: Some(Some(role)),
            ..Default::default()
        };
        let updated = self
            .repos
            .projects
            .update(&patch)
            .await
            .map_err(map_storage_err)?;

        let response = build_project_response(self, &updated).await?;
        let updates = vec![EntityUpdate {
            kind: EntityKind::Project,
            id,
        }];
        Ok((response, updates))
    }

    /// Compute focus quality and insight freshness metrics for a project.
    pub async fn project_health_metrics(
        &self,
        project_id: String,
    ) -> Result<ProjectHealthMetricsResponse, ApiError> {
        // Focus quality: average quality_score from focus sessions in this project (last 30 days)
        let focus_quality = if let Some(ref pr) = self.productivity_repos {
            pr.sessions
                .avg_quality_by_project(&project_id, 30)
                .await
                .unwrap_or(None)
        } else {
            None
        };

        // Insight freshness: find linked notes, check how recently each was reviewed
        let insight_freshness = self
            .compute_insight_freshness(&project_id)
            .await
            .unwrap_or(None);

        Ok(ProjectHealthMetricsResponse {
            focus_quality,
            insight_freshness,
        })
    }

    /// Compute average insight freshness across notes linked to a project.
    /// Freshness = max(0, 1.0 - days_since_review / 7.0), averaged.
    async fn compute_insight_freshness(&self, project_id: &str) -> Result<Option<f64>, ApiError> {
        let links = self
            .repos
            .entity_links
            .get_project_links(project_id)
            .await
            .map_err(map_storage_err)?;

        // Extract note IDs from entity links (either source or target)
        let note_ids: Vec<&str> = links
            .iter()
            .filter_map(|link| {
                if link.target_kind == "note" {
                    Some(link.target_id.as_str())
                } else if link.source_kind == "note" {
                    Some(link.source_id.as_str())
                } else {
                    None
                }
            })
            .collect();

        if note_ids.is_empty() {
            return Ok(None);
        }

        // Batch query: latest generated_at per note in a single round-trip
        let pool = self.storage_pool.inner();
        let placeholders: Vec<String> = (1..=note_ids.len()).map(|i| format!("?{i}")).collect();
        let sql = format!(
            "SELECT note_id, MAX(generated_at) as generated_at FROM insight_reviews
             WHERE note_id IN ({}) AND superseded_at IS NULL
             GROUP BY note_id",
            placeholders.join(", ")
        );
        let mut query = sqlx::query_as::<_, (String, String)>(&sql);
        for note_id in &note_ids {
            query = query.bind(*note_id);
        }
        let rows = query.fetch_all(pool).await.unwrap_or_default();

        if rows.is_empty() {
            return Ok(None);
        }

        let now = jiff::Timestamp::now();
        let mut total_freshness = 0.0f64;
        let mut count = 0usize;
        for (_note_id, generated_at_str) in &rows {
            if let Ok(generated_at) = generated_at_str.parse::<jiff::Timestamp>() {
                let days = ((now.as_millisecond() - generated_at.as_millisecond()).max(0) as f64)
                    / 86_400_000.0;
                total_freshness += (1.0 - days / 7.0).max(0.0);
                count += 1;
            }
        }

        if count == 0 {
            return Ok(None);
        }

        Ok(Some(total_freshness / count as f64))
    }
}
