//! Repository for the `key_results` table.

use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::rows::key_result::KeyResultRow;

/// Repository for key result CRUD and progress tracking.
#[derive(Debug, Clone)]
pub struct KeyResultRepo {
    pool: SqlitePool,
}

impl KeyResultRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, row: &KeyResultRow) -> Result<KeyResultRow, StorageError> {
        let inserted = sqlx::query_as::<_, KeyResultRow>(
            r#"
            INSERT INTO key_results (
                id, objective_id, title, description, status, tracking_mode,
                target_value, current_value, unit, progress, due_date,
                created_at, updated_at, completed_at
            )
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.objective_id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.status)
        .bind(&row.tracking_mode)
        .bind(row.target_value)
        .bind(row.current_value)
        .bind(&row.unit)
        .bind(row.progress)
        .bind(row.due_date)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    pub async fn get(&self, id: &str) -> Result<Option<KeyResultRow>, StorageError> {
        let row = sqlx::query_as::<_, KeyResultRow>("SELECT * FROM key_results WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    pub async fn get_or_err(&self, id: &str) -> Result<KeyResultRow, StorageError> {
        self.get(id)
            .await?
            .ok_or_not_found(&format!("key_result {id}"))
    }

    pub async fn list(
        &self,
        objective_id: Option<&str>,
    ) -> Result<Vec<KeyResultRow>, StorageError> {
        if let Some(oid) = objective_id {
            let rows = sqlx::query_as::<_, KeyResultRow>(
                "SELECT * FROM key_results WHERE objective_id = ?1 ORDER BY created_at",
            )
            .bind(oid)
            .fetch_all(&self.pool)
            .await?;
            Ok(rows)
        } else {
            let rows =
                sqlx::query_as::<_, KeyResultRow>("SELECT * FROM key_results ORDER BY created_at")
                    .fetch_all(&self.pool)
                    .await?;
            Ok(rows)
        }
    }

    pub async fn update(
        &self,
        id: &str,
        title: Option<&str>,
        description: Option<Option<&str>>,
        status: Option<&str>,
        due_date: Option<Option<chrono::DateTime<chrono::Utc>>>,
    ) -> Result<KeyResultRow, StorageError> {
        let row = sqlx::query_as::<_, KeyResultRow>(
            r#"
            UPDATE key_results SET
                title       = COALESCE(?2, title),
                description = CASE WHEN ?3 THEN ?4 ELSE description END,
                status      = COALESCE(?5, status),
                due_date    = CASE WHEN ?6 THEN ?7 ELSE due_date END,
                completed_at = CASE
                    WHEN ?5 IN ('completed', 'abandoned') AND completed_at IS NULL THEN datetime('now')
                    WHEN ?5 IS NOT NULL AND ?5 NOT IN ('completed', 'abandoned') THEN NULL
                    ELSE completed_at
                END,
                updated_at  = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(title)
        .bind(description.is_some())
        .bind(description.unwrap_or_default())
        .bind(status)
        .bind(due_date.is_some())
        .bind(due_date.unwrap_or_default())
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("key_result {id}"))?;
        Ok(row)
    }

    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM key_results WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Update metric value and recalculate progress for a metric-type KR.
    pub async fn update_metric(
        &self,
        id: &str,
        current_value: f64,
    ) -> Result<KeyResultRow, StorageError> {
        let row = sqlx::query_as::<_, KeyResultRow>(
            r#"
            UPDATE key_results SET
                current_value = ?2,
                progress = CASE
                    WHEN tracking_mode = 'metric' AND target_value IS NOT NULL AND target_value > 0
                    THEN MIN(100.0, MAX(0.0, ?2 / target_value * 100.0))
                    ELSE progress
                END,
                updated_at = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(current_value)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("key_result {id}"))?;
        Ok(row)
    }

    /// Update progress directly (used for action-tracking mode).
    pub async fn update_progress(&self, id: &str, progress: f64) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE key_results SET progress = ?2, updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .bind(progress)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Count actions linked to a KR (total and completed).
    /// Returns `(total, completed)`.
    pub async fn count_actions(&self, kr_id: &str) -> Result<(i64, i64), StorageError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), SUM(CASE WHEN status = 'done' THEN 1 ELSE 0 END) \
             FROM actions WHERE key_result_id = ?1 AND is_template = FALSE",
        )
        .bind(kr_id)
        .fetch_one(&self.pool)
        .await?;

        Ok((row.0, row.1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repos::area::AreaRepo;
    use crate::repos::objective::ObjectiveRepo;
    use crate::repos::project_repo::ProjectRepo;
    use crate::rows::area::AreaRow;
    use crate::rows::objective::ObjectiveRow;
    use crate::rows::project::ProjectRow;

    async fn setup() -> (KeyResultRepo, crate::StoragePool) {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();

        // Create prerequisite area + project + objective.
        AreaRepo::new(db.clone())
            .create(&AreaRow {
                id: "a1".into(),
                name: "Work".into(),
                description: None,
                color: "blue".into(),
                icon: None,
                position: 0,
                status: "active".into(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await
            .unwrap();

        ProjectRepo::new(db.clone())
            .create(&ProjectRow {
                id: "p1".into(),
                area_id: "a1".into(),
                name: "Project A".into(),
                description: None,
                color: "orange".into(),
                tags: vec![],
                status: "active".into(),
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
            })
            .await
            .unwrap();

        ObjectiveRepo::new(db.clone())
            .create(&ObjectiveRow {
                id: "obj1".into(),
                project_id: "p1".into(),
                title: "Test objective".into(),
                description: None,
                status: "active".into(),
                priority: None,
                due_date: None,
                progress: 0.0,
                created_at: chrono::Utc::now(),
                updated_at: chrono::Utc::now(),
                completed_at: None,
            })
            .await
            .unwrap();

        let repo = KeyResultRepo::new(db);
        (repo, pool)
    }

    fn sample_kr(id: &str) -> KeyResultRow {
        KeyResultRow {
            id: id.to_string(),
            objective_id: "obj1".to_string(),
            title: "Test KR".to_string(),
            description: None,
            status: "active".to_string(),
            tracking_mode: "metric".to_string(),
            target_value: Some(100.0),
            current_value: 0.0,
            unit: Some("%".to_string()),
            progress: 0.0,
            due_date: None,
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            completed_at: None,
        }
    }

    #[tokio::test]
    async fn test_create_and_get_kr() {
        let (repo, _pool) = setup().await;
        let kr = sample_kr("kr1");
        let created = repo.create(&kr).await.unwrap();
        assert_eq!(created.title, "Test KR");

        let fetched = repo.get("kr1").await.unwrap().unwrap();
        assert_eq!(fetched.title, "Test KR");
    }

    #[tokio::test]
    async fn test_update_metric() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_kr("kr1")).await.unwrap();
        let updated = repo.update_metric("kr1", 60.0).await.unwrap();
        assert!((updated.current_value - 60.0).abs() < f64::EPSILON);
        assert!((updated.progress - 60.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_delete_kr() {
        let (repo, _pool) = setup().await;
        repo.create(&sample_kr("kr1")).await.unwrap();
        assert!(repo.delete("kr1").await.unwrap());
        assert!(repo.get("kr1").await.unwrap().is_none());
    }
}
