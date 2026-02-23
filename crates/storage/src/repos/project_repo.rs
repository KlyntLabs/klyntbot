//! Repository for the `projects` table.

use sqlx::SqlitePool;

use crate::error::StorageError;
use crate::rows::project::ProjectRow;

/// Filter criteria for listing projects.
#[derive(Debug, Default, Clone)]
pub struct ProjectFilter {
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub limit: Option<i64>,
}

/// Project with aggregated task counts.
#[derive(Debug, Clone)]
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
            INSERT INTO projects (id, name, description, color, tags, status, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.name)
        .bind(&row.description)
        .bind(&row.color)
        .bind(sqlx::types::Json(&row.tags))
        .bind(&row.status)
        .bind(row.created_at)
        .bind(row.updated_at)
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
            .ok_or_else(|| StorageError::NotFound(format!("project {id}")))
    }

    /// Update mutable fields on a project. Only non-None patch fields are overwritten.
    pub async fn update(&self, patch: &ProjectPatch) -> Result<ProjectRow, StorageError> {
        let row = sqlx::query_as::<_, ProjectRow>(
            r#"
            UPDATE projects SET
                name        = COALESCE(?2, name),
                description = CASE WHEN ?3 THEN ?4 ELSE description END,
                color       = COALESCE(?5, color),
                tags        = COALESCE(?6, tags),
                status      = COALESCE(?7, status),
                updated_at  = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(&patch.id)
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
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StorageError::NotFound(format!("project {}", patch.id)))?;

        Ok(row)
    }

    /// Delete a project. Todos with this project_id will have it set to NULL (FK ON DELETE SET NULL).
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
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM projects");
        let mut has_where = false;

        if let Some(ref status) = filter.status {
            qb.push(" WHERE status = ");
            qb.push_bind(status);
            has_where = true;
        }

        if let Some(ref tags) = filter.tags {
            for tag in tags {
                qb.push(if has_where { " AND " } else { " WHERE " });
                has_where = true;
                qb.push("EXISTS (SELECT 1 FROM json_each(tags) WHERE value = ");
                qb.push_bind(tag);
                qb.push(")");
            }
        }

        let _ = has_where;
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
            FROM todos
            WHERE project_id = ?1 AND is_template = FALSE
            GROUP BY status
            "#,
        )
        .bind(project_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
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
    pub name: Option<String>,
    pub description: Option<Option<String>>,
    pub color: Option<String>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
}
