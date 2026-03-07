//! Repository for `status_workflows` and `status_labels` tables.

use crate::error::{OptionExt, StorageError};
use crate::rows::status::{StatusLabelRow, StatusWorkflowRow};

/// Repository for status workflow and label CRUD operations.
#[derive(Debug, Clone)]
pub struct StatusWorkflowRepo {
    pool: sqlx::SqlitePool,
}

impl StatusWorkflowRepo {
    pub fn new(pool: sqlx::SqlitePool) -> Self {
        Self { pool }
    }

    // ── Workflow CRUD ─────────────────────────────────────

    /// Fetch a single workflow by ID.
    pub async fn get(&self, id: &str) -> Result<Option<StatusWorkflowRow>, StorageError> {
        let row = sqlx::query_as::<_, StatusWorkflowRow>(
            "SELECT * FROM status_workflows WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// Fetch the global default workflow (is_global_default = 1).
    pub async fn get_global_default(&self) -> Result<Option<StatusWorkflowRow>, StorageError> {
        let row = sqlx::query_as::<_, StatusWorkflowRow>(
            "SELECT * FROM status_workflows WHERE is_global_default = 1 LIMIT 1",
        )
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List all template workflows.
    pub async fn list_templates(&self) -> Result<Vec<StatusWorkflowRow>, StorageError> {
        let rows = sqlx::query_as::<_, StatusWorkflowRow>(
            "SELECT * FROM status_workflows WHERE is_template = 1 ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// List every workflow.
    pub async fn list_all(&self) -> Result<Vec<StatusWorkflowRow>, StorageError> {
        let rows = sqlx::query_as::<_, StatusWorkflowRow>(
            "SELECT * FROM status_workflows ORDER BY is_global_default DESC, name",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Create a new workflow.
    pub async fn create(
        &self,
        name: &str,
        is_template: bool,
    ) -> Result<StatusWorkflowRow, StorageError> {
        let id = format!("wf_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let row = sqlx::query_as::<_, StatusWorkflowRow>(
            r#"
            INSERT INTO status_workflows (id, name, is_template, is_global_default)
            VALUES (?1, ?2, ?3, 0)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(name)
        .bind(is_template)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Rename an existing workflow.
    pub async fn update_name(
        &self,
        id: &str,
        name: &str,
    ) -> Result<StatusWorkflowRow, StorageError> {
        let row = sqlx::query_as::<_, StatusWorkflowRow>(
            r#"
            UPDATE status_workflows
            SET name = ?2, updated_at = strftime('%Y-%m-%dT%H:%M:%SZ', 'now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(name)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found("workflow")?;
        Ok(row)
    }

    /// Delete a workflow. Returns false if not found.
    /// MUST NOT delete the global default.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "DELETE FROM status_workflows WHERE id = ?1 AND is_global_default = 0",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Duplicate a workflow (copy workflow + all labels).
    pub async fn duplicate(
        &self,
        source_id: &str,
        new_name: &str,
    ) -> Result<StatusWorkflowRow, StorageError> {
        let _source = self
            .get(source_id)
            .await?
            .ok_or_not_found("source workflow")?;

        let new_wf = self.create(new_name, false).await?;

        let source_labels = self.get_labels(source_id).await?;
        for label in &source_labels {
            self.add_label(
                &new_wf.id,
                &label.name,
                &label.color,
                &label.status_group,
                label.position,
            )
            .await?;
        }

        Ok(new_wf)
    }

    // ── Label CRUD ────────────────────────────────────────

    /// Fetch a single label by ID.
    pub async fn get_label(&self, id: &str) -> Result<Option<StatusLabelRow>, StorageError> {
        let row =
            sqlx::query_as::<_, StatusLabelRow>("SELECT * FROM status_labels WHERE id = ?1")
                .bind(id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row)
    }

    /// Fetch all labels for a workflow, ordered by position.
    pub async fn get_labels(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<StatusLabelRow>, StorageError> {
        let rows = sqlx::query_as::<_, StatusLabelRow>(
            "SELECT * FROM status_labels WHERE workflow_id = ?1 ORDER BY position",
        )
        .bind(workflow_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Add a new label to a workflow.
    pub async fn add_label(
        &self,
        workflow_id: &str,
        name: &str,
        color: &str,
        status_group: &str,
        position: i32,
    ) -> Result<StatusLabelRow, StorageError> {
        let id = format!("sl_{}", &uuid::Uuid::new_v4().to_string()[..8]);
        let row = sqlx::query_as::<_, StatusLabelRow>(
            r#"
            INSERT INTO status_labels (id, workflow_id, name, color, status_group, position)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(workflow_id)
        .bind(name)
        .bind(color)
        .bind(status_group)
        .bind(position)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Partially update a label. Uses COALESCE for optional fields.
    pub async fn update_label(
        &self,
        id: &str,
        name: Option<&str>,
        color: Option<&str>,
        status_group: Option<&str>,
        position: Option<i32>,
    ) -> Result<StatusLabelRow, StorageError> {
        let row = sqlx::query_as::<_, StatusLabelRow>(
            r#"
            UPDATE status_labels
            SET name         = COALESCE(?2, name),
                color        = COALESCE(?3, color),
                status_group = COALESCE(?4, status_group),
                position     = COALESCE(?5, position)
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(name)
        .bind(color)
        .bind(status_group)
        .bind(position)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found("label")?;
        Ok(row)
    }

    /// Delete a label by ID. Returns true if deleted.
    pub async fn delete_label(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM status_labels WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Reorder labels within a workflow by setting positions from the given ID order.
    pub async fn reorder_labels(
        &self,
        workflow_id: &str,
        label_ids: &[String],
    ) -> Result<(), StorageError> {
        for (i, label_id) in label_ids.iter().enumerate() {
            sqlx::query(
                "UPDATE status_labels SET position = ?1 WHERE id = ?2 AND workflow_id = ?3",
            )
            .bind(i as i32)
            .bind(label_id)
            .bind(workflow_id)
            .execute(&self.pool)
            .await?;
        }
        Ok(())
    }

    // ── Effective workflow resolution ─────────────────────

    /// Get labels for a project's workflow, falling back to the global default.
    pub async fn get_effective_labels(
        &self,
        project_workflow_id: Option<&str>,
    ) -> Result<Vec<StatusLabelRow>, StorageError> {
        match project_workflow_id {
            Some(wf_id) => self.get_labels(wf_id).await,
            None => {
                let default = self
                    .get_global_default()
                    .await?
                    .ok_or_not_found("global default workflow")?;
                self.get_labels(&default.id).await
            }
        }
    }

    /// Find the first label matching a status group within a workflow.
    pub async fn find_label_by_group(
        &self,
        workflow_id: &str,
        status_group: &str,
    ) -> Result<Option<StatusLabelRow>, StorageError> {
        let row = sqlx::query_as::<_, StatusLabelRow>(
            r#"
            SELECT * FROM status_labels
            WHERE workflow_id = ?1 AND status_group = ?2
            ORDER BY position
            LIMIT 1
            "#,
        )
        .bind(workflow_id)
        .bind(status_group)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn setup() -> StatusWorkflowRepo {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        StatusWorkflowRepo::new(pool.inner().clone())
    }

    #[tokio::test]
    async fn test_get_global_default() {
        let repo = setup().await;
        let wf = repo.get_global_default().await.unwrap();
        assert!(wf.is_some());
        let wf = wf.unwrap();
        assert_eq!(wf.id, "wf_default");
        assert!(wf.is_global_default);
    }

    #[tokio::test]
    async fn test_list_templates() {
        let repo = setup().await;
        let templates = repo.list_templates().await.unwrap();
        assert_eq!(templates.len(), 3);
    }

    #[tokio::test]
    async fn test_get_labels_for_workflow() {
        let repo = setup().await;
        let labels = repo.get_labels("wf_default").await.unwrap();
        assert_eq!(labels.len(), 6);
        // Verify ordering by position
        for (i, label) in labels.iter().enumerate() {
            assert_eq!(label.position, i as i32);
        }
    }

    #[tokio::test]
    async fn test_create_workflow_with_labels() {
        let repo = setup().await;
        let wf = repo.create("Custom", false).await.unwrap();
        assert!(wf.id.starts_with("wf_"));
        assert_eq!(wf.name, "Custom");
        assert!(!wf.is_template);

        let l1 = repo
            .add_label(&wf.id, "Open", "#aaa", "not_started", 0)
            .await
            .unwrap();
        let l2 = repo
            .add_label(&wf.id, "Closed", "#bbb", "done", 1)
            .await
            .unwrap();
        assert!(l1.id.starts_with("sl_"));
        assert!(l2.id.starts_with("sl_"));

        let labels = repo.get_labels(&wf.id).await.unwrap();
        assert_eq!(labels.len(), 2);
    }

    #[tokio::test]
    async fn test_update_label() {
        let repo = setup().await;
        let updated = repo
            .update_label("sl_backlog", Some("Icebox"), Some("#000000"), None, None)
            .await
            .unwrap();
        assert_eq!(updated.name, "Icebox");
        assert_eq!(updated.color, "#000000");
        // status_group unchanged
        assert_eq!(updated.status_group, "not_started");
    }

    #[tokio::test]
    async fn test_delete_label() {
        let repo = setup().await;
        let before = repo.get_labels("wf_default").await.unwrap().len();
        let deleted = repo.delete_label("sl_blocked").await.unwrap();
        assert!(deleted);
        let after = repo.get_labels("wf_default").await.unwrap().len();
        assert_eq!(after, before - 1);
    }

    #[tokio::test]
    async fn test_delete_workflow_cascades_labels() {
        let repo = setup().await;
        let wf = repo.create("Temp", false).await.unwrap();
        repo.add_label(&wf.id, "A", "#fff", "not_started", 0)
            .await
            .unwrap();

        let deleted = repo.delete(&wf.id).await.unwrap();
        assert!(deleted);

        let labels = repo.get_labels(&wf.id).await.unwrap();
        assert!(labels.is_empty());
    }

    #[tokio::test]
    async fn test_duplicate_workflow() {
        let repo = setup().await;
        let dup = repo.duplicate("wf_default", "My Copy").await.unwrap();
        assert_ne!(dup.id, "wf_default");
        assert_eq!(dup.name, "My Copy");

        let labels = repo.get_labels(&dup.id).await.unwrap();
        assert_eq!(labels.len(), 6);
    }

    #[tokio::test]
    async fn test_get_effective_workflow_fallback() {
        let repo = setup().await;
        let labels = repo.get_effective_labels(None).await.unwrap();
        assert_eq!(labels.len(), 6);
    }

    #[tokio::test]
    async fn test_resolve_status_group() {
        let repo = setup().await;
        let label = repo.get_label("sl_done").await.unwrap().unwrap();
        assert_eq!(label.status_group, "done");
    }
}
