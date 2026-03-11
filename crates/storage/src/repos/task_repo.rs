//! Repository for the `tasks` table and its satellite tables
//! (`task_attachments`, `task_time_entries`, `task_dependencies`,
//!  `task_activity`, `task_executions`, `task_suggestions`, `task_estimation_history`).

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::SqlitePool;

use crate::error::{OptionExt, StorageError};
use crate::rows::task::{
    TaskActivityRow, TaskAttachmentRow, TaskDecompositionRow, TaskEstimationRow, TaskExecutionRow,
    TaskRow, TaskSuggestionRow, TaskTimeEntryRow,
};

/// Filter criteria for listing tasks.
#[derive(Debug, Default, Clone)]
pub struct TaskFilter {
    pub status: Option<String>,
    pub tags: Option<Vec<String>>,
    pub area_id: Option<String>,
    pub project_id: Option<String>,
    pub key_result_id: Option<String>,
    pub unassigned: bool,
    pub root_only: bool,
    pub priority_min: Option<i16>,
    pub due_after: Option<DateTime<Utc>>,
    pub due_before: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
    pub templates_only: bool,
    pub status_group: Option<String>,
    pub group_id: Option<String>,
    pub task_type: Option<String>,
    pub execution_state: Option<String>,
    pub energy_level: Option<String>,
    pub completed: Option<bool>,
}

/// Patch struct for partial task updates. Only non-None fields are overwritten.
#[derive(Debug, Default, Clone)]
pub struct TaskPatch {
    pub id: String,
    pub title: Option<String>,
    pub description: Option<Option<String>>,
    pub priority: Option<Option<i16>>,
    pub due_date: Option<Option<DateTime<Utc>>>,
    pub tags: Option<Vec<String>>,
    pub status: Option<String>,
    pub calendar_event_uid: Option<Option<String>>,
    pub next_instance_date: Option<Option<DateTime<Utc>>>,
    pub last_reminded_at: Option<Option<DateTime<Utc>>>,
    pub estimated_minutes: Option<Option<i32>>,
    pub recurrence_rule: Option<Option<String>>,
    pub area_id: Option<String>,
    pub project_id: Option<Option<String>>,
    pub key_result_id: Option<Option<String>>,
    pub status_label_id: Option<Option<String>>,
    pub position: Option<i32>,
    pub group_id: Option<Option<String>>,
    pub task_type: Option<String>,
    pub acceptance_criteria: Option<Option<String>>,
    pub agent_config: Option<Option<String>>,
    pub execution_state: Option<String>,
    pub spawned_execution_id: Option<Option<String>>,
    pub energy_level: Option<Option<String>>,
    pub complexity_score: Option<Option<i32>>,
    pub completed: Option<bool>,
    pub actual_minutes: Option<Option<i32>>,
    pub objective_id: Option<Option<String>>,
}

/// Aggregate counts by status.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskSummary {
    pub todo: i64,
    pub doing: i64,
    pub done: i64,
    pub total: i64,
}

/// Repository for task CRUD, hierarchy, focus, dependencies, attachments,
/// time tracking, activity log, executions, suggestions, and estimation.
#[derive(Debug, Clone)]
pub struct TaskRepo {
    pool: SqlitePool,
}

