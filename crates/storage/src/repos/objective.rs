//! Repository for the `objectives` table.

use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::rows::objective::ObjectiveRow;

/// Repository for objective CRUD and progress aggregation.
#[derive(Debug, Clone)]
pub struct ObjectiveRepo {
    pool: SqlitePool,
}

impl ObjectiveRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, row: &ObjectiveRow) -> Result<ObjectiveRow, StorageError> {
        let inserted = sqlx::query_as::<_, ObjectiveRow>(
            r#"
            INSERT INTO objectives (id, project_id, title, description, status, priority, due_date, progress, created_at, updated_at, completed_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.project_id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(row.priority)
        .bind(row.due_date)
        .bind(row.progress)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    pub async fn get(&self, id: &str) -> Result<Option<ObjectiveRow>, StorageError> {
        let row = sqlx::query_as::<_, ObjectiveRow>("SELECT * FROM objectives WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn get_or_err(&self, id: &str) -> Result<ObjectiveRow, StorageError> {
        self.get(id)
            .await?
            .ok_or_not_found(&format!("objective {id}"))
    }

    pub async fn list(
        &self,
        project_id: Option<&str>,
        status: Option<&str>,
    ) -> Result<Vec<ObjectiveRow>, StorageError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM objectives WHERE 1=1");

        if let Some(pid) = project_id {
            qb.push(" AND project_id = ");
            qb.push_bind(pid);
        }

        if let Some(s) = status {
            qb.push(" AND status = ");
            qb.push_bind(s);
        }

        qb.push(" ORDER BY priority ASC NULLS LAST, created_at");

        let rows = qb
            .build_query_as::<ObjectiveRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    pub async fn update(
        &self,
        id: &str,
        title: Option<&str>,
        description: Option<Option<&str>>,
        status: Option<&str>,
        priority: Option<Option<i16>>,
        due_date: Option<Option<jiff::Timestamp>>,
    ) -> Result<ObjectiveRow, StorageError> {
        let row = sqlx::query_as::<_, ObjectiveRow>(
            r#"
            UPDATE objectives SET
                title       = COALESCE(?2, title),
                description = CASE WHEN ?3 THEN ?4 ELSE description END,
                status      = COALESCE(?5, status),
                priority    = CASE WHEN ?6 THEN ?7 ELSE priority END,
                due_date    = CASE WHEN ?8 THEN ?9 ELSE due_date END,
                completed_at = CASE
                    WHEN ?5 IN ('completed', 'abandoned') AND completed_at IS NULL THEN (unixepoch('now') * 1000)
                    WHEN ?5 IS NOT NULL AND ?5 NOT IN ('completed', 'abandoned') THEN NULL
                    ELSE completed_at
                END,
                updated_at  = (unixepoch('now') * 1000)
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(title)
        .bind(description.is_some())
        .bind(description.unwrap_or_default())
        .bind(status)
        .bind(priority.is_some())
        .bind(priority.unwrap_or_default())
        .bind(due_date.is_some())
        .bind(due_date.unwrap_or_default().map(|t| t.as_millisecond()))
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("objective {id}"))?;
        Ok(row)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM objectives WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Recalculate objective progress as average of child KR progresses.
    pub async fn recalculate_progress(&self, id: &str) -> Result<f64, StorageError> {
        let row: (f64,) = sqlx::query_as(
            "UPDATE objectives SET \
                progress = COALESCE((SELECT AVG(progress) FROM key_results WHERE objective_id = ?1), 0.0), \
                updated_at = (unixepoch('now') * 1000) \
             WHERE id = ?1 \
             RETURNING progress",
        )
        .bind(id)
        .fetch_one(&self.pool)
        .await?;

        Ok(row.0)
    }

    get_by_ids_impl!("objectives", ObjectiveRow);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::area::AreaRepo;
    use crate::repos::project_repo::ProjectRepo;
    use crate::rows::area::AreaRow;
    use crate::rows::project::ProjectRow;

    async fn setup() -> (ObjectiveRepo, crate::StoragePool) {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();

        // Create prerequisite area + project.
        let area_repo = AreaRepo::new(db.clone());
        area_repo
            .create(&AreaRow {
                id: "a1".into(),
                name: "Work".into(),
                description: None,
                color: "blue".into(),
                icon: None,
                position: 0,
                status: "active".into(),
                created_at: crate::sqlite_types::SqlTs::from(jiff::Timestamp::now()),
                updated_at: crate::sqlite_types::SqlTs::from(jiff::Timestamp::now()),
            })
            .await
            .unwrap();

        let proj_repo = ProjectRepo::new(db.clone());
        proj_repo
            .create(&ProjectRow {
                id: "p1".into(),
                area_id: "a1".into(),
                name: "Project A".into(),
                description: None,
                color: "orange".into(),
                tags: vec![],
                status: "active".into(),
                created_at: crate::sqlite_types::SqlTs::from(jiff::Timestamp::now()),
                updated_at: crate::sqlite_types::SqlTs::from(jiff::Timestamp::now()),
                workflow_id: None,
                instructions: None,
                ai_personality: None,
                user_role: None,
                start_date: None,
                target_end_date: None,
                settings: None,
            })
            .await
            .unwrap();

        let repo = ObjectiveRepo::new(db);
        (repo, pool)
    }

    fn sample_objective(id: &str) -> ObjectiveRow {
        ObjectiveRow {
            id: id.to_string(),
            project_id: "p1".to_string(),
            title: "Test objective".to_string(),
            description: None,
            status: "active".to_string(),
            priority: None,
            due_date: None,
            progress: 0.0,
            created_at: crate::sqlite_types::SqlTs::from(jiff::Timestamp::now()),
            updated_at: crate::sqlite_types::SqlTs::from(jiff::Timestamp::now()),
            completed_at: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_objective() {
        let (repo, _pool) = setup().await;
        let obj = sample_objective("obj1");
        let created = repo.create(&obj).await.unwrap();
        assert_eq!(created.title, "Test objective");

        let fetched = repo.get("obj1").await.unwrap().unwrap();
        assert_eq!(fetched.title, "Test objective");
    }

    #[tokio::test]
    async fn test_list_objectives_by_project() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_objective("obj1")).await.unwrap();
        repo.create(&sample_objective("obj2")).await.unwrap();

        let list = repo.list(Some("p1"), None).await.unwrap();
        assert_eq!(list.len(), 2);

        let list = repo.list(Some("nonexistent"), None).await.unwrap();
        assert_eq!(list.len(), 0);
    }

    #[tokio::test]
    async fn test_update_objective() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_objective("obj1")).await.unwrap();
        let updated = repo
            .update("obj1", Some("Updated"), None, None, None, None)
            .await
            .unwrap();
        assert_eq!(updated.title, "Updated");
    }

    #[tokio::test]
    async fn test_delete_objective() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_objective("obj1")).await.unwrap();
        assert!(repo.delete("obj1").await.unwrap());
        assert!(repo.get("obj1").await.unwrap().is_none());
    }
}
