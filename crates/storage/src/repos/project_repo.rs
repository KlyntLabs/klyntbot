//! Repository for the `projects` table.

use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::rows::project::ProjectRow;

/// Filter criteria for listing projects.
#[derive(Debug, Default, Clone)]
pub struct ProjectFilter {
    pub area_id: Option<String>,
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<i64>,
}

/// Project with aggregated task counts.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectWithStats {
    pub project: ProjectRow,
    pub task_count_todo: i64,
    pub task_count_doing: i64,
    pub task_count_done: i64,
    pub task_count_total: i64,
}

/// Repository for project CRUD and aggregation.
#[derive(Debug, Clone)]
pub struct ProjectRepo {
    pool: SqlitePool,
}

impl ProjectRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // CRUD
    // -----------------------------------------------------------------------

    /// Insert a new project. Returns the inserted row.
    pub async fn create(&self, row: &ProjectRow) -> Result<ProjectRow, StorageError> {
        let inserted = sqlx::query_as::<_, ProjectRow>(
            r#"
            INSERT INTO projects (id, area_id, name, description, color, tags, status, created_at, updated_at, workflow_id,
                                  instructions, ai_personality, user_role, start_date, target_end_date, settings)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.area_id)
        .bind(&row.name)
        .bind(&row.description)
        .bind(&row.color)
        .bind(sqlx::types::Json(&row.tags))
        .bind(&row.status)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(&row.workflow_id)
        .bind(&row.instructions)
        .bind(&row.ai_personality)
        .bind(&row.user_role)
        .bind(&row.start_date)
        .bind(&row.target_end_date)
        .bind(&row.settings)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Get a project by id. Returns `None` if not found.
    pub async fn get(&self, id: &str) -> Result<Option<ProjectRow>, StorageError> {
        let row = sqlx::query_as::<_, ProjectRow>("SELECT * FROM projects WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Get a project by id, returning `StorageError::NotFound` if missing.
    pub async fn get_or_err(&self, id: &str) -> Result<ProjectRow, StorageError> {
        self.get(id)
            .await?
            .ok_or_not_found(&format!("project {id}"))
    }

    /// Update mutable fields on a project. Only non-None patch fields are overwritten.
    pub async fn update(&self, patch: &ProjectPatch) -> Result<ProjectRow, StorageError> {
        let row = sqlx::query_as::<_, ProjectRow>(
            r#"
            UPDATE projects SET
                area_id         = COALESCE(?2, area_id),
                name            = COALESCE(?3, name),
                description     = CASE WHEN ?4 THEN ?5 ELSE description END,
                color           = COALESCE(?6, color),
                tags            = COALESCE(?7, tags),
                status          = COALESCE(?8, status),
                workflow_id     = CASE WHEN ?9 THEN ?10 ELSE workflow_id END,
                instructions    = CASE WHEN ?11 THEN ?12 ELSE instructions END,
                ai_personality  = CASE WHEN ?13 THEN ?14 ELSE ai_personality END,
                user_role       = CASE WHEN ?15 THEN ?16 ELSE user_role END,
                start_date      = CASE WHEN ?17 THEN ?18 ELSE start_date END,
                target_end_date = CASE WHEN ?19 THEN ?20 ELSE target_end_date END,
                settings        = CASE WHEN ?21 THEN ?22 ELSE settings END,
                updated_at      = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(&patch.id)
        .bind(&patch.area_id)
        .bind(&patch.name)
        .bind(patch.description.is_some())
        .bind(
            patch
                .description
                .as_ref()
                .and_then(|d| d.as_deref())
                .unwrap_or_default(),
        )
        .bind(&patch.color)
        .bind(patch.tags.as_ref().map(sqlx::types::Json))
        .bind(&patch.status)
        .bind(patch.workflow_id.is_some())
        .bind(patch.workflow_id.as_ref().and_then(|w| w.as_deref()))
        .bind(patch.instructions.is_some())
        .bind(patch.instructions.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.ai_personality.is_some())
        .bind(patch.ai_personality.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.user_role.is_some())
        .bind(patch.user_role.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.start_date.is_some())
        .bind(patch.start_date.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.target_end_date.is_some())
        .bind(patch.target_end_date.as_ref().and_then(|v| v.as_deref()))
        .bind(patch.settings.is_some())
        .bind(patch.settings.as_ref().and_then(|v| v.as_deref()))
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("project {}", patch.id))?;

        Ok(row)
    }

    /// Delete a project. Actions with this project_id will have it set to NULL (FK ON DELETE SET NULL).
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM projects WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Archive a project (set status = 'archived').
    pub async fn archive(&self, id: &str) -> Result<ProjectRow, StorageError> {
        self.update(&ProjectPatch {
            id: id.to_string(),
            status: Some("archived".to_string()),
            ..Default::default()
        })
        .await
    }

    // -----------------------------------------------------------------------
    // Listing / Filtering
    // -----------------------------------------------------------------------

    /// List projects matching the given filter criteria.
    pub async fn list(&self, filter: &ProjectFilter) -> Result<Vec<ProjectRow>, StorageError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM projects WHERE 1=1");

        if let Some(ref area_id) = filter.area_id {
            qb.push(" AND area_id = ");
            qb.push_bind(area_id);
        }

        if let Some(ref status) = filter.status {
            qb.push(" AND status = ");
            qb.push_bind(status);
        }

        if let Some(ref tags) = filter.tags {
            for tag in tags {
                qb.push(" AND EXISTS (SELECT 1 FROM json_each(tags) WHERE value = ");
                qb.push_bind(tag);
                qb.push(")");
            }
        }

        qb.push(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            qb.push(" LIMIT ");
            qb.push_bind(limit);
        }

        let rows = qb
            .build_query_as::<ProjectRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// List all projects (no filter).
    pub async fn all(&self) -> Result<Vec<ProjectRow>, StorageError> {
        self.list(&ProjectFilter::default()).await
    }

    // -----------------------------------------------------------------------
    // Aggregation
    // -----------------------------------------------------------------------

    /// Count tasks by status for a given project.
    pub async fn count_tasks_by_status(
        &self,
        project_id: &str,
    ) -> Result<Vec<(String, i64)>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT status, COUNT(*) as count
            FROM actions
            WHERE project_id = ?1 AND is_template = FALSE
            GROUP BY status
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Update only the instructions JSON field on a project.
    pub async fn update_instructions(
        &self,
        id: &str,
        instructions: &str,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE projects SET instructions = ?2, updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .bind(instructions)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update only the user_role field on a project.
    pub async fn update_user_role(&self, id: &str, role: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE projects SET user_role = ?2, updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .bind(role)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get a project with aggregated task statistics.
    pub async fn get_with_stats(&self, id: &str) -> Result<Option<ProjectWithStats>, StorageError> {
        let project = match self.get(id).await? {
            Some(p) => p,
            None => return Ok(None),
        };

        let counts = self.count_tasks_by_status(id).await?;
        let mut stats = ProjectWithStats {
            project,
            task_count_todo: 0,
            task_count_doing: 0,
            task_count_done: 0,
            task_count_total: 0,
        };

        for (status, count) in &counts {
            match status.as_str() {
                "todo" => stats.task_count_todo = *count,
                "doing" => stats.task_count_doing = *count,
                "done" => stats.task_count_done = *count,
                _ => {}
            }
            stats.task_count_total += count;
        }

        Ok(Some(stats))
    }
}

/// Patch struct for partial project updates.
#[derive(Debug, Default, Clone)]
pub struct ProjectPatch {
    pub id: String,
    pub area_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub color: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub workflow_id: Option<Option<String>>,
    pub instructions: Option<Option<String>>,
    pub ai_personality: Option<Option<String>>,
    pub user_role: Option<Option<String>>,
    pub start_date: Option<Option<String>>,
    pub target_end_date: Option<Option<String>>,
    pub settings: Option<Option<String>>,
}