impl TaskRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    // -----------------------------------------------------------------------
    // CRUD
    // -----------------------------------------------------------------------

    /// Insert a new task. Returns the inserted row.
    pub async fn add(&self, row: &TaskRow) -> Result<TaskRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskRow>(
            r#"
            INSERT INTO tasks (
                id, title, description, area_id, project_id, key_result_id,
                parent_id, priority, due_date, tags, status,
                focused_at, focus_deadline, focus_expired_count,
                created_at, updated_at, completed_at,
                total_tracked_secs, estimated_minutes,
                calendar_event_uid, last_reminded_at,
                recurrence_rule, recurrence_parent_id, is_template, next_instance_date,
                status_label_id, position, group_id,
                task_type, acceptance_criteria, agent_config,
                execution_state, spawned_execution_id, context_snapshot,
                energy_level, estimated_focus_blocks, actual_minutes,
                complexity_score, completed, objective_id
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10, ?11,
                ?12, ?13, ?14,
                ?15, ?16, ?17,
                ?18, ?19,
                ?20, ?21,
                ?22, ?23, ?24, ?25,
                ?26, ?27, ?28,
                ?29, ?30, ?31,
                ?32, ?33, ?34,
                ?35, ?36, ?37,
                ?38, ?39, ?40
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.title)
        .bind(&row.description)
        .bind(&row.area_id)
        .bind(&row.project_id)
        .bind(&row.key_result_id)
        .bind(&row.parent_id)
        .bind(row.priority)
        .bind(row.due_date)
        .bind(sqlx::types::Json(&row.tags))
        .bind(&row.status)
        .bind(row.focused_at)
        .bind(row.focus_deadline)
        .bind(row.focus_expired_count)
        .bind(row.created_at)
        .bind(row.updated_at)
        .bind(row.completed_at)
        .bind(row.total_tracked_secs)
        .bind(row.estimated_minutes)
        .bind(&row.calendar_event_uid)
        .bind(row.last_reminded_at)
        .bind(&row.recurrence_rule)
        .bind(&row.recurrence_parent_id)
        .bind(row.is_template)
        .bind(row.next_instance_date)
        .bind(&row.status_label_id)
        .bind(row.position)
        .bind(&row.group_id)
        .bind(&row.task_type)
        .bind(&row.acceptance_criteria)
        .bind(&row.agent_config)
        .bind(&row.execution_state)
        .bind(&row.spawned_execution_id)
        .bind(&row.context_snapshot)
        .bind(&row.energy_level)
        .bind(row.estimated_focus_blocks)
        .bind(row.actual_minutes)
        .bind(row.complexity_score)
        .bind(row.completed)
        .bind(&row.objective_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(inserted)
    }

    /// Get a single task by id. Returns `None` if not found.
    pub async fn get(&self, id: &str) -> Result<Option<TaskRow>, StorageError> {
        let row = sqlx::query_as::<_, TaskRow>("SELECT * FROM tasks WHERE id = ?1")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(row)
    }

    /// Get a single task by id, returning `StorageError::NotFound` if missing.
    pub async fn get_or_err(&self, id: &str) -> Result<TaskRow, StorageError> {
        self.get(id).await?.ok_or_not_found(&format!("task {id}"))
    }

    /// Fetch tasks by a list of IDs. Missing IDs are silently skipped.
    pub async fn get_by_ids(&self, ids: &[String]) -> Result<Vec<TaskRow>, StorageError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM tasks WHERE id IN (");
        let mut sep = qb.separated(", ");
        for id in ids {
            sep.push_bind(id);
        }
        qb.push(")");
        let rows = qb.build_query_as::<TaskRow>().fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// Update mutable fields on a task. Only non-None fields are overwritten.
    pub async fn update(&self, patch: &TaskPatch) -> Result<TaskRow, StorageError> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            UPDATE tasks SET
                title              = COALESCE(?2, title),
                description        = CASE WHEN ?3 THEN ?4 ELSE description END,
                priority           = CASE WHEN ?5 THEN ?6 ELSE priority END,
                due_date           = CASE WHEN ?7 THEN ?8 ELSE due_date END,
                tags               = COALESCE(?9, tags),
                status             = COALESCE(?10, status),
                completed_at       = CASE
                    WHEN ?10 = 'done' AND completed_at IS NULL THEN datetime('now')
                    WHEN ?10 IS NOT NULL AND ?10 != 'done' THEN NULL
                    ELSE completed_at
                END,
                calendar_event_uid = CASE WHEN ?11 THEN ?12 ELSE calendar_event_uid END,
                next_instance_date = CASE WHEN ?13 THEN ?14 ELSE next_instance_date END,
                last_reminded_at   = CASE WHEN ?15 THEN ?16 ELSE last_reminded_at END,
                estimated_minutes  = CASE WHEN ?17 THEN ?18 ELSE estimated_minutes END,
                recurrence_rule    = CASE WHEN ?19 THEN ?20 ELSE recurrence_rule END,
                area_id            = COALESCE(?21, area_id),
                project_id         = CASE WHEN ?22 THEN ?23 ELSE project_id END,
                key_result_id      = CASE WHEN ?24 THEN ?25 ELSE key_result_id END,
                status_label_id    = CASE WHEN ?26 THEN ?27 ELSE status_label_id END,
                position           = COALESCE(?28, position),
                group_id           = CASE WHEN ?29 THEN ?30 ELSE group_id END,
                task_type          = COALESCE(?31, task_type),
                acceptance_criteria = CASE WHEN ?32 THEN ?33 ELSE acceptance_criteria END,
                agent_config       = CASE WHEN ?34 THEN ?35 ELSE agent_config END,
                execution_state    = COALESCE(?36, execution_state),
                spawned_execution_id = CASE WHEN ?37 THEN ?38 ELSE spawned_execution_id END,
                energy_level       = CASE WHEN ?39 THEN ?40 ELSE energy_level END,
                complexity_score   = CASE WHEN ?41 THEN ?42 ELSE complexity_score END,
                completed          = COALESCE(?43, completed),
                actual_minutes     = CASE WHEN ?44 THEN ?45 ELSE actual_minutes END,
                objective_id       = CASE WHEN ?46 THEN ?47 ELSE objective_id END,
                updated_at         = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(&patch.id) // ?1
        .bind(&patch.title) // ?2
        .bind(patch.description.is_some()) // ?3
        .bind(patch.description.as_ref().and_then(|d| d.as_deref())) // ?4
        .bind(patch.priority.is_some()) // ?5
        .bind(patch.priority.unwrap_or_default()) // ?6
        .bind(patch.due_date.is_some()) // ?7
        .bind(patch.due_date.unwrap_or_default()) // ?8
        .bind(patch.tags.as_ref().map(sqlx::types::Json)) // ?9
        .bind(&patch.status) // ?10
        .bind(patch.calendar_event_uid.is_some()) // ?11
        .bind(patch.calendar_event_uid.as_ref().and_then(|v| v.as_deref())) // ?12
        .bind(patch.next_instance_date.is_some()) // ?13
        .bind(patch.next_instance_date.unwrap_or_default()) // ?14
        .bind(patch.last_reminded_at.is_some()) // ?15
        .bind(patch.last_reminded_at.unwrap_or_default()) // ?16
        .bind(patch.estimated_minutes.is_some()) // ?17
        .bind(patch.estimated_minutes.unwrap_or_default()) // ?18
        .bind(patch.recurrence_rule.is_some()) // ?19
        .bind(patch.recurrence_rule.as_ref().and_then(|v| v.as_deref())) // ?20
        .bind(&patch.area_id) // ?21
        .bind(patch.project_id.is_some()) // ?22
        .bind(patch.project_id.as_ref().and_then(|v| v.as_deref())) // ?23
        .bind(patch.key_result_id.is_some()) // ?24
        .bind(patch.key_result_id.as_ref().and_then(|v| v.as_deref())) // ?25
        .bind(patch.status_label_id.is_some()) // ?26
        .bind(patch.status_label_id.as_ref().and_then(|v| v.as_deref())) // ?27
        .bind(patch.position) // ?28
        .bind(patch.group_id.is_some()) // ?29
        .bind(patch.group_id.as_ref().and_then(|v| v.as_deref())) // ?30
        .bind(&patch.task_type) // ?31
        .bind(patch.acceptance_criteria.is_some()) // ?32
        .bind(
            patch
                .acceptance_criteria
                .as_ref()
                .and_then(|v| v.as_deref()),
        ) // ?33
        .bind(patch.agent_config.is_some()) // ?34
        .bind(patch.agent_config.as_ref().and_then(|v| v.as_deref())) // ?35
        .bind(&patch.execution_state) // ?36
        .bind(patch.spawned_execution_id.is_some()) // ?37
        .bind(patch.spawned_execution_id.as_ref().and_then(|v| v.as_deref())) // ?38
        .bind(patch.energy_level.is_some()) // ?39
        .bind(patch.energy_level.as_ref().and_then(|v| v.as_deref())) // ?40
        .bind(patch.complexity_score.is_some()) // ?41
        .bind(patch.complexity_score.unwrap_or_default()) // ?42
        .bind(patch.completed.map(|b| b as i32)) // ?43
        .bind(patch.actual_minutes.is_some()) // ?44
        .bind(patch.actual_minutes.unwrap_or_default()) // ?45
        .bind(patch.objective_id.is_some()) // ?46
        .bind(patch.objective_id.as_ref().and_then(|v| v.as_deref())) // ?47
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("task {}", patch.id))?;

        Ok(row)
    }

    /// Delete a task and all its cascade dependents.
    pub async fn delete(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM tasks WHERE id = ?1")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Listing / Filtering
    // -----------------------------------------------------------------------

    /// List tasks matching the given filter criteria.
    pub async fn list(&self, filter: &TaskFilter) -> Result<Vec<TaskRow>, StorageError> {
        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new("SELECT * FROM tasks WHERE ");

        if filter.templates_only {
            qb.push("is_template = TRUE");
        } else {
            qb.push("is_template = FALSE");
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

        if let Some(ref area_id) = filter.area_id {
            qb.push(" AND area_id = ");
            qb.push_bind(area_id);
        }

        if let Some(ref project_id) = filter.project_id {
            qb.push(" AND project_id = ");
            qb.push_bind(project_id);
        }

        if let Some(ref kr_id) = filter.key_result_id {
            qb.push(" AND key_result_id = ");
            qb.push_bind(kr_id);
        }

        if filter.unassigned {
            qb.push(" AND project_id IS NULL");
        }

        if filter.root_only {
            qb.push(" AND parent_id IS NULL");
        }

        if let Some(pmin) = filter.priority_min {
            qb.push(" AND priority >= ");
            qb.push_bind(pmin);
        }

        if let Some(ref after) = filter.due_after {
            qb.push(" AND due_date >= ");
            qb.push_bind(after);
        }

        if let Some(ref before) = filter.due_before {
            qb.push(" AND due_date < ");
            qb.push_bind(before);
        }

        if let Some(ref group) = filter.status_group {
            qb.push(" AND status_label_id IN (SELECT id FROM status_labels WHERE status_group = ");
            qb.push_bind(group);
            qb.push(")");
        }

        if let Some(ref gid) = filter.group_id {
            qb.push(" AND group_id = ");
            qb.push_bind(gid);
        }

        if let Some(ref tt) = filter.task_type {
            qb.push(" AND task_type = ");
            qb.push_bind(tt);
        }

        if let Some(ref es) = filter.execution_state {
            qb.push(" AND execution_state = ");
            qb.push_bind(es);
        }

        if let Some(ref el) = filter.energy_level {
            qb.push(" AND energy_level = ");
            qb.push_bind(el);
        }

        if let Some(completed) = filter.completed {
            qb.push(" AND completed = ");
            qb.push_bind(completed as i32);
        }

        qb.push(" ORDER BY created_at DESC");

        if let Some(limit) = filter.limit {
            qb.push(" LIMIT ");
            qb.push_bind(limit);
        }

        let rows = qb.build_query_as::<TaskRow>().fetch_all(&self.pool).await?;
        Ok(rows)
    }

    /// List all templates (is_template = true).
    pub async fn list_templates(&self) -> Result<Vec<TaskRow>, StorageError> {
        self.list(&TaskFilter {
            templates_only: true,
            ..Default::default()
        })
        .await
    }

    /// Search tasks by keyword in title or description (case-insensitive).
    pub async fn search_by_keyword(
        &self,
        query: &str,
        limit: Option<i64>,
    ) -> Result<Vec<TaskRow>, StorageError> {
        let escaped = query
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let pattern = format!("%{escaped}%");
        let effective_limit = limit.unwrap_or(i64::MAX);
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT * FROM tasks
            WHERE is_template = FALSE
              AND (title LIKE ?1 ESCAPE '\' OR description LIKE ?1 ESCAPE '\')
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(&pattern)
        .bind(effective_limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Focus Slots
    // -----------------------------------------------------------------------

    /// Focus a task. Returns true if the focus was set, false if at max_slots.
    pub async fn focus(
        &self,
        id: &str,
        max_slots: i64,
        deadline: Option<DateTime<Utc>>,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query(
            r#"
            UPDATE tasks
            SET focused_at = datetime('now'), focus_deadline = ?3, updated_at = datetime('now')
            WHERE id = ?1
              AND focused_at IS NULL
              AND (SELECT COUNT(*) FROM tasks WHERE focused_at IS NOT NULL) < ?2
            "#,
        )
        .bind(id)
        .bind(max_slots)
        .bind(deadline)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Unfocus a task.
    pub async fn unfocus(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE tasks SET focused_at = NULL, focus_deadline = NULL, updated_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List currently focused tasks.
    pub async fn list_focused(&self) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE focused_at IS NOT NULL ORDER BY focused_at",
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Dependencies (edge table: task_dependencies)
    // -----------------------------------------------------------------------

    /// Add a dependency edge (task_id is blocked by blocker_id) with a dependency type.
    pub async fn add_dependency(
        &self,
        task_id: &str,
        blocker_id: &str,
        dep_type: &str,
    ) -> Result<(), StorageError> {
        let would_cycle = self.would_create_cycle(task_id, blocker_id).await?;
        if would_cycle {
            return Err(StorageError::Conflict(format!(
                "Adding dependency {task_id} -> {blocker_id} would create a cycle"
            )));
        }

        sqlx::query(
            "INSERT INTO task_dependencies (task_id, blocker_id, dep_type) VALUES (?1, ?2, ?3)",
        )
        .bind(task_id)
        .bind(blocker_id)
        .bind(dep_type)
        .execute(&self.pool)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(ref db_err) = e {
                if db_err.constraint().is_some() {
                    return StorageError::Conflict(format!(
                        "Cannot add dependency: {task_id} -> {blocker_id}"
                    ));
                }
            }
            StorageError::Sqlx(e)
        })?;

        Ok(())
    }

    /// Remove a dependency edge.
    pub async fn remove_dependency(
        &self,
        task_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let result =
            sqlx::query("DELETE FROM task_dependencies WHERE task_id = ?1 AND blocker_id = ?2")
                .bind(task_id)
                .bind(blocker_id)
                .execute(&self.pool)
                .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Get all blockers for a task.
    pub async fn get_blockers(&self, task_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN task_dependencies d ON d.blocker_id = t.id
            WHERE d.task_id = ?1
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get incomplete blockers for a task (completed = false).
    pub async fn incomplete_blockers(&self, task_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN task_dependencies d ON d.blocker_id = t.id
            WHERE d.task_id = ?1 AND t.completed = 0
            "#,
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Get tasks blocked by this task.
    pub async fn get_blocking(&self, blocker_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT t.* FROM tasks t
            INNER JOIN task_dependencies d ON d.task_id = t.id
            WHERE d.blocker_id = ?1
            "#,
        )
        .bind(blocker_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    async fn would_create_cycle(
        &self,
        task_id: &str,
        blocker_id: &str,
    ) -> Result<bool, StorageError> {
        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE reachable AS (
                SELECT blocker_id AS node FROM task_dependencies WHERE task_id = ?2
                UNION
                SELECT d.blocker_id FROM task_dependencies d
                INNER JOIN reachable r ON d.task_id = r.node
            )
            SELECT EXISTS(SELECT 1 FROM reachable WHERE node = ?1)
            "#,
        )
        .bind(task_id)
        .bind(blocker_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }

    // -----------------------------------------------------------------------
    // Attachments (join table: task_attachments)
    // -----------------------------------------------------------------------

    /// Add an attachment to a task.
    pub async fn add_attachment(
        &self,
        task_id: &str,
        attachment_type: &str,
        value: &str,
        title: Option<&str>,
        tags: &[String],
        source: &str,
    ) -> Result<TaskAttachmentRow, StorageError> {
        let id = uuid::Uuid::new_v4();
        let row = sqlx::query_as::<_, TaskAttachmentRow>(
            r#"
            INSERT INTO task_attachments (id, task_id, attachment_type, value, title, tags, source)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(task_id)
        .bind(attachment_type)
        .bind(value)
        .bind(title)
        .bind(sqlx::types::Json(tags))
        .bind(source)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Remove an attachment by its UUID.
    pub async fn remove_attachment(
        &self,
        task_id: &str,
        attachment_id: uuid::Uuid,
    ) -> Result<bool, StorageError> {
        let result = sqlx::query("DELETE FROM task_attachments WHERE id = ?1 AND task_id = ?2")
            .bind(attachment_id)
            .bind(task_id)
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List all attachments for a task.
    pub async fn list_attachments(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskAttachmentRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskAttachmentRow>(
            "SELECT * FROM task_attachments WHERE task_id = ?1 ORDER BY created_at",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Time Entries (join table: task_time_entries)
    // -----------------------------------------------------------------------

    /// Add a time entry.
    pub async fn add_time_entry(
        &self,
        task_id: &str,
        source: &str,
        started_at: DateTime<Utc>,
        duration_secs: Option<i64>,
        note: Option<&str>,
        energy_level: Option<&str>,
    ) -> Result<TaskTimeEntryRow, StorageError> {
        let ended_at = duration_secs.map(|d| started_at + chrono::Duration::seconds(d));
        let id = uuid::Uuid::new_v4();
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, TaskTimeEntryRow>(
            r#"
            INSERT INTO task_time_entries (id, task_id, source, started_at, ended_at, duration_secs, note, energy_level)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(task_id)
        .bind(source)
        .bind(started_at)
        .bind(ended_at)
        .bind(duration_secs)
        .bind(note)
        .bind(energy_level)
        .fetch_one(&mut *tx)
        .await?;

        if let Some(secs) = duration_secs {
            sqlx::query(
                "UPDATE tasks SET total_tracked_secs = total_tracked_secs + ?2, updated_at = datetime('now') WHERE id = ?1",
            )
            .bind(task_id)
            .bind(secs)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(row)
    }

    /// Close an open time entry.
    pub async fn close_time_entry(
        &self,
        task_id: &str,
        entry_id: uuid::Uuid,
    ) -> Result<TaskTimeEntryRow, StorageError> {
        let mut tx = self.pool.begin().await?;

        let row = sqlx::query_as::<_, TaskTimeEntryRow>(
            r#"
            UPDATE task_time_entries
            SET ended_at = datetime('now'),
                duration_secs = (unixepoch('now') - unixepoch(started_at))
            WHERE id = ?1 AND task_id = ?2 AND ended_at IS NULL
            RETURNING *
            "#,
        )
        .bind(entry_id)
        .bind(task_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(|| {
            StorageError::NotFound(format!("open time entry {entry_id} for task {task_id}"))
        })?;

        if let Some(secs) = row.duration_secs {
            sqlx::query(
                "UPDATE tasks SET total_tracked_secs = total_tracked_secs + ?2, updated_at = datetime('now') WHERE id = ?1",
            )
            .bind(task_id)
            .bind(secs)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(row)
    }

    /// List time entries for a task.
    pub async fn list_time_entries(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskTimeEntryRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskTimeEntryRow>(
            "SELECT * FROM task_time_entries WHERE task_id = ?1 ORDER BY started_at",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Hierarchy
    // -----------------------------------------------------------------------

    /// Get immediate children of a task.
    pub async fn get_children(&self, parent_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT * FROM tasks WHERE parent_id = ?1 ORDER BY created_at",
        )
        .bind(parent_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Count immediate children, returning (total, completed) using the `completed` bool column.
    pub async fn count_children(&self, parent_id: &str) -> Result<(i64, i64), StorageError> {
        let row: (i64, i64) = sqlx::query_as(
            "SELECT COUNT(*), SUM(CASE WHEN completed = 1 THEN 1 ELSE 0 END) FROM tasks WHERE parent_id = ?1",
        )
        .bind(parent_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// Bulk count immediate children for multiple parents, returning a map of id -> (total, completed).
    pub async fn count_children_bulk(
        &self,
        parent_ids: &[String],
    ) -> Result<HashMap<String, (i64, i64)>, StorageError> {
        if parent_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
            "SELECT parent_id, COUNT(*), SUM(CASE WHEN completed = 1 THEN 1 ELSE 0 END) FROM tasks WHERE parent_id IN (",
        );
        let mut sep = qb.separated(", ");
        for id in parent_ids {
            sep.push_bind(id);
        }
        qb.push(") GROUP BY parent_id");

        let rows = qb
            .build_query_as::<(String, i64, i64)>()
            .fetch_all(&self.pool)
            .await?;
        Ok(rows
            .into_iter()
            .map(|(id, total, done)| (id, (total, done)))
            .collect())
    }

    /// Get full subtree of a task (recursive CTE).
    pub async fn get_subtree(&self, root_id: &str) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            WITH RECURSIVE subtree AS (
                SELECT * FROM tasks WHERE id = ?1
                UNION ALL
                SELECT t.* FROM tasks t
                INNER JOIN subtree s ON t.parent_id = s.id
            )
            SELECT * FROM subtree ORDER BY created_at
            "#,
        )
        .bind(root_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Move a task to a new parent and/or project.
    pub async fn move_task(
        &self,
        id: &str,
        new_parent_id: Option<&str>,
        new_project_id: Option<&str>,
    ) -> Result<TaskRow, StorageError> {
        if let Some(parent_id) = new_parent_id {
            if self.would_create_parent_cycle(id, parent_id).await? {
                return Err(StorageError::Conflict(format!(
                    "Setting parent {parent_id} for task {id} would create a cycle"
                )));
            }
        }

        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            UPDATE tasks
            SET parent_id = ?2, project_id = ?3, updated_at = datetime('now')
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(new_parent_id)
        .bind(new_project_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("task {id}"))?;

        Ok(row)
    }

    async fn would_create_parent_cycle(
        &self,
        child_id: &str,
        new_parent_id: &str,
    ) -> Result<bool, StorageError> {
        if child_id == new_parent_id {
            return Ok(true);
        }

        let row: Option<(bool,)> = sqlx::query_as(
            r#"
            WITH RECURSIVE ancestors AS (
                SELECT parent_id FROM tasks WHERE id = ?2
                UNION ALL
                SELECT t.parent_id FROM tasks t
                INNER JOIN ancestors a ON t.id = a.parent_id
                WHERE a.parent_id IS NOT NULL
            )
            SELECT EXISTS(SELECT 1 FROM ancestors WHERE parent_id = ?1)
            "#,
        )
        .bind(child_id)
        .bind(new_parent_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| r.0).unwrap_or(false))
    }

    /// Mark a task and all children as completed recursively using the `completed` column.
    pub async fn cascade_complete(&self, root_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query(
            r#"
            WITH RECURSIVE subtree AS (
                SELECT id FROM tasks WHERE id = ?1
                UNION ALL
                SELECT t.id FROM tasks t
                INNER JOIN subtree s ON t.parent_id = s.id
            )
            UPDATE tasks SET completed = 1, status = 'done', completed_at = datetime('now'), updated_at = datetime('now')
            WHERE id IN (SELECT id FROM subtree) AND completed = 0
            "#,
        )
        .bind(root_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // -----------------------------------------------------------------------
    // Aggregation
    // -----------------------------------------------------------------------

    /// Count tasks by status.
    pub async fn summary(&self) -> Result<TaskSummary, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT status, COUNT(*) as count
            FROM tasks
            WHERE is_template = FALSE
            GROUP BY status
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        let mut summary = TaskSummary::default();
        for (status, count) in &rows {
            match status.as_str() {
                "todo" => summary.todo = *count,
                "doing" => summary.doing = *count,
                "done" => summary.done = *count,
                _ => {}
            }
            summary.total += count;
        }
        Ok(summary)
    }

    /// Aggregate task counts by status group (via status_labels JOIN).
    pub async fn summary_by_group(&self) -> Result<HashMap<String, i64>, StorageError> {
        let rows: Vec<(String, i64)> = sqlx::query_as(
            r#"
            SELECT sl.status_group, COUNT(*) as cnt
            FROM tasks t
            JOIN status_labels sl ON t.status_label_id = sl.id
            WHERE t.is_template = 0
            GROUP BY sl.status_group
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().collect())
    }

    /// Get overdue tasks.
    pub async fn overdue(&self) -> Result<Vec<TaskRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT * FROM tasks
            WHERE due_date < datetime('now')
              AND completed = 0
              AND is_template = FALSE
            ORDER BY due_date
            "#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Build a context string of active tasks for LLM context injection.
    #[allow(clippy::type_complexity)]
    pub async fn to_context_string(&self) -> Result<String, StorageError> {
        let rows: Vec<(
            String,
            String,
            Option<i16>,
            Option<chrono::DateTime<chrono::Utc>>,
            String,
        )> = sqlx::query_as(
            r#"
                SELECT t.title, t.status, t.priority, t.focused_at, ar.name
                FROM tasks t
                JOIN areas ar ON t.area_id = ar.id
                WHERE t.status IN ('todo', 'doing')
                  AND t.is_template = FALSE
                ORDER BY
                    CASE WHEN t.focused_at IS NOT NULL THEN 0 ELSE 1 END,
                    t.priority ASC NULLS LAST,
                    t.created_at
                "#,
        )
        .fetch_all(&self.pool)
        .await?;

        if rows.is_empty() {
            return Ok("No active tasks.".to_string());
        }

        let mut out = String::from("Active tasks:\n");
        for (title, status, priority, focused_at, area_name) in &rows {
            let focus_marker = if focused_at.is_some() {
                " [FOCUSED]"
            } else {
                ""
            };
            let priority_str = priority.map(|p| format!(" P{p}")).unwrap_or_default();
            out.push_str(&format!(
                "- [{}]{}{} {} ({})\n",
                status, focus_marker, priority_str, title, area_name
            ));
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Activity Log
    // -----------------------------------------------------------------------

    /// Log an activity entry for a task.
    #[allow(clippy::too_many_arguments)]
    pub async fn log_activity(
        &self,
        task_id: &str,
        activity_type: &str,
        field_changed: Option<&str>,
        old_value: Option<&str>,
        new_value: Option<&str>,
        actor_type: &str,
        summary: Option<&str>,
    ) -> Result<TaskActivityRow, StorageError> {
        let id = uuid::Uuid::new_v4().to_string();
        let row = sqlx::query_as::<_, TaskActivityRow>(
            r#"
            INSERT INTO task_activity (id, task_id, activity_type, field_changed, old_value, new_value, actor_type, summary)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            RETURNING *
            "#,
        )
        .bind(&id)
        .bind(task_id)
        .bind(activity_type)
        .bind(field_changed)
        .bind(old_value)
        .bind(new_value)
        .bind(actor_type)
        .bind(summary)
        .fetch_one(&self.pool)
        .await?;
        Ok(row)
    }

    /// List activity entries for a task, most recent first, up to `limit`.
    pub async fn list_activity(
        &self,
        task_id: &str,
        limit: i64,
    ) -> Result<Vec<TaskActivityRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskActivityRow>(
            r#"
            SELECT * FROM task_activity
            WHERE task_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )
        .bind(task_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Execution Tracking
    // -----------------------------------------------------------------------

    /// Create an execution record.
    pub async fn create_execution(
        &self,
        row: &TaskExecutionRow,
    ) -> Result<TaskExecutionRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskExecutionRow>(
            r#"
            INSERT INTO task_executions (
                id, task_id, status, agent_profile, started_at, completed_at,
                duration_secs, tokens_used, cost_usd, input_context,
                output_summary, error_message, artifacts, metrics, retry_count
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6,
                ?7, ?8, ?9, ?10,
                ?11, ?12, ?13, ?14, ?15
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(&row.status)
        .bind(&row.agent_profile)
        .bind(row.started_at)
        .bind(row.completed_at)
        .bind(row.duration_secs)
        .bind(row.tokens_used)
        .bind(row.cost_usd)
        .bind(&row.input_context)
        .bind(&row.output_summary)
        .bind(&row.error_message)
        .bind(&row.artifacts)
        .bind(&row.metrics)
        .bind(row.retry_count)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Update an execution record's status and optional output fields.
    pub async fn update_execution(
        &self,
        id: &str,
        status: &str,
        output_summary: Option<&str>,
        error_message: Option<&str>,
        metrics: Option<&str>,
    ) -> Result<TaskExecutionRow, StorageError> {
        let row = sqlx::query_as::<_, TaskExecutionRow>(
            r#"
            UPDATE task_executions
            SET status = ?2,
                output_summary = COALESCE(?3, output_summary),
                error_message = COALESCE(?4, error_message),
                metrics = COALESCE(?5, metrics),
                completed_at = CASE WHEN ?2 IN ('completed', 'failed') THEN datetime('now') ELSE completed_at END
            WHERE id = ?1
            RETURNING *
            "#,
        )
        .bind(id)
        .bind(status)
        .bind(output_summary)
        .bind(error_message)
        .bind(metrics)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_not_found(&format!("execution {id}"))?;
        Ok(row)
    }

    /// Get a single execution by ID.
    pub async fn get_execution(&self, id: &str) -> Result<Option<TaskExecutionRow>, StorageError> {
        let row = sqlx::query_as::<_, TaskExecutionRow>(
            "SELECT * FROM task_executions WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List executions for a task.
    pub async fn list_executions(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskExecutionRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskExecutionRow>(
            "SELECT * FROM task_executions WHERE task_id = ?1 ORDER BY created_at DESC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    // -----------------------------------------------------------------------
    // Suggestion Management
    // -----------------------------------------------------------------------

    /// Create a suggestion record.
    pub async fn create_suggestion(
        &self,
        row: &TaskSuggestionRow,
    ) -> Result<TaskSuggestionRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskSuggestionRow>(
            r#"
            INSERT INTO task_suggestions (
                id, task_id, suggestion_type, title, description,
                confidence, action_payload, status, trigger
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(&row.suggestion_type)
        .bind(&row.title)
        .bind(&row.description)
        .bind(row.confidence)
        .bind(&row.action_payload)
        .bind(&row.status)
        .bind(&row.trigger)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Resolve a suggestion by updating its status and resolved_at timestamp.
    pub async fn resolve_suggestion(&self, id: &str, status: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE task_suggestions SET status = ?2, resolved_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .bind(status)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    /// List pending suggestions, optionally filtered by task_id.
    pub async fn list_pending_suggestions(
        &self,
        task_id: Option<&str>,
    ) -> Result<Vec<TaskSuggestionRow>, StorageError> {
        if let Some(tid) = task_id {
            let rows = sqlx::query_as::<_, TaskSuggestionRow>(
                "SELECT * FROM task_suggestions WHERE status = 'pending' AND task_id = ?1 ORDER BY created_at DESC",
            )
            .bind(tid)
            .fetch_all(&self.pool)
            .await?;
            Ok(rows)
        } else {
            let rows = sqlx::query_as::<_, TaskSuggestionRow>(
                "SELECT * FROM task_suggestions WHERE status = 'pending' ORDER BY created_at DESC",
            )
            .fetch_all(&self.pool)
            .await?;
            Ok(rows)
        }
    }

    /// Expire all pending suggestions for a task.
    pub async fn expire_suggestions_for_task(&self, task_id: &str) -> Result<u64, StorageError> {
        let result = sqlx::query(
            "UPDATE task_suggestions SET status = 'expired', resolved_at = datetime('now') WHERE task_id = ?1 AND status = 'pending'",
        )
        .bind(task_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    // -----------------------------------------------------------------------
    // Decomposition Management
    // -----------------------------------------------------------------------

    /// Create a decomposition plan record.
    pub async fn create_decomposition(
        &self,
        row: &TaskDecompositionRow,
    ) -> Result<TaskDecompositionRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskDecompositionRow>(
            r#"
            INSERT INTO task_decompositions (id, task_id, plan, confidence, status, reasoning)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(&row.plan)
        .bind(row.confidence)
        .bind(&row.status)
        .bind(&row.reasoning)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Get a decomposition by ID.
    pub async fn get_decomposition(
        &self,
        id: &str,
    ) -> Result<Option<TaskDecompositionRow>, StorageError> {
        let row = sqlx::query_as::<_, TaskDecompositionRow>(
            "SELECT * FROM task_decompositions WHERE id = ?1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    /// List pending decompositions for a task.
    pub async fn list_pending_decompositions(
        &self,
        task_id: &str,
    ) -> Result<Vec<TaskDecompositionRow>, StorageError> {
        let rows = sqlx::query_as::<_, TaskDecompositionRow>(
            "SELECT * FROM task_decompositions WHERE task_id = ?1 AND status = 'pending' ORDER BY created_at DESC",
        )
        .bind(task_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    /// Mark a decomposition as applied.
    pub async fn apply_decomposition(&self, id: &str) -> Result<bool, StorageError> {
        let result = sqlx::query(
            "UPDATE task_decompositions SET status = 'applied', applied_at = datetime('now') WHERE id = ?1",
        )
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    // -----------------------------------------------------------------------
    // Estimation Tracking
    // -----------------------------------------------------------------------

    /// Record an estimation history entry.
    pub async fn record_estimation(
        &self,
        row: &TaskEstimationRow,
    ) -> Result<TaskEstimationRow, StorageError> {
        let inserted = sqlx::query_as::<_, TaskEstimationRow>(
            r#"
            INSERT INTO task_estimation_history (
                id, task_id, estimated_minutes, actual_minutes, deviation_pct,
                complexity_score, energy_level, tags, completed_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5,
                ?6, ?7, ?8, ?9
            )
            RETURNING *
            "#,
        )
        .bind(&row.id)
        .bind(&row.task_id)
        .bind(row.estimated_minutes)
        .bind(row.actual_minutes)
        .bind(row.deviation_pct)
        .bind(row.complexity_score)
        .bind(&row.energy_level)
        .bind(sqlx::types::Json(&row.tags))
        .bind(row.completed_at)
        .fetch_one(&self.pool)
        .await?;
        Ok(inserted)
    }

    /// Get estimation accuracy stats: (avg_deviation_pct, count), optionally filtered by tags.
    pub async fn estimation_stats(
        &self,
        tags: Option<&[String]>,
    ) -> Result<(f64, i64), StorageError> {
        if let Some(tag_list) = tags {
            if tag_list.is_empty() {
                let row: (f64, i64) = sqlx::query_as(
                    "SELECT COALESCE(AVG(deviation_pct), 0.0), COUNT(*) FROM task_estimation_history",
                )
                .fetch_one(&self.pool)
                .await?;
                return Ok(row);
            }

            // Filter by tags: entries must contain ALL specified tags.
            let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
                "SELECT COALESCE(AVG(deviation_pct), 0.0), COUNT(*) FROM task_estimation_history WHERE 1=1",
            );
            for tag in tag_list {
                qb.push(" AND EXISTS (SELECT 1 FROM json_each(tags) WHERE value = ");
                qb.push_bind(tag);
                qb.push(")");
            }
            let row = qb
                .build_query_as::<(f64, i64)>()
                .fetch_one(&self.pool)
                .await?;
            Ok(row)
        } else {
            let row: (f64, i64) = sqlx::query_as(
                "SELECT COALESCE(AVG(deviation_pct), 0.0), COUNT(*) FROM task_estimation_history",
            )
            .fetch_one(&self.pool)
            .await?;
            Ok(row)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    /// Create all task-related tables needed for tests.
    /// The tasks table is from the feature-tasks migration, not core migrations,
    /// so we create them manually in tests.
    async fn create_test_tables(db: &SqlitePool) {
        // We need the areas table to exist (it's a core migration table, already present),
        // but we need to create all task-* tables.
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS tasks (
                id                   TEXT PRIMARY KEY,
                title                TEXT NOT NULL,
                description          TEXT,
                area_id              TEXT NOT NULL REFERENCES areas(id) ON DELETE CASCADE,
                project_id           TEXT,
                key_result_id        TEXT,
                objective_id         TEXT,
                parent_id            TEXT REFERENCES tasks(id) ON DELETE SET NULL,
                status_label_id      TEXT,
                group_id             TEXT,
                priority             INTEGER,
                position             INTEGER NOT NULL DEFAULT 0,
                due_date             TEXT,
                tags                 TEXT NOT NULL DEFAULT '[]',
                status               TEXT NOT NULL DEFAULT 'todo',
                task_type            TEXT NOT NULL DEFAULT 'manual',
                focused_at           TEXT,
                focus_deadline       TEXT,
                focus_expired_count  INTEGER NOT NULL DEFAULT 0,
                created_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                updated_at           TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                completed_at         TEXT,
                completed            INTEGER NOT NULL DEFAULT 0,
                total_tracked_secs   INTEGER NOT NULL DEFAULT 0,
                estimated_minutes    INTEGER,
                actual_minutes       INTEGER,
                calendar_event_uid   TEXT,
                last_reminded_at     TEXT,
                recurrence_rule      TEXT,
                recurrence_parent_id TEXT,
                is_template          INTEGER NOT NULL DEFAULT 0,
                next_instance_date   TEXT,
                acceptance_criteria  TEXT,
                agent_config         TEXT,
                execution_state      TEXT NOT NULL DEFAULT 'idle',
                spawned_execution_id TEXT,
                context_snapshot     TEXT,
                energy_level         TEXT DEFAULT 'medium',
                estimated_focus_blocks INTEGER,
                complexity_score     INTEGER
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_activity (
                id            TEXT PRIMARY KEY,
                task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                activity_type TEXT NOT NULL,
                field_changed TEXT,
                old_value     TEXT,
                new_value     TEXT,
                actor_type    TEXT NOT NULL DEFAULT 'user',
                actor_id      TEXT,
                summary       TEXT,
                created_at    TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_executions (
                id             TEXT PRIMARY KEY,
                task_id        TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                status         TEXT NOT NULL DEFAULT 'pending',
                agent_profile  TEXT,
                started_at     TEXT,
                completed_at   TEXT,
                duration_secs  INTEGER,
                tokens_used    INTEGER,
                cost_usd       REAL,
                input_context  TEXT,
                output_summary TEXT,
                error_message  TEXT,
                artifacts      TEXT,
                metrics        TEXT,
                retry_count    INTEGER NOT NULL DEFAULT 0,
                created_at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_suggestions (
                id              TEXT PRIMARY KEY,
                task_id         TEXT REFERENCES tasks(id) ON DELETE CASCADE,
                suggestion_type TEXT NOT NULL,
                title           TEXT NOT NULL,
                description     TEXT,
                confidence      REAL NOT NULL DEFAULT 0.0,
                action_payload  TEXT,
                status          TEXT NOT NULL DEFAULT 'pending',
                trigger         TEXT,
                created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                resolved_at     TEXT
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_attachments (
                id              TEXT PRIMARY KEY,
                task_id         TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                attachment_type TEXT NOT NULL,
                value           TEXT NOT NULL,
                title           TEXT,
                tags            TEXT NOT NULL DEFAULT '[]',
                source          TEXT NOT NULL DEFAULT 'user',
                created_at      TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now'))
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_time_entries (
                id            TEXT PRIMARY KEY,
                task_id       TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                source        TEXT NOT NULL DEFAULT 'focus',
                started_at    TEXT NOT NULL,
                ended_at      TEXT,
                duration_secs INTEGER,
                energy_level  TEXT,
                note          TEXT
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_dependencies (
                task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                blocker_id TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                dep_type   TEXT NOT NULL DEFAULT 'blocks',
                PRIMARY KEY (task_id, blocker_id),
                CHECK (task_id != blocker_id)
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_estimation_history (
                id                TEXT PRIMARY KEY,
                task_id           TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                estimated_minutes INTEGER NOT NULL,
                actual_minutes    INTEGER NOT NULL,
                deviation_pct     REAL NOT NULL DEFAULT 0.0,
                complexity_score  INTEGER,
                energy_level      TEXT,
                tags              TEXT NOT NULL DEFAULT '[]',
                completed_at      TEXT NOT NULL
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS task_decompositions (
                id         TEXT PRIMARY KEY,
                task_id    TEXT NOT NULL REFERENCES tasks(id) ON DELETE CASCADE,
                plan       TEXT NOT NULL,
                confidence REAL NOT NULL DEFAULT 0.0,
                status     TEXT NOT NULL DEFAULT 'pending',
                reasoning  TEXT,
                created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%SZ', 'now')),
                applied_at TEXT
            )
            "#,
        )
        .execute(db)
        .await
        .unwrap();

        // Insert a test area for FK references.
        sqlx::query(
            "INSERT OR IGNORE INTO areas (id, name, color, status) VALUES ('test-area', 'Test Area', '#000', 'active')",
        )
        .execute(db)
        .await
        .unwrap();
    }

    /// Helper to create a ready-to-use TaskRepo backed by an in-memory DB.
    async fn setup_repo() -> TaskRepo {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        TaskRepo::new(db)
    }

    /// Helper to create a minimal TaskRow for tests.
    fn make_task(id: &str, title: &str) -> TaskRow {
        let now = Utc::now();
        TaskRow {
            id: id.to_string(),
            title: title.to_string(),
            description: None,
            area_id: "test-area".to_string(),
            project_id: None,
            key_result_id: None,
            parent_id: None,
            priority: None,
            due_date: None,
            tags: vec![],
            status: "todo".to_string(),
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
            status_label_id: None,
            position: 0,
            group_id: None,
            task_type: "manual".to_string(),
            acceptance_criteria: None,
            agent_config: None,
            execution_state: "idle".to_string(),
            spawned_execution_id: None,
            context_snapshot: None,
            energy_level: Some("medium".to_string()),
            estimated_focus_blocks: None,
            actual_minutes: None,
            complexity_score: None,
            completed: false,
            objective_id: None,
        }
    }

    #[tokio::test]
    async fn test_add_and_get() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("t1", "Buy groceries");
        let inserted = repo.add(&task).await.unwrap();
        assert_eq!(inserted.id, "t1");
        assert_eq!(inserted.title, "Buy groceries");
        assert_eq!(inserted.task_type, "manual");
        assert_eq!(inserted.execution_state, "idle");
        assert!(!inserted.completed);

        // get
        let fetched = repo.get("t1").await.unwrap().unwrap();
        assert_eq!(fetched.title, "Buy groceries");

        // get_or_err
        let fetched2 = repo.get_or_err("t1").await.unwrap();
        assert_eq!(fetched2.id, "t1");

        // get_or_err on missing
        let err = repo.get_or_err("nonexistent").await;
        assert!(matches!(err, Err(StorageError::NotFound(_))));

        // get_by_ids
        let task2 = make_task("t2", "Walk the dog");
        repo.add(&task2).await.unwrap();
        let batch = repo
            .get_by_ids(&["t1".into(), "t2".into(), "missing".into()])
            .await
            .unwrap();
        assert_eq!(batch.len(), 2);

        // get_by_ids with empty input
        let empty = repo.get_by_ids(&[]).await.unwrap();
        assert!(empty.is_empty());
    }

    #[tokio::test]
    async fn test_update_task() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("u1", "Original title");
        repo.add(&task).await.unwrap();

        // Update title and new agentic fields.
        let patch = TaskPatch {
            id: "u1".to_string(),
            title: Some("Updated title".to_string()),
            task_type: Some("agent_delegated".to_string()),
            execution_state: Some("running".to_string()),
            energy_level: Some(Some("high".to_string())),
            complexity_score: Some(Some(7)),
            completed: Some(true),
            ..Default::default()
        };
        let updated = repo.update(&patch).await.unwrap();
        assert_eq!(updated.title, "Updated title");
        assert_eq!(updated.task_type, "agent_delegated");
        assert_eq!(updated.execution_state, "running");
        assert_eq!(updated.energy_level.as_deref(), Some("high"));
        assert_eq!(updated.complexity_score, Some(7));
        assert!(updated.completed);

        // Update non-existent task.
        let bad_patch = TaskPatch {
            id: "nonexistent".to_string(),
            title: Some("Nope".to_string()),
            ..Default::default()
        };
        let err = repo.update(&bad_patch).await;
        assert!(matches!(err, Err(StorageError::NotFound(_))));
    }

    #[tokio::test]
    async fn test_delete_task() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("d1", "Delete me");
        repo.add(&task).await.unwrap();

        let deleted = repo.delete("d1").await.unwrap();
        assert!(deleted);

        let gone = repo.get("d1").await.unwrap();
        assert!(gone.is_none());

        // Delete non-existent
        let not_deleted = repo.delete("d1").await.unwrap();
        assert!(!not_deleted);
    }

    #[tokio::test]
    async fn test_list_with_filters() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let mut t1 = make_task("f1", "Manual task");
        t1.task_type = "manual".to_string();
        t1.completed = false;
        repo.add(&t1).await.unwrap();

        let mut t2 = make_task("f2", "Agent task");
        t2.task_type = "agent_delegated".to_string();
        t2.completed = true;
        repo.add(&t2).await.unwrap();

        let mut t3 = make_task("f3", "Another manual");
        t3.task_type = "manual".to_string();
        t3.completed = true;
        repo.add(&t3).await.unwrap();

        // Filter by task_type
        let results = repo
            .list(&TaskFilter {
                task_type: Some("agent_delegated".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "f2");

        // Filter by completed
        let results = repo
            .list(&TaskFilter {
                completed: Some(true),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 2);

        let results = repo
            .list(&TaskFilter {
                completed: Some(false),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "f1");

        // Filter by energy_level
        let results = repo
            .list(&TaskFilter {
                energy_level: Some("medium".to_string()),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(results.len(), 3);

        // Search (SQLite LIKE is case-insensitive for ASCII)
        let results = repo.search_by_keyword("manual", None).await.unwrap();
        assert_eq!(results.len(), 2);

        let results = repo.search_by_keyword("Agent", None).await.unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, "f2");
    }

    #[tokio::test]
    async fn test_focus_unfocus() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let t1 = make_task("fo1", "Focus me");
        let t2 = make_task("fo2", "Also focus me");
        repo.add(&t1).await.unwrap();
        repo.add(&t2).await.unwrap();

        // Focus first task
        let focused = repo.focus("fo1", 2, None).await.unwrap();
        assert!(focused);

        let list = repo.list_focused().await.unwrap();
        assert_eq!(list.len(), 1);

        // Focus second task
        let focused = repo.focus("fo2", 2, None).await.unwrap();
        assert!(focused);

        // Can't focus third (max_slots = 2)
        let t3 = make_task("fo3", "No room");
        repo.add(&t3).await.unwrap();
        let focused = repo.focus("fo3", 2, None).await.unwrap();
        assert!(!focused);

        // Unfocus
        let unfocused = repo.unfocus("fo1").await.unwrap();
        assert!(unfocused);

        let list = repo.list_focused().await.unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].id, "fo2");
    }

    #[tokio::test]
    async fn test_log_activity() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("act1", "Activity test");
        repo.add(&task).await.unwrap();

        let entry = repo
            .log_activity(
                "act1",
                "field_change",
                Some("status"),
                Some("todo"),
                Some("doing"),
                "user",
                Some("Status changed"),
            )
            .await
            .unwrap();
        assert_eq!(entry.task_id, "act1");
        assert_eq!(entry.activity_type, "field_change");
        assert_eq!(entry.field_changed.as_deref(), Some("status"));

        let entry2 = repo
            .log_activity(
                "act1",
                "comment",
                None,
                None,
                None,
                "agent",
                Some("Looks good"),
            )
            .await
            .unwrap();
        assert_eq!(entry2.actor_type, "agent");

        let entries = repo.list_activity("act1", 10).await.unwrap();
        assert_eq!(entries.len(), 2);
        let types: Vec<&str> = entries.iter().map(|e| e.activity_type.as_str()).collect();
        assert!(types.contains(&"field_change"));
        assert!(types.contains(&"comment"));
    }

    #[tokio::test]
    async fn test_children_and_subtree() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let parent = make_task("p1", "Parent");
        repo.add(&parent).await.unwrap();

        let mut child1 = make_task("c1", "Child 1");
        child1.parent_id = Some("p1".to_string());
        child1.completed = true;
        repo.add(&child1).await.unwrap();

        let mut child2 = make_task("c2", "Child 2");
        child2.parent_id = Some("p1".to_string());
        repo.add(&child2).await.unwrap();

        let mut grandchild = make_task("gc1", "Grandchild");
        grandchild.parent_id = Some("c1".to_string());
        repo.add(&grandchild).await.unwrap();

        // get_children
        let children = repo.get_children("p1").await.unwrap();
        assert_eq!(children.len(), 2);

        // count_children
        let (total, completed) = repo.count_children("p1").await.unwrap();
        assert_eq!(total, 2);
        assert_eq!(completed, 1);

        // count_children_bulk
        let bulk = repo
            .count_children_bulk(&["p1".into(), "c1".into()])
            .await
            .unwrap();
        assert_eq!(bulk.get("p1"), Some(&(2, 1)));
        assert_eq!(bulk.get("c1"), Some(&(1, 0)));

        // get_subtree
        let subtree = repo.get_subtree("p1").await.unwrap();
        assert_eq!(subtree.len(), 4); // parent + 2 children + grandchild

        // cascade_complete
        let completed_count = repo.cascade_complete("p1").await.unwrap();
        // parent, child2, grandchild were not completed (child1 was already completed)
        assert_eq!(completed_count, 3);

        let all = repo.list(&TaskFilter::default()).await.unwrap();
        assert!(all.iter().all(|t| t.completed));

        // move_task
        let moved = repo.move_task("gc1", Some("p1"), None).await.unwrap();
        assert_eq!(moved.parent_id.as_deref(), Some("p1"));

        // move_task cycle detection
        let err = repo.move_task("p1", Some("gc1"), None).await;
        assert!(matches!(err, Err(StorageError::Conflict(_))));
    }

    #[tokio::test]
    async fn test_dependencies() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let t1 = make_task("dep1", "Task 1");
        let t2 = make_task("dep2", "Task 2");
        let t3 = make_task("dep3", "Task 3");
        repo.add(&t1).await.unwrap();
        repo.add(&t2).await.unwrap();
        repo.add(&t3).await.unwrap();

        // dep1 is blocked by dep2
        repo.add_dependency("dep1", "dep2", "blocks").await.unwrap();

        let blockers = repo.get_blockers("dep1").await.unwrap();
        assert_eq!(blockers.len(), 1);
        assert_eq!(blockers[0].id, "dep2");

        let blocking = repo.get_blocking("dep2").await.unwrap();
        assert_eq!(blocking.len(), 1);
        assert_eq!(blocking[0].id, "dep1");

        // incomplete_blockers
        let incomplete = repo.incomplete_blockers("dep1").await.unwrap();
        assert_eq!(incomplete.len(), 1);

        // Cycle detection: dep2 -> dep3 -> dep1 would create a cycle since dep1 -> dep2 exists
        repo.add_dependency("dep2", "dep3", "blocks").await.unwrap();
        let err = repo.add_dependency("dep3", "dep1", "blocks").await;
        assert!(matches!(err, Err(StorageError::Conflict(_))));

        // Remove dependency
        let removed = repo.remove_dependency("dep1", "dep2").await.unwrap();
        assert!(removed);

        let blockers = repo.get_blockers("dep1").await.unwrap();
        assert!(blockers.is_empty());
    }

    #[tokio::test]
    async fn test_attachments() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("att1", "Attach test");
        repo.add(&task).await.unwrap();

        let att = repo
            .add_attachment(
                "att1",
                "link",
                "https://example.com",
                Some("Example"),
                &["ref".to_string()],
                "user",
            )
            .await
            .unwrap();
        assert_eq!(att.task_id, "att1");
        assert_eq!(att.source, "user");

        let list = repo.list_attachments("att1").await.unwrap();
        assert_eq!(list.len(), 1);

        let removed = repo.remove_attachment("att1", att.id).await.unwrap();
        assert!(removed);

        let list = repo.list_attachments("att1").await.unwrap();
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn test_time_entries() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("te1", "Time test");
        repo.add(&task).await.unwrap();

        let now = Utc::now();
        let entry = repo
            .add_time_entry(
                "te1",
                "manual",
                now,
                Some(3600),
                Some("Did some work"),
                Some("high"),
            )
            .await
            .unwrap();
        assert_eq!(entry.task_id, "te1");
        assert_eq!(entry.duration_secs, Some(3600));
        assert_eq!(entry.energy_level.as_deref(), Some("high"));

        // Check total_tracked_secs updated
        let updated_task = repo.get_or_err("te1").await.unwrap();
        assert_eq!(updated_task.total_tracked_secs, 3600);

        let list = repo.list_time_entries("te1").await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_executions() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("ex1", "Exec test");
        repo.add(&task).await.unwrap();

        let now = Utc::now();
        let exec_row = TaskExecutionRow {
            id: "exec-1".to_string(),
            task_id: "ex1".to_string(),
            status: "pending".to_string(),
            agent_profile: Some("general".to_string()),
            started_at: Some(now),
            completed_at: None,
            duration_secs: None,
            tokens_used: None,
            cost_usd: None,
            input_context: Some("test context".to_string()),
            output_summary: None,
            error_message: None,
            artifacts: None,
            metrics: None,
            retry_count: 0,
            created_at: now,
        };
        let created = repo.create_execution(&exec_row).await.unwrap();
        assert_eq!(created.id, "exec-1");
        assert_eq!(created.status, "pending");

        let updated = repo
            .update_execution("exec-1", "completed", Some("All done"), None, None)
            .await
            .unwrap();
        assert_eq!(updated.status, "completed");
        assert_eq!(updated.output_summary.as_deref(), Some("All done"));
        assert!(updated.completed_at.is_some());

        let list = repo.list_executions("ex1").await.unwrap();
        assert_eq!(list.len(), 1);
    }

    #[tokio::test]
    async fn test_suggestions() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("sug1", "Suggestion test");
        repo.add(&task).await.unwrap();

        let now = Utc::now();
        let suggestion = TaskSuggestionRow {
            id: "s1".to_string(),
            task_id: Some("sug1".to_string()),
            suggestion_type: "decompose".to_string(),
            title: "Break this into subtasks".to_string(),
            description: None,
            confidence: 0.85,
            action_payload: None,
            status: "pending".to_string(),
            trigger: Some("complexity".to_string()),
            created_at: now,
            resolved_at: None,
        };
        let created = repo.create_suggestion(&suggestion).await.unwrap();
        assert_eq!(created.status, "pending");

        let pending = repo.list_pending_suggestions(Some("sug1")).await.unwrap();
        assert_eq!(pending.len(), 1);

        let resolved = repo.resolve_suggestion("s1", "accepted").await.unwrap();
        assert!(resolved);

        let pending = repo.list_pending_suggestions(Some("sug1")).await.unwrap();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_estimation_stats() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let task = make_task("est1", "Estimation test");
        repo.add(&task).await.unwrap();

        let now = Utc::now();
        let est1 = TaskEstimationRow {
            id: "e1".to_string(),
            task_id: "est1".to_string(),
            estimated_minutes: 60,
            actual_minutes: 90,
            deviation_pct: 50.0,
            complexity_score: Some(5),
            energy_level: Some("medium".to_string()),
            tags: vec!["coding".to_string()],
            completed_at: now,
        };
        repo.record_estimation(&est1).await.unwrap();

        let est2 = TaskEstimationRow {
            id: "e2".to_string(),
            task_id: "est1".to_string(),
            estimated_minutes: 30,
            actual_minutes: 30,
            deviation_pct: 0.0,
            complexity_score: Some(3),
            energy_level: Some("low".to_string()),
            tags: vec!["review".to_string()],
            completed_at: now,
        };
        repo.record_estimation(&est2).await.unwrap();

        // Overall stats
        let (avg, count) = repo.estimation_stats(None).await.unwrap();
        assert_eq!(count, 2);
        assert!((avg - 25.0).abs() < 0.01); // (50.0 + 0.0) / 2

        // Filter by tag
        let (avg, count) = repo
            .estimation_stats(Some(&["coding".to_string()]))
            .await
            .unwrap();
        assert_eq!(count, 1);
        assert!((avg - 50.0).abs() < 0.01);
    }

    #[tokio::test]
    async fn test_summary_and_overdue() {
        let pool = crate::StoragePool::connect_in_memory().await.unwrap();
        let db = pool.inner().clone();
        create_test_tables(&db).await;
        let repo = TaskRepo::new(db);

        let t1 = make_task("sum1", "Todo task");
        repo.add(&t1).await.unwrap();

        let mut t2 = make_task("sum2", "Doing task");
        t2.status = "doing".to_string();
        repo.add(&t2).await.unwrap();

        let mut t3 = make_task("sum3", "Done task");
        t3.status = "done".to_string();
        t3.completed = true;
        repo.add(&t3).await.unwrap();

        let summary = repo.summary().await.unwrap();
        assert_eq!(summary.todo, 1);
        assert_eq!(summary.doing, 1);
        assert_eq!(summary.done, 1);
        assert_eq!(summary.total, 3);

        // Overdue: task with past due date and not completed
        let mut overdue_task = make_task("sum4", "Overdue task");
        overdue_task.due_date = Some(Utc::now() - chrono::Duration::days(1));
        repo.add(&overdue_task).await.unwrap();

        let overdue = repo.overdue().await.unwrap();
        assert_eq!(overdue.len(), 1);
        assert_eq!(overdue[0].id, "sum4");
    }

    #[tokio::test]
    async fn test_get_execution() {
        let repo = setup_repo().await;

        let task = make_task("exec-get-t", "Exec test");
        repo.add(&task).await.unwrap();

        let exec_row = TaskExecutionRow {
            id: "exec-1".to_string(),
            task_id: task.id.clone(),
            status: "pending".to_string(),
            agent_profile: Some("task".to_string()),
            started_at: None,
            completed_at: None,
            duration_secs: None,
            tokens_used: None,
            cost_usd: None,
            input_context: None,
            output_summary: None,
            error_message: None,
            artifacts: None,
            metrics: None,
            retry_count: 0,
            created_at: Utc::now(),
        };
        repo.create_execution(&exec_row).await.unwrap();

        let fetched = repo.get_execution("exec-1").await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().status, "pending");

        let missing = repo.get_execution("nonexistent").await.unwrap();
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn test_decomposition_crud() {
        let repo = setup_repo().await;

        let task = make_task("decomp-t", "Decomp test");
        repo.add(&task).await.unwrap();

        let row = TaskDecompositionRow {
            id: "decomp-1".to_string(),
            task_id: task.id.clone(),
            plan: r#"{"subtasks":[]}"#.to_string(),
            confidence: 0.85,
            status: "pending".to_string(),
            reasoning: Some("test".to_string()),
            created_at: Utc::now(),
            applied_at: None,
        };
        let created = repo.create_decomposition(&row).await.unwrap();
        assert_eq!(created.id, "decomp-1");

        let fetched = repo.get_decomposition("decomp-1").await.unwrap();
        assert!(fetched.is_some());

        let pending = repo.list_pending_decompositions(&task.id).await.unwrap();
        assert_eq!(pending.len(), 1);

        repo.apply_decomposition("decomp-1").await.unwrap();
        let applied = repo.get_decomposition("decomp-1").await.unwrap().unwrap();
        assert_eq!(applied.status, "applied");

        let pending_after = repo.list_pending_decompositions(&task.id).await.unwrap();
        assert!(pending_after.is_empty());
    }

    #[tokio::test]
    async fn test_expire_suggestions() {
        let repo = setup_repo().await;

        let task = make_task("expire-t", "Expire test");
        repo.add(&task).await.unwrap();

        let sugg = TaskSuggestionRow {
            id: "sugg-1".to_string(),
            task_id: Some(task.id.clone()),
            suggestion_type: "Decompose".to_string(),
            title: "Break down".to_string(),
            description: None,
            confidence: 0.8,
            action_payload: None,
            status: "pending".to_string(),
            trigger: None,
            created_at: Utc::now(),
            resolved_at: None,
        };
        repo.create_suggestion(&sugg).await.unwrap();

        let expired = repo.expire_suggestions_for_task(&task.id).await.unwrap();
        assert_eq!(expired, 1);

        let pending = repo.list_pending_suggestions(Some(&task.id)).await.unwrap();
        assert!(pending.is_empty());
    }
}
