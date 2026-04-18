use sqlx::SqlitePool;
use tracing::warn;

use crate::types::ProductivityProject;

#[derive(sqlx::FromRow)]
struct ProjectRow {
    id: String,
    display_name: String,
    path: String,
    url_patterns: Option<String>,
    color: Option<String>,
    is_auto_detected: bool,
    created_at: String,
}

impl From<ProjectRow> for ProductivityProject {
    fn from(row: ProjectRow) -> Self {
        let url_patterns: Vec<String> = row
            .url_patterns
            .as_deref()
            .and_then(|s| serde_json::from_str(s).ok())
            .unwrap_or_default();
        let created_at = common::parse_datetime_jiff(&row.created_at, "UTC").unwrap_or_else(|| {
            warn!(raw = %row.created_at, "unparseable created_at in productivity_projects");
            jiff::Timestamp::now()
        });
        Self {
            id: row.id,
            display_name: row.display_name,
            path: row.path,
            url_patterns,
            color: row.color,
            is_auto_detected: row.is_auto_detected,
            created_at,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ProjectRepo {
    pool: SqlitePool,
}

impl ProjectRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn upsert(&self, project: &ProductivityProject) -> common::Result<()> {
        let url_patterns_json =
            serde_json::to_string(&project.url_patterns).unwrap_or_else(|_| "[]".to_string());
        sqlx::query(
            r#"INSERT INTO productivity_projects (id, display_name, path, url_patterns, color, is_auto_detected)
               VALUES (?1, ?2, ?3, ?4, ?5, ?6)
               ON CONFLICT(id) DO UPDATE SET
                   display_name = excluded.display_name,
                   url_patterns = excluded.url_patterns,
                   color = excluded.color"#,
        )
        .bind(&project.id)
        .bind(&project.display_name)
        .bind(&project.path)
        .bind(&url_patterns_json)
        .bind(&project.color)
        .bind(project.is_auto_detected)
        .execute(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(())
    }

    pub async fn list_all(&self) -> common::Result<Vec<ProductivityProject>> {
        let rows = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, display_name, path, url_patterns, color, is_auto_detected, created_at FROM productivity_projects ORDER BY display_name",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(rows.into_iter().map(ProductivityProject::from).collect())
    }

    pub async fn find_by_path(&self, path: &str) -> common::Result<Option<ProductivityProject>> {
        let row = sqlx::query_as::<_, ProjectRow>(
            "SELECT id, display_name, path, url_patterns, color, is_auto_detected, created_at FROM productivity_projects WHERE path = ?1",
        )
        .bind(path)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(row.map(ProductivityProject::from))
    }

    pub async fn delete(&self, id: &str) -> common::Result<bool> {
        let result = sqlx::query("DELETE FROM productivity_projects WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| common::KlyntbotError::Storage(e.to_string()))?;
        Ok(result.rows_affected() > 0)
    }
}
