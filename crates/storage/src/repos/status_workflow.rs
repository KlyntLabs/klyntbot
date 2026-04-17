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
        let row =
            sqlx::query_as::<_, StatusWorkflowRow>("SELECT * FROM status_workflows WHERE id = ?1")
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
        let result =
            sqlx::query("DELETE FROM status_workflows WHERE id = ?1 AND is_global_default = 0")
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
        let row = sqlx::query_as::<_, StatusLabelRow>("SELECT * FROM status_labels WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Fetch all labels for a workflow, ordered by position.
    pub async fn get_labels(&self, workflow_id: &str) -> Result<Vec<StatusLabelRow>, StorageError> {
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

    /// Fetch labels by a list of IDs (batch load to avoid N+1).
    pub async fn get_labels_by_ids(
        &self,
        ids: &[&str],
    ) -> Result<Vec<StatusLabelRow>, StorageError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut qb =
            sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM status_labels WHERE id IN (");
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(*id);
        }
        sep.push_unseparated(")");
        let rows = qb
            .build_query_as::<StatusLabelRow>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
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

    #[tokio::test]
    async fn test_full_workflow_lifecycle() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        // The tasks table lives in the feature-tasks migration, not core migrations.
        // Create a minimal stub so task insertion works.
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS tasks (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                description TEXT,
                area_id TEXT NOT NULL,
                project_id TEXT,
                key_result_id TEXT,
                objective_id TEXT,
                parent_id TEXT,
                status_label_id TEXT,
                group_id TEXT,
                priority INTEGER,
                position INTEGER NOT NULL DEFAULT 0,
                due_date INTEGER,
                tags TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'todo',
                task_type TEXT NOT NULL DEFAULT 'manual',
                focused_at INTEGER,
                focus_deadline INTEGER,
                focus_expired_count INTEGER NOT NULL DEFAULT 0,
                created_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                updated_at INTEGER NOT NULL DEFAULT (unixepoch('now') * 1000),
                completed_at INTEGER,
                completed INTEGER NOT NULL DEFAULT 0,
                total_tracked_secs INTEGER NOT NULL DEFAULT 0,
                estimated_minutes INTEGER,
                actual_minutes INTEGER,
                calendar_event_uid TEXT,
                last_reminded_at INTEGER,
                recurrence_rule TEXT,
                recurrence_parent_id TEXT,
                is_template INTEGER NOT NULL DEFAULT 0,
                next_instance_date INTEGER,
                acceptance_criteria TEXT,
                agent_config TEXT,
                execution_state TEXT NOT NULL DEFAULT 'idle',
                spawned_execution_id TEXT,
                context_snapshot TEXT,
                energy_level TEXT DEFAULT 'medium',
                estimated_focus_blocks INTEGER,
                complexity_score INTEGER,
                scheduled_start INTEGER,
                scheduled_end INTEGER
            )",
        )
        .execute(pool.inner())
        .await
        .unwrap();
        let repos = crate::Repos::from_pool(&pool);

        // 1. Global default exists with 6 labels
        let default = repos
            .status_workflows
            .get_global_default()
            .await
            .unwrap()
            .unwrap();
        let labels = repos
            .status_workflows
            .get_labels(&default.id)
            .await
            .unwrap();
        assert_eq!(labels.len(), 6);
        assert_eq!(labels[0].name, "Backlog");
        assert_eq!(labels[5].name, "Blocked");

        // 2. Templates exist
        let templates = repos.status_workflows.list_templates().await.unwrap();
        assert_eq!(templates.len(), 3);

        // 3. Duplicate the global default as a custom workflow
        let custom = repos
            .status_workflows
            .duplicate(&default.id, "My Project")
            .await
            .unwrap();
        assert_ne!(custom.id, default.id);
        assert_eq!(custom.name, "My Project");
        let custom_labels = repos.status_workflows.get_labels(&custom.id).await.unwrap();
        assert_eq!(custom_labels.len(), 6);

        // 4. Add a custom label to the custom workflow
        let testing_label = repos
            .status_workflows
            .add_label(&custom.id, "Testing", "#8b5cf6", "active", 4)
            .await
            .unwrap();
        assert_eq!(testing_label.name, "Testing");
        assert_eq!(testing_label.status_group, "active");
        let updated_labels = repos.status_workflows.get_labels(&custom.id).await.unwrap();
        assert_eq!(updated_labels.len(), 7);

        // 5. Update a label
        let updated_label = repos
            .status_workflows
            .update_label(
                &custom_labels[1].id,
                Some("To Do"),
                Some("#60a5fa"),
                None,
                None,
            )
            .await
            .unwrap();
        assert_eq!(updated_label.name, "To Do");
        assert_eq!(updated_label.color, "#60a5fa");

        // 6. Delete a label
        let deleted = repos
            .status_workflows
            .delete_label(&custom_labels[0].id)
            .await
            .unwrap();
        assert!(deleted);
        let labels_after_delete = repos.status_workflows.get_labels(&custom.id).await.unwrap();
        assert_eq!(labels_after_delete.len(), 6);

        // 7. Reorder labels
        let label_ids: Vec<String> = labels_after_delete.iter().map(|l| l.id.clone()).collect();
        let mut reversed = label_ids.clone();
        reversed.reverse();
        repos
            .status_workflows
            .reorder_labels(&custom.id, &reversed)
            .await
            .unwrap();
        let reordered = repos.status_workflows.get_labels(&custom.id).await.unwrap();
        assert_eq!(reordered[0].id, reversed[0]);

        // 8. Create a task with status_label_id
        let todo_label = &custom_labels[1]; // "Todo" label

        // Create the area row needed for the action's foreign key
        let now: crate::sqlite_types::SqlTs = jiff::Timestamp::now().into();
        let area_row = crate::AreaRow {
            id: "default".into(),
            name: "Default".into(),
            description: None,
            color: "#888888".into(),
            icon: None,
            position: 0,
            status: "active".into(),
            created_at: now,
            updated_at: now,
        };
        repos.areas.create(&area_row).await.ok();

        let task_row = crate::TaskRow {
            id: "task-wf-test".into(),
            title: "Test task".into(),
            description: None,
            area_id: "default".into(),
            project_id: None,
            key_result_id: None,
            parent_id: None,
            priority: None,
            due_date: None,
            tags: vec![],
            status: "todo".into(),
            focused_at: None,
            focus_deadline: None,
            focus_expired_count: 0,
            created_at: now,
            updated_at: now,
            completed_at: None,
            total_tracked_secs: 0,
            estimated_minutes: None,
            calendar_event_uid: None,
            last_reminded_at: None,
            recurrence_rule: None,
            recurrence_parent_id: None,
            is_template: false,
            next_instance_date: None,
            status_label_id: Some(todo_label.id.clone()),
            position: 0,
            group_id: None,
            task_type: "standard".into(),
            acceptance_criteria: None,
            agent_config: None,
            execution_state: "idle".into(),
            spawned_execution_id: None,
            context_snapshot: None,
            energy_level: None,
            estimated_focus_blocks: None,
            actual_minutes: None,
            complexity_score: None,
            completed: false,
            objective_id: None,
            scheduled_start: None,
            scheduled_end: None,
        };
        let inserted = repos.tasks.add(&task_row).await.unwrap();
        assert_eq!(inserted.status_label_id, Some(todo_label.id.clone()));

        // 9. Summary by group should work
        let group_summary = repos.tasks.summary_by_group().await.unwrap();
        assert_eq!(*group_summary.get("not_started").unwrap_or(&0), 1);

        // 10. Effective labels with None falls back to global default
        let effective = repos
            .status_workflows
            .get_effective_labels(None)
            .await
            .unwrap();
        assert_eq!(effective.len(), 6);

        // 11. Find label by group
        let done_label = repos
            .status_workflows
            .find_label_by_group(&custom.id, "done")
            .await
            .unwrap();
        assert!(done_label.is_some());
        assert_eq!(done_label.unwrap().status_group, "done");

        // 12. Cannot delete global default
        let cannot_delete = repos.status_workflows.delete(&default.id).await.unwrap();
        assert!(!cannot_delete);

        // 13. Delete custom workflow cascades labels
        let deleted_wf = repos.status_workflows.delete(&custom.id).await.unwrap();
        assert!(deleted_wf);
        let orphaned_labels = repos.status_workflows.get_labels(&custom.id).await.unwrap();
        assert!(orphaned_labels.is_empty());
    }
}
